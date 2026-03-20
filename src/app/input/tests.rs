use super::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    env, fs,
    fs::File,
    io::Write,
    path::PathBuf,
    thread,
    time::Duration,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!("elio-events-{label}-{unique}"))
}

fn wait_for_directory_load(app: &mut App) {
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if app.directory_runtime.pending_load.is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for directory load");
}

fn wait_for_background_preview(app: &mut App) {
    for _ in 0..200 {
        if app.process_background_jobs() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for background preview");
}

fn write_binary_zip_entries(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, contents) in entries {
        zip.start_file(name, options)
            .expect("failed to start zip entry");
        zip.write_all(contents).expect("failed to write zip entry");
    }

    zip.finish().expect("failed to finish zip");
}

fn write_epub_fixture(path: &std::path::Path, sections: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("META-INF/container.xml", options)
        .expect("failed to start container entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
            <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
              <rootfiles>
                <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
              </rootfiles>
            </container>"#,
    )
    .expect("failed to write container entry");

    let manifest = sections
        .iter()
        .enumerate()
        .map(|(index, _)| {
            format!(
                r#"<item id="chapter-{id}" href="text/chapter-{id}.xhtml" media-type="application/xhtml+xml"/>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let spine = sections
        .iter()
        .enumerate()
        .map(|(index, _)| format!(r#"<itemref idref="chapter-{}"/>"#, index + 1))
        .collect::<Vec<_>>()
        .join("");
    let nav = sections
        .iter()
        .enumerate()
        .map(|(index, (title, _))| {
            format!(
                r#"<li><a href="text/chapter-{id}.xhtml">{title}</a></li>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let package = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
              <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                <dc:title>Wheel Book</dc:title>
                <dc:creator>Regueiro</dc:creator>
              </metadata>
              <manifest>
                <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                {manifest}
              </manifest>
              <spine>{spine}</spine>
            </package>"#
    );
    zip.start_file("OPS/package.opf", options)
        .expect("failed to start package entry");
    zip.write_all(package.as_bytes())
        .expect("failed to write package entry");

    let nav_document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
              <body>
                <nav epub:type="toc">
                  <ol>{nav}</ol>
                </nav>
              </body>
            </html>"#
    );
    zip.start_file("OPS/nav.xhtml", options)
        .expect("failed to start nav entry");
    zip.write_all(nav_document.as_bytes())
        .expect("failed to write nav entry");

    for (index, (title, body)) in sections.iter().enumerate() {
        let chapter = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <h1>{title}</h1>
                    {body}
                  </body>
                </html>"#
        );
        zip.start_file(format!("OPS/text/chapter-{}.xhtml", index + 1), options)
            .expect("failed to start chapter entry");
        zip.write_all(chapter.as_bytes())
            .expect("failed to write chapter entry");
    }

    zip.finish().expect("failed to finish epub");
}

#[test]
fn right_arrow_does_not_open_selected_file_in_list_view() {
    let root = temp_path("right-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("note.txt");
    fs::write(&file_path, "hello").expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");

    assert_eq!(app.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(file_path.as_path())
    );
    assert_eq!(app.status_message(), "Press Enter to open files");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shift_slash_opens_and_closes_help_overlay() {
    let root = temp_path("help-shift-slash");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::SHIFT,
    )))
    .expect("shift-slash should open help");
    assert!(app.help_open);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::SHIFT,
    )))
    .expect("shift-slash should close help");
    assert!(!app.help_open);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn right_arrow_enters_selected_directory_in_list_view() {
    let root = temp_path("right-dir");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create temp dirs");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, child);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn left_arrow_in_list_view_reselects_previous_directory_in_parent() {
    let root = temp_path("left-parent-selection");
    let alpha = root.join("alpha");
    let child = root.join("child");
    fs::create_dir_all(&alpha).expect("failed to create alpha dir");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(1);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)))
        .expect("left arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn go_back_reselects_previous_directory_in_parent() {
    let root = temp_path("history-back-selection");
    let alpha = root.join("alpha");
    let child = root.join("child");
    fs::create_dir_all(&alpha).expect("failed to create alpha dir");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(1);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn go_forward_reselects_previous_directory_in_parent() {
    let root = temp_path("history-forward-selection");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);
    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    app.go_forward().expect("go forward should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, child);
    assert!(app.selected_entry().is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn go_forward_restores_last_selected_entry_in_directory() {
    let root = temp_path("history-forward-restore-selection");
    let child = root.join("child");
    let alpha = child.join("alpha.txt");
    let beta = child.join("beta.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    app.go_forward().expect("go forward should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, child);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn reopening_directory_restores_last_selected_entry() {
    let root = temp_path("reopen-directory-selection");
    let child = root.join("child");
    let alpha = child.join("alpha.txt");
    let beta = child.join("beta.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    app.go_parent().expect("go parent should succeed");
    wait_for_directory_load(&mut app);
    app.open_selected()
        .expect("reopening selected directory should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, child);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn reopening_directory_restores_scroll_position() {
    let root = temp_path("reopen-directory-scroll");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");
    for index in 0..8 {
        fs::write(child.join(format!("file-{index}.txt")), format!("{index}"))
            .expect("failed to write file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.set_frame_state(FrameState {
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 3,
        },
        ..FrameState::default()
    });
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(6);
    assert_eq!(app.scroll_row, 4);

    app.go_parent().expect("go parent should succeed");
    wait_for_directory_load(&mut app);
    app.open_selected()
        .expect("reopening selected directory should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, child);
    assert_eq!(app.selected, 6);
    assert_eq!(app.scroll_row, 4);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn reopening_parent_restores_last_selected_child_directory() {
    let root = temp_path("reopen-parent-selection");
    let home = root.join("home");
    let aaa = home.join("aaa");
    let regueiro = home.join("regueiro");
    fs::create_dir_all(&aaa).expect("failed to create aaa dir");
    fs::create_dir_all(&regueiro).expect("failed to create regueiro dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected().expect("opening home should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(regueiro.as_path())
    );

    app.open_selected()
        .expect("opening regueiro should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to home should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to root should succeed");
    wait_for_directory_load(&mut app);

    app.open_selected().expect("reopening home should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, home);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(regueiro.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn reopening_parent_restores_scroll_position() {
    let root = temp_path("reopen-parent-scroll");
    let home = root.join("home");
    let child_paths = (0..8)
        .map(|index| home.join(format!("child-{index}")))
        .collect::<Vec<_>>();
    for child in &child_paths {
        fs::create_dir_all(child).expect("failed to create child dir");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.set_frame_state(FrameState {
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 3,
        },
        ..FrameState::default()
    });
    app.select_index(0);
    app.open_selected().expect("opening home should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(6);
    assert_eq!(app.scroll_row, 4);

    app.open_selected()
        .expect("opening remembered child should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to home should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to root should succeed");
    wait_for_directory_load(&mut app);

    app.open_selected().expect("reopening home should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, home);
    assert_eq!(app.selected, 6);
    assert_eq!(app.scroll_row, 4);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_horizontal_scroll_works_in_list_view() {
    let root = temp_path("preview-horizontal-list");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.rs");
    fs::write(
        &file_path,
        "fn main() { let preview_line = \"this line is intentionally long for horizontal preview scrolling\"; }\n",
    )
    .expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 12,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollRight,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll right should be handled");
    assert!(app.process_pending_scroll());
    assert_eq!(app.preview_state.horizontal_scroll, 2);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn wheel_burst_smoothing_coalesces_dense_input() {
    let mut lane = ScrollLane::new();

    for _ in 0..6 {
        App::queue_scroll(&mut lane, 1, ENTRY_WHEEL_TUNING);
    }

    assert!(lane.pending.abs() < 6);
    assert!(lane.pending > 0);
}

#[test]
fn short_entry_wheel_burst_keeps_full_distance() {
    let mut lane = ScrollLane::new();

    for _ in 0..3 {
        App::queue_scroll(&mut lane, 1, ENTRY_WHEEL_TUNING);
    }

    assert_eq!(lane.pending, 3);
}

#[test]
fn repeated_down_arrow_is_throttled_without_starving_hold_repeat() {
    let root = temp_path("down-arrow-debounce");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("first down arrow should be handled");

    let throttled_at = app
        .last_navigation_key
        .expect("accepted navigation key should be tracked")
        .1;
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("second down arrow should be handled");

    assert_eq!(app.selected, 1);
    assert_eq!(
        app.last_navigation_key
            .expect("throttled navigation key should keep prior timestamp")
            .1,
        throttled_at
    );

    app.last_navigation_key = Some((
        NavigationRepeatKey::Down,
        Instant::now() - KEY_REPEAT_NAV_INTERVAL - Duration::from_millis(1),
    ));
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("third down arrow should be handled");

    assert_eq!(app.selected, 2);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn browser_wheel_updates_selection_and_preview_immediately() {
    let root = temp_path("wheel-selection-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::Default;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });
    let initial_preview_token = app.preview_state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");
    assert!(app.process_pending_scroll());

    assert_eq!(app.selected, 1);
    assert_eq!(app.scroll_row, 1);
    assert!(app.preview_state.token > initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_moves_selection_immediately() {
    let root = temp_path("wheel-high-frequency-immediate");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });
    let initial_preview_token = app.preview_state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");

    assert_eq!(app.selected, 1);
    assert_eq!(app.scroll_row, 1);
    assert!(app.preview_state.token > initial_preview_token);
    assert!(!app.has_pending_scroll());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_preview_wheel_scrolls_preview_after_entries_scroll() {
    // Reproduces: cursor moves to preview after scrolling entries in Alacritty/Ghostty.
    // Even after entry scroll set last_wheel_target=Entries, a scroll event with
    // coordinates inside the preview panel must route to preview.
    let root = temp_path("wheel-hf-preview-after-entries");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }
    let long_file = root.join("long.txt");
    let contents = (0..60)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long_file, &contents).expect("failed to write long file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    // Select the long file so preview has content to scroll.
    // Plain text preview is built synchronously — no background wait needed.
    let long_index = app
        .entries
        .iter()
        .position(|e| e.path == long_file)
        .expect("long.txt should be in entries");
    app.select_index(long_index);

    // Side-by-side layout: entries on the left, preview on the right
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_panel: Some(Rect {
            x: 40,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_rows_visible: 16,
        preview_cols_visible: 38,
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 8,
        },
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    // Step 1: scroll entries (cursor in entries area)
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("entry scroll should be handled");
    assert_eq!(app.last_wheel_target, Some(WheelTarget::Entries));

    // Step 2: cursor moves to preview (Moved event — as sent by ?1003h)
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("hover on preview should be handled");
    assert_eq!(app.last_wheel_target, Some(WheelTarget::Preview));

    // Step 3: scroll with cursor over preview — must scroll preview, not entries
    let before_scroll = app.preview_state.scroll;
    let before_selected = app.selected;
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview scroll should be handled");

    assert_eq!(
        app.selected, before_selected,
        "entry selection must not change when scrolling preview"
    );
    assert!(
        app.preview_state.scroll > before_scroll,
        "preview must have scrolled"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_preview_wheel_scrolls_preview_without_prior_moved_event() {
    // Reproduces: in terminals where Moved events may not fire, the scroll event's own
    // coordinates (cursor in preview panel) must be enough to route to preview.
    let root = temp_path("wheel-hf-preview-no-moved");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let long_file = root.join("long.txt");
    let contents = (0..60)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long_file, &contents).expect("failed to write long file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    // Force last_wheel_target to Entries (simulates: user was scrolling entries, then moves
    // cursor to preview with no Moved event generated)
    app.last_wheel_target = Some(WheelTarget::Entries);

    let long_index = app
        .entries
        .iter()
        .position(|e| e.path == long_file)
        .expect("long.txt should be in entries");
    app.select_index(long_index);

    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_panel: Some(Rect {
            x: 40,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_rows_visible: 16,
        preview_cols_visible: 38,
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 8,
        },
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let before_scroll = app.preview_state.scroll;
    let before_selected = app.selected;

    // Scroll with cursor over preview, no prior Moved event
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview scroll should be handled");

    assert_eq!(
        app.selected, before_selected,
        "entry selection must not change when scrolling preview"
    );
    assert!(
        app.preview_state.scroll > before_scroll,
        "preview must have scrolled without a prior Moved event"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn hover_panel_routes_scroll_when_event_coords_are_outside_panels() {
    // Reproduces the Alacritty/Ghostty bug: scroll events arrive with column/row that
    // don't land inside the preview panel rect (e.g. zero or stale coords), but a prior
    // Moved event correctly set hover_panel=Preview. hover_panel must win over coords.
    let root = temp_path("wheel-hover-panel-routing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let long_file = root.join("long.txt");
    let contents = (0..60)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long_file, &contents).expect("failed to write long file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.last_wheel_target = Some(WheelTarget::Entries);
    app.hover_panel = None;

    let long_index = app
        .entries
        .iter()
        .position(|e| e.path == long_file)
        .expect("long.txt should be in entries");
    app.select_index(long_index);

    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_panel: Some(Rect {
            x: 40,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_rows_visible: 16,
        preview_cols_visible: 38,
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 8,
        },
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    // Moved event puts cursor in preview — hover_panel = Preview
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("moved event should be handled");
    assert_eq!(app.hover_panel, Some(WheelTarget::Preview));

    // Scroll event arrives with column=0 — outside both panels in terms of coordinate
    // routing but hover_panel from the Moved event should still win.
    let before_scroll = app.preview_state.scroll;
    let before_selected = app.selected;
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll should be handled");

    assert_eq!(
        app.selected, before_selected,
        "entry selection must not change"
    );
    assert!(
        app.preview_state.scroll > before_scroll,
        "hover_panel should have routed scroll to preview despite wrong coords"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_keeps_large_flick_distance() {
    let root = temp_path("wheel-high-frequency-distance");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..12 {
        fs::write(root.join(format!("{index}.txt")), format!("{index}"))
            .expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });

    for _ in 0..4 {
        app.handle_event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("scroll down should be handled");
    }

    assert_eq!(app.selected, 4);
    assert_eq!(app.scroll_row, 4);
    assert!(!app.has_pending_scroll());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_defers_preview_refresh_during_burst() {
    let root = temp_path("wheel-high-frequency-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });

    let initial_token = app.preview_state.token;
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("first scroll down should be handled");
    let after_first_token = app.preview_state.token;
    assert!(after_first_token > initial_token);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("second scroll down should be handled");

    assert_eq!(app.selected, 2);
    assert_eq!(app.preview_state.token, after_first_token);

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(app.preview_state.token > after_first_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_requests_post_burst_redraw() {
    let root = temp_path("wheel-high-frequency-post-burst-redraw");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");

    assert!(app.browser_wheel_post_burst_pending);
    assert!(!app.process_browser_wheel_timers());

    thread::sleep(WHEEL_SCROLL_BURST_WINDOW + Duration::from_millis(20));
    assert!(app.process_browser_wheel_timers());
    assert!(!app.browser_wheel_post_burst_pending);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn browser_wheel_preserves_preview_when_selection_does_not_change() {
    let root = temp_path("wheel-selection-clamp");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 2,
        },
        ..FrameState::default()
    });
    app.select_index(0);
    let initial_preview_token = app.preview_state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll up should be handled");
    assert!(!app.process_pending_scroll());

    assert_eq!(app.scroll_row, 0);
    assert_eq!(app.selected, 0);
    assert_eq!(app.preview_state.token, initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_uses_last_focused_panel_when_coordinates_miss() {
    let root = temp_path("preview-wheel-focus");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.txt");
    let contents = (0..40)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file_path, contents).expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview click should be handled");
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 80,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("wheel should fall back to last focused preview panel");

    assert!(app.process_pending_scroll());
    assert!(app.preview_state.scroll > 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_follows_hovered_panel_without_click() {
    let root = temp_path("preview-wheel-hover");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.txt");
    let contents = (0..40)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file_path, contents).expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview hover should be handled");
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 80,
        row: 20,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("wheel should fall back to hovered preview panel");

    assert!(app.process_pending_scroll());
    assert!(app.preview_state.scroll > 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_uses_preview_column_when_row_is_unreliable() {
    let root = temp_path("preview-wheel-column-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.txt");
    let contents = (0..40)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file_path, contents).expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 20,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("wheel should use preview column fallback");

    assert!(app.process_pending_scroll());
    assert!(app.preview_state.scroll > 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_steps_comic_pages_instead_of_scrolling_summary_text() {
    let root = temp_path("preview-wheel-comic-pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    write_binary_zip_entries(
        &archive,
        &[
            ("1.jpg", b"page-one"),
            ("2.jpg", b"page-two"),
            ("3.jpg", b"page-three"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 20,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert_eq!(
        app.preview_state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert!(app.pending_preview_refresh_timer().is_some());
    assert_eq!(app.preview_state.scroll, 0);

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    wait_for_background_preview(&mut app);

    assert_eq!(
        app.preview_state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert_eq!(app.preview_state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_preview_wheel_clears_pending_entry_scroll_before_page_turns() {
    let root = temp_path("preview-wheel-comic-clears-entry-scroll");
    fs::create_dir_all(&root).expect("failed to create temp root");
    write_binary_zip_entries(
        &root.join("a.cbz"),
        &[("1.jpg", b"a-one"), ("2.jpg", b"a-two")],
    );
    fs::write(root.join("b.txt"), "next entry").expect("failed to write temp text");
    fs::write(root.join("c.txt"), "another entry").expect("failed to write temp text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    app.wheel_scroll.vertical.pending = 3;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert_eq!(app.selected, 0);
    assert_eq!(app.wheel_scroll.vertical.pending, 0);
    assert_eq!(
        app.current_preview_request_options().comic_page_index(),
        Some(1)
    );

    let _ = app.process_pending_scroll();
    assert_eq!(app.selected, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_steps_cbr_pages_instead_of_scrolling_summary_text() {
    let root = temp_path("preview-wheel-cbr-pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbr");
    write_binary_zip_entries(
        &archive,
        &[
            ("1.jpg", b"page-one"),
            ("2.jpg", b"page-two"),
            ("3.jpg", b"page-three"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 20,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert_eq!(
        app.preview_state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert!(app.pending_preview_refresh_timer().is_some());
    assert_eq!(app.preview_state.scroll, 0);

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    wait_for_background_preview(&mut app);

    assert_eq!(
        app.preview_state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert_eq!(app.preview_state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_scrolls_epub_section_before_advancing_to_next_section() {
    let root = temp_path("preview-wheel-epub-sections");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("story.epub");
    let long_body = (1..=30)
        .map(|index| format!("<p>Paragraph {index} {} </p>", "word ".repeat(20)))
        .collect::<Vec<_>>()
        .join("");
    write_epub_fixture(
        &archive,
        &[
            ("Opening", long_body.as_str()),
            ("Second Step", "<p>Second chapter text.</p>"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 24,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 24,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert!(app.process_pending_scroll());
    assert!(app.preview_state.scroll > 0);
    assert_eq!(app.preview_state.content.ebook_section_index, Some(0));

    let max_scroll = app
        .preview_total_lines(app.frame_state.preview_cols_visible.max(1))
        .saturating_sub(app.frame_state.preview_rows_visible.max(1));
    app.preview_state.scroll = max_scroll;
    app.sync_preview_scroll();

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should advance the section at the bottom boundary");

    assert_eq!(app.preview_state.scroll, 0);
    assert_eq!(app.preview_state.content.ebook_section_index, Some(1));
    assert_eq!(app.preview_state.content.ebook_section_count, Some(2));
    assert!(
        app.preview_header_detail(10)
            .as_deref()
            .is_some_and(|detail| detail.contains("Section 2/2"))
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_state.content.ebook_section_index, Some(1));
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Second chapter text."))
    );
    assert_eq!(app.preview_state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_alt_right_scrolls_preview_instead_of_history() {
    let root = temp_path("preview-horizontal-alt-right");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.rs");
    fs::write(
        &file_path,
        "fn main() { let preview_line = \"this line is intentionally long for horizontal preview scrolling\"; }\n",
    )
    .expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.last_wheel_target = Some(WheelTarget::Entries);
    app.select_index(0);
    app.last_selection_change_at =
        Instant::now() - PREVIEW_AUTO_FOCUS_DELAY - Duration::from_millis(1);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 12,
        ..FrameState::default()
    });

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)))
        .expect("alt-right should be handled");

    assert!(app.preview_state.horizontal_scroll > 0);
    assert_eq!(app.selected, 0);
    assert_eq!(app.last_wheel_target, Some(WheelTarget::Preview));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_down_arrow_keeps_browser_navigation() {
    let root = temp_path("high-frequency-down-keeps-browser");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.last_wheel_target = Some(WheelTarget::Preview);
    app.last_selection_change_at =
        Instant::now() - PREVIEW_AUTO_FOCUS_DELAY - Duration::from_millis(1);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("down arrow should be handled");

    assert_eq!(app.selected, 1);
    assert_eq!(app.preview_state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_right_arrow_in_list_view_still_enters_directory() {
    let root = temp_path("high-frequency-right-enters");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.last_wheel_target = Some(WheelTarget::Preview);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, child);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_alt_right_does_not_trigger_history_navigation() {
    let root = temp_path("high-frequency-alt-right-no-history");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);
    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)))
        .expect("alt-right should be handled");

    assert_eq!(app.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_scroll_resets_when_reselecting_a_file() {
    let root = temp_path("preview-scroll-restore");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let long = root.join("a.txt");
    let other = root.join("b.txt");
    let contents = (0..24)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long, contents).expect("failed to write long text file");
    fs::write(&other, "short\ntext").expect("failed to write other text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 40,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.preview_state.scroll = 5;
    app.sync_preview_scroll();
    assert_eq!(app.preview_state.scroll, 5);

    app.select_index(1);
    app.select_index(0);

    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(long.as_path())
    );
    assert_eq!(app.preview_state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_horizontal_scroll_resets_when_reselecting_code() {
    let root = temp_path("preview-horizontal-restore");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let code = root.join("a.rs");
    let other = root.join("b.txt");
    fs::write(
        &code,
        "fn main() { let preview_line = \"this line is intentionally long for horizontal preview scrolling\"; }\n",
    )
    .expect("failed to write code file");
    fs::write(&other, "short\ntext").expect("failed to write other text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 12,
        ..FrameState::default()
    });

    app.preview_state.horizontal_scroll = 3;
    app.sync_preview_scroll();
    assert_eq!(app.preview_state.horizontal_scroll, 3);

    app.select_index(1);
    app.select_index(0);

    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(code.as_path())
    );
    assert_eq!(app.preview_state.horizontal_scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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
