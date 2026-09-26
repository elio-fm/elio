use super::super::status_bar::{
    compact_footer_summary, git_label_for_width, render_status_bar, status_section_width,
};
use crate::{
    app::{App, ScreenRegions},
    theme,
    ui::helpers,
};
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-status-bar-{label}-{unique}"))
}

fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
}

fn render_footer(app: &App, width: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).expect("terminal should init");
    terminal
        .draw(|frame| render_status_bar(frame, frame.area(), app, theme::palette()))
        .expect("status should render");
    terminal.backend().buffer().clone()
}

#[test]
fn folder_size_chip_tracks_pending_completion_cancellation_and_sort_mode() {
    use crate::filesystem::SortMode;
    use ratatui::style::Modifier;

    let root = temp_path("folder-size-state");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("logo.png"), "png").unwrap();
    let mut app = App::new_at(root.clone()).unwrap();
    let path = root.join("pending");
    app.file_browser.sort_mode = SortMode::Size;
    app.file_browser
        .folder_sizes
        .begin(1, std::slice::from_ref(&path));

    let buffer = render_footer(&app, 100);
    let rendered = row_text(&buffer, 0);
    assert!(
        rendered.starts_with(" Calculating folder sizes…   1/1  logo.png"),
        "{rendered:?}"
    );
    let palette = theme::palette();
    for x in 0..helpers::display_width(" Calculating folder sizes… ") as u16 {
        assert_eq!(buffer[(x, 0)].bg, palette.progress_bar);
        assert_eq!(buffer[(x, 0)].fg, palette.chip_text);
        assert!(buffer[(x, 0)].modifier.contains(Modifier::BOLD));
    }

    assert!(
        app.file_browser
            .folder_sizes
            .apply(1, path.clone(), Some(0))
    );
    assert!(!row_text(&render_footer(&app, 100), 0).contains("Calculating"));
    app.file_browser
        .folder_sizes
        .begin(2, std::slice::from_ref(&path));
    app.cancel_folder_sizes();
    assert!(!row_text(&render_footer(&app, 100), 0).contains("Calculating"));
    app.file_browser.folder_sizes.begin(3, &[path]);
    for mode in [SortMode::Name, SortMode::Modified] {
        app.file_browser.sort_mode = mode;
        assert!(!row_text(&render_footer(&app, 100), 0).contains("Calculating"));
    }
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn folder_size_chip_preserves_clipboard_selection_and_narrow_summary() {
    let root = temp_path("folder-size-width");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("logo.png"), "png").unwrap();
    let mut app = App::new_at(root.clone()).unwrap();
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('y'))))
        .unwrap();
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
        .unwrap();
    app.file_browser.sort_mode = crate::filesystem::SortMode::Size;
    app.file_browser
        .folder_sizes
        .begin(1, &[root.join("pending")]);

    let wide = row_text(&render_footer(&app, 120), 0);
    assert!(wide.contains("1 yanked"), "{wide:?}");
    assert!(wide.contains("1 selected"), "{wide:?}");
    assert!(wide.contains("Calculating folder sizes…"), "{wide:?}");
    assert!(wide.contains("1/1  logo.png"), "{wide:?}");
    for width in [40, 50] {
        let narrow = row_text(&render_footer(&app, width), 0);
        assert!(!narrow.contains("Calculating"), "{narrow:?}");
        assert!(narrow.contains("1 yanked"), "{narrow:?}");
        assert!(narrow.contains("1 selected"), "{narrow:?}");
        assert!(narrow.contains("1/1"), "{narrow:?}");
    }
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn idle_status_keeps_the_message_area_empty() {
    assert_eq!(status_section_width(100, ""), 1);
}

#[test]
fn real_status_messages_expand_to_fit_their_text() {
    assert!(status_section_width(100, "Clipboard helper not found while copying") > 1);
}

#[test]
fn narrow_status_messages_truncate_at_the_end() {
    let rendered = helpers::clamp_label("Clipboard helper not found", 18);
    assert_eq!(rendered, "Clipboard helper …");
}

#[test]
fn compact_footer_summary_keeps_position_before_name() {
    assert_eq!(compact_footer_summary("11/17  CHANGELOG.md", 8), "11/17");
    assert_eq!(compact_footer_summary("5/17  src/", 10), "5/17  src/");
    assert_eq!(
        compact_footer_summary("11/17  CHANGELOG.md", 14),
        "11/17  CHA….md"
    );
}

#[test]
fn git_label_uses_the_available_width_before_hiding() {
    assert_eq!(git_label_for_width("chore/footer-cleanup", true, 5), None);
    assert_eq!(
        git_label_for_width("chore/footer-cleanup", true, 8).as_deref(),
        Some(" ch…p *")
    );
    assert_eq!(
        git_label_for_width("chore/footer-cleanup", true, 16).as_deref(),
        Some(" chore/…eanup *")
    );

    let branch = "fix/git-branch-indicator-truncation";
    let label = format!(" {branch} *");
    let width = helpers::display_width(&label);
    assert_eq!(git_label_for_width(branch, true, width), Some(label));
    let truncated = git_label_for_width(branch, true, width - 1).unwrap();
    assert!(truncated.contains('…'));
    assert_eq!(helpers::display_width(&truncated), width - 1);
    assert!(truncated.ends_with(" *"));
}

#[test]
fn git_branch_renders_after_position_summary() {
    let root = temp_path("git-chip");
    fs::create_dir_all(&root).expect("failed to create temp dir");
    fs::write(root.join("logo.png"), "png").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_git_branch_for_test(Some("main"));
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
        .expect("selection shortcut should succeed");

    let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("terminal should init");
    terminal
        .draw(|frame| render_status_bar(frame, frame.area(), &app, theme::palette()))
        .expect("status should render");

    let rendered = row_text(terminal.backend().buffer(), 0);
    assert!(
        rendered.contains(" 1 selected   1/1  logo.png │  main"),
        "status row should place git branch after position summary, got: {rendered:?}"
    );

    app.set_screen_regions(ScreenRegions::default());
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp dir");
}

#[test]
fn dirty_git_branch_renders_star_suffix() {
    let root = temp_path("dirty-git-chip");
    fs::create_dir_all(&root).expect("failed to create temp dir");
    fs::write(root.join("logo.png"), "png").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_git_branch_for_test(Some("main"));
    app.set_git_dirty_for_test(true);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
        .expect("selection shortcut should succeed");

    let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("terminal should init");
    terminal
        .draw(|frame| render_status_bar(frame, frame.area(), &app, theme::palette()))
        .expect("status should render");

    let rendered = row_text(terminal.backend().buffer(), 0);
    assert!(
        rendered.contains(" 1 selected   1/1  logo.png │  main *"),
        "status row should mark dirty git branches, got: {rendered:?}"
    );

    app.set_screen_regions(ScreenRegions::default());
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp dir");
}

#[test]
fn local_filter_renders_as_left_footer_command_line() {
    let root = temp_path("local-filter-footer");
    fs::create_dir_all(&root).expect("failed to create temp dir");
    fs::write(root.join("ubuntu.iso"), "iso").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('/'))))
        .expect("filter shortcut should open filter");
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('u'))))
        .expect("filter input should accept text");

    let mut terminal = Terminal::new(TestBackend::new(40, 1)).expect("terminal should init");
    terminal
        .draw(|frame| render_status_bar(frame, frame.area(), &app, theme::palette()))
        .expect("status should render");

    let rendered = row_text(terminal.backend().buffer(), 0);
    assert!(
        rendered.starts_with("/u"),
        "filter should render on the left footer, got: {rendered:?}"
    );
    terminal.backend_mut().assert_cursor_position((2, 0));

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("enter should leave filter editing mode");
    terminal
        .draw(|frame| render_status_bar(frame, frame.area(), &app, theme::palette()))
        .expect("status should render");
    let rendered = row_text(terminal.backend().buffer(), 0);
    assert!(
        rendered.starts_with("/u  "),
        "inactive filter should keep a left footer indicator, got: {rendered:?}"
    );

    app.set_screen_regions(ScreenRegions::default());
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp dir");
}

#[test]
fn paste_status_chip_shows_queued_count() {
    let src_dir = temp_path("paste-chip-src");
    let dst_dir = temp_path("paste-chip-dst");
    fs::create_dir_all(&src_dir).expect("failed to create source dir");
    fs::create_dir_all(&dst_dir).expect("failed to create destination dir");
    fs::write(src_dir.join("a.txt"), "a").expect("failed to write first file");
    fs::write(src_dir.join("b.txt"), "b").expect("failed to write second file");

    let mut app = App::new_at(src_dir.clone()).expect("failed to create app");
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('y'))))
        .expect("yank shortcut should succeed");
    app.file_browser.cwd = dst_dir.clone();
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('p'))))
        .expect("paste shortcut should succeed");
    app.file_browser.cwd = src_dir.clone();
    app.file_browser.selected = 1;
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('y'))))
        .expect("second yank shortcut should succeed");
    app.file_browser.cwd = dst_dir.clone();
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('p'))))
        .expect("second paste should be queued");

    app.file_browser.sort_mode = crate::filesystem::SortMode::Size;
    app.file_browser
        .folder_sizes
        .begin(1, &[src_dir.join("pending")]);
    let mut terminal = Terminal::new(TestBackend::new(120, 1)).expect("terminal should init");
    terminal
        .draw(|frame| render_status_bar(frame, frame.area(), &app, theme::palette()))
        .expect("status should render");

    let rendered = row_text(terminal.backend().buffer(), 0);
    assert!(
        rendered.contains("(+1 queued)"),
        "status row should show queued paste count, got: {rendered:?}"
    );

    assert!(!rendered.contains("Calculating"), "{rendered:?}");

    app.set_screen_regions(ScreenRegions::default());
    drop(app);
    fs::remove_dir_all(src_dir).expect("failed to remove source dir");
    fs::remove_dir_all(dst_dir).expect("failed to remove destination dir");
}
