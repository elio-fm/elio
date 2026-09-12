use crate::{
    app::{App, ScreenRegions},
    theme,
    ui::{self, helpers},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Modifier};
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
    std::env::temp_dir().join(format!("elio-overlay-{label}-{unique}"))
}

fn draw_ui(terminal: &mut Terminal<TestBackend>, app: &mut App) -> ScreenRegions {
    let mut screen_regions = ScreenRegions::default();
    terminal
        .draw(|frame| ui::render(frame, app, &mut screen_regions))
        .expect("ui should render");
    app.set_screen_regions(screen_regions.clone());
    screen_regions
}

fn wait_for_search_index(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.search_is_open() && !app.search_is_loading() && app.search_candidate_count() > 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out waiting for search index");
}

fn rect_row_text(buffer: &Buffer, rect: Rect, y: u16) -> String {
    (rect.x..rect.x.saturating_add(rect.width))
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
}

fn buffer_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn create_overlay_uses_themed_bold_icon_for_live_json_names() {
    let root = temp_path("create-overlay-json-icon");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('a'))))
        .expect("create overlay should open");
    for ch in "i.json".chars() {
        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(ch))))
            .expect("typing into create overlay should succeed");
    }

    let state = draw_ui(&mut terminal, &mut app);
    let list_area = state
        .create_list_area
        .expect("create list area should be rendered");
    let icon_cell = &terminal.backend().buffer()[(list_area.x, list_area.y)];

    assert_eq!(
        icon_cell.symbol(),
        "",
        "create overlay should resolve the JSON icon while typing",
    );
    assert!(
        icon_cell.modifier.contains(Modifier::BOLD),
        "create overlay icon should use the same bold styling as other file icon surfaces",
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
#[test]
fn create_overlay_scrolls_to_keep_the_active_line_visible() {
    let root = temp_path("create-overlay-scroll");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('a'))))
        .expect("create overlay should open");
    for index in 0..14 {
        for ch in format!("file-{index:02}.txt").chars() {
            app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(ch))))
                .expect("typing create line should succeed");
        }
        if index < 13 {
            app.handle_event(Event::Key(KeyEvent::new(
                KeyCode::Char('j'),
                KeyModifiers::CONTROL,
            )))
            .expect("inserting another create line should succeed");
        }
    }

    let state = draw_ui(&mut terminal, &mut app);
    let list_area = state
        .create_list_area
        .expect("create overlay should render a list area");

    assert_eq!(
        state.create_scroll_top, 2,
        "create overlay should scroll once the cursor moves past the twelfth visible line"
    );
    assert!(
        rect_row_text(terminal.backend().buffer(), list_area, list_area.y).contains("file-02.txt"),
        "expected the first visible create row to track the computed scroll top"
    );
    assert!(
        rect_row_text(
            terminal.backend().buffer(),
            list_area,
            list_area
                .y
                .saturating_add(list_area.height.saturating_sub(1)),
        )
        .contains("file-13.txt"),
        "expected the active create line to remain visible at the bottom of the list"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn bulk_rename_overlay_scrolls_to_keep_the_active_row_visible() {
    let root = temp_path("bulk-rename-overlay-scroll");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..14 {
        fs::write(root.join(format!("file-{index:02}.txt")), "content")
            .expect("failed to write test file");
    }

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    app.file_browser.view_mode = crate::file_browser::ViewMode::List;
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

    for _ in 0..14 {
        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
            .expect("selection toggle should succeed");
    }
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('r'))))
        .expect("bulk rename overlay should open");
    for _ in 0..13 {
        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Down)))
            .expect("bulk rename cursor movement should succeed");
    }

    let state = draw_ui(&mut terminal, &mut app);
    let list_area = state
        .bulk_rename_list_area
        .expect("bulk rename overlay should render a list area");

    assert!(
        state.rename_panel.is_some(),
        "bulk rename overlay should keep using the shared rename panel slot"
    );
    assert_eq!(
        state.bulk_rename_scroll_top, 2,
        "bulk rename overlay should scroll once the active row moves past the twelfth visible line"
    );
    assert!(
        rect_row_text(terminal.backend().buffer(), list_area, list_area.y).contains("file-02.txt"),
        "expected the first visible bulk rename row to match the computed scroll top"
    );
    assert!(
        rect_row_text(
            terminal.backend().buffer(),
            list_area,
            list_area
                .y
                .saturating_add(list_area.height.saturating_sub(1)),
        )
        .contains("file-13.txt"),
        "expected the active bulk rename row to remain visible at the bottom of the list"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn archive_create_overlay_adapts_contents_to_short_terminals() {
    let root = temp_path("archive-create-short-terminal");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("source.txt"), "alpha").expect("failed to write source");

    for (height, shows_contents) in [(8, false), (10, true)] {
        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('C'))))
            .expect("archive create overlay should open");
        let mut terminal =
            Terminal::new(TestBackend::new(94, height)).expect("terminal should init");

        let state = draw_ui(&mut terminal, &mut app);
        let panel = state
            .archive_create_panel
            .expect("archive create panel should render");

        assert_eq!(
            state.archive_create_list_area.is_some(),
            shows_contents,
            "contents visibility should track whether a complete row fits at height {height}"
        );
        assert!(
            app.collect_popup_rects().contains(&panel),
            "archive creation popup should mask terminal image previews"
        );
        if !shows_contents {
            assert_eq!(panel.height, 6, "compact popup should not leave dead rows");
        }
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn copy_overlay_renders_expected_labels_and_hit_rects() {
    let root = temp_path("copy-overlay-render");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    fs::write(root.join("docs/report.final.md"), "hello\n").expect("failed to write temp file");

    let mut app = App::new_at(root.join("docs")).expect("app should load temp directory");
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('c'))))
        .expect("copy overlay should open");

    let state = draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        state.copy_panel.is_some(),
        "copy overlay should render a popup panel"
    );
    assert_eq!(
        state.copy_hits.len(),
        4,
        "copy overlay should expose one hit rect per visible row"
    );
    assert!(
        rendered.contains("Copy to clipboard"),
        "expected copy overlay title to be rendered, got: {rendered:?}"
    );
    assert!(
        rendered.contains("c -> file name"),
        "expected copy overlay to render the file-name shortcut row, got: {rendered:?}"
    );
    assert!(
        rendered.contains("d -> directory path"),
        "expected copy overlay to render the directory-path shortcut row, got: {rendered:?}"
    );
    assert!(
        rendered.contains("p -> file path"),
        "expected copy overlay to render the file-path shortcut row, got: {rendered:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn goto_overlay_renders_expected_labels_and_hit_rects() {
    let root = temp_path("goto-overlay-render");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    fs::write(root.join("docs/report.final.md"), "hello\n").expect("failed to write temp file");

    let mut app = App::new_at(root.join("docs")).expect("app should load temp directory");
    let mut terminal = Terminal::new(TestBackend::new(110, 24)).expect("terminal should init");

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('g'))))
        .expect("goto overlay should open");

    let state = draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        state.goto_panel.is_some(),
        "goto overlay should render a popup panel"
    );
    assert_eq!(
        state.goto_hits.len(),
        5,
        "goto overlay should expose one hit rect per visible shortcut"
    );
    assert!(
        rendered.contains("Go to"),
        "expected goto overlay title to be rendered, got: {rendered:?}"
    );
    assert!(
        rendered.contains("g -> top"),
        "expected goto overlay to render the top shortcut row, got: {rendered:?}"
    );
    // The label is ".config" on Linux/BSD, "Application Support" on macOS,
    // and "AppData" on Windows — check for "c ->" which is present on all.
    assert!(
        rendered.contains("c ->"),
        "expected goto overlay to render the config shortcut row, got: {rendered:?}"
    );
    assert!(
        rendered.contains("t -> trash"),
        "expected goto overlay to render the trash shortcut row, got: {rendered:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn open_with_overlay_renders_expected_hits() {
    let root = temp_path("open-with-overlay-render");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("document.txt"), "hello\n").expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    // Wait for the directory to load so the file entry is visible.
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if !app.file_browser.entries.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

    app.inject_open_with_for_test("Text Editor", "/usr/bin/true", vec![], false);

    let state = draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        state.open_with_panel.is_some(),
        "open-with overlay should render a popup panel"
    );
    assert!(
        !state.open_with_hits.is_empty(),
        "open-with overlay should expose at least one hit rect"
    );
    assert!(
        rendered.contains("Open With"),
        "expected open-with title to be rendered, got: {rendered:?}"
    );
    // Shortcut '1' always maps to the first row when apps are found.
    assert!(
        rendered.contains("1 ->"),
        "expected first shortcut row to be rendered, got: {rendered:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn open_with_overlay_draws_scrollbar_only_when_rows_overflow() {
    let root = temp_path("open-with-overlay-scrollbar");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("document.txt"), "hello\n").expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    let mut terminal = Terminal::new(TestBackend::new(90, 12)).expect("terminal should init");

    app.inject_open_with_for_test("Text Editor", "/usr/bin/true", vec![], false);
    let state = draw_ui(&mut terminal, &mut app);
    let panel = state
        .open_with_panel
        .expect("open-with panel should render");
    let inner = helpers::inner_with_padding(panel);
    let right_column = (inner.y..inner.y + inner.height)
        .map(|y| terminal.backend().buffer()[(inner.x + inner.width - 1, y)].symbol())
        .collect::<String>();
    assert!(
        !right_column.contains('┃'),
        "single-row open-with overlay should not draw a scrollbar"
    );

    app.inject_open_with_rows_for_test(
        (0..20)
            .map(|index| {
                (
                    format!("App {index}"),
                    "/usr/bin/true".to_string(),
                    Vec::new(),
                    false,
                )
            })
            .collect(),
    );
    let state = draw_ui(&mut terminal, &mut app);
    let panel = state
        .open_with_panel
        .expect("open-with panel should render");
    let inner = helpers::inner_with_padding(panel);
    let right_column = (inner.y..inner.y + inner.height)
        .map(|y| terminal.backend().buffer()[(inner.x + inner.width - 1, y)].symbol())
        .collect::<String>();

    assert!(
        right_column.contains('┃'),
        "overflowing open-with overlay should draw a scrollbar thumb"
    );
    assert!(
        right_column.contains('│'),
        "overflowing open-with overlay should draw a scrollbar track"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn trash_overlay_tabs_focus_between_confirm_and_cancel_buttons() {
    let root = temp_path("trash-overlay-focus");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("draft.txt"), "hello\n").expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");
    let palette = theme::palette();

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('d'))))
        .expect("trash overlay should open");
    let initial_state = draw_ui(&mut terminal, &mut app);
    let confirm_rect = initial_state
        .trash_confirm_btn
        .expect("trash confirm button should be rendered");
    let cancel_rect = initial_state
        .trash_cancel_btn
        .expect("trash cancel button should be rendered");

    let confirm_cell = &terminal.backend().buffer()[(
        confirm_rect.x.saturating_add(confirm_rect.width / 2),
        confirm_rect.y,
    )];
    let cancel_cell = &terminal.backend().buffer()[(
        cancel_rect.x.saturating_add(cancel_rect.width / 2),
        cancel_rect.y,
    )];
    assert_eq!(
        confirm_cell.bg, palette.selected_bg,
        "confirm button should start focused"
    );
    assert_eq!(
        cancel_cell.bg, palette.chrome_alt,
        "cancel button should start unfocused"
    );

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Tab)))
        .expect("focus toggle should succeed");
    let toggled_state = draw_ui(&mut terminal, &mut app);
    let confirm_cell = &terminal.backend().buffer()[(
        toggled_state
            .trash_confirm_btn
            .expect("confirm button should remain rendered")
            .x
            .saturating_add(confirm_rect.width / 2),
        confirm_rect.y,
    )];
    let cancel_cell = &terminal.backend().buffer()[(
        toggled_state
            .trash_cancel_btn
            .expect("cancel button should remain rendered")
            .x
            .saturating_add(cancel_rect.width / 2),
        cancel_rect.y,
    )];
    assert_eq!(
        confirm_cell.bg, palette.chrome_alt,
        "confirm button should lose focus after tabbing"
    );
    assert_eq!(
        cancel_cell.bg, palette.selected_bg,
        "cancel button should receive focus after tabbing"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn search_overlay_scrolls_selected_results_and_tracks_hit_rects() {
    let root = temp_path("search-overlay-scroll");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..12 {
        fs::create_dir_all(root.join(format!("folder-{index:02}")))
            .expect("failed to create search folder");
    }

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");
    let palette = theme::palette();

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('f'))))
        .expect("search overlay should open");
    wait_for_search_index(&mut app);

    let initial_state = draw_ui(&mut terminal, &mut app);
    let search_panel = initial_state
        .search_panel
        .expect("search overlay should render a popup panel");
    assert!(
        initial_state.search_rows_visible > 0,
        "search overlay should expose the visible row budget through frame state"
    );
    let inner = helpers::inner_with_padding(search_panel);
    let results_area = Rect {
        x: inner.x,
        y: inner.y + 4,
        width: inner.width,
        height: inner.height.saturating_sub(4),
    };
    let bottom_row = rect_row_text(
        terminal.backend().buffer(),
        Rect {
            x: results_area.x,
            y: results_area.y + results_area.height.saturating_sub(1),
            width: results_area.width,
            height: 1,
        },
        results_area.y + results_area.height.saturating_sub(1),
    );
    assert!(
        !bottom_row.contains("Enter open")
            && !bottom_row.contains("Esc close")
            && !bottom_row.contains("move"),
        "search results bottom row should not render generic key hints, got: {bottom_row:?}"
    );
    let right_column = (results_area.y..results_area.y + results_area.height)
        .map(|y| terminal.backend().buffer()[(results_area.x + results_area.width - 1, y)].symbol())
        .collect::<String>();
    assert!(
        right_column.contains('┃'),
        "overflowing search overlay should draw a scrollbar thumb"
    );
    assert!(
        right_column.contains('│'),
        "overflowing search overlay should draw a scrollbar track"
    );

    for _ in 0..8 {
        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Down)))
            .expect("search selection movement should succeed");
    }

    let state = draw_ui(&mut terminal, &mut app);
    let visible_rows = app.search_rows(state.search_rows_visible);
    let selected_offset = visible_rows
        .iter()
        .position(|row| row.selected)
        .expect("search overlay should keep one visible row selected");
    let selected_rect = state
        .search_hits
        .get(selected_offset)
        .expect("search overlay should expose hit rects for visible rows")
        .rect;
    let selected_cell =
        &terminal.backend().buffer()[(selected_rect.x.saturating_add(2), selected_rect.y)];

    assert!(
        visible_rows.first().is_some_and(|row| row.index > 0),
        "search overlay should scroll once the selected result moves past the visible window"
    );
    assert_eq!(
        state.search_hits.len(),
        visible_rows.len(),
        "search hit rects should stay aligned with the rendered visible rows"
    );
    assert_eq!(
        state.search_hits[selected_offset].index, visible_rows[selected_offset].index,
        "search hit rect indexes should stay aligned with the visible search rows"
    );
    assert_eq!(
        selected_cell.bg, palette.selected_bg,
        "selected search rows should keep the focused row background after scrolling"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn help_overlay_keeps_controls_readable_and_drops_auto_reload_row() {
    let root = temp_path("help-overlay-format");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    app.overlays.help = true;
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).expect("terminal should init");

    draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("Double-click"),
        "expected help overlay to keep the double-click label readable, got: {rendered:?}"
    );
    assert!(
        rendered.contains("open item"),
        "expected help overlay to keep the action text readable, got: {rendered:?}"
    );
    assert!(
        rendered.contains("Ctrl+F"),
        "expected help overlay to keep the file search shortcut visible, got: {rendered:?}"
    );
    assert!(
        rendered.contains("zoxide history"),
        "expected help overlay to list the zoxide shortcut, got: {rendered:?}"
    );
    assert!(
        rendered.contains("/… or …/") && rendered.contains("folder in create prompt"),
        "expected help overlay to show the create prompt folder-name hint, got: {rendered:?}"
    );
    assert!(
        rendered.contains("Alt/Shift+Enter"),
        "expected help overlay to show the current create prompt newline hint, got: {rendered:?}"
    );
    assert!(
        rendered.contains("delete permanently"),
        "expected help overlay to list the permanent delete shortcut, got: {rendered:?}"
    );
    assert!(
        rendered.contains("Wheel              scroll"),
        "expected help overlay to describe wheel routing accurately, got: {rendered:?}"
    );
    assert!(
        rendered.contains("Preview"),
        "expected help overlay to include the Preview section header, got: {rendered:?}"
    );
    assert!(
        rendered.contains("K/Shift+↑") && rendered.contains("J/Shift+↓"),
        "expected help overlay to list the vertical preview scroll keys, got: {rendered:?}"
    );
    assert!(
        rendered.contains("H/Shift+←") && rendered.contains("L/Shift+→"),
        "expected help overlay to list uppercase and Shift+arrow horizontal preview scroll keys, got: {rendered:?}"
    );
    assert!(
        rendered.contains("symlink relative"),
        "expected help overlay to keep the final clipboard entry visible without clipping, got: {rendered:?}"
    );
    assert!(
        rendered.contains("View"),
        "expected help overlay to include the View section header, got: {rendered:?}"
    );
    assert!(
        rendered.contains("toggle grid / list"),
        "expected help overlay to keep View entries visible, got: {rendered:?}"
    );
    assert!(
        !rendered.contains("Double clickopen"),
        "help overlay fused the key and action labels together: {rendered:?}"
    );
    assert!(
        !rendered.contains("current folder reloads itself"),
        "help overlay should not list auto-reload as a control: {rendered:?}"
    );
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn compact_help_overlay_scrolls_instead_of_truncating_small_terminals() {
    let root = temp_path("compact-help-overlay");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    app.overlays.help = true;
    let mut terminal = Terminal::new(TestBackend::new(60, 18)).expect("terminal should init");

    let screen_regions = draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("Navigation"),
        "compact help should start with the first section, got: {rendered:?}"
    );
    assert!(
        !rendered.contains("? / Esc") && !rendered.contains("close help"),
        "compact help should not render an obvious close footer, got: {rendered:?}"
    );
    assert!(
        rendered.contains("┃"),
        "scrollable compact help should draw a right-side scrollbar, got: {rendered:?}"
    );
    assert!(
        screen_regions.help_scroll_max > 0,
        "compact help should publish its real scroll limit"
    );
    let max_scroll = screen_regions.help_scroll_max;
    let page_step = screen_regions.help_rows_visible.saturating_sub(2).max(1);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::End)))
        .expect("End should jump to the bottom of help");
    assert_eq!(
        app.overlays.help_scroll, max_scroll,
        "End should use the real scroll limit instead of an arbitrary sentinel"
    );
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::PageUp)))
        .expect("PageUp should move by a viewport-sized step");
    assert_eq!(
        app.overlays.help_scroll,
        max_scroll.saturating_sub(page_step),
        "PageUp should keep context instead of jumping by a fixed magic number"
    );
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::End)))
        .expect("End should return to the bottom of help");
    draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("File Actions") && rendered.contains("open with"),
        "compact help should keep late sections reachable, got: {rendered:?}"
    );
    assert!(
        !rendered.contains("?/Esc") && !rendered.contains("? / Esc"),
        "compact help should not keep the obvious close hint visible, got: {rendered:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn medium_help_overlay_uses_two_columns_before_scrolling() {
    let root = temp_path("medium-help-overlay");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    app.overlays.help = true;
    let mut terminal = Terminal::new(TestBackend::new(92, 37)).expect("terminal should init");

    draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("Navigation") && rendered.contains("File Actions"),
        "medium help should use both sides instead of a sparse single column, got: {rendered:?}"
    );
    assert!(
        rendered.contains("create file or folder"),
        "right-side actions should be visible before scrolling, got: {rendered:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn chooser_help_overlay_uses_chooser_actions_without_changing_esc() {
    let root = temp_path("chooser-help-overlay");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should load temp directory");
    app.enable_chooser_mode();
    app.overlays.help = true;
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).expect("terminal should init");

    draw_ui(&mut terminal, &mut app);
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("Chooser controls"),
        "expected chooser help title, got: {rendered:?}"
    );
    assert!(
        rendered.contains("choose"),
        "expected chooser help to list choose action, got: {rendered:?}"
    );
    assert!(
        rendered.contains("enter folder / choose"),
        "expected chooser double-click help to fit on one line, got: {rendered:?}"
    );
    assert!(
        !rendered.contains("enter folder / open"),
        "expected chooser help to hide the fully shadowed open-or-enter action, got: {rendered:?}"
    );
    assert!(
        rendered.contains("cancel chooser"),
        "expected chooser help to relabel quit as cancellation, got: {rendered:?}"
    );
    assert!(
        rendered.contains("Esc"),
        "expected chooser help to keep normal Esc behavior visible, got: {rendered:?}"
    );
    assert!(
        rendered.contains("clear selection"),
        "expected chooser help to keep Esc as clear selection, got: {rendered:?}"
    );
    assert!(
        rendered.contains("copy path details"),
        "expected chooser help to keep copy path visible, got: {rendered:?}"
    );
    assert!(
        rendered.contains("cut"),
        "expected chooser help to keep cut visible, got: {rendered:?}"
    );
    assert!(
        rendered.contains("paste"),
        "expected chooser help to keep paste visible, got: {rendered:?}"
    );
    assert!(
        !rendered.contains("quit without cd"),
        "chooser help should not describe quit_without_cd as normal quit, got: {rendered:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
