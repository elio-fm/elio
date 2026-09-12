use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn read_png_dimensions_reads_ihdr_size() {
    let root = temp_root("png-dimensions");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("page.png");
    write_test_png(&path, 600, 300);

    assert_eq!(
        read_png_dimensions(&path),
        Some(RenderedImageDimensions {
            width_px: 600,
            height_px: 300,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn fit_image_area_preserves_actual_rendered_png_aspect_ratio() {
    let area = fit_image_area(
        Rect {
            x: 10,
            y: 4,
            width: 30,
            height: 20,
        },
        TerminalWindowSize {
            cells_width: 100,
            cells_height: 50,
            pixels_width: 1000,
            pixels_height: 1000,
        },
        0.25,
    );

    assert_eq!(area.width, 10);
    assert_eq!(area.height, 20);
    assert_eq!(area.x, 20);
    assert_eq!(area.y, 4);
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-terminal-image-{label}-{unique}"))
}

fn write_test_png(path: &std::path::Path, width_px: u32, height_px: u32) {
    let mut bytes = vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R',
    ];
    bytes.extend_from_slice(&width_px.to_be_bytes());
    bytes.extend_from_slice(&height_px.to_be_bytes());
    fs::write(path, bytes).expect("failed to write png header");
}
