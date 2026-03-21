use super::super::*;
use super::helpers::{temp_path, wait_for_directory_load};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn opening_a_removed_directory_does_not_bubble_an_error() {
    let root = temp_path("removed-directory-open");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create temp dirs");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    fs::remove_dir_all(&child).expect("failed to remove child dir");

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .expect("stale directory open should be handled");

    assert_eq!(app.cwd, root);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn opening_a_protected_directory_reports_permission_denied() {
    let root = temp_path("protected-directory-open");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create temp dirs");
    fs::set_permissions(&child, fs::Permissions::from_mode(0o000))
        .expect("failed to lock child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .expect("protected directory open should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, root);
    assert!(app.status_message().contains("Permission denied"));

    fs::set_permissions(&child, fs::Permissions::from_mode(0o755))
        .expect("failed to unlock child dir");
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
