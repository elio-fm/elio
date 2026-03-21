mod entries;
mod grid;
mod layout;
mod list;
mod preview;
mod scrollbar;
mod sidebar;

use super::theme::Palette;
use crate::app::{App, FrameState};
use ratatui::{Frame, layout::Rect};

pub(super) fn render_body(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    layout::render_body(frame, area, app, state, palette);
}

#[cfg(test)]
mod tests {
    use super::super::theme;
    use super::scrollbar::split_scrollbar_area;
    use crate::app::{App, FrameState};
    use crate::ui;
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
        std::env::temp_dir().join(format!("elio-browser-{label}-{unique}"))
    }

    fn draw_ui(terminal: &mut Terminal<TestBackend>, app: &mut App) -> FrameState {
        let mut frame_state = FrameState::default();
        terminal
            .draw(|frame| ui::render(frame, app, &mut frame_state))
            .expect("ui should render");
        app.set_frame_state(frame_state.clone());
        frame_state
    }

    fn wait_for_directory_counts(app: &mut App) {
        for _ in 0..100 {
            let _ = app.process_background_jobs();
            let all_visible_directory_counts_loaded = app
                .entries
                .iter()
                .filter(|entry| entry.is_dir())
                .all(|entry| app.directory_item_count_label(entry).is_some());
            if all_visible_directory_counts_loaded {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out waiting for directory counts");
    }

    fn wait_for_background_preview(app: &mut App) {
        for _ in 0..200 {
            if app.process_background_jobs() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out waiting for background preview");
    }

    fn wait_for_search_index(app: &mut App) {
        for _ in 0..200 {
            let _ = app.process_background_jobs();
            if app.search_is_open() && !app.search_is_loading() && app.search_candidate_count() > 0
            {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out waiting for search index");
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }

    fn rect_row_text(buffer: &Buffer, rect: Rect, y: u16) -> String {
        (rect.x..rect.x.saturating_add(rect.width))
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }

    fn buffer_text(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| row_text(buffer, y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rect_inside(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x.saturating_add(inner.width) <= outer.x.saturating_add(outer.width)
            && inner.y.saturating_add(inner.height) <= outer.y.saturating_add(outer.height)
    }

    #[test]
    fn wide_browser_layout_keeps_entries_and_preview_side_by_side() {
        let root = temp_path("wide-browser-layout");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("report.txt"), "hello\nworld\n").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(140, 30)).expect("terminal should init");

        let state = draw_ui(&mut terminal, &mut app);
        let entries_panel = state
            .entries_panel
            .expect("entries panel should be rendered");
        let preview_panel = state
            .preview_panel
            .expect("preview panel should be rendered");
        let sidebar_rect = state
            .sidebar_hits
            .first()
            .map(|hit| hit.rect)
            .expect("sidebar should expose at least one hit rect");

        assert!(
            sidebar_rect.x.saturating_add(sidebar_rect.width) <= entries_panel.x,
            "wide layout should keep the sidebar to the left of the entries panel"
        );
        assert_eq!(
            entries_panel.y, preview_panel.y,
            "wide layout should align entries and preview panels on the same row"
        );
        assert_eq!(
            entries_panel.height, preview_panel.height,
            "wide layout should keep entries and preview panels at the same height"
        );
        assert!(
            entries_panel.x.saturating_add(entries_panel.width) <= preview_panel.x,
            "wide layout should place the preview panel to the right of the entries panel"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn narrow_browser_layout_stacks_preview_below_entries() {
        let root = temp_path("narrow-browser-layout");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("report.txt"), "hello\nworld\n").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(110, 30)).expect("terminal should init");

        let state = draw_ui(&mut terminal, &mut app);
        let entries_panel = state
            .entries_panel
            .expect("entries panel should be rendered");
        let preview_panel = state
            .preview_panel
            .expect("preview panel should be rendered");

        assert_eq!(
            entries_panel.x, preview_panel.x,
            "narrow layout should keep entries and preview aligned on the same right column"
        );
        assert_eq!(
            entries_panel.width, preview_panel.width,
            "narrow layout should keep entries and preview at the same width"
        );
        assert!(
            entries_panel.y.saturating_add(entries_panel.height) <= preview_panel.y,
            "narrow layout should stack the preview panel below the entries panel"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn split_scrollbar_area_only_reserves_a_column_when_width_allows() {
        let tight = Rect {
            x: 3,
            y: 4,
            width: 5,
            height: 7,
        };
        let (content, scrollbar) = split_scrollbar_area(tight);
        assert_eq!(content, tight);
        assert_eq!(scrollbar, None);

        let roomy = Rect {
            x: 8,
            y: 2,
            width: 6,
            height: 9,
        };
        let (content, scrollbar) = split_scrollbar_area(roomy);
        let scrollbar = scrollbar.expect("wide enough areas should reserve a scrollbar column");
        assert_eq!(content.width, 5);
        assert_eq!(scrollbar.width, 1);
        assert_eq!(content.height, roomy.height);
        assert_eq!(scrollbar.height, roomy.height);
        assert_eq!(scrollbar.x, content.x.saturating_add(content.width));
    }

    #[test]
    fn grid_view_keeps_entry_hits_inside_the_entries_panel() {
        let root = temp_path("grid-layout-hits");
        fs::create_dir_all(&root).expect("failed to create temp root");
        for index in 0..12 {
            fs::write(root.join(format!("item-{index:02}.txt")), "content\n")
                .expect("failed to write temp file");
        }

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        app.view_mode = crate::app::ViewMode::Grid;
        let mut terminal = Terminal::new(TestBackend::new(140, 30)).expect("terminal should init");

        let state = draw_ui(&mut terminal, &mut app);
        let entries_panel = state
            .entries_panel
            .expect("entries panel should be rendered");

        assert!(
            state.metrics.cols >= 2,
            "wide grid layouts should expose multiple columns through view metrics"
        );
        assert!(
            !state.entry_hits.is_empty(),
            "grid rendering should expose hit rects for visible entries"
        );
        for hit in &state.entry_hits {
            assert!(
                rect_inside(entries_panel, hit.rect),
                "entry hit {:?} should stay inside the entries panel {:?}",
                hit.rect,
                entries_panel
            );
        }

        fs::remove_dir_all(root).expect("failed to remove temp root");
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
        for index in 0..10 {
            for ch in format!("file-{index:02}.txt").chars() {
                app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(ch))))
                    .expect("typing create line should succeed");
            }
            if index < 9 {
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
            "create overlay should scroll once the cursor moves past the eighth visible line"
        );
        assert!(
            rect_row_text(terminal.backend().buffer(), list_area, list_area.y)
                .contains("file-02.txt"),
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
            .contains("file-09.txt"),
            "expected the active create line to remain visible at the bottom of the list"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn bulk_rename_overlay_scrolls_to_keep_the_active_row_visible() {
        let root = temp_path("bulk-rename-overlay-scroll");
        fs::create_dir_all(&root).expect("failed to create temp root");
        for index in 0..10 {
            fs::write(root.join(format!("file-{index:02}.txt")), "content")
                .expect("failed to write test file");
        }

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        app.view_mode = crate::app::ViewMode::List;
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        for _ in 0..10 {
            app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
                .expect("selection toggle should succeed");
        }
        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('r'))))
            .expect("bulk rename overlay should open");
        for _ in 0..9 {
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
            "bulk rename overlay should scroll once the active row moves past the eighth visible line"
        );
        assert!(
            rect_row_text(terminal.backend().buffer(), list_area, list_area.y)
                .contains("file-02.txt"),
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
            .contains("file-09.txt"),
            "expected the active bulk rename row to remain visible at the bottom of the list"
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
        assert!(
            initial_state.search_panel.is_some(),
            "search overlay should render a popup panel"
        );
        assert!(
            initial_state.search_rows_visible > 0,
            "search overlay should expose the visible row budget through frame state"
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
    fn preview_title_row_is_cleared_when_switching_to_shorter_names() {
        let root = temp_path("preview-title");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(
            root.join("a-this-is-a-very-long-preview-marker-name.txt"),
            "first\n",
        )
        .expect("failed to write long file");
        fs::write(root.join("b.txt"), "second\n").expect("failed to write short file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        let initial_state = draw_ui(&mut terminal, &mut app);
        let preview_panel = initial_state
            .preview_panel
            .expect("preview panel should be rendered");
        let initial_title = row_text(terminal.backend().buffer(), preview_panel.y);
        assert!(
            initial_title.contains("preview-marker-name"),
            "expected initial preview title row to show the long file name, got: {initial_title:?}"
        );

        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Down)))
            .expect("selection change should succeed");
        let second_state = draw_ui(&mut terminal, &mut app);
        let second_title = row_text(
            terminal.backend().buffer(),
            second_state.preview_panel.unwrap().y,
        );

        assert!(
            second_title.contains("b.txt"),
            "expected second preview title row to show the shorter file name, got: {second_title:?}"
        );
        assert!(
            !second_title.contains("preview-marker-name"),
            "stale preview title text remained after rerender: {second_title:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn filenames_with_control_characters_are_rendered_safely() {
        let root = temp_path("control-char-name");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("bad\rname.c"), "int main(void) { return 0; }\n")
            .expect("failed to write control-char file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("bad^Mname.c"),
            "expected control characters to be sanitized in the UI, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_panel_does_not_repeat_generic_metadata() {
        let root = temp_path("preview-details");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("report.txt"), "hello\n").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(
            !rendered.contains("Type     "),
            "preview panel should not repeat generic type metadata, got: {rendered:?}"
        );
        assert!(
            !rendered.contains("Size     "),
            "preview panel should not repeat generic size metadata, got: {rendered:?}"
        );
        assert!(
            !rendered.contains("Modified "),
            "preview panel should not repeat generic modified metadata, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn help_overlay_keeps_controls_readable_and_drops_auto_reload_row() {
        let root = temp_path("help-overlay-format");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        app.help_open = true;
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

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
    fn entries_and_preview_panels_keep_top_border_segments() {
        let root = temp_path("panel-top-borders");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("report.txt"), "hello\nworld\n").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        let state = draw_ui(&mut terminal, &mut app);
        let entries_panel = state
            .entries_panel
            .expect("entries panel should be rendered");
        let preview_panel = state
            .preview_panel
            .expect("preview panel should be rendered");

        let entries_top = row_text(terminal.backend().buffer(), entries_panel.y);
        let preview_top = row_text(terminal.backend().buffer(), preview_panel.y);

        assert!(
            entries_top.contains("─"),
            "expected entries panel to keep top border segments, got: {entries_top:?}"
        );
        assert!(
            preview_top.contains("─"),
            "expected preview panel to keep top border segments, got: {preview_top:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_header_detail_uses_compact_labels_before_final_clamp() {
        let root = temp_path("preview-header-clamp");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let contents = (1..=300)
            .map(|index| format!("line {index} {}", "word ".repeat(30)))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(root.join("report.txt"), contents).expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("terminal should init");
        wait_for_background_preview(&mut app);

        let state = draw_ui(&mut terminal, &mut app);
        let preview_panel = state
            .preview_panel
            .expect("preview panel should be rendered");
        let header_row = row_text(terminal.backend().buffer(), preview_panel.y + 1);

        assert!(
            header_row.contains("Text"),
            "expected preview header row to contain the section label, got: {header_row:?}"
        );
        assert!(
            header_row.contains("240 / 300 lines shown"),
            "expected preview header row to show semantic line coverage, got: {header_row:?}"
        );
        assert!(
            !header_row.contains("240-line cap"),
            "expected preview header row to avoid internal cap wording, got: {header_row:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn visible_directory_rows_show_cached_item_counts() {
        let root = temp_path("directory-counts");
        let photos = root.join("photos");
        fs::create_dir_all(&photos).expect("failed to create folder");
        fs::write(photos.join("one.jpg"), "a").expect("failed to write first file");
        fs::write(photos.join("two.jpg"), "b").expect("failed to write second file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        wait_for_directory_counts(&mut app);
        draw_ui(&mut terminal, &mut app);

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("2 items"),
            "expected visible directory rows to show cached item counts, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn compact_list_rows_keep_metadata_visible_for_wide_names() {
        let root = temp_path("wide-list-metadata");
        let series = root.join("北斗の拳究極版北斗の拳究極版北斗の拳究極版北斗の拳究極版");
        fs::create_dir_all(&series).expect("failed to create series folder");
        for index in 0..10 {
            fs::write(series.join(format!("chapter-{index}.txt")), "x")
                .expect("failed to write child file");
        }

        let epub_path =
            root.join("北斗の拳究極版北斗の拳究極版北斗の拳究極版北斗の拳究極版13.epub");
        let epub = fs::File::create(&epub_path).expect("failed to create epub");
        epub.set_len(13_000_000).expect("failed to size epub");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        wait_for_directory_counts(&mut app);
        let state = draw_ui(&mut terminal, &mut app);
        let entries_panel = state
            .entries_panel
            .expect("entries panel should be rendered");

        let rows = (entries_panel.y..entries_panel.y.saturating_add(entries_panel.height))
            .map(|y| rect_row_text(terminal.backend().buffer(), entries_panel, y))
            .collect::<Vec<_>>();
        let rendered = rows.join("\n");
        let folder_row = rows
            .iter()
            .find(|row| row.contains("10 items"))
            .expect("folder row should keep its item count visible");
        let epub_row = rows
            .iter()
            .find(|row| row.contains("13 MB"))
            .expect("epub row should keep its size visible");

        assert!(
            folder_row.contains("ago"),
            "expected wide directory rows to keep modified timestamps visible, got: {folder_row:?}"
        );
        assert!(
            epub_row.contains("ago"),
            "expected wide epub rows to keep modified timestamps visible, got: {epub_row:?}"
        );
        assert!(
            rendered.contains("10 items") && rendered.contains("13 MB"),
            "expected wide-name rows to keep full metadata visible, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
