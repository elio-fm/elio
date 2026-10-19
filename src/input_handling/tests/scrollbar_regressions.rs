use super::helpers::cleanup_app_temp_root;
use crate::file_browser::ViewMode;
use crate::{
    app::{App, EntryHit, OpenWithHit, ScreenRegions, ScrollbarHit, ScrollbarTarget, ViewMetrics},
    duplicate_finder::{DuplicateFile, DuplicateFinderSession, DuplicateGroup},
    fuzzy_finder::{SearchCandidate, SearchIndexStats, SearchScope, SearchState},
    opening::open_with::ApplicationSelection,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("elio-scrollbar-regression-{label}-{unique}"));
    fs::create_dir_all(&path).expect("create temporary root");
    path
}

fn scrollbar(target: ScrollbarTarget, scroll_row: usize) -> ScrollbarHit {
    let track = Rect::new(10, 0, 1, 100);
    ScrollbarHit {
        rect: track,
        track,
        total_rows: 100,
        visible_rows: 10,
        scroll_row,
        target,
    }
}

fn mouse(kind: MouseEventKind, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: 10,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn start_thumb_drag(app: &mut App, target: ScrollbarTarget, scroll_row: usize) {
    app.input.screen_regions = ScreenRegions {
        scrollbar_hits: vec![scrollbar(target, scroll_row)],
        search_rows_visible: 10,
        duplicate_rows_visible: 10,
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 100,
        },
        ..ScreenRegions::default()
    };
    // With a 10-row thumb on a 100-row track, row 25 is inside the thumb at scroll 20.
    app.handle_event(mouse(MouseEventKind::Down(MouseButton::Left), 25))
        .expect("thumb press should be handled");
    assert!(
        app.input.scrollbar_drag.is_some(),
        "thumb press should start dragging"
    );
}

fn fake_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    let app = App::new_at(root.clone()).expect("create app");
    (app, root)
}

fn populated_browser(label: &str, count: usize) -> (App, PathBuf) {
    let root = temp_root(label);
    for index in 0..count {
        fs::write(root.join(format!("item-{index:03}.txt")), "data")
            .expect("write browser test entry");
    }
    let mut app = App::new_at(root.clone()).expect("create app");
    super::helpers::wait_for_directory_load(&mut app);
    (app, root)
}

fn start_browser_scrollbar_drag(
    app: &mut App,
    total_rows: usize,
    visible_rows: usize,
    scroll_row: usize,
    cols: usize,
    press_row: u16,
) {
    app.input.screen_regions = ScreenRegions {
        scrollbar_hits: vec![scrollbar_with_geometry(
            ScrollbarTarget::Browser,
            total_rows,
            visible_rows,
            scroll_row,
        )],
        metrics: ViewMetrics {
            cols,
            rows_visible: visible_rows,
        },
        ..ScreenRegions::default()
    };
    app.handle_event(mouse(MouseEventKind::Down(MouseButton::Left), press_row))
        .expect("browser scrollbar press should be handled");
    assert!(app.input.scrollbar_drag.is_some());
}

fn scrollbar_with_geometry(
    target: ScrollbarTarget,
    total_rows: usize,
    visible_rows: usize,
    scroll_row: usize,
) -> ScrollbarHit {
    let track = Rect::new(10, 0, 1, 100);
    ScrollbarHit {
        rect: track,
        track,
        total_rows,
        visible_rows,
        scroll_row,
        target,
    }
}

#[test]
fn modal_dialog_blocks_scrollbar_hits_from_the_browser_behind_it() {
    let (mut app, root) = fake_app("modal-shield");
    app.file_browser.scroll_row = 20;
    app.open_create_prompt();
    app.input.screen_regions = ScreenRegions {
        scrollbar_hits: vec![scrollbar(ScrollbarTarget::Browser, 20)],
        create_panel: Some(Rect::new(0, 0, 30, 100)),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 100,
        },
        ..ScreenRegions::default()
    };

    app.handle_event(mouse(MouseEventKind::Down(MouseButton::Left), 25))
        .expect("click inside the dialog should be handled");

    assert!(app.file_operations.create_is_open());
    assert_eq!(app.file_browser.scroll_row, 20);
    assert!(app.input.scrollbar_drag.is_none());
    cleanup_app_temp_root(app, root);
}

#[test]
fn scrollbar_drag_keeps_the_focused_row_at_the_same_screen_position() {
    let (mut search_app, search_root) = fake_app("search-focus-row");
    let candidates = (0..100)
        .map(|index| {
            let name = format!("item-{index}");
            SearchCandidate {
                path: search_root.join(&name),
                name: name.clone(),
                name_key: name.clone(),
                relative: name.clone(),
                relative_key: name,
                is_dir: true,
                symlink: None,
            }
        })
        .collect();
    let mut search = SearchState::new(
        SearchScope::Files,
        Arc::new(candidates),
        SearchIndexStats::default(),
        false,
    );
    search.selected = 25;
    search.scroll = 20;
    search_app.fuzzy_finder.search = Some(search);
    start_thumb_drag(&mut search_app, ScrollbarTarget::Search, 20);
    search_app
        .handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 45))
        .expect("search scrollbar drag should be handled");
    let search = search_app.fuzzy_finder.search.as_ref().unwrap();
    assert_eq!(search.scroll, 40);
    assert_eq!(search.selected - search.scroll, 5);
    cleanup_app_temp_root(search_app, search_root);

    let (mut duplicate_app, duplicate_root) = fake_app("duplicate-focus-row");
    let files = (0..100)
        .map(|index| DuplicateFile {
            path: duplicate_root.join(format!("item-{index}")),
            name: format!("item-{index}"),
            relative: format!("item-{index}"),
            size: 1,
            modified: None,
        })
        .collect();
    let mut duplicates = DuplicateFinderSession::new(duplicate_root.clone());
    duplicates.groups = vec![DuplicateGroup {
        id: 0,
        size: 1,
        files,
    }];
    duplicates.loading = false;
    duplicates.selected = 25;
    duplicates.scroll = 20;
    duplicate_app.duplicate_finder.session = Some(duplicates);
    duplicate_app.input.screen_regions.duplicate_rows_visible = 10;
    start_thumb_drag(&mut duplicate_app, ScrollbarTarget::DuplicateResults, 20);
    duplicate_app
        .handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 45))
        .expect("duplicate results scrollbar drag should be handled");
    let duplicates = duplicate_app.duplicate_finder.session.as_ref().unwrap();
    assert_eq!(duplicates.scroll, 40);
    assert_eq!(duplicates.selected - duplicates.scroll, 5);
    cleanup_app_temp_root(duplicate_app, duplicate_root);

    let (mut open_with_app, open_with_root) = fake_app("open-with-focus-row");
    open_with_app.overlays.open_with = Some(ApplicationSelection::from_test_rows(
        (0..100)
            .map(|index| (format!("App {index}"), "app".to_string(), Vec::new(), false))
            .collect(),
    ));
    open_with_app.select_open_with_index_from_scrollbar(25, 20);
    open_with_app.set_screen_regions(ScreenRegions {
        scrollbar_hits: vec![scrollbar(ScrollbarTarget::OpenWith, 20)],
        open_with_hits: (0..10)
            .map(|index| OpenWithHit {
                rect: Rect::new(0, index, 10, 1),
                index: index as usize,
            })
            .collect(),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 100,
        },
        ..ScreenRegions::default()
    });
    open_with_app
        .handle_event(mouse(MouseEventKind::Down(MouseButton::Left), 25))
        .expect("Open With thumb press should be handled");
    open_with_app
        .handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 45))
        .expect("Open With scrollbar drag should be handled");
    assert_eq!(open_with_app.open_with_selected_index(), 45);
    assert_eq!(open_with_app.open_with_scroll_top(10), 40);
    cleanup_app_temp_root(open_with_app, open_with_root);
}

#[test]
fn open_with_keyboard_navigation_keeps_drag_viewport_until_selection_leaves_it() {
    let (mut app, root) = fake_app("open-with-keyboard-after-drag");
    let rows = (0..120)
        .map(|index| (format!("App {index}"), "app".to_string(), Vec::new(), false))
        .collect();
    app.overlays.open_with = Some(ApplicationSelection::from_test_rows(rows));
    app.select_open_with_index_from_scrollbar(102, 100);
    app.set_screen_regions(ScreenRegions {
        open_with_hits: (0..8)
            .map(|index| OpenWithHit {
                rect: Rect::new(0, index, 10, 1),
                index: index as usize,
            })
            .collect(),
        ..ScreenRegions::default()
    });
    assert_eq!(app.open_with_scroll_top(8), 100);

    app.move_open_with_selection(1);
    assert_eq!(app.open_with_selected_index(), 103);
    assert_eq!(app.open_with_scroll_top(8), 100);

    for _ in 0..5 {
        app.move_open_with_selection(1);
    }
    assert_eq!(app.open_with_selected_index(), 108);
    assert_eq!(app.open_with_scroll_top(8), 101);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("normal Open With navigation should remain available");
    assert_eq!(app.open_with_selected_index(), 109);
    assert_eq!(app.open_with_scroll_top(8), 102);
    cleanup_app_temp_root(app, root);
}

#[test]
fn interrupted_scrollbar_drag_is_canceled_by_modal_or_focus_loss() {
    let (mut app, root) = fake_app("interrupted-drag");
    app.file_browser.scroll_row = 20;
    start_thumb_drag(&mut app, ScrollbarTarget::Browser, 20);
    app.open_create_prompt();

    app.handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 45))
        .expect("drag into a newly opened modal should be consumed");
    assert!(app.input.scrollbar_drag.is_none());
    assert_eq!(app.file_browser.scroll_row, 20);

    let (mut focus_app, focus_root) = fake_app("interrupted-focus-loss");
    focus_app.file_browser.scroll_row = 20;
    start_thumb_drag(&mut focus_app, ScrollbarTarget::Browser, 20);
    focus_app
        .handle_event(Event::FocusLost)
        .expect("focus loss should cancel an active scrollbar drag");
    assert!(focus_app.input.scrollbar_drag.is_none());
    focus_app
        .handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 60))
        .expect("stale drag motion should not move the browser");
    assert_eq!(focus_app.file_browser.scroll_row, 20);
    cleanup_app_temp_root(app, root);
    cleanup_app_temp_root(focus_app, focus_root);
}

#[test]
fn any_key_press_cancels_scrollbar_drag_but_key_release_does_not() {
    let (mut app, root) = populated_browser("list-key-cancels-drag", 100);
    app.file_browser.view_mode = ViewMode::List;
    app.file_browser.selected = 25;
    app.file_browser.scroll_row = 20;
    start_browser_scrollbar_drag(&mut app, 100, 10, 20, 1, 25);

    app.handle_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Down,
        KeyModifiers::NONE,
        crossterm::event::KeyEventKind::Release,
    )))
    .expect("navigation key release should be ignored");
    assert!(app.input.scrollbar_drag.is_some());

    app.handle_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('x'),
        KeyModifiers::NONE,
        crossterm::event::KeyEventKind::Repeat,
    )))
    .expect("repeated key should be handled normally");
    assert!(app.input.scrollbar_drag.is_none());
    assert!(app.input.scrollbar_drag_cancelled_until_release);

    app.handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 45))
        .expect("remaining drag motion should be ignored");
    assert_eq!(app.file_browser.selected, 25);
    assert_eq!(app.file_browser.scroll_row, 20);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Down)))
        .expect("down should move the selected entry");
    assert_eq!(app.file_browser.selected, 26);
    assert_eq!(app.file_browser.scroll_row, 20);
    assert!(app.input.scrollbar_drag.is_none());
    assert!(app.input.scrollbar_drag_cancelled_until_release);

    app.handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 45))
        .expect("remaining drag motion should be ignored");
    assert_eq!(app.file_browser.selected, 26);
    assert_eq!(app.file_browser.scroll_row, 20);
    cleanup_app_temp_root(app, root);
}

#[test]
fn grid_horizontal_navigation_cancels_scrollbar_drag() {
    let (mut app, root) = populated_browser("grid-key-cancels-drag", 100);
    app.file_browser.view_mode = ViewMode::Grid;
    app.file_browser.selected = 10;
    app.file_browser.scroll_row = 2;
    start_browser_scrollbar_drag(&mut app, 25, 10, 2, 4, 10);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Right)))
        .expect("right should move focus across the grid");
    assert_eq!(app.file_browser.selected, 11);
    assert_eq!(app.file_browser.scroll_row, 2);
    assert!(app.input.scrollbar_drag.is_none());

    app.handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30))
        .expect("remaining grid scrollbar motion should be ignored");
    assert_eq!(app.file_browser.selected, 11);
    assert_eq!(app.file_browser.scroll_row, 2);
    cleanup_app_temp_root(app, root);
}

#[test]
fn releasing_canceled_scrollbar_drag_restores_mouse_and_dnd_input() {
    let (mut app, root) = populated_browser("cancelled-drag-release", 100);
    app.file_browser.view_mode = ViewMode::List;
    app.file_browser.selected = 25;
    app.file_browser.scroll_row = 20;
    let focused_path = app.file_browser.entries[25].path.clone();
    app.input.screen_regions = ScreenRegions {
        scrollbar_hits: vec![scrollbar_with_geometry(
            ScrollbarTarget::Browser,
            100,
            10,
            20,
        )],
        entry_hits: vec![EntryHit {
            rect: Rect::new(30, 5, 20, 1),
            index: 25,
        }],
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 10,
        },
        ..ScreenRegions::default()
    };
    app.handle_event(mouse(MouseEventKind::Down(MouseButton::Left), 25))
        .expect("thumb press should start drag");
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Down)))
        .expect("keyboard navigation should cancel drag");
    assert!(app.take_drag_export_paths_at(35, 5).is_empty());

    app.handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 45))
        .expect("canceled drag motion should be ignored");
    assert!(app.input.scrollbar_drag_cancelled_until_release);
    app.handle_event(mouse(MouseEventKind::Up(MouseButton::Left), 45))
        .expect("left release should finish the canceled gesture");
    assert!(!app.input.scrollbar_drag_cancelled_until_release);
    assert_eq!(app.take_drag_export_paths_at(35, 5), vec![focused_path]);

    app.handle_event(mouse(MouseEventKind::Down(MouseButton::Left), 25))
        .expect("a later thumb press should start a fresh drag");
    assert!(app.input.scrollbar_drag.is_some());

    app.handle_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('x'),
        KeyModifiers::NONE,
        crossterm::event::KeyEventKind::Press,
    )))
    .expect("a key press should cancel the new drag");
    assert!(app.input.scrollbar_drag_cancelled_until_release);
    // The previous left-button release may be lost. A new press must recover the
    // mouse state and begin a fresh scrollbar interaction.
    app.handle_event(mouse(MouseEventKind::Down(MouseButton::Left), 25))
        .expect("fresh press should recover after a missed release");
    assert!(!app.input.scrollbar_drag_cancelled_until_release);
    assert!(app.input.scrollbar_drag.is_some());
    cleanup_app_temp_root(app, root);
}
