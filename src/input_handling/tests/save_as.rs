use super::super::*;
use super::helpers::{
    OpenInSystemCaptureGuard, cleanup_app_temp_root, read_open_capture, temp_path,
    wait_for_directory_load,
};
use crate::chooser::ChooserExit;
use std::fs;

fn key(app: &mut App, code: KeyCode) {
    app.handle_save_as_key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn save_as_preserves_name_across_navigation_and_does_not_create_destination() {
    let root = temp_path("save-as-navigation");
    let child = root.join("child");
    fs::create_dir(&child).unwrap();
    let mut app = App::new_at(root.clone()).unwrap();
    app.enable_save_as_mode("résumé.txt".into());
    app.confirm_chooser();
    key(&mut app, KeyCode::Esc);
    app.set_dir(child.clone()).unwrap();
    super::helpers::wait_for_directory_load(&mut app);
    app.confirm_chooser();
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.take_chooser_exit(),
        Some(ChooserExit::Confirmed(vec![child.join("résumé.txt")]))
    );
    assert!(!child.join("résumé.txt").exists());
    cleanup_app_temp_root(app, root);
}

#[test]
fn overwrite_confirmation_stays_bound_to_submitted_directory() {
    let root = temp_path("save-as-pending-navigation");
    let child = root.join("child");
    fs::create_dir(&child).unwrap();
    let original = root.join("document");
    let navigated = child.join("document");
    fs::write(&original, "original").unwrap();
    fs::write(&navigated, "other").unwrap();
    let mut app = App::new_at(root.clone()).unwrap();
    app.enable_save_as_mode("document".into());
    app.set_dir(child.clone()).unwrap();
    app.confirm_chooser();
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.chooser.save_as().unwrap().overwrite_path(),
        Some(original.as_path())
    );
    super::helpers::wait_for_directory_load(&mut app);
    assert_eq!(app.file_browser.cwd, child);
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.take_chooser_exit(),
        Some(ChooserExit::Confirmed(vec![original.clone()]))
    );
    assert_eq!(fs::read_to_string(original).unwrap(), "original");
    assert_eq!(fs::read_to_string(navigated).unwrap(), "other");
    cleanup_app_temp_root(app, root);
}

#[test]
fn save_as_editing_handles_unicode_and_single_line_paste() {
    let root = temp_path("save-as-editing");
    let mut app = App::new_at(root.clone()).unwrap();
    app.enable_save_as_mode(String::new());
    app.confirm_chooser();
    for ch in "résumé".chars() {
        key(&mut app, KeyCode::Char(ch));
    }
    app.handle_event(Event::Paste(" café\r\nignored".into()))
        .unwrap();
    let state = app.chooser.save_as().unwrap();
    assert_eq!(state.input(), "résumé café ignored");
    assert_eq!(state.cursor_col(), state.input().chars().count());
    cleanup_app_temp_root(app, root);
}

#[test]
fn overwrite_requires_confirmation_and_selection_cancel_returns_to_input() {
    let root = temp_path("save-as-overwrite");
    let target = root.join("existing");
    fs::write(&target, "keep me").unwrap();
    let mut app = App::new_at(root.clone()).unwrap();
    app.enable_save_as_mode("existing".into());
    app.confirm_chooser();
    key(&mut app, KeyCode::Enter);
    assert!(app.chooser.save_as().unwrap().overwrite());
    key(&mut app, KeyCode::Esc);
    assert!(!app.chooser.save_as().unwrap().overwrite());
    assert!(app.chooser.save_as().unwrap().is_open());
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Right);
    assert!(!app.chooser.save_as().unwrap().overwrite_confirmed());
    key(&mut app, KeyCode::Left);
    assert!(app.chooser.save_as().unwrap().overwrite_confirmed());
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Enter);
    assert!(!app.chooser.save_as().unwrap().overwrite());
    assert!(app.take_chooser_exit().is_none());
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.take_chooser_exit(),
        Some(ChooserExit::Confirmed(vec![target.clone()]))
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "keep me");
    cleanup_app_temp_root(app, root);
}

#[test]
fn save_as_rejects_invalid_names_and_directories() {
    let root = temp_path("save-as-invalid");
    fs::create_dir(root.join("directory")).unwrap();
    let mut app = App::new_at(root.clone()).unwrap();
    for name in ["directory", "", "..", "a/b"] {
        app.enable_save_as_mode(name.into());
        app.confirm_chooser();
        key(&mut app, KeyCode::Enter);
        assert!(app.chooser.save_as().unwrap().error().is_some(), "{name:?}");
        assert!(!app.should_quit);
    }
    cleanup_app_temp_root(app, root);
}

#[test]
fn save_as_double_click_opens_file_instead_of_save_prompt() {
    let root = temp_path("save-as-mouse");
    let file = root.join("file");
    fs::write(&file, "unchanged").unwrap();
    let capture = root.join("capture.txt");
    let _capture_guard = OpenInSystemCaptureGuard::install(capture.clone());
    let mut app = App::new_at(root.clone()).unwrap();
    wait_for_directory_load(&mut app);
    app.enable_save_as_mode("draft".into());
    let file_index = app
        .file_browser
        .entries
        .iter()
        .position(|entry| entry.path == file)
        .unwrap();
    app.set_screen_regions(ScreenRegions {
        entry_hits: vec![EntryHit {
            rect: Rect::new(0, 1, 20, 1),
            index: file_index,
        }],
        ..ScreenRegions::default()
    });
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_event(Event::Mouse(click)).unwrap();
    app.handle_event(Event::Mouse(click)).unwrap();
    assert!(!app.chooser.save_as().unwrap().is_open());
    assert!(app.take_chooser_exit().is_none());
    assert_eq!(read_open_capture(&capture), file.display().to_string());
    assert_eq!(app.status, "Opened file");
    cleanup_app_temp_root(app, root);
}

#[cfg(unix)]
#[test]
fn save_as_rejects_symlink_and_special_file_destinations() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let root = std::path::Path::new("/tmp").join(format!(
        "elio-save-as-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    fs::write(root.join("target"), "keep").unwrap();
    symlink(root.join("target"), root.join("link")).unwrap();
    let socket = root.join("socket");
    let listener = UnixListener::bind(&socket).unwrap();
    let mut app = App::new_at(root.clone()).unwrap();
    for name in ["link", "socket"] {
        app.enable_save_as_mode(name.into());
        app.confirm_chooser();
        key(&mut app, KeyCode::Enter);
        assert!(app.chooser.save_as().unwrap().error().is_some());
        assert!(!app.should_quit);
    }
    assert_eq!(fs::read_to_string(root.join("target")).unwrap(), "keep");
    drop(listener);
    cleanup_app_temp_root(app, root);
}
