use super::{fake_open_with_app, fake_terminal_app, temp_dir_path};
use crate::app::state::PendingTerminalTask;
use crate::app::{App, actions::open_with::FallbackOpenOutcome};
use std::{
    cell::{Cell, RefCell},
    fs,
};

#[test]
fn zero_discovered_apps_fall_back_to_default_open() {
    let root = temp_dir_path("fallback-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let fallback_called = Cell::new(false);
    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![],
        |_| {
            fallback_called.set(true);
            Ok(FallbackOpenOutcome::DefaultApp)
        },
        |_| unreachable!("launch should not be called when no apps were discovered"),
    );

    assert!(fallback_called.get(), "fallback opener must be called");
    assert!(
        app.overlays.open_with.is_none(),
        "overlay must remain closed"
    );
    assert_eq!(app.status, "No apps found, opened with default");

    fs::remove_dir_all(root).ok();
}

#[test]
fn single_discovered_app_launches_without_opening_overlay() {
    let root = temp_dir_path("single-launch-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let launched = RefCell::new(None::<String>);
    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![fake_open_with_app("Fake App")],
        |_| unreachable!("fallback should not be called when one app was discovered"),
        |app| {
            *launched.borrow_mut() = Some(app.display_name.clone());
            Ok(())
        },
    );

    assert_eq!(launched.into_inner().as_deref(), Some("Fake App"));
    assert!(
        app.overlays.open_with.is_none(),
        "overlay must remain closed"
    );
    assert_eq!(
        app.status, "Opened with Fake App",
        "successful direct launch should name the app"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn single_discovered_app_launch_failure_sets_status_without_overlay() {
    let root = temp_dir_path("single-launch-fail-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![fake_open_with_app("Ghost App")],
        |_| unreachable!("fallback should not be called when one app was discovered"),
        |_| Err(std::io::Error::new(std::io::ErrorKind::NotFound, "missing")),
    );

    assert!(
        app.overlays.open_with.is_none(),
        "overlay must remain closed"
    );
    assert_eq!(app.status, "Failed to open with Ghost App: missing");

    fs::remove_dir_all(root).ok();
}

#[test]
fn single_terminal_app_queues_pending_command_without_overlay() {
    let root = temp_dir_path("terminal-single-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![fake_terminal_app("Neovim")],
        |_| unreachable!("fallback should not be called"),
        |_| unreachable!("detached launch should not be called for terminal apps"),
    );

    assert!(
        app.overlays.open_with.is_none(),
        "overlay must remain closed for direct terminal launch"
    );
    assert_eq!(
        app.pending_terminal_task,
        Some(PendingTerminalTask::Command {
            program: "nvim".to_string(),
            args: vec!["/tmp/file.txt".to_string()],
        })
    );
    assert!(app.status.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn env_editor_label_does_not_repeat_terminal_suffix() {
    let root = temp_dir_path("env-editor-label-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![
            fake_terminal_app("Neovim ($VISUAL)"),
            fake_open_with_app("Gedit"),
        ],
        |_| unreachable!(),
        |_| unreachable!(),
    );

    assert_eq!(app.open_with_row_label(0), "Neovim ($VISUAL)");

    fs::remove_dir_all(root).ok();
}

#[test]
fn confirm_terminal_app_from_overlay_queues_pending_command() {
    let root = temp_dir_path("terminal-overlay-root");
    fs::create_dir_all(&root).expect("create temp root");
    fs::write(root.join("file.txt"), "hello").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    // Put two apps in the overlay (terminal + gui) so it opens.
    app.handle_discovered_open_with_apps(
        &root.join("file.txt"),
        vec![fake_terminal_app("Neovim"), fake_open_with_app("Gedit")],
        |_| unreachable!(),
        |_| unreachable!(),
    );
    assert!(app.overlays.open_with.is_some(), "overlay should be open");

    // The first row is the terminal app. Confirm it.
    app.confirm_open_with_index(0)
        .expect("confirm should not error");

    assert!(app.overlays.open_with.is_none(), "overlay must close");
    assert_eq!(
        app.pending_terminal_task,
        Some(PendingTerminalTask::Command {
            program: "nvim".to_string(),
            args: vec!["/tmp/file.txt".to_string()],
        })
    );
    assert!(app.status.is_empty());

    fs::remove_dir_all(root).ok();
}

#[cfg(target_os = "macos")]
#[test]
fn zero_discovered_apps_can_report_text_editor_fallback() {
    let root = temp_dir_path("text-editor-fallback-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.rs");
    fs::write(&path, "fn main() {}\n").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![],
        |_| Ok(FallbackOpenOutcome::TextEditor),
        |_| unreachable!("launch should not be called when no apps were discovered"),
    );

    assert!(app.overlays.open_with.is_none());
    assert_eq!(app.status, "No apps found, opened in text editor");

    fs::remove_dir_all(root).ok();
}
