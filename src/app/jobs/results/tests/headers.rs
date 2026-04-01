use super::super::*;
use super::helpers::*;
use crate::preview::{PreviewContent, PreviewKind, default_code_preview_line_limit};
use ratatui::text::Line;

#[test]
fn directory_header_upgrades_to_exact_recursive_totals_after_background_stats() {
    let root = temp_path("directory-total-header");
    let folder = root.join("folder");
    let nested = folder.join("nested");
    fs::create_dir_all(&nested).expect("failed to create nested directory");
    fs::write(folder.join("visible.txt"), vec![b'a'; 500]).expect("failed to write file");
    fs::write(folder.join(".hidden"), vec![b'b'; 200]).expect("failed to write hidden file");
    fs::write(nested.join("deep.bin"), vec![b'c'; 1_000]).expect("failed to write nested file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_preview_header(
        &mut app,
        8,
        80,
        &format!("4 total items • {}", crate::app::format_size(1_700)),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn directory_header_marks_incomplete_totals_without_claiming_exactness() {
    let root = temp_path("directory-partial-header");
    let folder = root.join("folder");
    fs::create_dir_all(&folder).expect("failed to create folder");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let entry = app
        .selected_entry()
        .cloned()
        .expect("directory entry should be selected");
    let token = app.preview_state.token.wrapping_add(1);
    app.scheduler.cancel_directory_stats();
    // Seed a settled directory preview state so this assertion does not depend
    // on how quickly the background preview worker finishes on slower CI VMs.
    app.preview_state.token = token;
    app.preview_state.content =
        PreviewContent::new(PreviewKind::Directory, Vec::new()).with_detail("1 item");
    app.preview_state.load_state = None;
    app.preview_state.directory_stats = Some(PreviewDirectoryStatsState::Loading {
        token,
        path: entry.path.clone(),
    });
    app.scheduler
        .defer_result(JobResult::DirectoryStats(DirectoryStatsBuild {
            token,
            path: entry.path.clone(),
            result: crate::fs::DirectoryStatsScanResult::Incomplete {
                partial: crate::fs::DirectoryStats {
                    item_count: 4,
                    folder_count: 1,
                    file_count: 3,
                    total_size_bytes: 1_700,
                },
                error: "Some entries unreadable".to_string(),
            },
        }));

    let _ = app.process_background_jobs();

    assert_eq!(
        app.preview_header_detail_for_width(8, 120).as_deref(),
        Some("At least 4 items • at least 1.7 kB • Some entries unreadable"),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn stale_directory_totals_result_is_ignored_after_selection_changes() {
    let root = temp_path("directory-totals-stale-result");
    let a_dir = root.join("a-dir");
    let b_dir = root.join("b-dir");
    fs::create_dir_all(&a_dir).expect("failed to create a-dir");
    fs::create_dir_all(&b_dir).expect("failed to create b-dir");
    fs::write(a_dir.join("a.txt"), vec![b'a'; 100]).expect("failed to write a.txt");
    fs::write(b_dir.join("b.txt"), vec![b'b'; 200]).expect("failed to write b.txt");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let stale_token = app.preview_state.token;
    let stale_entry = app
        .selected_entry()
        .cloned()
        .expect("a-dir should be selected first");

    app.set_selected(1);
    let current_entry = app
        .selected_entry()
        .cloned()
        .expect("b-dir should be selected second");

    app.scheduler
        .defer_result(JobResult::DirectoryStats(DirectoryStatsBuild {
            token: stale_token,
            path: stale_entry.path.clone(),
            result: crate::fs::DirectoryStatsScanResult::Complete(crate::fs::DirectoryStats {
                item_count: 999,
                folder_count: 0,
                file_count: 999,
                total_size_bytes: 9_990_000_000,
            }),
        }));

    let _ = app.process_background_jobs();
    assert_eq!(
        app.preview_state
            .directory_stats
            .as_ref()
            .map(PreviewDirectoryStatsState::path),
        Some(&current_entry.path),
    );

    wait_for_preview_header(
        &mut app,
        8,
        80,
        &format!("1 total item • {}", crate::app::format_size(200)),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn unreadable_directory_keeps_permission_denied_header_without_fake_partial_totals() {
    if unsafe { libc::getuid() } == 0 {
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    let root = temp_path("directory-header-permission-denied");
    let locked = root.join("locked");
    fs::create_dir_all(&locked).expect("failed to create locked dir");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("failed to lock dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_preview_header(&mut app, 8, 80, "Permission denied");

    assert_eq!(
        app.preview_header_detail_for_width(8, 80).as_deref(),
        Some("Permission denied"),
    );
    assert!(
        !app.preview_header_detail_for_width(8, 80)
            .unwrap_or_default()
            .contains("0 total items")
    );

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("failed to unlock dir");
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn wrapped_text_header_reports_visual_cap_compactly() {
    let root = temp_path("wrapped-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    // At preview_cols_visible=20, "word " (5 chars) wraps 4 per line = 5_000 words → 1_250
    // wrapped lines, which exceeds default_code_preview_line_limit() of 800.
    fs::write(&text, "word ".repeat(5_000)).expect("failed to write text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 8,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let header = app
        .preview_header_detail(8)
        .expect("header detail should be present");

    assert!(header.contains("1 lines"));
    assert!(header.contains(&format!(
        "first {} wrapped",
        default_code_preview_line_limit()
    )));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn narrow_code_header_prefers_compact_subtype_and_drops_low_priority_notes() {
    let root = temp_path("narrow-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.preview_state.content =
        PreviewContent::new(PreviewKind::Code, vec![Line::from("fn main() {}")])
            .with_detail("Rust source file")
            .with_line_coverage(default_code_preview_line_limit(), None, true);
    app.preview_state.content.set_total_line_count_pending(true);

    assert_eq!(
        app.preview_header_detail_for_width(8, 20).as_deref(),
        Some(format!("Rust • {} shown", default_code_preview_line_limit()).as_str())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn byte_truncated_code_header_upgrades_to_exact_total_lines_after_background_count() {
    let root = temp_path("byte-truncated-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("main.rs");
    // Lines are ~67 chars: fits 800 within 64 KiB (line cap hits first),
    // but 1500 lines exceed 64 KiB (file is byte-truncated overall).
    let contents = (1..=1_500)
        .map(|index| {
            format!(
                "fn line_{index}() {{ println!(\"{}\"); }}",
                "word ".repeat(8)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&source, contents).expect("failed to write source");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_preview_total_line_count(&mut app, 1_500);
    wait_for_preview_header(
        &mut app,
        8,
        40,
        &format!(
            "Rust • {} / 1,500 lines shown",
            default_code_preview_line_limit()
        ),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn source_truncated_text_header_prefers_line_limit_over_wrapped_cap_note() {
    let root = temp_path("source-truncated-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    let total_lines = default_code_preview_line_limit() + 40;
    // Short lines so all total_lines fit within 64 KiB (no byte truncation),
    // but total_lines exceeds the line cap so line truncation fires.
    let contents = (1..=total_lines)
        .map(|index| format!("line {index} {}", "word ".repeat(3)))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&text, contents).expect("failed to write text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 8,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let header = app
        .preview_header_detail(8)
        .expect("header detail should be present");

    assert!(header.contains(&format!("{total_lines} lines")));
    assert!(header.contains(&format!(
        "showing first {} lines",
        default_code_preview_line_limit()
    )));
    assert!(!header.contains("wrapped"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
