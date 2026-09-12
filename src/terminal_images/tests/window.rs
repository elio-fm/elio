use super::*;

#[test]
fn parse_cell_pixel_response_reads_cell_dimensions() {
    assert_eq!(parse_cell_pixel_response("\x1b[6;20;10t"), Some((10, 20)));
}

#[test]
fn parse_cell_pixel_response_finds_response_after_leading_bytes() {
    assert_eq!(parse_cell_pixel_response("ab\x1b[6;20;10t"), Some((10, 20)));
}

#[test]
fn parse_cell_pixel_response_rejects_zero_dimensions() {
    assert_eq!(parse_cell_pixel_response("\x1b[6;0;10t"), None);
    assert_eq!(parse_cell_pixel_response("\x1b[6;20;0t"), None);
}

#[test]
fn parse_cell_pixel_response_rejects_malformed_input() {
    assert_eq!(parse_cell_pixel_response("not a response"), None);
    assert_eq!(parse_cell_pixel_response("\x1b[6;20t"), None);
}

#[test]
fn fallback_window_size_pixels_uses_reasonable_cell_defaults() {
    assert_eq!(fallback_window_size_pixels(100, 40), (800, 640));
    assert_eq!(fallback_window_size_pixels(0, 0), (8, 16));
}
