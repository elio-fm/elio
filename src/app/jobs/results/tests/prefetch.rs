use super::super::*;
use super::helpers::*;

#[test]
fn comic_preview_prefetches_adjacent_pages_for_instant_page_steps() {
    let root = temp_path("comic-page-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    write_binary_zip_entries(
        &archive,
        &[
            ("1.jpg", b"page-one"),
            ("2.jpg", b"page-two"),
            ("3.jpg", b"page-three"),
            ("4.jpg", b"page-four"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);
    wait_for_preview_prefetch(&mut app);

    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.has_cached_comic_preview_page(&archive, 1)
            && app.has_cached_comic_preview_page(&archive, 2)
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(app.has_cached_comic_preview_page(&archive, 1));
    assert!(app.has_cached_comic_preview_page(&archive, 2));
    assert!(app.scheduler_metrics().preview_jobs_submitted_low >= 2);

    let preview_metrics = app.preview_metrics();
    assert!(app.step_comic_page(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 2/4")
    );

    let preview_metrics = app.preview_metrics();
    assert!(app.step_comic_page(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 3/4")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_prefetches_adjacent_sections_for_instant_page_steps() {
    let root = temp_path("epub-section-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("story.epub");
    write_fixed_layout_epub_fixture(&archive, &["Page 1", "Page 2", "Page 3", "Page 4"]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);
    wait_for_preview_prefetch(&mut app);

    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.has_cached_epub_preview_section(&archive, 1)
            && app.has_cached_epub_preview_section(&archive, 2)
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(app.has_cached_epub_preview_section(&archive, 1));
    assert!(app.has_cached_epub_preview_section(&archive, 2));
    assert!(app.scheduler_metrics().preview_jobs_submitted_low >= 2);

    let preview_metrics = app.preview_metrics();
    assert!(app.step_epub_section(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("EPUB ebook  •  Section 2/4")
    );

    let preview_metrics = app.preview_metrics();
    assert!(app.step_epub_section(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("EPUB ebook  •  Section 3/4")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn nearby_archive_preview_skips_heavy_prefetch_work() {
    let root = temp_path("archive-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("a.zip");
    let second = root.join("b.zip");
    write_zip_entries(&first, &[("docs/first.txt", "hello")]);
    write_zip_entries(&second, &[("docs/second.txt", "world")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_preview_prefetch(&mut app);
    for _ in 0..100 {
        let _ = app.process_preview_prefetch_timers();
        let _ = app.process_background_jobs();
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!app.has_cached_preview_for_path(&second));
    let scheduler_metrics = app.scheduler_metrics();
    assert!(scheduler_metrics.preview_jobs_submitted_high >= 1);
    assert_eq!(scheduler_metrics.preview_jobs_submitted_low, 0);

    app.set_selected(1);
    wait_for_background_preview(&mut app);
    assert_eq!(app.preview_section_label(), "Archive");
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("second.txt"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn nearby_comic_entry_prefetch_warms_adjacent_file_preview() {
    let root = temp_path("comic-entry-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("001.cbz");
    let second = root.join("002.cbz");
    write_binary_zip_entries(&first, &[("1.jpg", b"page-one")]);
    write_binary_zip_entries(&second, &[("1.jpg", b"page-two")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);
    wait_for_preview_prefetch(&mut app);

    for _ in 0..200 {
        let _ = app.process_preview_prefetch_timers();
        let _ = app.process_background_jobs();
        if app.has_cached_comic_preview_page(&second, 0) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(app.has_cached_comic_preview_page(&second, 0));
    assert!(app.scheduler_metrics().preview_jobs_submitted_low >= 1);

    let preview_metrics = app.preview_metrics();
    app.set_selected(1);
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(app.preview_section_label(), "Comic");
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn nearby_audio_preview_prefetch_warms_adjacent_file_preview() {
    let root = temp_path("audio-entry-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("001.mp3");
    let second = root.join("002.mp3");
    fs::write(&first, b"audio-one").expect("failed to write first audio fixture");
    fs::write(&second, b"audio-two").expect("failed to write second audio fixture");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.set_media_ffprobe_available_for_tests(false);
    app.set_media_ffmpeg_available_for_tests(false);
    app.refresh_preview();
    wait_for_background_preview(&mut app);
    wait_for_preview_prefetch(&mut app);

    for _ in 0..500 {
        let _ = app.process_preview_prefetch_timers();
        let _ = app.process_background_jobs();
        if app.has_cached_preview_for_path(&second) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(app.has_cached_preview_for_path(&second));
    assert!(app.scheduler_metrics().preview_jobs_submitted_low >= 1);

    let preview_metrics = app.preview_metrics();
    app.set_selected(1);
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(app.preview_section_label(), "Audio");
    assert_eq!(app.preview_header_detail(10).as_deref(), Some("MP3 audio"));
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
