use super::{fake_open_with_app, fake_terminal_app, temp_dir_path};
use crate::app::{App, PendingTerminalTask};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::fs;

#[test]
fn open_with_nav_keys_move_selection() {
    let root = temp_dir_path("nav-keys-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![fake_open_with_app("Gedit"), fake_terminal_app("Neovim")],
        |_| unreachable!(),
        |_| unreachable!(),
    );

    assert_eq!(app.open_with_selected_index(), 0);

    app.handle_open_with_key(KeyEvent::from(KeyCode::Char('j')))
        .expect("nav_down should be handled");
    assert_eq!(app.open_with_selected_index(), 1);

    app.handle_open_with_key(KeyEvent::from(KeyCode::Char('j')))
        .expect("nav_down should clamp");
    assert_eq!(app.open_with_selected_index(), 1);

    app.handle_open_with_key(KeyEvent::from(KeyCode::Char('k')))
        .expect("nav_up should be handled");
    assert_eq!(app.open_with_selected_index(), 0);

    app.handle_open_with_key(KeyEvent::from(KeyCode::Char('k')))
        .expect("nav_up should clamp");
    assert_eq!(app.open_with_selected_index(), 0);

    fs::remove_dir_all(root).ok();
}

#[test]
fn open_with_shortcuts_skip_nav_keys_without_dropping_rows() {
    let root = temp_dir_path("shortcut-precedence-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let apps: Vec<_> = (0..40)
        .map(|index| fake_open_with_app(&format!("App {index}")))
        .collect();

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(&path, apps, |_| unreachable!(), |_| unreachable!());

    assert_eq!(app.open_with_row_count(), 40);
    assert!(
        (0..app.open_with_row_count())
            .all(|index| { !matches!(app.open_with_row_shortcut(index), Some('j' | 'k')) })
    );

    app.handle_open_with_key(KeyEvent::from(KeyCode::Char('j')))
        .expect("nav_down should be handled");

    assert!(app.overlays.open_with.is_some(), "overlay must stay open");
    assert_eq!(app.open_with_selected_index(), 1);

    fs::remove_dir_all(root).ok();
}

#[test]
fn open_with_mouse_wheel_moves_selection() {
    let root = temp_dir_path("mouse-wheel-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![fake_open_with_app("Gedit"), fake_terminal_app("Neovim")],
        |_| unreachable!(),
        |_| unreachable!(),
    );

    app.handle_open_with_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    })
    .expect("wheel down should be handled");
    assert_eq!(app.open_with_selected_index(), 1);

    app.handle_open_with_mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    })
    .expect("wheel up should be handled");
    assert_eq!(app.open_with_selected_index(), 0);

    fs::remove_dir_all(root).ok();
}

#[test]
fn open_with_open_or_enter_confirms_selected_row() {
    let root = temp_dir_path("nav-confirm-root");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("file.txt");
    fs::write(&path, "hello").expect("write temp file");

    let mut app = App::new_at(root.clone()).expect("create app");
    app.handle_discovered_open_with_apps(
        &path,
        vec![fake_open_with_app("Gedit"), fake_terminal_app("Neovim")],
        |_| unreachable!(),
        |_| unreachable!(),
    );

    app.handle_open_with_key(KeyEvent::from(KeyCode::Char('j')))
        .expect("nav_down should be handled");
    app.handle_open_with_key(KeyEvent::from(KeyCode::Enter))
        .expect("open_or_enter should confirm selection");

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
