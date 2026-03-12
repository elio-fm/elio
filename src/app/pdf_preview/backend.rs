use super::{RenderedImageDimensions, TerminalImageBackend, TerminalWindowSize};
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use crossterm::terminal;
use ratatui::layout::Rect;
use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::Command,
};

pub(super) fn detect_terminal_pdf_preview_backend() -> Option<TerminalImageBackend> {
    if !command_exists("pdftocairo") {
        return None;
    }

    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();
    let kitten_available = command_exists("kitten");
    let kitten_detected = kitten_available && detect_kitten_backend_support();

    select_terminal_image_backend(
        &term,
        &term_program,
        env::var_os("KITTY_WINDOW_ID").is_some(),
        kitten_available,
        kitten_detected,
    )
}

pub(super) fn select_terminal_image_backend(
    term: &str,
    term_program: &str,
    kitty_window_id_present: bool,
    kitten_available: bool,
    kitten_detected: bool,
) -> Option<TerminalImageBackend> {
    let term = term.to_ascii_lowercase();
    let term_program = term_program.to_ascii_lowercase();
    let supports_kitty_protocol = kitty_window_id_present
        || term.contains("xterm-kitty")
        || term.contains("ghostty")
        || term.contains("wezterm")
        || matches!(term_program.as_str(), "kitty" | "ghostty" | "wezterm");

    if supports_kitty_protocol {
        Some(TerminalImageBackend::KittyProtocol)
    } else if kitten_available && kitten_detected {
        Some(TerminalImageBackend::Kitten)
    } else {
        None
    }
}

pub(super) fn query_terminal_window_size() -> Option<TerminalWindowSize> {
    let terminal_size = terminal::window_size().ok();
    let (cells_width, cells_height) = terminal_size
        .as_ref()
        .map(|size| (size.columns, size.rows))
        .or_else(|| terminal::size().ok())?;
    let (pixels_width, pixels_height) = terminal_size
        .as_ref()
        .and_then(|size| {
            let width = u32::from(size.width);
            let height = u32::from(size.height);
            (width > 0 && height > 0).then_some((width, height))
        })
        .or_else(query_kitten_window_size)
        .unwrap_or_else(|| fallback_window_size_pixels(cells_width, cells_height));
    Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width,
        pixels_height,
    })
}

pub(super) fn parse_window_size(output: &str) -> Option<(u32, u32)> {
    let trimmed = output.trim();
    let (width, height) = trimmed.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

pub(super) fn fallback_window_size_pixels(cells_width: u16, cells_height: u16) -> (u32, u32) {
    (
        u32::from(cells_width.max(1)) * 8,
        u32::from(cells_height.max(1)) * 16,
    )
}

pub(super) fn read_png_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
    let mut file = File::open(path).ok()?;
    let mut header = [0_u8; 24];
    file.read_exact(&mut header).ok()?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return None;
    }

    let width_px = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height_px = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (width_px > 0 && height_px > 0).then_some(RenderedImageDimensions {
        width_px,
        height_px,
    })
}

pub(super) fn place_pdf_image(
    backend: TerminalImageBackend,
    path: &Path,
    area: Rect,
) -> Result<()> {
    match backend {
        TerminalImageBackend::Kitten => place_pdf_image_with_kitten(path, area),
        TerminalImageBackend::KittyProtocol => place_pdf_image_with_kitty_protocol(path, area),
    }
}

pub(super) fn clear_pdf_images(backend: TerminalImageBackend) -> Result<()> {
    match backend {
        TerminalImageBackend::Kitten => clear_pdf_images_with_kitten(),
        TerminalImageBackend::KittyProtocol => clear_pdf_images_with_kitty_protocol(),
    }
}

pub(super) fn build_kitty_display_sequence(path: &Path, area: Rect) -> String {
    let payload = BASE64_STANDARD.encode(path.as_os_str().as_encoded_bytes());
    format!(
        "\u{1b}[{};{}H\u{1b}_Ga=T,q=2,f=100,t=f,c={},r={},C=1;{}\u{1b}\\",
        area.y.saturating_add(1),
        area.x.saturating_add(1),
        area.width.max(1),
        area.height.max(1),
        payload
    )
}

pub(super) fn build_kitty_clear_sequence() -> &'static str {
    "\u{1b}_Ga=d,d=A,q=2\u{1b}\\"
}

fn detect_kitten_backend_support() -> bool {
    Command::new("kitten")
        .arg("icat")
        .arg("--stdin=no")
        .arg("--detect-support")
        .arg("--detection-timeout=1")
        .status()
        .is_ok_and(|status| status.success())
}

fn command_exists(program: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {program} >/dev/null 2>&1"))
        .status()
        .is_ok_and(|status| status.success())
}

fn query_kitten_window_size() -> Option<(u32, u32)> {
    if !command_exists("kitten") {
        return None;
    }

    let output = Command::new("kitten")
        .arg("icat")
        .arg("--print-window-size")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_window_size(&String::from_utf8_lossy(&output.stdout))
}

fn place_pdf_image_with_kitten(path: &Path, area: Rect) -> Result<()> {
    let place = format!(
        "{}x{}@{}x{}",
        area.width.max(1),
        area.height.max(1),
        area.x,
        area.y
    );
    let status = Command::new("kitten")
        .arg("icat")
        .arg("--stdin=no")
        .arg("--transfer-mode=file")
        .arg("--place")
        .arg(place)
        .arg("--scale-up")
        .arg(path)
        .status()
        .context("failed to start kitten icat")?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("kitten icat exited with {status}");
    }
}

fn place_pdf_image_with_kitty_protocol(path: &Path, area: Rect) -> Result<()> {
    write_terminal_escape(&build_kitty_display_sequence(path, area))
}

fn clear_pdf_images_with_kitten() -> Result<()> {
    let status = Command::new("kitten")
        .arg("icat")
        .arg("--stdin=no")
        .arg("--clear")
        .status()
        .context("failed to start kitten icat")?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("kitten icat exited with {status}");
    }
}

fn clear_pdf_images_with_kitty_protocol() -> Result<()> {
    write_terminal_escape(build_kitty_clear_sequence())
}

fn write_terminal_escape(sequence: &str) -> Result<()> {
    let mut stdout = io::stdout();
    stdout
        .write_all(sequence.as_bytes())
        .context("failed to write terminal escape")?;
    stdout.flush().context("failed to flush terminal escape")?;
    Ok(())
}
