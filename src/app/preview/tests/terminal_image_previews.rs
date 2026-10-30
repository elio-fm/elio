use super::super::terminal_image_previews::{expand_raster_erase_area, push_blank_cell_runs};
use crate::{
    app::{App, ScreenRegions},
    terminal_images::ImageProtocol,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};
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
    std::env::temp_dir().join(format!("elio-item-details-icon-{label}-{unique}"))
}

#[test]
fn item_details_kitty_icon_is_cleared_on_close_and_replaced_after_resize() {
    let root = temp_path("lifecycle");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("notes.txt");
    fs::write(&path, "details").expect("failed to write test item");
    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let entry = crate::filesystem::entry_from_path(path, "notes.txt".to_owned())
        .expect("entry should load");
    app.preview.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.overlays.item_details = Some(app.build_item_details(&entry));
    app.set_screen_regions(ScreenRegions {
        item_details_icon_area: Some(Rect::new(4, 3, 4, 3)),
        ..ScreenRegions::default()
    });

    let placed = app
        .present_item_details_icon(false)
        .expect("item-details icon should render");
    assert!(String::from_utf8_lossy(&placed).contains("a=T"));
    assert!(app.overlays.item_details_icon_displayed.is_some());

    app.invalidate_item_details_icon_geometry();
    assert!(app.stale_item_details_icon_area().is_some());
    let cleared = app
        .clear_stale_item_details_icon()
        .expect("stale Kitty item-details icon should clear");
    assert!(String::from_utf8_lossy(&cleared).contains("a=d"));

    app.set_screen_regions(ScreenRegions {
        item_details_icon_area: Some(Rect::new(8, 5, 4, 3)),
        ..ScreenRegions::default()
    });
    let repainted = app
        .present_item_details_icon(false)
        .expect("item-details icon should redraw after resize");
    assert!(String::from_utf8_lossy(&repainted).contains("a=T"));

    app.overlays.item_details = None;
    let cleared = app
        .clear_stale_item_details_icon()
        .expect("closed item-details icon should clear");
    assert!(String::from_utf8_lossy(&cleared).contains("a=d"));
    assert!(app.overlays.item_details_icon_displayed.is_none());

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn item_details_icon_clear_uses_image_ids_or_cell_overpainting_by_protocol() {
    let kitty =
        crate::terminal_images::clear_item_details_icon(ImageProtocol::KittyGraphics, Some(42))
            .expect("Kitty clear should succeed");
    assert!(String::from_utf8_lossy(&kitty).contains("a=d"));
    assert!(String::from_utf8_lossy(&kitty).contains("i=42"));

    let konsole = crate::terminal_images::clear_item_details_icon(
        ImageProtocol::KittyDirectGraphics,
        Some(42),
    )
    .expect("Konsole clear should succeed");
    assert!(String::from_utf8_lossy(&konsole).contains("a=d"));
    assert!(String::from_utf8_lossy(&konsole).contains("i=42"));

    for protocol in [ImageProtocol::ItermInline, ImageProtocol::Sixel] {
        assert!(
            crate::terminal_images::clear_item_details_icon(protocol, None)
                .expect("raster clear should succeed")
                .is_empty(),
            "{protocol:?} relies on ratatui cell overpainting"
        );
    }
}

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
