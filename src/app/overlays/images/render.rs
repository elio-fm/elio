use super::format::StaticImageFormat;
use crate::app::overlays::inline_image::{RenderedImageDimensions, TerminalWindowSize};
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ratatui::layout::Rect;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn render_svg_to_png_with_resvg(
    input_path: &Path,
    output_path: &Path,
    source_dimensions: RenderedImageDimensions,
    target_width_px: u32,
    target_height_px: u32,
    canceled: &impl Fn() -> bool,
) -> bool {
    if let Some(parent) = output_path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }

    let (width_arg, height_arg) =
        fit_svg_render_dimensions(source_dimensions, target_width_px, target_height_px);
    let mut command = Command::new("resvg");
    if let Some(width_px) = width_arg {
        command.arg("--width").arg(width_px.to_string());
    }
    if let Some(height_px) = height_arg {
        command.arg("--height").arg(height_px.to_string());
    }
    command
        .arg(input_path)
        .arg(output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    run_cancelable_command(&mut command, canceled)
        .is_some_and(|status| status.success() && output_path.exists())
}

pub(super) fn render_svg_to_png_with_magick(
    input_path: &Path,
    output_path: &Path,
    target_width_px: u32,
    target_height_px: u32,
    canceled: &impl Fn() -> bool,
) -> bool {
    if let Some(parent) = output_path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }

    run_cancelable_command(
        Command::new("magick")
            .arg(input_path)
            .arg("-resize")
            .arg(format!(
                "{}x{}>",
                target_width_px.max(1),
                target_height_px.max(1)
            ))
            .arg(output_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
        canceled,
    )
    .is_some_and(|status| status.success() && output_path.exists())
}

fn fit_svg_render_dimensions(
    source_dimensions: RenderedImageDimensions,
    target_width_px: u32,
    target_height_px: u32,
) -> (Option<u32>, Option<u32>) {
    let source_width = source_dimensions.width_px.max(1) as f32;
    let source_height = source_dimensions.height_px.max(1) as f32;
    let scale = (target_width_px.max(1) as f32 / source_width)
        .min(target_height_px.max(1) as f32 / source_height)
        .min(1.0);
    if scale >= 1.0 {
        return (None, None);
    }

    let fitted_width = (source_width * scale).round().max(1.0) as u32;
    let fitted_height = (source_height * scale).round().max(1.0) as u32;
    let width_ratio = target_width_px.max(1) as f32 / source_width;
    let height_ratio = target_height_px.max(1) as f32 / source_height;
    if width_ratio <= height_ratio {
        (Some(fitted_width), None)
    } else {
        (None, Some(fitted_height))
    }
}

pub(super) fn render_raster_to_png_with_ffmpeg(
    input_path: &Path,
    output_path: &Path,
    target_width_px: u32,
    target_height_px: u32,
    force_render_to_cache: bool,
    canceled: &impl Fn() -> bool,
) -> bool {
    if let Some(parent) = output_path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }

    let mut command = Command::new("ffmpeg");
    command
        .arg("-v")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!(
            "scale=w={}:h={}:force_original_aspect_ratio=decrease",
            target_width_px.max(1),
            target_height_px.max(1)
        ));
    command.args(super::ffmpeg_raster_render_args(force_render_to_cache));
    command
        .arg(output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    run_cancelable_command(&mut command, canceled)
        .is_some_and(|status| status.success() && output_path.exists())
}

pub(super) fn apply_raster_orientation(image: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate90().flipv(),
        8 => image.rotate270(),
        _ => image,
    }
}

pub(super) fn shrink_image_to_fit(
    image: DynamicImage,
    target_width_px: u32,
    target_height_px: u32,
) -> DynamicImage {
    let (width_px, height_px) = image.dimensions();
    if width_px <= target_width_px.max(1) && height_px <= target_height_px.max(1) {
        image
    } else {
        image.resize(
            target_width_px.max(1),
            target_height_px.max(1),
            FilterType::Triangle,
        )
    }
}

pub(in crate::app) fn image_target_width_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    let (cell_width_px, _) = image_cell_pixels(window_size);
    (f32::from(area.width.max(1)) * cell_width_px)
        .round()
        .max(1.0) as u32
}

pub(in crate::app) fn image_target_height_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    let (_, cell_height_px) = image_cell_pixels(window_size);
    (f32::from(area.height.max(1)) * cell_height_px)
        .round()
        .max(1.0) as u32
}

fn image_cell_pixels(window_size: Option<TerminalWindowSize>) -> (f32, f32) {
    match window_size {
        Some(window_size) => (
            window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1)),
            window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1)),
        ),
        None => (8.0, 16.0),
    }
}

fn run_cancelable_command<F>(
    command: &mut Command,
    canceled: &F,
) -> Option<std::process::ExitStatus>
where
    F: Fn() -> bool,
{
    let mut child = command.spawn().ok()?;
    loop {
        if canceled() {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        if let Some(status) = child.try_wait().ok()? {
            return Some(status);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub(super) fn should_render_raster_with_ffmpeg(format: StaticImageFormat) -> bool {
    matches!(
        format,
        StaticImageFormat::Jpeg | StaticImageFormat::Gif | StaticImageFormat::Webp
    )
}
