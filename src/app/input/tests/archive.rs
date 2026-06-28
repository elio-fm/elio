use super::super::*;
use super::helpers::{
    temp_path, wait_for_directory_load, write_binary_zip_entries, write_encrypted_seven_zip_entries,
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
        "Extraction supports ZIP, 7z, TAR, TAR.GZ, TAR.XZ, TAR.BZ2, and TAR.ZST"
    );
    assert!(app.jobs.archive_extract_progress.is_none());

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
