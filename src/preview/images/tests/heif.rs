use super::*;

#[test]
fn primary_tile_grid_builds_a_filter_from_every_tile() {
    let info = parse_render_info(
        r#"{
            "stream_groups": [{
                "type": "Tile Grid",
                "components": [{
                    "width": 5712, "height": 4284,
                    "subcomponents": [
                        {"stream_index": 0, "tile_horizontal_offset": 0, "tile_vertical_offset": 0},
                        {"stream_index": 1, "tile_horizontal_offset": 640, "tile_vertical_offset": 0},
                        {"stream_index": 2, "tile_horizontal_offset": 0, "tile_vertical_offset": 896},
                        {"stream_index": 3, "tile_horizontal_offset": 640, "tile_vertical_offset": 896}
                    ]
                }],
                "disposition": {"default": 1}
            }],
            "streams": []
        }"#,
    )
    .expect("tile grid should be parsed");

    assert_eq!(
        info.dimensions,
        RenderedImageDimensions {
            width_px: 5712,
            height_px: 4284,
        }
    );
    assert_eq!(
        info.tile_grid_filter("scale=800:600"),
        Some(
            "[0:v:0][0:v:1][0:v:2][0:v:3]xstack=inputs=4:layout=0_0|640_0|0_896|640_896:fill=black,crop=5712:4284:0:0,scale=800:600[out]".to_owned()
        )
    );
}

#[test]
fn non_tiled_heif_uses_its_default_stream_dimensions() {
    let info = parse_render_info(
        r#"{
            "stream_groups": [],
            "streams": [{"width": 4032, "height": 3024, "disposition": {"default": 1}}]
        }"#,
    )
    .expect("default stream should be parsed");

    assert_eq!(
        info.dimensions,
        RenderedImageDimensions {
            width_px: 4032,
            height_px: 3024,
        }
    );
    assert!(info.tile_grid_filter("scale=800:600").is_none());
}
