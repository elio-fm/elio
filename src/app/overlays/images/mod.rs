mod cache;
mod format;
mod preload;
mod prepare;
mod present;
mod render;
mod state;
mod types;

use self::format::read_raster_dimensions;
use super::super::*;
use super::inline_image::TerminalWindowSize;
use super::inline_image::read_png_dimensions;
use ratatui::layout::Rect;

pub(crate) use self::prepare::prepare_static_image_asset;
pub(in crate::app) use self::types::{
    ImagePreviewState, PreparedStaticImage, PreparedStaticImageAsset, StaticImageKey,
    StaticImageOverlayMode, StaticImageOverlayPreparation, StaticImageOverlayRequest,
};

const STATIC_IMAGE_RENDER_CACHE_LIMIT: usize = 24;
const STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT: usize = 10;
const STATIC_IMAGE_PRELOAD_LIMIT: usize = 6;
const STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES: u64 = 512 * 1024;
const STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const STATIC_IMAGE_RENDER_CACHE_VERSION: usize = 3;
const FAST_FORCE_RENDER_FFMPEG_RASTER_ARGS: [&str; 4] =
    ["-compression_level", "1", "-sws_flags", "fast_bilinear"];
const DEFAULT_FFMPEG_RASTER_ARGS: [&str; 0] = [];

pub(in crate::app) fn static_image_detail_label(entry: &Entry) -> Option<&'static str> {
    format::static_image_detail_label(entry)
}

pub(in crate::app) fn ffmpeg_raster_render_args(
    force_render_to_cache: bool,
) -> &'static [&'static str] {
    if force_render_to_cache {
        &FAST_FORCE_RENDER_FFMPEG_RASTER_ARGS
    } else {
        &DEFAULT_FFMPEG_RASTER_ARGS
    }
}

pub(in crate::app) fn image_target_width_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    render::image_target_width_px(area, window_size)
}

pub(in crate::app) fn image_target_height_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    render::image_target_height_px(area, window_size)
}

impl App {
    pub(in crate::app) fn prepared_static_image_for_overlay(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> StaticImageOverlayPreparation {
        let key = StaticImageKey::from_request(request);
        if let Some(prepared) = self.cached_prepared_static_image_for_overlay(&key, request) {
            return StaticImageOverlayPreparation::Ready(prepared);
        }
        if let Some(prepared) = self.direct_static_image_for_overlay(request) {
            return StaticImageOverlayPreparation::Ready(prepared);
        }
        if self.image_preview.pending_prepares.contains(&key) {
            return StaticImageOverlayPreparation::Pending;
        }
        if self.image_preview.failed_images.contains(&key) {
            StaticImageOverlayPreparation::Failed
        } else {
            StaticImageOverlayPreparation::Pending
        }
    }

    fn direct_static_image_for_overlay(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> Option<PreparedStaticImage> {
        if !self.static_image_can_display_directly_now(request) {
            return None;
        }

        let key = StaticImageKey::from_request(request);
        self.image_preview.failed_images.remove(&key);
        let dimensions = self
            .image_preview
            .dimensions
            .get(&key)
            .copied()
            .or_else(|| read_png_dimensions(&request.path))
            .or_else(|| read_raster_dimensions(&request.path))?;
        self.image_preview.dimensions.insert(key, dimensions);

        Some(PreparedStaticImage {
            display_path: request.path.clone(),
            dimensions,
            inline_payload: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::overlays::inline_image::{
        ImageProtocol, OverlayPresentState, RenderedImageDimensions, TerminalWindowSize,
        build_kitty_clear_sequence,
    };
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use ratatui::layout::Rect;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::Arc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-static-image-{label}-{unique}"))
    }

    fn configure_terminal_image_support(app: &mut App) {
        let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
        app.terminal_images.protocol = ImageProtocol::KittyGraphics;
        app.terminal_images.window = Some(TerminalWindowSize {
            cells_width,
            cells_height,
            pixels_width: 1920,
            pixels_height: 1080,
        });
    }

    fn write_test_raster_image(path: &Path, format: ImageFormat, width_px: u32, height_px: u32) {
        let mut image = RgbaImage::new(width_px, height_px);
        for pixel in image.pixels_mut() {
            *pixel = Rgba([32, 128, 224, 255]);
        }

        DynamicImage::ImageRgba8(image)
            .save_with_format(path, format)
            .expect("failed to write raster test image");
    }

    fn set_single_test_entry(app: &mut App, path: &Path) {
        let metadata = fs::metadata(path).expect("file metadata should exist");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("file name should be valid utf-8");
        app.entries = vec![Entry {
            path: path.to_path_buf(),
            name: name.to_string(),
            name_key: name.to_ascii_lowercase(),
            kind: EntryKind::File,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            readonly: false,
        }];
        app.selected = 0;
        app.frame_state.preview_content_area = Some(Rect {
            x: 2,
            y: 3,
            width: 48,
            height: 20,
        });
        app.frame_state.metrics.cols = 1;
        app.frame_state.metrics.rows_visible = 6;
    }

    fn build_selected_static_image_app(label: &str, file_name: &str) -> (App, PathBuf, PathBuf) {
        let root = temp_root(label);
        fs::create_dir_all(&root).expect("failed to create temp root");
        let image_path = root.join(file_name);
        write_test_raster_image(&image_path, ImageFormat::Png, 600, 300);

        let mut app = App::new_at(root.clone()).expect("app should initialize");
        configure_terminal_image_support(&mut app);
        app.pdf_preview.pdf_tools_available = true;
        set_single_test_entry(&mut app, &image_path);
        app.refresh_preview();

        (app, root, image_path)
    }

    fn ready_static_image_overlay(app: &mut App) -> StaticImageOverlayRequest {
        app.image_preview.selection_activation_delay = Duration::ZERO;
        app.sync_image_preview_selection_activation();
        app.active_static_image_overlay_request()
            .expect("static image overlay request should exist")
    }

    #[test]
    fn kitty_png_overlay_uses_source_path_for_direct_display() {
        let (mut app, root, image_path) =
            build_selected_static_image_app("direct-source", "demo.png");
        let request = ready_static_image_overlay(&mut app);
        let key = StaticImageKey::from_request(&request);

        match app.prepared_static_image_for_overlay(&request) {
            StaticImageOverlayPreparation::Ready(prepared) => {
                assert_eq!(prepared.display_path, image_path);
                assert_eq!(
                    prepared.dimensions,
                    RenderedImageDimensions {
                        width_px: 600,
                        height_px: 300,
                    }
                );
                assert!(prepared.inline_payload.is_none());
            }
            StaticImageOverlayPreparation::Pending => {
                panic!("png source path should display directly in kitty")
            }
            StaticImageOverlayPreparation::Failed => {
                panic!("png source path should not fail direct display")
            }
        }

        assert!(app.image_preview.dimensions.contains_key(&key));
        assert!(!app.image_preview.pending_prepares.contains(&key));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn cached_rendered_overlay_reuses_cached_path_and_inline_payload() {
        let (mut app, root, image_path) =
            build_selected_static_image_app("cache-reuse", "demo.png");
        let mut request = ready_static_image_overlay(&mut app);
        request.force_render_to_cache = true;
        request.prepare_inline_payload = true;
        let key = StaticImageKey::from_request(&request);
        let rendered_path = root.join("demo-rendered.png");
        write_test_raster_image(&rendered_path, ImageFormat::Png, 320, 180);
        let payload: Arc<str> = Arc::from("YWJj");

        app.image_preview.dimensions.insert(
            key.clone(),
            RenderedImageDimensions {
                width_px: 320,
                height_px: 180,
            },
        );
        app.remember_rendered_static_image(key.clone(), rendered_path.clone());
        app.remember_static_image_inline_payload(key.clone(), Arc::clone(&payload));

        let prepared = app
            .cached_prepared_static_image_for_overlay(&key, &request)
            .expect("cached rendered overlay should be reused");

        assert_eq!(prepared.display_path, rendered_path);
        assert_eq!(
            prepared.dimensions,
            RenderedImageDimensions {
                width_px: 320,
                height_px: 180,
            }
        );
        assert_eq!(prepared.inline_payload.as_deref(), Some(payload.as_ref()));
        assert_ne!(prepared.display_path, image_path);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn repeated_present_static_image_overlay_is_a_noop_when_nothing_changed() {
        let (mut app, root, _image_path) =
            build_selected_static_image_app("no-op-render", "demo.png");
        app.image_preview.selection_activation_delay = Duration::ZERO;
        app.sync_image_preview_selection_activation();

        let mut first = Vec::new();
        let first_state = app
            .present_static_image_overlay(ImageProtocol::KittyGraphics, &[], &mut first)
            .expect("first static image presentation should succeed");
        assert_eq!(first_state, OverlayPresentState::Displayed);
        assert!(!first.is_empty());
        assert!(app.static_image_overlay_displayed());

        let mut second = Vec::new();
        let second_state = app
            .present_static_image_overlay(ImageProtocol::KittyGraphics, &[], &mut second)
            .expect("repeat static image presentation should succeed");
        assert_eq!(second_state, OverlayPresentState::Displayed);
        assert!(second.is_empty(), "unchanged image should not redraw");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn exclusion_only_updates_redraw_without_clearing_the_existing_image() {
        let (mut app, root, _image_path) =
            build_selected_static_image_app("excluded-redraw", "demo.png");
        app.image_preview.selection_activation_delay = Duration::ZERO;
        app.sync_image_preview_selection_activation();

        let mut initial = Vec::new();
        app.present_static_image_overlay(ImageProtocol::KittyGraphics, &[], &mut initial)
            .expect("initial static image presentation should succeed");

        let excluded = [Rect {
            x: 4,
            y: 5,
            width: 6,
            height: 3,
        }];
        let mut updated = Vec::new();
        let state = app
            .present_static_image_overlay(ImageProtocol::KittyGraphics, &excluded, &mut updated)
            .expect("excluded-only redraw should succeed");
        let output = String::from_utf8(updated).expect("kitty redraw should be utf8");

        assert_eq!(state, OverlayPresentState::Displayed);
        assert!(
            !output.is_empty(),
            "changed exclusions should trigger a redraw"
        );
        assert!(
            !output.contains(build_kitty_clear_sequence()),
            "exclusion-only redraw should not clear the previous image first"
        );
        assert_eq!(app.image_preview.displayed_excluded, excluded);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
