use super::*;
use std::{
    env,
    ffi::OsString,
    io::Write,
    os::unix::fs::PermissionsExt,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let original = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }
        Self { key, original }
    }

    fn remove(key: &'static str) -> Self {
        let original = env::var_os(key);
        unsafe {
            env::remove_var(key);
        }
        Self { key, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.original.as_ref() {
            Some(value) => unsafe {
                env::set_var(self.key, value);
            },
            None => unsafe {
                env::remove_var(self.key);
            },
        }
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!("elio-editor-fallback-{label}-{unique}"))
}

fn write_executable(path: &Path) {
    let mut file = fs::File::create(path).expect("create executable");
    writeln!(file, "#!/bin/sh").expect("write shebang");
    let mut permissions = file.metadata().expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod executable");
}

#[test]
fn editor_fallback_adds_path_editor_for_text_files() {
    let _lock = env_lock().lock().expect("lock env");
    let root = temp_dir("adds-editor");
    fs::create_dir_all(&root).expect("create root");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("create bin");
    write_executable(&bin.join("hx"));
    let file = root.join("note.txt");
    fs::write(&file, "hello\n").expect("write text file");

    let _path = EnvGuard::set("PATH", &bin);
    let _visual = EnvGuard::remove("VISUAL");
    let _editor = EnvGuard::set("EDITOR", "hx");

    let mut applications = vec![
        OpenWithApplication {
            display_name: "Text Editor".to_string(),
            application_id: Some("org.gnome.gedit.desktop".to_string()),
            program: "gedit".to_string(),
            args: vec![file.display().to_string()],
            is_default: true,
            requires_terminal: false,
        },
        OpenWithApplication {
            display_name: "Other App".to_string(),
            application_id: Some("other.desktop".to_string()),
            program: "other-app".to_string(),
            args: vec![file.display().to_string()],
            is_default: false,
            requires_terminal: false,
        },
    ];
    append_environment_editors(&mut applications, &file, true);
    let _ = fs::remove_dir_all(&root);

    assert_eq!(applications.len(), 3);
    assert_eq!(applications[0].display_name, "Text Editor");
    assert_eq!(applications[1].display_name, "hx ($EDITOR)");
    assert_eq!(applications[1].program, "hx");
    assert_eq!(applications[1].args, vec![file.display().to_string()]);
    assert!(applications[1].requires_terminal);
    assert_eq!(applications[2].display_name, "Other App");
}

#[test]
fn editor_fallback_dedupes_matching_program() {
    let _lock = env_lock().lock().expect("lock env");
    let root = temp_dir("dedupe");
    fs::create_dir_all(&root).expect("create root");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("create bin");
    write_executable(&bin.join("hx"));
    let file = root.join("note.txt");
    fs::write(&file, "hello\n").expect("write text file");

    let _path = EnvGuard::set("PATH", &bin);
    let _visual = EnvGuard::remove("VISUAL");
    let _editor = EnvGuard::set("EDITOR", "hx");

    let mut applications = vec![OpenWithApplication {
        display_name: "Helix".to_string(),
        application_id: Some("Helix.desktop".to_string()),
        program: bin.join("hx").display().to_string(),
        args: vec![file.display().to_string()],
        is_default: true,
        requires_terminal: true,
    }];
    append_environment_editors(&mut applications, &file, true);
    let _ = fs::remove_dir_all(&root);

    assert_eq!(applications.len(), 1);
    assert_eq!(applications[0].display_name, "hx ($EDITOR)");
}

#[test]
fn editor_fallback_keeps_visual_label_when_editor_matches_visual() {
    let _lock = env_lock().lock().expect("lock env");
    let root = temp_dir("visual-before-editor");
    fs::create_dir_all(&root).expect("create root");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("create bin");
    write_executable(&bin.join("nvim"));
    let file = root.join("note.txt");
    fs::write(&file, "hello\n").expect("write text file");

    let _path = EnvGuard::set("PATH", &bin);
    let _visual = EnvGuard::set("VISUAL", "nvim");
    let _editor = EnvGuard::set("EDITOR", "nvim");

    let mut applications = vec![OpenWithApplication {
        display_name: "Neovim".to_string(),
        application_id: Some("nvim.desktop".to_string()),
        program: bin.join("nvim").display().to_string(),
        args: vec![file.display().to_string()],
        is_default: false,
        requires_terminal: true,
    }];
    append_environment_editors(&mut applications, &file, true);
    let _ = fs::remove_dir_all(&root);

    assert_eq!(applications.len(), 1);
    assert_eq!(applications[0].display_name, "Neovim ($VISUAL)");
}
