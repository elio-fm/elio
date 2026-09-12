use super::clipboard_test_support::*;

#[cfg(unix)]
#[test]
fn link_yanked_creates_absolute_symlink_in_current_directory() {
    let src_dir = temp_path("link-absolute-src");
    let dst_dir = temp_path("link-absolute-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    let source = src_dir.join("repo");
    fs::create_dir_all(&source).unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst_dir.clone();

    app.link_yanked(false).unwrap();

    let link = dst_dir.join("repo");
    assert_eq!(fs::read_link(&link).unwrap(), source);
    assert_eq!(app.status_message(), "Created symlink \"repo\"");
    assert_eq!(
        app.file_operations.clipboard_info(),
        Some((1, ClipOp::Yank))
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

#[cfg(unix)]
#[test]
fn link_yanked_creates_relative_symlink_in_current_directory() {
    let root = temp_path("link-relative");
    let source = root.join("sources/repo");
    let dest = root.join("project/context");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&dest).unwrap();

    let mut app = App::new_at(source.parent().unwrap().to_path_buf()).unwrap();
    app.yank();
    app.file_browser.cwd = dest.clone();

    app.link_yanked(true).unwrap();

    assert_eq!(
        fs::read_link(dest.join("repo")).unwrap(),
        PathBuf::from("../../sources/repo")
    );
    assert_eq!(app.status_message(), "Created symlink \"repo\"");

    fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn link_yanked_uses_unique_destination_names() {
    let src_dir = temp_path("link-unique-src");
    let dst_dir = temp_path("link-unique-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("note.txt"), "note").unwrap();
    fs::write(dst_dir.join("note.txt"), "existing").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst_dir.clone();

    app.link_yanked(false).unwrap();

    assert_eq!(
        fs::read_link(dst_dir.join("note_1.txt")).unwrap(),
        src_dir.join("note.txt")
    );
    assert_eq!(app.status_message(), "Created symlink \"note_1.txt\"");

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

#[cfg(unix)]
#[test]
fn link_yanked_refuses_cut_clipboard() {
    let src_dir = temp_path("link-cut-src");
    let dst_dir = temp_path("link-cut-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("move.txt"), "move").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.cut();
    app.file_browser.cwd = dst_dir.clone();

    app.link_yanked(false).unwrap();

    assert_eq!(app.status_message(), "Yank items before linking");
    assert!(!dst_dir.join("move.txt").exists());
    assert_eq!(app.file_operations.clipboard_info(), Some((1, ClipOp::Cut)));

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}
