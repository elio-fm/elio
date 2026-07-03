use super::super::*;
use super::helpers::{
    temp_path, wait_for_directory_load, write_binary_zip_entries,
    write_encrypted_seven_zip_entries, write_encrypted_zip_entries,
};
use std::{fs, thread, time::Duration};

#[test]
fn e_extracts_focused_zip_archive() {
    let root = temp_path("extract-zip-key");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("sample.zip");
    write_binary_zip_entries(&archive, &[("dir/file.txt", b"hello")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('e'))))
        .expect("e should start archive extraction");

    let extracted_file = root.join("sample/dir/file.txt");
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if extracted_file.exists() && app.jobs.archive_extract_progress.is_none() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    wait_for_directory_load(&mut app);

    assert_eq!(fs::read_to_string(&extracted_file).unwrap(), "hello");
    assert_eq!(app.status_message(), "Extracted 1 item to \"sample\"");
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(root.join("sample").as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn e_prompts_and_retries_encrypted_seven_zip_archive() {
    let root = temp_path("extract-encrypted-7z-key");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("sample.7z");
    let password = archive_test_password(&root);
    let wrong_password = format!("{password}-wrong");
    write_encrypted_seven_zip_entries(&archive, &password, &[("dir/file.txt", b"hello")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('e'))))
        .expect("e should start archive extraction");
    wait_for_archive_password_prompt(&mut app);

    assert!(app.archive_password_is_open());
    assert_eq!(app.archive_password_error(), None);
    assert!(!root.join("sample").exists());

    type_archive_password(&mut app, &wrong_password);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should submit wrong password");
    wait_for_archive_password_prompt(&mut app);

    assert!(app.archive_password_is_open());
    assert_eq!(app.archive_password_error(), Some("Wrong password"));
    assert_eq!(app.archive_password_input(), "");

    type_archive_password(&mut app, &password);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should submit correct password");

    let extracted_file = root.join("sample/dir/file.txt");
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if extracted_file.exists()
            && app.jobs.archive_extract_progress.is_none()
            && !app.archive_password_is_open()
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    wait_for_directory_load(&mut app);

    assert_eq!(fs::read_to_string(&extracted_file).unwrap(), "hello");
    assert_eq!(app.status_message(), "Extracted 1 item to \"sample\"");
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(root.join("sample").as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn e_prompts_and_retries_encrypted_rar_archive() {
    let root = temp_path("extract-encrypted-rar-key");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("sample.rar");
    let password = archive_test_password(&root);
    let wrong_password = format!("{password}-wrong");
    write_encrypted_seven_zip_entries(&archive, &password, &[("dir/file.txt", b"hello")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('e'))))
        .expect("e should start archive extraction");
    wait_for_archive_password_prompt(&mut app);

    assert!(app.archive_password_is_open());
    assert_eq!(app.archive_password_error(), None);
    assert!(!root.join("sample").exists());

    type_archive_password(&mut app, &wrong_password);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should submit wrong password");
    wait_for_archive_password_prompt(&mut app);

    assert!(app.archive_password_is_open());
    assert_eq!(app.archive_password_error(), Some("Wrong password"));
    assert_eq!(app.archive_password_input(), "");

    type_archive_password(&mut app, &password);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should submit correct password");

    let extracted_file = root.join("sample/dir/file.txt");
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if extracted_file.exists()
            && app.jobs.archive_extract_progress.is_none()
            && !app.archive_password_is_open()
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    wait_for_directory_load(&mut app);

    assert_eq!(fs::read_to_string(&extracted_file).unwrap(), "hello");
    assert_eq!(app.status_message(), "Extracted 1 item to \"sample\"");
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(root.join("sample").as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn e_prompts_and_retries_encrypted_zip_archive() {
    let root = temp_path("extract-encrypted-zip-key");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("sample.zip");
    let password = archive_test_password(&root);
    let wrong_password = format!("{password}-wrong");
    write_encrypted_zip_entries(&archive, &password, &[("dir/file.txt", b"hello")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('e'))))
        .expect("e should start archive extraction");
    wait_for_archive_password_prompt(&mut app);

    assert!(app.archive_password_is_open());
    assert_eq!(app.archive_password_error(), None);
    assert!(!root.join("sample").exists());

    type_archive_password(&mut app, &wrong_password);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should submit wrong password");
    wait_for_archive_password_prompt(&mut app);

    assert!(app.archive_password_is_open());
    assert_eq!(app.archive_password_error(), Some("Wrong password"));
    assert_eq!(app.archive_password_input(), "");

    type_archive_password(&mut app, &password);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should submit correct password");

    let extracted_file = root.join("sample/dir/file.txt");
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if extracted_file.exists()
            && app.jobs.archive_extract_progress.is_none()
            && !app.archive_password_is_open()
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    wait_for_directory_load(&mut app);

    assert_eq!(fs::read_to_string(&extracted_file).unwrap(), "hello");
    assert_eq!(app.status_message(), "Extracted 1 item to \"sample\"");
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(root.join("sample").as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn e_reports_unsupported_archive_format() {
    let root = temp_path("extract-unsupported-key");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("note.txt"), "hello").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('e'))))
        .expect("e should handle unsupported files");

    assert_eq!(
        app.status_message(),
        "Extraction supports ZIP, 7z, RAR, TAR, TAR.GZ, TAR.XZ, TAR.BZ2, and TAR.ZST"
    );
    assert!(app.jobs.archive_extract_progress.is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn c_create_archive_clears_selection_when_started() {
    let root = temp_path("create-clears-selection");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let alpha = root.join("alpha.txt");
    let beta = root.join("beta.txt");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(alpha.clone());
    app.navigation.selected_paths.insert(beta.clone());

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('C'))))
        .expect("C should open archive creation");
    assert!(app.archive_create_is_open());
    assert_eq!(app.archive_create_input(), "archive.zip");
    assert_eq!(app.archive_create_cursor_col(), "archive".chars().count());
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should start archive creation");

    assert!(
        app.navigation.selected_paths.is_empty(),
        "starting archive creation should clear the consumed selection"
    );

    let archive = root.join("archive.zip");
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if archive.exists() && app.jobs.archive_create_progress.is_none() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    wait_for_directory_load(&mut app);

    assert!(archive.exists());
    assert_eq!(app.status_message(), "Created \"archive.zip\"");
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(archive.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cancel_keys_clear_selection_before_cancelling_archive_creation() {
    for (label, key) in [
        ("esc", KeyEvent::from(KeyCode::Esc)),
        (
            "ctrl-c",
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ),
    ] {
        let root = temp_path(&format!("archive-cancel-selection-first-{label}"));
        fs::create_dir_all(&root).expect("failed to create temp root");
        let alpha = root.join("alpha.txt");
        fs::write(&alpha, "alpha").expect("failed to write alpha");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        wait_for_directory_load(&mut app);
        app.navigation.selected_paths.insert(alpha);
        app.jobs.archive_create_progress = Some(crate::app::ArchiveCreateProgress {
            completed: 0,
            total: 1,
        });

        app.handle_event(Event::Key(key))
            .expect("cancel key should be handled");

        assert!(app.navigation.selected_paths.is_empty());
        assert!(
            app.jobs.archive_create_progress.is_some(),
            "first cancel key should clear selection instead of cancelling archive creation"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn archive_create_contents_list_scrolls_with_mouse_wheel() {
    let root = temp_path("archive-create-scroll");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..12 {
        let path = root.join(format!("item-{index}.txt"));
        fs::write(&path, "item").expect("failed to write item");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    for index in 0..12 {
        app.navigation
            .selected_paths
            .insert(root.join(format!("item-{index}.txt")));
    }

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('C'))))
        .expect("C should open archive creation");
    app.input.frame_state.archive_create_panel = Some(ratatui::layout::Rect::new(0, 0, 40, 12));
    app.input.frame_state.archive_create_list_area = Some(ratatui::layout::Rect::new(1, 4, 38, 8));

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");

    assert_eq!(
        app.overlays
            .archive_create
            .as_ref()
            .expect("archive create overlay should remain open")
            .source_scroll,
        3
    );

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll up should be handled");

    assert_eq!(
        app.overlays
            .archive_create
            .as_ref()
            .expect("archive create overlay should remain open")
            .source_scroll,
        0
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

fn archive_test_password(root: &std::path::Path) -> String {
    root.file_name()
        .expect("temp root should have a file name")
        .to_string_lossy()
        .into_owned()
}

fn wait_for_archive_password_prompt(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.archive_password_is_open() && app.jobs.archive_extract_progress.is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for archive password prompt");
}

fn type_archive_password(app: &mut App, password: &str) {
    for ch in password.chars() {
        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(ch))))
            .expect("password character should be handled");
    }
}
