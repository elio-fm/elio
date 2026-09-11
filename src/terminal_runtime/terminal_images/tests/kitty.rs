use super::*;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use image::ImageFormat;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-inline-image-{label}-{unique}"))
}

fn write_test_raster_image(path: &Path, format: ImageFormat, width: u32, height: u32) {
    let image =
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([(x % 255) as u8, (y % 255) as u8, 0x80, 0xff])
        }));
    image
        .save_with_format(path, format)
        .expect("test raster image should save");
}

#[test]
fn build_kitty_upload_sequence_uses_unicode_placeholder_mode() {
    let root = temp_root("kitty-upload-sequence");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.pdf-preview.png");
    write_test_raster_image(&path, ImageFormat::Png, 24, 16);
    let payload = fs::read(&path).expect("png payload should exist");
    let id = 42_u32;
    let area = Rect {
        x: 10,
        y: 4,
        width: 30,
        height: 20,
    };

    let sequence = String::from_utf8(
        build_kitty_upload_sequence(&path, id, area).expect("kitty upload sequence should build"),
    )
    .expect("kitty upload sequence should be utf8");

    assert!(sequence.starts_with("\u{1b}_G"));
    assert!(sequence.contains("a=T"));
    assert!(sequence.contains("q=2"));
    assert!(sequence.contains("U=1"));
    assert!(sequence.contains(&format!("i={id}")));
    assert!(sequence.contains("p=1"));
    assert!(sequence.contains("c=30"));
    assert!(sequence.contains("r=20"));
    assert!(sequence.contains("C=1"));
    assert!(sequence.contains("m=0"));
    assert!(!sequence.contains("t=f"));
    assert!(sequence.contains(&BASE64_STANDARD.encode(payload)));
    assert!(sequence.ends_with("\u{1b}\\"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn kitty_placeholder_sequence_sets_panel_background_for_transparency() {
    let sequence = String::from_utf8(build_kitty_placeholder_sequence(
        42,
        Rect {
            x: 1,
            y: 2,
            width: 2,
            height: 2,
        },
        &[],
    ))
    .expect("placeholder sequence should be utf8");

    assert!(sequence.contains("[38;2;"));
    assert!(sequence.contains(";48;2;"));
    assert!(sequence.contains(";58;2;0;0;1m"));
}

#[test]
fn build_kitty_clear_sequence_deletes_visible_images() {
    assert_eq!(build_kitty_clear_sequence(), "\u{1b}_Ga=d,d=A,q=2\u{1b}\\");
}
