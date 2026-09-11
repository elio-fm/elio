use super::*;

fn image_prepare_request(
    force_render_to_cache: bool,
    prepare_inline_payload: bool,
) -> ImagePrepareRequest {
    ImagePrepareRequest {
        path: PathBuf::from("demo.gif"),
        size: 1024,
        modified: None,
        target_width_px: 320,
        target_height_px: 180,
        ffmpeg_available: true,
        resvg_available: false,
        magick_available: false,
        force_render_to_cache,
        prepare_inline_payload,
        sixel_prepare: None,
    }
}

#[test]
fn iterm_inline_forced_cache_does_not_use_fast_png_rendering() {
    assert!(!static_image_use_fast_png_render(&image_prepare_request(
        true, true,
    )));
    assert!(static_image_use_fast_png_render(&image_prepare_request(
        true, false,
    )));
    assert!(!static_image_use_fast_png_render(&image_prepare_request(
        false, true,
    )));
}
