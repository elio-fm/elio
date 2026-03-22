use super::{PreviewContent, PreviewKind};
use crate::app::Entry;
use crate::ui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use serde::Deserialize;
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq)]
struct VideoMetadata {
    dimensions: Option<(u32, u32)>,
    duration_seconds: Option<f64>,
    codec: Option<String>,
    fps: Option<f64>,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

pub(super) fn build_video_preview(
    entry: &Entry,
    type_detail: Option<&'static str>,
    ffprobe_available: bool,
    _ffmpeg_available: bool,
) -> PreviewContent {
    let detail = type_detail.unwrap_or("Video");
    let byte_size = std::fs::metadata(&entry.path)
        .map(|metadata| metadata.len())
        .unwrap_or(entry.size);
    let metadata = ffprobe_available
        .then(|| probe_video_metadata(entry))
        .flatten()
        .unwrap_or_default();
    let lines = render_video_metadata_lines(&metadata, byte_size);

    PreviewContent::new(PreviewKind::Video, lines).with_detail(detail)
}

fn probe_video_metadata(entry: &Entry) -> Option<VideoMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,avg_frame_rate,r_frame_rate:format=duration",
            "-of",
            "json",
        ])
        .arg(&entry.path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    parse_ffprobe_metadata(&String::from_utf8_lossy(&output.stdout))
}

fn parse_ffprobe_metadata(raw: &str) -> Option<VideoMetadata> {
    let parsed = serde_json::from_str::<FfprobeOutput>(raw).ok()?;
    let stream = parsed.streams.first()?;
    let dimensions = stream.width.zip(stream.height);
    let duration_seconds = parsed
        .format
        .as_ref()
        .and_then(|format| format.duration.as_deref())
        .and_then(parse_duration_seconds);
    let codec = stream.codec_name.as_deref().map(codec_display_name);
    let fps = stream
        .avg_frame_rate
        .as_deref()
        .and_then(parse_frame_rate)
        .or_else(|| stream.r_frame_rate.as_deref().and_then(parse_frame_rate));

    Some(VideoMetadata {
        dimensions,
        duration_seconds,
        codec,
        fps,
    })
}

fn parse_duration_seconds(raw: &str) -> Option<f64> {
    let seconds = raw.parse::<f64>().ok()?;
    seconds.is_finite().then_some(seconds.max(0.0))
}

fn parse_frame_rate(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "0/0" {
        return None;
    }
    if let Some((numerator, denominator)) = trimmed.split_once('/') {
        let numerator = numerator.parse::<f64>().ok()?;
        let denominator = denominator.parse::<f64>().ok()?;
        if denominator == 0.0 {
            return None;
        }
        let rate = numerator / denominator;
        return (rate.is_finite() && rate > 0.0).then_some(rate);
    }

    let rate = trimmed.parse::<f64>().ok()?;
    (rate.is_finite() && rate > 0.0).then_some(rate)
}

fn codec_display_name(raw: &str) -> String {
    raw.replace('_', " ")
}

fn render_video_metadata_lines(metadata: &VideoMetadata, byte_size: u64) -> Vec<Line<'static>> {
    let palette = theme::palette();
    let mut fields = vec![("File Size", crate::app::format_size(byte_size))];
    if let Some((width, height)) = metadata.dimensions {
        fields.insert(0, ("Dimensions", format!("{width}x{height}")));
    }
    if let Some(duration_seconds) = metadata.duration_seconds {
        fields.push(("Duration", format_duration(duration_seconds)));
    }
    if let Some(codec) = metadata.codec.as_deref() {
        fields.push(("Video Codec", codec.to_string()));
    }
    if let Some(fps) = metadata.fps {
        fields.push(("FPS", format_fps(fps)));
    }

    let label_width = fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(8);
    let mut lines = vec![preview_section_line("Video", palette)];
    for (label, value) in fields {
        lines.push(preview_field_line(label, &value, label_width, palette));
    }
    lines
}

fn format_duration(duration_seconds: f64) -> String {
    let rounded = duration_seconds.round().max(0.0) as u64;
    let hours = rounded / 3_600;
    let minutes = (rounded % 3_600) / 60;
    let seconds = rounded % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn format_fps(fps: f64) -> String {
    let rounded = (fps * 100.0).round() / 100.0;
    if (rounded.fract()).abs() < f64::EPSILON {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.2}")
    }
}

fn preview_section_line(title: &str, palette: theme::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn preview_field_line(
    label: &str,
    value: &str,
    label_width: usize,
    palette: theme::Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = label_width + 1),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_string(), Style::default().fg(palette.text)),
    ])
}
