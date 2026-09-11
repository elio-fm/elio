use super::*;

fn item(path: &Path, is_dir: bool) -> BulkRenameItem {
    BulkRenameItem {
        path: path.to_path_buf(),
        original_name: path_name(path),
        is_dir,
    }
}

#[cfg(unix)]
#[test]
fn unresolved_elevation_cannot_prepare_editor_document() {
    assert!(
        editor_temp_owner(&crate::elevated_session::InvocationContext::ElevatedUnresolved).is_err()
    );
    assert!(
        editor_temp_owner(&crate::elevated_session::InvocationContext::Normal)
            .expect("normal session should be accepted")
            .is_none()
    );
    assert!(
        editor_temp_owner(&crate::elevated_session::InvocationContext::RootSession)
            .expect("root session should be accepted")
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn invoking_user_editor_document_is_private_and_readable() {
    use std::os::unix::fs::MetadataExt;

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let path = create_temp_file(&["alpha.txt".to_string()], Some((uid, gid)))
        .expect("failed to create invoking-user editor document");
    let metadata = fs::metadata(&path).expect("failed to stat editor document");

    assert!(path.is_absolute());
    assert_eq!(path.parent(), Some(Path::new("/tmp")));
    assert_eq!(metadata.uid(), uid);
    assert_eq!(metadata.gid(), gid);
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(
        read_editor_rename_file(&path, Some(uid)).expect("failed to read editor document"),
        "alpha.txt\n"
    );
    fs::remove_file(path).expect("failed to remove editor document");
}

#[cfg(unix)]
#[test]
fn invoking_user_editor_document_accepts_atomic_save_replacement() {
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let path = create_temp_file(&["alpha.txt".to_string()], Some((uid, gid)))
        .expect("failed to create invoking-user editor document");
    let replacement = path.with_extension("replacement");
    fs::write(&replacement, "renamed.txt\n").expect("failed to write replacement document");
    fs::rename(&replacement, &path).expect("failed to replace editor document");

    assert_eq!(
        read_editor_rename_file(&path, Some(uid)).expect("failed to read replacement document"),
        "renamed.txt\n"
    );
    fs::remove_file(path).expect("failed to remove editor document");
}

#[cfg(unix)]
#[test]
fn invoking_user_editor_document_rejects_symlinks_and_oversized_files() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let path = create_temp_file(&["alpha.txt".to_string()], Some((uid, gid)))
        .expect("failed to create invoking-user editor document");
    let target = path.with_extension("target");
    fs::write(&target, "renamed.txt\n").expect("failed to write symlink target");
    fs::remove_file(&path).expect("failed to remove editor document");
    symlink(&target, &path).expect("failed to create editor document symlink");
    assert!(read_editor_rename_file(&path, Some(uid)).is_err());
    fs::remove_file(&path).expect("failed to remove editor document symlink");
    fs::remove_file(target).expect("failed to remove symlink target");

    let path = create_temp_file(&["alpha.txt".to_string()], Some((uid, gid)))
        .expect("failed to create invoking-user editor document");
    fs::write(&path, vec![b'a'; MAX_EDITOR_RENAME_BYTES as usize + 1])
        .expect("failed to write oversized editor document");
    assert!(read_editor_rename_file(&path, Some(uid)).is_err());
    fs::remove_file(path).expect("failed to remove oversized editor document");

    let path = create_temp_file(&["alpha.txt".to_string()], Some((uid, gid)))
        .expect("failed to create invoking-user editor document");
    fs::set_permissions(&path, std::fs::Permissions::from_mode(0o622))
        .expect("failed to make editor document world-writable");
    assert!(read_editor_rename_file(&path, Some(uid)).is_err());
    fs::remove_file(path).expect("failed to remove world-writable editor document");
}

#[test]
fn common_root_uses_parent_paths() {
    let paths = vec![
        PathBuf::from("/tmp/root/left/a.txt"),
        PathBuf::from("/tmp/root/right/b.txt"),
    ];
    assert_eq!(common_root(&paths), PathBuf::from("/tmp/root"));
}

#[test]
fn editor_plan_accepts_relative_paths_in_multiple_directories() {
    let root = std::env::temp_dir().join(format!("elio-editor-rename-plan-{}", std::process::id()));
    std::fs::create_dir_all(root.join("left")).expect("failed to create left dir");
    std::fs::create_dir_all(root.join("right")).expect("failed to create right dir");
    let items = vec![
        item(&root.join("left/a.txt"), false),
        item(&root.join("right/b.txt"), false),
    ];
    let names = vec![
        "left/renamed.txt".to_string(),
        "right/renamed.txt".to_string(),
    ];
    let plan = build_rename_plan(&items, &names, Some(&root)).expect("plan should build");
    assert_eq!(plan[0].new_path, root.join("left/renamed.txt"));
    assert_eq!(plan[1].new_path, root.join("right/renamed.txt"));
    std::fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn editor_plan_rejects_parent_traversal() {
    let root = PathBuf::from("/tmp/root");
    let items = vec![item(&root.join("a.txt"), false)];
    let names = vec!["../outside.txt".to_string()];
    let errors = build_rename_plan(&items, &names, Some(&root)).expect_err("plan should fail");
    assert_eq!(errors[0].as_deref(), Some("Path cannot contain . or .."));
}

#[test]
fn apply_failure_rolls_back_chained_renames_without_overwriting() {
    let root = std::env::temp_dir().join(format!(
        "elio-editor-rename-rollback-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("failed to create temp root");
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    let c = root.join("c.txt");
    std::fs::write(&a, "alpha").expect("failed to write a");
    std::fs::write(&b, "beta").expect("failed to write b");
    std::fs::create_dir(&c).expect("failed to create blocking directory");

    let ops = vec![
        RenameOp {
            old_path: a.clone(),
            original_label: "a.txt".to_string(),
            new_label: "b.txt".to_string(),
            new_path: b.clone(),
        },
        RenameOp {
            old_path: b.clone(),
            original_label: "b.txt".to_string(),
            new_label: "c.txt".to_string(),
            new_path: c.clone(),
        },
    ];

    let error = apply_rename_ops(&ops).expect_err("second apply should fail");
    assert!(error.to_string().contains("Could not rename \"b.txt\""));
    assert_eq!(
        std::fs::read_to_string(&a).expect("a should be restored"),
        "alpha"
    );
    assert_eq!(
        std::fs::read_to_string(&b).expect("b should be restored"),
        "beta"
    );
    assert!(c.is_dir());
    assert!(
        std::fs::read_dir(&root)
            .expect("root should be readable")
            .all(|entry| !entry
                .expect("entry should be readable")
                .file_name()
                .to_string_lossy()
                .starts_with(".elio-rename-"))
    );

    std::fs::remove_dir_all(root).expect("failed to remove temp root");
}
