use super::super::terminal_image_previews::{expand_raster_erase_area, push_blank_cell_runs};
use crate::app::ScreenRegions;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

#[test]
fn modal_mask_only_targets_reset_background_blank_cells() {
    let mut buffer = Buffer::empty(Rect {
        x: 0,
        y: 0,
        width: 5,
        height: 1,
    });
    buffer.set_string(1, 0, "  ", Style::default().bg(Color::Blue));
    buffer.set_string(3, 0, "x", Style::default());

    let mut rects = Vec::new();
    push_blank_cell_runs(
        &mut rects,
        Rect {
            x: 0,
            y: 0,
            width: 5,
            height: 1,
        },
        &buffer,
    );

    assert_eq!(
        rects,
        vec![
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            Rect {
                x: 4,
                y: 0,
                width: 1,
                height: 1,
            },
        ]
    );
}

#[test]
fn raster_erase_area_can_grow_right_and_bottom_within_preview_bounds() {
    let screen_regions = ScreenRegions {
        preview_panel: Some(Rect {
            x: 10,
            y: 5,
            width: 40,
            height: 20,
        }),
        preview_body_area: Some(Rect {
            x: 12,
            y: 7,
            width: 30,
            height: 10,
        }),
        ..ScreenRegions::default()
    };
    let area = Rect {
        x: 12,
        y: 7,
        width: 20,
        height: 8,
    };

    assert_eq!(
        expand_raster_erase_area(&screen_regions, area, 1, 1),
        Rect {
            x: 12,
            y: 7,
            width: 21,
            height: 9,
        }
    );
}

#[test]
fn raster_erase_area_does_not_grow_into_preview_border() {
    let screen_regions = ScreenRegions {
        preview_panel: Some(Rect {
            x: 10,
            y: 5,
            width: 40,
            height: 20,
        }),
        preview_body_area: Some(Rect {
            x: 12,
            y: 7,
            width: 30,
            height: 10,
        }),
        ..ScreenRegions::default()
    };
    let area = Rect {
        x: 12,
        y: 14,
        width: 20,
        height: 3,
    };

    assert_eq!(
        expand_raster_erase_area(&screen_regions, area, 1, 2),
        Rect {
            x: 12,
            y: 14,
            width: 21,
            height: 3,
        }
    );
}
