use super::clipboard_test_support::*;

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
#[test]
fn wayland_offer_includes_file_operation_and_portable_uri_targets() {
    let payloads =
        crate::file_operations::copy_to_clipboard::item_clipboard_payloads(&[PathBuf::from(
            "/tmp/résumé #1",
        )]);
    assert_eq!(
        payloads
            .iter()
            .map(|(mime, _)| mime.as_str())
            .collect::<Vec<_>>(),
        [
            "x-special/gnome-copied-files",
            "text/uri-list",
            "text/plain;charset=utf-8",
        ]
    );
    assert_eq!(
        String::from_utf8_lossy(&payloads[0].1),
        "copy\nfile:///tmp/r%C3%A9sum%C3%A9%20%231"
    );
    assert_eq!(
        String::from_utf8_lossy(&payloads[1].1),
        "file:///tmp/r%C3%A9sum%C3%A9%20%231"
    );
}

#[test]
fn copy_overlay_populates_expected_rows_for_selected_file() {
    let root = temp_path("copy-overlay-rows");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report.final.md");
    fs::write(&file, "notes").expect("failed to write test file");

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();

    assert!(
        app.file_operations.copy_is_open(),
        "copy overlay should open"
    );
    assert_eq!(app.file_operations.copy_title(), "Copy to clipboard");
    assert_eq!(app.file_operations.copy_row_count(), 5);
    assert_eq!(app.file_operations.copy_row_label(0), "Copy file name");
    assert_eq!(
        app.file_operations.copy_row_label(1),
        "Name without extension"
    );
    assert_eq!(app.file_operations.copy_row_label(2), "File path");
    assert_eq!(app.file_operations.copy_row_label(3), "Directory path");
    assert_eq!(app.file_operations.copy_row_label(4), "Item");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn item_shortcut_copies_uri_list_references_for_multiple_items() {
    let _lock = clipboard_env_lock();
    let root = temp_path("copy-overlay-items");
    let source = root.join("résumé #1.txt");
    let link = root.join("linked file");
    let capture = root.join("clipboard.bin");
    fs::create_dir_all(&root).expect("failed to create test root");
    fs::write(&source, "notes").expect("failed to write source");
    std::os::unix::fs::symlink(&source, &link).expect("failed to create symlink");
    let tool = install_fake_clipboard_tool(&root, &capture);
    let _env = ClipboardEnvGuard::isolate();

    unsafe {
        env::set_var("ELIO_TEST_ITEM_CLIPBOARD_TOOL", &tool);
        env::set_var("XDG_CURRENT_DESKTOP", "GNOME");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(source.clone());
    app.file_browser.selected_paths.insert(link.clone());
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('i'),
    ))
    .expect("item copy shortcut should succeed");

    assert_eq!(
        fs::read_to_string(&capture).expect("fake clipboard tool should capture payload"),
        format!(
            "file://{}\r\nfile://{}",
            link.as_os_str()
                .as_encoded_bytes()
                .iter()
                .map(|byte| match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'.' | b'_' | b'~' => {
                        (*byte as char).to_string()
                    }
                    byte => format!("%{byte:02X}"),
                })
                .collect::<String>(),
            source
                .as_os_str()
                .as_encoded_bytes()
                .iter()
                .map(|byte| match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'.' | b'_' | b'~' => {
                        (*byte as char).to_string()
                    }
                    byte => format!("%{byte:02X}"),
                })
                .collect::<String>(),
        )
    );
    assert_eq!(app.status, "Copied items");
    assert!(!app.file_operations.copy_is_open());

    fs::remove_dir_all(root).expect("failed to remove test root");
}

#[cfg(unix)]
#[test]
fn item_shortcut_copies_uri_list_outside_gnome() {
    let _lock = clipboard_env_lock();
    let root = temp_path("copy-overlay-uri-list");
    let first = root.join("first file");
    let second = root.join("second file");
    let capture = root.join("clipboard.bin");
    fs::create_dir_all(&root).expect("failed to create test root");
    fs::write(&first, "first").expect("failed to write first file");
    fs::write(&second, "second").expect("failed to write second file");
    let tool = install_fake_clipboard_tool(&root, &capture);
    let _env = ClipboardEnvGuard::isolate();

    unsafe {
        env::set_var("ELIO_TEST_ITEM_CLIPBOARD_TOOL", &tool);
        env::set_var("XDG_CURRENT_DESKTOP", "KDE");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(first.clone());
    app.file_browser.selected_paths.insert(second.clone());
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('i'),
    ))
    .expect("item copy shortcut should succeed");

    let payload = fs::read_to_string(&capture).expect("fake clipboard tool should capture payload");
    assert!(payload.contains("\r\n"), "URI lists use CRLF separators");
    assert!(payload.contains(&format!("file://{}", first.display()).replace(' ', "%20")));
    assert!(payload.contains(&format!("file://{}", second.display()).replace(' ', "%20")));

    fs::remove_dir_all(root).expect("failed to remove test root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_writes_expected_text_to_system_clipboard() {
    let _lock = clipboard_env_lock();
    let root = temp_path("copy-overlay-copy");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("clipboard.txt");
    fs::write(&file, "notes").expect("failed to write test file");
    let tool = install_fake_clipboard_tool(&root, &capture);
    let _env = ClipboardEnvGuard::isolate();

    unsafe {
        env::set_var("ELIO_TEST_CLIPBOARD_TOOL", &tool);
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let copied = fs::read_to_string(&capture).expect("fake clipboard tool should capture text");
    assert_eq!(
        copied,
        file.display().to_string(),
        "fake clipboard tool should capture the copied file path"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.file_operations.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_uses_osc52_when_no_clipboard_tool_is_installed() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-osc52");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("osc52.txt");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("ELIO_TEST_OSC52_CAPTURE", &capture);
        env::set_var("TERM", "xterm-kitty");
        env::set_var("KITTY_WINDOW_ID", "1");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let osc52 = fs::read_to_string(&capture).expect("osc52 capture should exist");
    assert!(
        osc52.starts_with("\u{1b}]52;c;"),
        "expected osc52 clipboard escape, got: {osc52:?}"
    );
    assert!(
        osc52.ends_with("\u{1b}\\"),
        "expected osc52 clipboard escape terminator, got: {osc52:?}"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.file_operations.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_uses_osc52_in_alacritty_without_clipboard_tool() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-osc52-alacritty");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("osc52.txt");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("ELIO_TEST_OSC52_CAPTURE", &capture);
        env::set_var("TERM", "alacritty");
        env::set_var("ALACRITTY_SOCKET", "/tmp/elio-alacritty.sock");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let osc52 = fs::read_to_string(&capture).expect("osc52 capture should exist");
    assert!(
        osc52.starts_with("\u{1b}]52;c;"),
        "expected osc52 clipboard escape, got: {osc52:?}"
    );
    assert!(
        osc52.ends_with("\u{1b}\\"),
        "expected osc52 clipboard escape terminator, got: {osc52:?}"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.file_operations.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_uses_osc52_override_for_unknown_terminals() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-osc52-override");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("osc52.txt");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("ELIO_TEST_OSC52_CAPTURE", &capture);
        env::set_var("TERM", "vt100-unknown");
        env::set_var("ELIO_CLIPBOARD_OSC52", "1");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let osc52 = fs::read_to_string(&capture).expect("osc52 capture should exist");
    assert!(
        osc52.starts_with("\u{1b}]52;c;"),
        "expected osc52 clipboard escape, got: {osc52:?}"
    );
    assert!(
        osc52.ends_with("\u{1b}\\"),
        "expected osc52 clipboard escape terminator, got: {osc52:?}"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.file_operations.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_skips_osc52_in_tmux_when_tmux_rejects_application_clipboard() {
    let _lock = clipboard_env_lock();
    let root = temp_path("copy-overlay-tmux-external");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("clipboard.txt");
    let osc52_capture = root.join("osc52.txt");
    fs::write(&file, "notes").expect("failed to write test file");
    let tool = install_fake_clipboard_tool(&root, &capture);
    let _env = ClipboardEnvGuard::isolate();

    unsafe {
        env::set_var("ELIO_TEST_CLIPBOARD_TOOL", &tool);
        env::set_var("ELIO_TEST_OSC52_CAPTURE", &osc52_capture);
        env::set_var("ELIO_TEST_TMUX_SET_CLIPBOARD", "external");
        env::set_var("TMUX", "/tmp/tmux-test,1,0");
        env::set_var("TERM", "xterm-kitty");
        env::set_var("KITTY_WINDOW_ID", "1");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    assert!(
        !osc52_capture.exists(),
        "tmux set-clipboard=external should prevent application OSC52 writes"
    );
    assert_eq!(
        fs::read_to_string(&capture).expect("fake clipboard tool should capture text"),
        file.display().to_string()
    );
    assert_eq!(app.status, "Copied file path");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_reports_short_error_when_no_clipboard_backend_is_available() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-no-backend");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("TERM", "vt100-unknown");
        env::set_var("PATH", "");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should not error");

    assert_eq!(app.status, "Clipboard helper not found");
    assert!(
        app.file_operations.copy_is_open(),
        "copy overlay should remain open when clipboard copy fails"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_does_not_block_on_backgrounding_clipboard_helpers() {
    let _lock = clipboard_env_lock();
    let root = temp_path("copy-overlay-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let report = root.join("aaa-report.txt");
    fs::write(&report, "hello").expect("failed to write test file");
    let capture = root.join("clipboard.txt");
    let tool = install_backgrounding_clipboard_tool(&root, &capture);
    let _env = ClipboardEnvGuard::isolate();

    unsafe {
        env::set_var("ELIO_TEST_CLIPBOARD_TOOL", &tool);
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.open_copy_overlay();
    let start = std::time::Instant::now();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('c'),
    ))
    .expect("copy confirmation should succeed");

    assert!(
        start.elapsed() < Duration::from_millis(500),
        "copy confirmation should not block on helpers that hand work off to background processes"
    );
    assert_eq!(
        fs::read_to_string(&capture).expect("backgrounding clipboard tool should capture stdin"),
        report
            .file_name()
            .expect("test file should have a file name")
            .to_string_lossy()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
