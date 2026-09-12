mod rendering;
mod status_bar;

use super::*;
use crate::app::{DuplicateHit, ScreenRegions};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn render_clears_duplicate_frame_hits_before_drawing() {
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).expect("terminal should init");
    let app = App::new().expect("app should init");
    let mut state = ScreenRegions {
        duplicate_hits: vec![DuplicateHit {
            rect: Rect::new(1, 1, 5, 1),
            index: 42,
        }],
        duplicate_panel: Some(Rect::new(0, 0, 10, 10)),
        duplicate_rows_visible: 9,
        ..ScreenRegions::default()
    };

    terminal
        .draw(|frame| render(frame, &app, &mut state))
        .expect("ui should render");

    assert!(state.duplicate_hits.is_empty());
    assert!(state.duplicate_panel.is_none());
    assert_eq!(state.duplicate_rows_visible, 0);
}
