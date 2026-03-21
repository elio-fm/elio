use super::super::*;
use super::helpers::*;

#[test]
fn stale_archive_preview_result_is_ignored_after_selection_changes() {
    let root = temp_path("archive-stale");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    assert_eq!(
        app.selected_entry().map(|entry| entry.name.as_str()),
        Some("a.zip")
    );

    app.set_selected(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.name.as_str()),
        Some("z.txt")
    );
    assert_eq!(app.preview_section_label(), "Text");
    assert!(app.preview_lines().iter().any(|line| {
        line.to_string()
            .contains("Preparing file preview in background")
    }));

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Text");
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("plain text"))
    );

    thread::sleep(Duration::from_millis(50));
    let _ = app.process_background_jobs();

    assert_eq!(app.preview_section_label(), "Text");
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("plain text"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn archive_preview_is_reused_from_cache_on_reselection() {
    let root = temp_path("archive-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);

    app.set_selected(1);
    assert_eq!(app.preview_section_label(), "Text");
    let metrics_before_reselect = app.preview_metrics();

    app.set_selected(0);
    assert_eq!(app.preview_section_label(), "Archive");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("ZIP archive")
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("docs/"))
    );
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );
    let metrics = app.preview_metrics();
    assert_eq!(metrics.cache_hits, metrics_before_reselect.cache_hits + 1);
    assert!(metrics.cache_misses >= 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn archive_preview_resets_scroll_after_async_refresh() {
    let root = temp_path("archive-scroll-restore");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    let archive_entries = (0..10)
        .map(|index| (format!("docs/{index}.txt"), format!("hello {index}")))
        .collect::<Vec<_>>();
    let archive_refs = archive_entries
        .iter()
        .map(|(name, contents)| (name.as_str(), contents.as_str()))
        .collect::<Vec<_>>();
    write_zip_entries(&archive, &archive_refs);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 4,
        preview_cols_visible: 40,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.preview_state.scroll = 2;
    app.sync_preview_scroll();
    assert_eq!(app.preview_state.scroll, 2);

    app.set_selected(1);

    let updated_entries = (0..12)
        .map(|index| (format!("docs/{index}.txt"), format!("updated {index}")))
        .collect::<Vec<_>>();
    let updated_refs = updated_entries
        .iter()
        .map(|(name, contents)| (name.as_str(), contents.as_str()))
        .collect::<Vec<_>>();
    write_zip_entries(&archive, &updated_refs);
    app.reload().expect("reload should queue successfully");
    wait_for_directory_load(&mut app);

    app.set_selected(0);
    assert_eq!(app.preview_section_label(), "Archive");
    assert_eq!(app.preview_state.scroll, 0);
    assert!(app.preview_header_detail(10).is_some());
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Archive");
    assert_eq!(app.preview_state.scroll, 0);
    assert!(
        app.preview_header_detail(10)
            .as_deref()
            .is_some_and(|detail| !detail.contains("Refreshing in background"))
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("docs/"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn stale_preview_results_are_counted_in_metrics() {
    let root = temp_path("archive-stale-metrics");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    let stale_entry = app
        .selected_entry()
        .cloned()
        .expect("archive entry should be selected first");

    app.set_selected(1);
    let metrics_before = app.preview_metrics();
    app.scheduler
        .defer_result(JobResult::Preview(Box::new(PreviewBuild {
            token: app.preview_state.token.wrapping_add(1),
            entry: stale_entry,
            variant: preview::PreviewRequestOptions::Default,
            code_line_limit: 0,
            result: preview::PreviewContent::new(
                preview::PreviewKind::Text,
                vec![ratatui::text::Line::from("stale preview")],
            ),
        })));

    let _ = app.process_background_jobs();

    let metrics = app.preview_metrics();
    assert!(metrics.stale_results_dropped > metrics_before.stale_results_dropped);
    assert!(metrics.applied_results <= metrics_before.applied_results + 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
