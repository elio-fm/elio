use super::*;

#[test]
fn visible_edit_rows_caps_and_shrinks_to_terminal_height() {
    let area = Rect::new(0, 0, 90, 24);
    assert_eq!(visible_edit_rows(area, 40, 5), 12);

    let short_area = Rect::new(0, 0, 90, 10);
    assert_eq!(visible_edit_rows(short_area, 40, 5), 3);
    assert_eq!(visible_edit_rows(short_area, 40, 7), 1);
    assert_eq!(visible_edit_rows(short_area, 0, 5), 1);
}
