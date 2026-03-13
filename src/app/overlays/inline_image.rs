use super::super::*;
use anyhow::{Context, Result};
use base64::Engine as _;
use crossterm::terminal;
use ratatui::layout::Rect;
use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::Command,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct TerminalImageState {
    pub(super) backend: Option<TerminalImageBackend>,
    pub(super) window: Option<TerminalWindowSize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum OverlayPresentState {
    NotRequested,
    Waiting,
    Displayed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum TerminalImageBackend {
    KittyProtocol,
    Kitten,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct TerminalWindowSize {
    pub(super) cells_width: u16,
    pub(super) cells_height: u16,
    pub(super) pixels_width: u32,
    pub(super) pixels_height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct RenderedImageDimensions {
    pub(super) width_px: u32,
    pub(super) height_px: u32,
}

impl App {
    pub(crate) fn enable_terminal_image_previews(&mut self) {
        self.terminal_images.backend = detect_terminal_image_backend();
        self.pdf_preview.pdf_tools_available = pdf_preview_tools_available();
        self.refresh_terminal_image_window_size();
        self.sync_pdf_preview_selection();
    }

    pub(crate) fn handle_terminal_image_resize(&mut self) {
        self.refresh_terminal_image_window_size();
        self.handle_pdf_overlay_resize();
    }

    pub(in crate::app) fn terminal_image_overlay_available(&self) -> bool {
        self.terminal_images.backend.is_some()
    }

    pub(in crate::app) fn cached_terminal_window(&self) -> Option<TerminalWindowSize> {
        self.terminal_images.window
    }

    pub(crate) fn present_preview_overlay(&mut self) -> Result<()> {
        if self.browser_wheel_burst_active() {
            return Ok(());
        }

        let Some(backend) = self.terminal_images.backend else {
            self.clear_preview_overlay()?;
            return Ok(());
        };

        if self.static_image_overlay_displayed() && !self.displayed_static_image_matches_active()
            || self.pdf_overlay_displayed() && !self.displayed_pdf_overlay_matches_active()
        {
            self.clear_preview_overlay()?;
        }

        match self.present_static_image_overlay(backend)? {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(()),
            OverlayPresentState::NotRequested => {}
        }

        match self.present_pdf_overlay(backend)? {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => Ok(()),
            OverlayPresentState::NotRequested => self.clear_preview_overlay(),
        }
    }

    pub(crate) fn clear_preview_overlay(&mut self) -> Result<()> {
        if !self.static_image_overlay_displayed() && !self.pdf_overlay_displayed() {
            return Ok(());
        }

        if let Some(backend) = self.terminal_images.backend {
            clear_terminal_images(backend).context("failed to clear preview overlay")?;
        }
        self.clear_displayed_static_image();
        self.clear_displayed_pdf_overlay();
        Ok(())
    }

    pub(crate) fn preview_uses_image_overlay(&self) -> bool {
        self.displayed_static_image_matches_active() || self.displayed_pdf_overlay_matches_active()
    }

    pub(crate) fn preview_prefers_image_surface(&self) -> bool {
        self.preview_prefers_static_image_surface() || self.preview_prefers_pdf_surface()
    }

    fn refresh_terminal_image_window_size(&mut self) {
        self.terminal_images.window = self
            .terminal_images
            .backend
            .and_then(|_| query_terminal_window_size());
    }
}

fn pdf_preview_tools_available() -> bool {
    command_exists("pdfinfo") && command_exists("pdftocairo")
}

pub(in crate::app) fn detect_terminal_image_backend() -> Option<TerminalImageBackend> {
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

pub(in crate::app) fn select_terminal_image_backend(
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

pub(in crate::app) fn query_terminal_window_size() -> Option<TerminalWindowSize> {
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

pub(in crate::app) fn parse_window_size(output: &str) -> Option<(u32, u32)> {
    let trimmed = output.trim();
    let (width, height) = trimmed.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

pub(in crate::app) fn fallback_window_size_pixels(
    cells_width: u16,
    cells_height: u16,
) -> (u32, u32) {
    (
        u32::from(cells_width.max(1)) * 8,
        u32::from(cells_height.max(1)) * 16,
    )
}

pub(in crate::app) fn read_png_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
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

pub(in crate::app) fn fit_image_pixels(
    area: Rect,
    window_size: TerminalWindowSize,
    aspect_ratio: f32,
) -> (f32, f32) {
    let aspect_ratio = aspect_ratio.max(f32::EPSILON);
    let cell_width_px = window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1));
    let cell_height_px =
        window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1));
    let area_width_px = f32::from(area.width.max(1)) * cell_width_px;
    let area_height_px = f32::from(area.height.max(1)) * cell_height_px;

    if area_width_px / area_height_px > aspect_ratio {
        let height = area_height_px;
        (height * aspect_ratio, height)
    } else {
        let width = area_width_px;
        (width, width / aspect_ratio)
    }
}

pub(in crate::app) fn fit_image_area(
    area: Rect,
    window_size: TerminalWindowSize,
    aspect_ratio: f32,
) -> Rect {
    let cell_width_px = window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1));
    let cell_height_px =
        window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1));
    let (fit_width_px, fit_height_px) = fit_image_pixels(area, window_size, aspect_ratio);
    let width_cells = ((fit_width_px / cell_width_px).round() as u16).clamp(1, area.width.max(1));
    let height_cells =
        ((fit_height_px / cell_height_px).round() as u16).clamp(1, area.height.max(1));

    Rect {
        x: area.x + (area.width.saturating_sub(width_cells)) / 2,
        y: area.y + (area.height.saturating_sub(height_cells)) / 2,
        width: width_cells,
        height: height_cells,
    }
}

pub(in crate::app) fn place_terminal_image(
    backend: TerminalImageBackend,
    path: &Path,
    area: Rect,
) -> Result<()> {
    match backend {
        TerminalImageBackend::Kitten => place_terminal_image_with_kitten(path, area),
        TerminalImageBackend::KittyProtocol => place_terminal_image_with_kitty_protocol(path, area),
    }
}

pub(in crate::app) fn clear_terminal_images(backend: TerminalImageBackend) -> Result<()> {
    match backend {
        TerminalImageBackend::Kitten => clear_terminal_images_with_kitten(),
        TerminalImageBackend::KittyProtocol => clear_terminal_images_with_kitty_protocol(),
    }
}

pub(in crate::app) fn build_kitty_display_sequence(path: &Path, area: Rect) -> String {
    let payload =
        base64::engine::general_purpose::STANDARD.encode(path.as_os_str().as_encoded_bytes());
    format!(
        "\u{1b}[{};{}H\u{1b}_Ga=T,q=2,f=100,t=f,c={},r={},C=1;{}\u{1b}\\",
        area.y.saturating_add(1),
        area.x.saturating_add(1),
        area.width.max(1),
        area.height.max(1),
        payload
    )
}

pub(in crate::app) fn build_kitty_clear_sequence() -> &'static str {
    "\u{1b}_Ga=d,d=A,q=2\u{1b}\\"
}

pub(in crate::app) fn command_exists(program: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {program} >/dev/null 2>&1"))
        .status()
        .is_ok_and(|status| status.success())
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

fn place_terminal_image_with_kitten(path: &Path, area: Rect) -> Result<()> {
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

fn place_terminal_image_with_kitty_protocol(path: &Path, area: Rect) -> Result<()> {
    write_terminal_escape(&build_kitty_display_sequence(path, area))
}

fn clear_terminal_images_with_kitten() -> Result<()> {
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

fn clear_terminal_images_with_kitty_protocol() -> Result<()> {
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
