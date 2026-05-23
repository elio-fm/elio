use super::{
    MANAGED_END, MANAGED_START, Shell, binary_command, init_script, managed_script,
    remove_managed_blocks, resolve_write_path, shell_name_from_command, uninstall_reload_command,
    upsert_managed_block, write_text_atomic,
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
fn binary_command_uses_path_for_local_invocations() {
    assert_eq!(
        binary_command(
            Some("target/debug/elio"),
            Path::new("/repo/target/debug/elio")
        ),
        "'/repo/target/debug/elio'"
    );
}

#[test]
fn binary_command_uses_path_for_absolute_invocations() {
    assert_eq!(
        binary_command(Some("/opt/elio/bin/elio"), Path::new("/opt/elio/bin/elio")),
        "'/opt/elio/bin/elio'"
    );
}

#[test]
fn binary_command_uses_path_lookup_for_normal_invocations() {
    assert_eq!(
        binary_command(Some("elio"), Path::new("/versioned/path/elio")),
        "command elio"
    );
}

#[test]
fn posix_init_script_passes_cli_commands_through() {
    let script = init_script(Shell::Bash, "command elio");

    assert!(script.contains("case \"${1-}\" in"));
    assert!(script.contains("shell|-*)"));
    assert!(script.contains("command elio \"$@\""));
    assert!(script.contains("local tmp cwd status_code"));
    assert!(script.contains("command elio --cwd-file \"$tmp\" \"$@\""));
    assert!(script.contains("status_code=$?"));
    assert!(script.contains("return \"$status_code\""));
    assert!(!script.contains("local tmp cwd status\n"));
}

#[test]
fn fish_init_script_passes_cli_commands_through() {
    let script = init_script(Shell::Fish, "command elio");

    assert!(script.contains("switch \"$argv[1]\""));
    assert!(script.contains("case shell '-*'"));
    assert!(script.contains("command elio $argv"));
    assert!(script.contains("command elio --cwd-file \"$tmp\" $argv"));
    assert!(script.contains("cd \"$cwd\"; or return $status"));
}

#[test]
fn shell_name_from_command_handles_paths_login_shells_and_arguments() {
    assert_eq!(
        shell_name_from_command("/usr/bin/zsh\n").as_deref(),
        Some("zsh")
    );
    assert_eq!(shell_name_from_command("-zsh").as_deref(), Some("zsh"));
    assert_eq!(
        shell_name_from_command("/opt/homebrew/bin/fish --login").as_deref(),
        Some("fish")
    );
    assert_eq!(shell_name_from_command("  "), None);
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
    let temp_files = fs::read_dir(&root)
        .expect("temp directory should be readable")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".elio-tmp-"))
        .count();
    assert_eq!(temp_files, 0);

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
