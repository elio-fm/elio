use super::{collect_buffer_cells, split_sixel_placement};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

#[test]
fn ratatui_diff_preserves_positions_beyond_u16_max_cells() {
    let area = Rect::new(0, 0, 400, 200);
    let previous = Buffer::empty(area);
    let mut next = Buffer::empty(area);
    next.set_string(123, 180, "X", Style::default());

    let diff = previous.diff(&next);

    assert!(
        diff.iter()
            .any(|(x, y, cell)| *x == 123 && *y == 180 && cell.symbol() == "X"),
        "expected diff to keep the changed cell at (123, 180), got: {:?}",
        diff.iter()
            .map(|(x, y, cell)| (*x, *y, cell.symbol().to_string()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn collect_buffer_cells_captures_popup_cells_with_styles() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 4));
    buffer.set_string(
        2,
        1,
        "OK",
        Style::default()
            .fg(Color::LightGreen)
            .bg(Color::Rgb(1, 2, 3))
            .add_modifier(Modifier::BOLD),
    );

    let cells = collect_buffer_cells(&[Rect::new(2, 1, 2, 1)], &buffer);

    assert_eq!(cells.len(), 2);
    assert_eq!((cells[0].0, cells[0].1, cells[0].2.symbol()), (2, 1, "O"));
    assert_eq!((cells[1].0, cells[1].1, cells[1].2.symbol()), (3, 1, "K"));
    assert_eq!(cells[0].2.fg, Color::LightGreen);
    assert_eq!(cells[0].2.bg, Color::Rgb(1, 2, 3));
    assert!(cells[0].2.modifier.contains(Modifier::BOLD));
}

#[test]
fn split_sixel_placement_separates_cursor_move_from_dcs() {
    let bytes = b"\x1b[12;34H\x1bP0;1;0qpayload\x1b\\";
    let (cursor_move, dcs) = split_sixel_placement(bytes).expect("valid sixel placement");

    assert_eq!(cursor_move, b"\x1b[12;34H");
    assert_eq!(dcs, b"\x1bP0;1;0qpayload\x1b\\");
}

#[test]
fn split_sixel_placement_rejects_non_sixel_output() {
    assert!(split_sixel_placement(b"\x1b[12;34Htext").is_none());
}
