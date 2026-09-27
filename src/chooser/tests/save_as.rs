use super::super::save_as::{SaveAsState, resolve_startup, validate_name, validate_windows_name};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "elio-save-as-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn startup_resolves_destinations_without_creating_them() {
    let root = temp("startup");
    let child = root.join("child");
    fs::create_dir_all(&child).unwrap();

    let target = root.join("report.txt");
    let startup = resolve_startup(&root, Some(&target)).unwrap();
    assert_eq!(startup.directory, root);
    assert_eq!(startup.name, "report.txt");
    assert!(!target.exists());

    let startup = resolve_startup(&root, Some(&child)).unwrap();
    assert_eq!(startup.directory, child);
    assert!(startup.name.is_empty());
    assert!(resolve_startup(&root, Some(std::path::Path::new("missing/file"))).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn startup_allows_directory_symlinks_but_rejects_file_symlinks() {
    use std::os::unix::fs::symlink;
    let root = temp("startup-symlinks");
    fs::create_dir_all(root.join("directory")).unwrap();
    fs::write(root.join("file"), "keep").unwrap();
    let directory_link = root.join("directory-link");
    let file_link = root.join("file-link");
    symlink(root.join("directory"), &directory_link).unwrap();
    symlink(root.join("file"), &file_link).unwrap();
    assert_eq!(
        resolve_startup(&root, Some(&directory_link))
            .unwrap()
            .directory,
        directory_link
    );
    assert!(resolve_startup(&root, Some(&file_link)).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn names_are_valid_basenames() {
    assert!(validate_name("report.txt").is_ok());
    for name in ["", "a/b", ".."] {
        assert!(validate_name(name).is_err(), "{name:?}");
    }
    for name in ["CON", "com1.txt", "report.", "a?b"] {
        assert!(validate_windows_name(name).is_err(), "{name:?}");
    }
    for name in ["résumé.txt", ".hidden", "COM10.txt"] {
        assert!(validate_windows_name(name).is_ok(), "{name:?}");
    }
}

#[test]
fn overwrite_path_is_bound_and_cleared() {
    let path = PathBuf::from("submitted/document");
    let mut state = SaveAsState::new("document".into());
    state.ask_overwrite(&path);
    *state.input_mut().0 = "changed".into();
    assert_eq!(state.overwrite_path(), Some(path.as_path()));
    state.cancel_overwrite();
    assert!(!state.overwrite());
}
