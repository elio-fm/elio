use super::*;
use image::ImageFormat;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-inline-image-{label}-{unique}"))
}

fn write_test_png(path: &Path, width: u32, height: u32) {
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x % 255) as u8, (y % 255) as u8, 0x80, 0xff])
    }));
    img.save_with_format(path, ImageFormat::Png)
        .expect("test png should save");
}

fn test_window_size() -> TerminalWindowSize {
    TerminalWindowSize {
        cells_width: 200,
        cells_height: 50,
        pixels_width: 1600,
        pixels_height: 800,
    }
}

fn terminal_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct TerminalEnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl TerminalEnvGuard {
    fn isolate() -> Self {
        const VARS: &[&str] = &[
            "TERM",
            "TERM_PROGRAM",
            "KITTY_WINDOW_ID",
            "WARP_SESSION_ID",
            "ALACRITTY_SOCKET",
            "WT_SESSION",
            "KONSOLE_DBUS_SESSION",
            "KONSOLE_DBUS_SERVICE",
            "KONSOLE_DBUS_WINDOW",
            "TMUX",
            "TMUX_PANE",
        ];

        let saved = VARS
            .iter()
            .map(|&var| (var, std::env::var_os(var)))
            .collect::<Vec<_>>();
        unsafe {
            for &var in VARS {
                std::env::remove_var(var);
            }
        }
        Self { saved }
    }
}

impl Drop for TerminalEnvGuard {
    fn drop(&mut self) {
        unsafe {
            for (var, value) in self.saved.drain(..) {
                match value {
                    Some(value) => std::env::set_var(var, value),
                    None => std::env::remove_var(var),
                }
            }
        }
    }
}

#[test]
fn sixel_sequence_has_dcs_preamble_and_terminator() {
    let _lock = terminal_env_lock();
    let _guard = TerminalEnvGuard::isolate();
    let root = temp_root("sixel-preamble");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.png");
    write_test_png(&path, 24, 16);

    let area = Rect {
        x: 5,
        y: 2,
        width: 20,
        height: 10,
    };
    let output = String::from_utf8(
        place_terminal_image_with_sixel_protocol(&path, area, test_window_size())
            .expect("sixel encoding should succeed"),
    )
    .expect("sixel output should be valid utf8");

    assert!(output.contains("\x1bP"), "missing DCS introducer");
    assert!(output.ends_with("\x1b\\"), "missing String Terminator");
    assert!(output.contains("q"), "missing 'q' Sixel introducer");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sixel_sequence_positions_cursor_at_area_top_left() {
    let _lock = terminal_env_lock();
    let _guard = TerminalEnvGuard::isolate();
    let root = temp_root("sixel-cursor");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.png");
    write_test_png(&path, 24, 16);

    let area = Rect {
        x: 3,
        y: 7,
        width: 20,
        height: 10,
    };
    let output = String::from_utf8(
        place_terminal_image_with_sixel_protocol(&path, area, test_window_size())
            .expect("sixel encoding should succeed"),
    )
    .expect("sixel output should be valid utf8");

    // Image aspect 3:2, area 20×10 cells at 8×16 px/cell (160×160 px).
    // Fit: width-constrained → 160×107 px → 20×7 cells.
    // x offset: (20-20)/2=0 → col 4 (3+1).
    // y offset: (10-7)/2=1 → row 9 (7+1+1).
    assert!(
        output.starts_with("\x1b[9;4H"),
        "cursor positioning missing or wrong"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sixel_sequence_contains_raster_attributes_and_palette() {
    let _lock = terminal_env_lock();
    let _guard = TerminalEnvGuard::isolate();
    let root = temp_root("sixel-raster");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.png");
    write_test_png(&path, 24, 16);

    let area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 5,
    };
    let output = String::from_utf8(
        place_terminal_image_with_sixel_protocol(&path, area, test_window_size())
            .expect("sixel encoding should succeed"),
    )
    .expect("sixel output should be valid utf8");

    // Raster attributes ("1;1;...)
    assert!(output.contains("\"1;1;"), "missing raster attributes");
    // Palette entries (#0;2;...)
    assert!(output.contains("#0;2;"), "missing palette definitions");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn clear_sixel_returns_empty() {
    let bytes = clear_terminal_images_with_sixel_protocol().expect("sixel clear should not fail");
    assert!(bytes.is_empty(), "sixel clear should return empty bytes");
}

#[test]
fn rle_encode_sixel_row_compresses_runs_of_three_or_more() {
    let mut out = Vec::new();
    rle_encode_sixel_row(&mut out, &[0, 0, 0, 0, 32]).expect("rle should succeed");
    let s = String::from_utf8(out).expect("rle output should be utf8");
    // Four '?' → !4?, then one '_'
    assert!(s.starts_with("!4?"), "expected RLE for 4x '?', got: {s}");
    assert!(s.ends_with('_'), "expected trailing '_', got: {s}");
}

#[test]
fn rle_encode_sixel_row_emits_short_runs_verbatim() {
    let mut out = Vec::new();
    rle_encode_sixel_row(&mut out, &[0, 32]).expect("rle should succeed");
    let s = String::from_utf8(out).expect("rle output should be utf8");
    assert_eq!(s, "?_", "two-byte run should be verbatim, got: {s}");
}

#[test]
fn encode_sixel_dcs_returns_dcs_without_cursor_prefix() {
    let root = temp_root("sixel-dcs-no-cursor");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.png");
    write_test_png(&path, 24, 16);

    let dcs = encode_sixel_dcs(&path, 80, 48).expect("encode_sixel_dcs should succeed");
    let s = String::from_utf8(dcs.to_vec()).expect("sixel dcs should be valid utf8");

    assert!(
        s.starts_with("\x1bP"),
        "dcs should start with DCS introducer, not cursor move"
    );
    assert!(s.ends_with("\x1b\\"), "missing String Terminator");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sixel_preserves_256_distinct_colors_in_foot() {
    let _lock = terminal_env_lock();
    let _guard = TerminalEnvGuard::isolate();
    unsafe {
        std::env::set_var("TERM", "foot");
    }
    let img = image::RgbImage::from_fn(16, 16, |x, y| {
        let i = y * 16 + x;
        image::Rgb([
            (i % 8 * 32) as u8,
            (i / 8 % 8 * 32) as u8,
            (i / 64 * 64) as u8,
        ])
    });
    let root = temp_root("sixel-full-palette");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("palette.png");
    img.save(&path).expect("test palette should save");
    let dcs = encode_sixel_dcs(&path, 16, 16).expect("full palette encoding should succeed");
    let output = std::str::from_utf8(&dcs).expect("sixel should be utf8");

    assert_eq!(output.matches(";2;").count(), 256);
    for pixel in img.pixels() {
        let percent = |c: u8| (u32::from(c) * 100 + 127) / 255;
        let color = format!(
            ";2;{};{};{}",
            percent(pixel[0]),
            percent(pixel[1]),
            percent(pixel[2])
        );
        assert!(output.contains(&color), "missing original color {pixel:?}");
    }
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sixel_composites_transparency_over_panel_background() {
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 255, 0]));
    let dcs = encode_sixel_dcs_from_image(img.into(), 1, 1)
        .expect("transparent image encoding should succeed");
    let output = std::str::from_utf8(&dcs).expect("sixel should be utf8");
    let (r, g, b) = panel_background();
    let percent = |c: u8| (u32::from(c) * 100 + 127) / 255;

    assert_eq!(output.matches(";2;").count(), 1);
    assert!(output.contains(&format!(
        "#0;2;{};{};{}",
        percent(r),
        percent(g),
        percent(b)
    )));
    assert!(output.contains("\"1;1;1;1"));
}

#[test]
fn place_sixel_from_dcs_prepends_cursor_move() {
    let dcs = b"\x1bP0;1;0q\"1;1;8;8#0;2;0;0;0\x1b\\";
    let placement = Rect {
        x: 4,
        y: 2,
        width: 1,
        height: 1,
    };
    let out = build_sixel_placement_sequence(dcs, placement);
    let s = String::from_utf8(out).expect("output should be valid utf8");

    assert!(
        s.starts_with("\x1b[3;5H"),
        "expected cursor move to row 3 col 5, got: {s}"
    );
    assert!(s.contains("\x1bP"), "DCS stream should follow cursor move");
}

#[test]
fn build_sixel_tmux_placement_wraps_absolute_cursor_and_dcs() {
    let dcs = b"\x1bP0;1;0qABC\x1b\\";
    let placement = Rect {
        x: 10,
        y: 4,
        width: 3,
        height: 2,
    };
    let origin = TmuxPaneOrigin { top: 2, left: 3 };
    let out = build_sixel_tmux_placement_sequence(dcs, placement, origin);
    let s = String::from_utf8(out).expect("output should be valid utf8");

    assert_eq!(
        s,
        "\x1bPtmux;\x1b\x1b[7;14H\x1b\x1bP0;1;0qABC\x1b\x1b\\\x1b\\"
    );
}

#[test]
fn build_sixel_tmux_native_placement_keeps_pane_local_cursor_and_raw_dcs() {
    let dcs = b"\x1bP0;1;0qABC\x1b\\";
    let placement = Rect {
        x: 10,
        y: 4,
        width: 3,
        height: 2,
    };
    let out = build_sixel_tmux_native_placement_sequence(dcs, placement);
    let s = String::from_utf8(out).expect("output should be valid utf8");

    assert_eq!(s, "\x1b[5;11H\x1bP0;1;0qABC\x1b\\");
}

#[test]
fn rle_encode_sixel_row_trims_trailing_blank_columns() {
    let mut out = Vec::new();

    rle_encode_sixel_row(&mut out, &[0, 0, 1, 1, 0, 0]).expect("row encode should succeed");

    let s = String::from_utf8(out).expect("row encode should be utf8");
    assert_eq!(s, "??@@");
}
