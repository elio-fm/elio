use super::*;

#[test]
fn svg_dimensions_handle_unicode_attributes_and_invalid_utf8() {
    let root = temp_path("svg-xml-encoding");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("sample.svg");
    fs::write(
        &path,
        "<!--日本語--><svg title=\"café &amp; 日本語\" width=\"&#54;40\" height=\"480\"/>",
    )
    .unwrap();
    let dimensions = read_svg_dimensions(&path).unwrap();
    assert_eq!((dimensions.width_px, dimensions.height_px), (640, 480));
    fs::write(&path, b"<!--\xff--><svg width=\"640\" height=\"480\"/>").unwrap();
    assert!(read_svg_dimensions(&path).is_none());
    fs::remove_dir_all(root).unwrap();
}

use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-static-image-format-{label}-{unique}"))
}

#[test]
fn static_image_format_sniffs_collision_suffixed_jpeg_path() {
    let root = temp_path("jpeg-collision-suffix");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("photo.jpeg.2");
    fs::write(&path, [0xff, 0xd8, 0xff, 0xdb]).expect("failed to write jpeg signature");

    assert_eq!(
        static_image_format_for_path(&path),
        Some(StaticImageFormat::Jpeg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
