use super::super::*;
pub(super) use crate::preview::PreviewKind;
pub(super) use crate::terminal_images::{ImageProtocol, TerminalWindowSize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-pdf-preview-{label}-{unique}"))
}

pub(super) fn configure_terminal_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.preview.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

pub(super) fn configure_iterm_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.preview.terminal_images.protocol = ImageProtocol::ItermInline;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

pub(super) fn build_pdf_overlay_test_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    app.preview.pdf.session = Some(PdfSession {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        current_page: 1,
        total_pages: None,
    });
    app.input.screen_regions.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview.pdf.activation_ready_at = Some(Instant::now());
    (app, root)
}

pub(super) fn build_selected_pdf_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let pdf_path = root.join("demo.pdf");
    fs::write(&pdf_path, b"%PDF-1.7\n").expect("failed to write pdf placeholder");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    app.input.screen_regions.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.refresh_preview();
    (app, root)
}

pub(super) fn write_test_png(path: &Path, width_px: u32, height_px: u32) {
    let mut bytes = vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R',
    ];
    bytes.extend_from_slice(&width_px.to_be_bytes());
    bytes.extend_from_slice(&height_px.to_be_bytes());
    fs::write(path, bytes).expect("failed to write png header");
}
