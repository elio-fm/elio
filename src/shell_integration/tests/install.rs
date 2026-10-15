#[cfg(unix)]
use super::super::install::resolve_write_path;
use super::super::{
    Shell,
    install::{
        MANAGED_END, MANAGED_START, install_at, managed_script, pwsh_profile_in,
        remove_managed_blocks, uninstall_at, uninstall_reload_command, upsert_managed_block,
        write_text_atomic,
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-shell-integration-{label}-{unique}"))
}

#[test]
fn uninstall_reload_command_removes_loaded_function() {
    assert_eq!(uninstall_reload_command(Shell::Bash), "unset -f elio");
    assert_eq!(
        uninstall_reload_command(Shell::Zsh),
        "unfunction elio 2>/dev/null || true"
    );
    assert_eq!(
        uninstall_reload_command(Shell::Fish),
        "functions --erase elio"
    );
    assert_eq!(uninstall_reload_command(Shell::Nu), "hide elio");
    assert_eq!(
        uninstall_reload_command(Shell::Pwsh),
        r"Remove-Item Function:\elio -ErrorAction SilentlyContinue"
    );
}

#[test]
fn pwsh_profile_in_follows_the_documents_layout() {
    let documents = Path::new("redirected").join("Documents");
    let profile = pwsh_profile_in(&documents);

    assert_eq!(
        profile.file_name().and_then(|name| name.to_str()),
        Some("Microsoft.PowerShell_profile.ps1")
    );
    assert_eq!(
        profile
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str()),
        Some("PowerShell")
    );
    assert!(profile.starts_with(&documents));
}

/// Windows resolves the profile from the Documents known folder, which no
/// environment variable redirects, so compare against what PowerShell reports.
#[cfg(windows)]
#[test]
fn pwsh_integration_path_matches_the_profile_powershell_loads() {
    use super::super::install::integration_path;

    let Ok(output) = std::process::Command::new("pwsh")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg("$PROFILE.CurrentUserCurrentHost")
        .output()
    else {
        return;
    };
    assert!(output.status.success(), "pwsh should report its profile");
    let reported = String::from_utf8_lossy(&output.stdout);

    let path = integration_path(Shell::Pwsh).expect("pwsh profile path should resolve");

    assert!(
        path.to_string_lossy().eq_ignore_ascii_case(reported.trim()),
        "elio targets {} but PowerShell loads {}",
        path.display(),
        reported.trim()
    );
}

#[test]
fn pwsh_install_reinstall_and_uninstall_preserve_existing_profile_contents() {
    let root = temp_path("pwsh-lifecycle");
    let profile = pwsh_profile_in(&root.join("Documents"));
    let existing = "Set-PSReadLineOption -EditMode Vi\r\nSet-Alias ll Get-ChildItem\r\n";
    fs::create_dir_all(profile.parent().expect("profile should have a parent"))
        .expect("profile directory should be created");
    fs::write(&profile, existing).expect("existing profile should be written");

    install_at(Shell::Pwsh, &profile, "'C:\\old\\elio.exe'").expect("install should succeed");
    install_at(Shell::Pwsh, &profile, "'C:\\new\\elio.exe'").expect("reinstall should succeed");

    let installed = fs::read_to_string(&profile).expect("profile should be readable");
    assert!(installed.starts_with(existing));
    assert_eq!(installed.matches(MANAGED_START).count(), 1);
    assert_eq!(installed.matches(MANAGED_END).count(), 1);
    assert!(installed.contains("$elioExe = 'C:\\new\\elio.exe'"));
    assert!(!installed.contains("C:\\old\\elio.exe"));

    assert!(uninstall_at(Shell::Pwsh, &profile).expect("uninstall should succeed"));
    assert_eq!(
        fs::read_to_string(&profile).expect("profile should be readable"),
        existing
    );
    assert!(!uninstall_at(Shell::Pwsh, &profile).expect("second uninstall should succeed"));
    assert_eq!(
        fs::read_to_string(&profile).expect("profile should be readable"),
        existing
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn pwsh_install_creates_a_missing_profile_and_uninstall_leaves_it_empty() {
    let root = temp_path("pwsh-new-profile");
    let profile = pwsh_profile_in(&root.join("Documents"));

    install_at(Shell::Pwsh, &profile, "'elio.exe'").expect("install should succeed");

    let installed = fs::read_to_string(&profile).expect("profile should be written");
    assert!(installed.starts_with(MANAGED_START));
    assert!(installed.contains("function elio {"));

    assert!(uninstall_at(Shell::Pwsh, &profile).expect("uninstall should succeed"));
    assert_eq!(
        fs::read_to_string(&profile).expect("profile should be readable"),
        ""
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

fn leftover_temp_files(root: &Path) -> usize {
    fs::read_dir(root)
        .expect("temp directory should be readable")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".elio-tmp-"))
        .count()
}

#[test]
fn write_text_atomic_cleans_up_when_the_destination_cannot_be_replaced() {
    let root = temp_path("atomic-blocked");
    let path = root.join("profile.ps1");
    fs::create_dir_all(&path).expect("blocking directory should be created");
    fs::write(path.join("keep"), "old").expect("blocking directory should be populated");

    let error = write_text_atomic(&path, "new\n").expect_err("a directory should not be replaced");

    assert!(error.to_string().contains("failed to replace"));
    assert_eq!(
        fs::read_to_string(path.join("keep")).expect("existing contents should survive"),
        "old"
    );
    assert_eq!(leftover_temp_files(&root), 0);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(windows)]
#[test]
fn write_text_atomic_keeps_the_existing_file_when_replacement_fails() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = temp_path("atomic-locked");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let path = root.join("Microsoft.PowerShell_profile.ps1");
    fs::write(&path, "old\n").expect("existing profile should be written");

    // Opened without sharing, so Windows refuses to replace the file.
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .expect("existing profile should be locked");
    let error =
        write_text_atomic(&path, "new\n").expect_err("a locked file should not be replaced");
    drop(locked);

    assert!(error.to_string().contains("failed to replace"));
    assert_eq!(
        fs::read_to_string(&path).expect("existing profile should survive"),
        "old\n"
    );
    assert_eq!(leftover_temp_files(&root), 0);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn write_text_atomic_replaces_existing_file_and_removes_temp_file() {
    let root = temp_path("atomic-replace");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let path = root.join(".bashrc");
    fs::write(&path, "old").expect("existing file should be written");

    write_text_atomic(&path, "new\n").expect("file should be replaced atomically");

    assert_eq!(
        fs::read_to_string(&path).expect("updated file should be readable"),
        "new\n"
    );
    assert_eq!(leftover_temp_files(&root), 0);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn write_text_atomic_preserves_existing_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_path("atomic-permissions");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let path = root.join(".zshrc");
    fs::write(&path, "old").expect("existing file should be written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .expect("permissions should be set");

    write_text_atomic(&path, "new").expect("file should be replaced atomically");

    let mode = fs::metadata(&path)
        .expect("updated file should have metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn write_text_atomic_preserves_symlink_and_updates_target() {
    use std::os::unix::fs::symlink;

    let root = temp_path("atomic-symlink");
    let home = root.join("home");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&home).expect("home directory should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles directory should be created");
    let target = dotfiles.join("bashrc");
    let link = home.join(".bashrc");
    fs::write(&target, "old\n").expect("target should be written");
    symlink(&target, &link).expect("symlink should be created");

    write_text_atomic(&link, "new\n").expect("symlink target should be replaced atomically");

    assert!(
        fs::symlink_metadata(&link)
            .expect("link metadata should be readable")
            .file_type()
            .is_symlink(),
        "shell integration writes should preserve symlinked startup files"
    );
    assert_eq!(
        fs::read_to_string(&target).expect("target should be readable"),
        "new\n"
    );
    assert_eq!(
        fs::read_to_string(&link).expect("link should still resolve"),
        "new\n"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn resolve_write_path_follows_relative_symlinks() {
    use std::os::unix::fs::symlink;

    let root = temp_path("relative-symlink");
    let home = root.join("home");
    fs::create_dir_all(&home).expect("home directory should be created");
    let target = home.join("actual-zshrc");
    let link = home.join(".zshrc");
    fs::write(&target, "old\n").expect("target should be written");
    symlink("actual-zshrc", &link).expect("relative symlink should be created");

    assert_eq!(
        resolve_write_path(&link).expect("relative symlink should resolve"),
        target
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn remove_managed_blocks_preserves_user_content() {
    let block = managed_script(Shell::Bash, "command elio");
    let existing = format!("alias ll='ls -la'\n\n{block}\nexport EDITOR=nvim\n");

    let updated = remove_managed_blocks(&existing)
        .expect("managed block should be removable")
        .expect("managed block should be found");

    assert_eq!(updated, "alias ll='ls -la'\n\nexport EDITOR=nvim\n");
}

#[test]
fn remove_managed_blocks_removes_duplicate_blocks() {
    let block = managed_script(Shell::Bash, "command elio");
    let existing = format!("{block}\nexport EDITOR=nvim\n\n{block}");

    let updated = remove_managed_blocks(&existing)
        .expect("managed blocks should be removable")
        .expect("managed blocks should be found");

    assert_eq!(updated, "export EDITOR=nvim\n");
}

#[test]
fn remove_managed_blocks_rejects_unclosed_block() {
    let existing = format!("before\n{MANAGED_START}\nfunction elio\n");

    let error = remove_managed_blocks(&existing)
        .expect_err("unclosed managed block should return an error");

    assert!(
        error
            .to_string()
            .contains("start marker without end marker")
    );
}

#[test]
fn upsert_managed_block_replaces_existing_block() {
    let old = format!("{MANAGED_START}\nold\n{MANAGED_END}\n");
    let new = format!("{MANAGED_START}\nnew\n{MANAGED_END}\n");

    let updated = upsert_managed_block(&old, &new).expect("managed block should be replaced");

    assert_eq!(updated.matches(MANAGED_START).count(), 1);
    assert!(updated.contains("new"));
    assert!(!updated.contains("old"));
}

#[test]
fn upsert_managed_block_collapses_duplicate_existing_blocks() {
    let old = format!("{MANAGED_START}\nold\n{MANAGED_END}\n");
    let new = format!("{MANAGED_START}\nnew\n{MANAGED_END}\n");
    let existing = format!("before\n\n{old}\nafter\n\n{old}tail\n");

    let updated = upsert_managed_block(&existing, &new)
        .expect("managed blocks should be replaced and deduplicated");

    assert_eq!(updated.matches(MANAGED_START).count(), 1);
    assert!(updated.contains("new"));
    assert!(updated.contains("before"));
    assert!(updated.contains("after"));
    assert!(updated.contains("tail"));
    assert!(!updated.contains("old"));
}

#[test]
fn upsert_managed_block_rejects_unclosed_block() {
    let existing = format!("{MANAGED_START}\nold\n");
    let new = format!("{MANAGED_START}\nnew\n{MANAGED_END}\n");

    let error =
        upsert_managed_block(&existing, &new).expect_err("unclosed block should return an error");

    assert!(
        error
            .to_string()
            .contains("start marker without end marker")
    );
}
