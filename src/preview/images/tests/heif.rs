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

#[test]
fn heif_rotation_metadata_maps_to_ffmpeg_filters() {
    let mut info = parse_render_info(r#"{"stream_groups":[],"streams":[{"width":4284,"height":5712,"disposition":{"default":1}}]}"#).expect("default stream should be parsed");
    info.transforms = vec![HeifTransform::Rotate(90)];
    assert_eq!(info.transform_filter().as_deref(), Some("transpose=clock"));
}

#[test]
fn tiled_heif_filter_crops_coded_dimensions_before_rotating() {
    let mut info = parse_render_info(
        r#"{"stream_groups":[{"type":"Tile Grid","components":[{"width":5712,"height":4284,"subcomponents":[{"stream_index":0,"tile_horizontal_offset":0,"tile_vertical_offset":0}]}],"disposition":{"default":1}}],"streams":[]}"#,
    ).expect("tile grid should be parsed");
    info.transforms = vec![HeifTransform::Rotate(90)];
    assert_eq!(
        info.tile_grid_filter("scale=800:600"),
        Some("[0:v:0]xstack=inputs=1:layout=0_0:fill=black,crop=5712:4284:0:0,transpose=clock,scale=800:600[out]".to_owned())
    );
    std::mem::swap(
        &mut info.dimensions.width_px,
        &mut info.dimensions.height_px,
    );
    assert_eq!(
        info.dimensions,
        RenderedImageDimensions {
            width_px: 4284,
            height_px: 5712
        }
    );
    assert_eq!(
        info.coded_dimensions,
        RenderedImageDimensions {
            width_px: 5712,
            height_px: 4284
        }
    );
}

fn bmff_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(8 + payload.len());
    result.extend_from_slice(&((8 + payload.len()) as u32).to_be_bytes());
    result.extend_from_slice(kind);
    result.extend_from_slice(payload);
    result
}

fn heif_meta(properties: &[(&[u8; 4], u8)]) -> Vec<u8> {
    let mut pitm_payload = vec![0, 0, 0, 0, 0, 1]; // v0, primary item id 1
    let pitm = bmff_box(b"pitm", &pitm_payload);
    pitm_payload.clear();
    let mut ipco_payload = Vec::new();
    for (kind, value) in properties {
        ipco_payload.extend(bmff_box(kind, &[*value]));
    }
    let ipco = bmff_box(b"ipco", &ipco_payload);
    let mut ipma_payload = vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 1, properties.len() as u8];
    for index in 1..=properties.len() {
        ipma_payload.push(index as u8);
    }
    let ipma = bmff_box(b"ipma", &ipma_payload);
    let mut iprp_payload = ipco;
    iprp_payload.extend(ipma);
    let iprp = bmff_box(b"iprp", &iprp_payload);
    let mut meta_payload = vec![0, 0, 0, 0];
    meta_payload.extend(pitm);
    meta_payload.extend(iprp);
    bmff_box(b"meta", &meta_payload)[8..].to_vec()
}

#[test]
fn iso_bmff_parser_reads_all_irot_angles() {
    for (encoded, expected) in [(0, 0), (1, 270), (2, 180), (3, 90)] {
        let parsed = parse_primary_item_transform_meta(&heif_meta(&[(b"irot", encoded)])).unwrap();
        assert_eq!(parsed, vec![HeifTransform::Rotate(expected)]);
    }
}

#[test]
fn iso_bmff_parser_reads_mirror_axis_and_ordered_combined_transforms() {
    assert_eq!(
        parse_primary_item_transform_meta(&heif_meta(&[(b"imir", 0)])).unwrap(),
        vec![HeifTransform::Mirror(false)]
    );
    assert_eq!(
        parse_primary_item_transform_meta(&heif_meta(&[(b"imir", 1)])).unwrap(),
        vec![HeifTransform::Mirror(true)]
    );
    let mut mode0 =
        parse_render_info(r#"{"streams":[{"width":20,"height":10,"disposition":{"default":1}}]}"#)
            .unwrap();
    mode0.transforms = vec![HeifTransform::Mirror(false)];
    assert_eq!(mode0.transform_filter().as_deref(), Some("vflip"));
    let mut mode1 = mode0.clone();
    mode1.transforms = vec![HeifTransform::Mirror(true)];
    assert_eq!(mode1.transform_filter().as_deref(), Some("hflip"));
    mode1.transforms = vec![HeifTransform::Rotate(270), HeifTransform::Mirror(true)];
    assert_eq!(
        mode1.transform_filter().as_deref(),
        Some("transpose=cclock,hflip")
    );
    assert_eq!(
        parse_primary_item_transform_meta(&heif_meta(&[(b"irot", 1), (b"imir", 1)])).unwrap(),
        vec![HeifTransform::Rotate(270), HeifTransform::Mirror(true)]
    );
}

#[test]
fn ffprobe_rotation_side_data_and_tag_fallback_are_applied() {
    let raw = r#"{"streams":[{"disposition":{"default":1},"side_data_list":[{"rotation":-90.0}],"tags":{"rotate":"180"}}]}"#;
    assert_eq!(ffprobe_rotation(raw), Some(90));
    let raw_without_side_data =
        r#"{"streams":[{"disposition":{"default":1},"tags":{"rotate":"270"}}]}"#;
    assert_eq!(ffprobe_rotation(raw_without_side_data), Some(270));
}

#[test]
fn iso_bmff_parser_rejects_truncated_and_invalid_metadata() {
    assert!(parse_primary_item_transform_meta(b"\0\0\0\0\0").is_none());
    let mut malformed = heif_meta(&[(b"irot", 0)]);
    malformed[4..8].copy_from_slice(b"free");
    assert!(parse_primary_item_transform_meta(&malformed).is_none());
}
