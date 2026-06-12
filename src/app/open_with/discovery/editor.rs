use std::{
    env, fs,
    path::{Path, PathBuf},
};

use super::super::super::state::OpenWithApp;
use super::exec::tokenize_exec;

pub(super) fn append_editor_fallback(apps: &mut Vec<OpenWithApp>, path: &Path) {
    let Some(app) = editor_fallback_for_path(path) else {
        return;
    };
    if !duplicates_discovered_app(&app, apps) {
        apps.push(app);
    }
}

pub(super) fn editor_fallback_for_path(path: &Path) -> Option<OpenWithApp> {
    if !super::super::path_is_text_like(path) {
        return None;
    }

    let path_str = path.to_str()?;

    for var in ["VISUAL", "EDITOR"] {
        let Some(value) = env::var_os(var).and_then(|value| value.into_string().ok()) else {
            continue;
        };
        if let Some(app) = editor_app_from_command(var, &value, path_str) {
            return Some(app);
        }
    }

    None
}

fn editor_app_from_command(var: &str, command: &str, path_str: &str) -> Option<OpenWithApp> {
    let mut tokens = tokenize_exec(command);
    if tokens.is_empty() {
        return None;
    }

    let program = tokens.remove(0);
    let resolved = resolve_executable(&program)?;
    let program_name = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .or_else(|| {
            Path::new(&program)
                .file_name()
                .and_then(|name| name.to_str())
        })?;
    let display_name = editor_display_name(program_name);

    tokens.push(path_str.to_string());
    Some(OpenWithApp {
        display_name: format!("{display_name} (${var})"),
        desktop_id: None,
        program,
        args: tokens,
        is_default: false,
        requires_terminal: true,
    })
}

fn duplicates_discovered_app(editor: &OpenWithApp, apps: &[OpenWithApp]) -> bool {
    let editor_program = program_key(&editor.program);
    apps.iter()
        .any(|app| program_key(&app.program) == editor_program)
}

fn program_key(program: &str) -> Option<String> {
    let path = resolve_executable(program).unwrap_or_else(|| PathBuf::from(program));
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
}

fn resolve_executable(program: &str) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }

    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return executable_file_exists(program_path).then(|| canonical_path(program_path));
    }

    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|path| executable_file_exists(path))
            .map(|path| canonical_path(&path))
    })
}

fn canonical_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn executable_file_exists(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

fn editor_display_name(program_name: &str) -> &str {
    match program_name.to_ascii_lowercase().as_str() {
        "nvim" => "Neovim",
        "vim" => "Vim",
        "vi" => "Vi",
        "helix" => "Helix",
        "micro" => "Micro",
        "nano" => "Nano",
        "emacs" => "Emacs",
        "kak" | "kakoune" => "Kakoune",
        _ => program_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsString,
        io::Write,
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = file.metadata().expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod executable");
        }
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

        let mut apps = vec![OpenWithApp {
            display_name: "Text Editor".to_string(),
            desktop_id: Some("org.gnome.gedit.desktop".to_string()),
            program: "gedit".to_string(),
            args: vec![file.display().to_string()],
            is_default: true,
            requires_terminal: false,
        }];
        append_editor_fallback(&mut apps, &file);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(apps.len(), 2);
        assert_eq!(apps[1].display_name, "hx ($EDITOR)");
        assert_eq!(apps[1].program, "hx");
        assert_eq!(apps[1].args, vec![file.display().to_string()]);
        assert!(apps[1].requires_terminal);
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

        let mut apps = vec![OpenWithApp {
            display_name: "Helix".to_string(),
            desktop_id: Some("Helix.desktop".to_string()),
            program: bin.join("hx").display().to_string(),
            args: vec![file.display().to_string()],
            is_default: true,
            requires_terminal: true,
        }];
        append_editor_fallback(&mut apps, &file);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(apps.len(), 1);
    }
}
