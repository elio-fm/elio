use super::*;
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
