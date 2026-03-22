use super::PreviewContent;
use crate::app::Entry;
use crate::ui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq)]
struct AudioMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    track: Option<String>,
    duration_seconds: Option<f64>,
    codec: Option<String>,
    bitrate_bps: Option<u64>,
    sample_rate_hz: Option<u32>,
    channels: Option<u32>,
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
    sample_rate: Option<String>,
    channels: Option<u32>,
    bit_rate: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    bit_rate: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

pub(super) fn build_audio_preview(
    entry: &Entry,
    type_detail: Option<&'static str>,
    ffprobe_available: bool,
) -> PreviewContent {
    let detail = type_detail.unwrap_or("Audio");
    let byte_size = audio_source_size(entry);
    let metadata = ffprobe_available
        .then(|| probe_audio_metadata(entry))
        .flatten();
    let lines = render_audio_metadata_lines(metadata.as_ref(), byte_size);

    PreviewContent::new(super::PreviewKind::Audio, lines).with_detail(detail)
}

fn probe_audio_metadata(entry: &Entry) -> Option<AudioMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,sample_rate,channels,bit_rate:stream_tags=title,artist,album,track,tracknumber:format=duration,bit_rate:format_tags=title,artist,album,track,tracknumber",
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

fn parse_ffprobe_metadata(raw: &str) -> Option<AudioMetadata> {
    let parsed = serde_json::from_str::<FfprobeOutput>(raw).ok()?;
    let stream = parsed.streams.first()?;
    let format = parsed.format.as_ref();

    let title = format
        .and_then(|format| tag_value(&format.tags, &["title"]))
        .or_else(|| tag_value(&stream.tags, &["title"]));
    let artist = format
        .and_then(|format| tag_value(&format.tags, &["artist"]))
        .or_else(|| tag_value(&stream.tags, &["artist"]));
    let album = format
        .and_then(|format| tag_value(&format.tags, &["album"]))
        .or_else(|| tag_value(&stream.tags, &["album"]));
    let track = format
        .and_then(|format| tag_value(&format.tags, &["track", "tracknumber"]))
        .or_else(|| tag_value(&stream.tags, &["track", "tracknumber"]));
    let duration_seconds = format
        .and_then(|format| format.duration.as_deref())
        .and_then(parse_duration_seconds);
    let codec = stream.codec_name.as_deref().map(codec_display_name);
    let bitrate_bps = format
        .and_then(|format| format.bit_rate.as_deref())
        .and_then(parse_u64)
        .or_else(|| stream.bit_rate.as_deref().and_then(parse_u64));
    let sample_rate_hz = stream.sample_rate.as_deref().and_then(parse_u32);
    let channels = stream.channels;

    Some(AudioMetadata {
        title,
        artist,
        album,
        track,
        duration_seconds,
        codec,
        bitrate_bps,
        sample_rate_hz,
        channels,
    })
}

fn tag_value(tags: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some((_, value)) = tags.iter().find(|(candidate, value)| {
            candidate.eq_ignore_ascii_case(key) && !value.trim().is_empty()
        }) {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn parse_duration_seconds(raw: &str) -> Option<f64> {
    let seconds = raw.parse::<f64>().ok()?;
    seconds.is_finite().then_some(seconds.max(0.0))
}

fn parse_u64(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

fn parse_u32(raw: &str) -> Option<u32> {
    raw.trim().parse::<u32>().ok()
}

fn codec_display_name(raw: &str) -> String {
    raw.replace('_', " ")
}

fn audio_source_size(entry: &Entry) -> u64 {
    std::fs::metadata(&entry.path)
        .map(|metadata| metadata.len())
        .unwrap_or(entry.size)
}

fn render_audio_metadata_lines(
    metadata: Option<&AudioMetadata>,
    byte_size: u64,
) -> Vec<Line<'static>> {
    let palette = theme::palette();
    let mut fields = Vec::new();
    if let Some(title) = metadata.and_then(|metadata| metadata.title.as_deref()) {
        fields.push(("Title", title.to_string()));
    }
    if let Some(artist) = metadata.and_then(|metadata| metadata.artist.as_deref()) {
        fields.push(("Artist", artist.to_string()));
    }
    if let Some(album) = metadata.and_then(|metadata| metadata.album.as_deref()) {
        fields.push(("Album", album.to_string()));
    }
    if let Some(track) = metadata.and_then(|metadata| metadata.track.as_deref()) {
        fields.push(("Track", track.to_string()));
    }
    if let Some(duration_seconds) = metadata.and_then(|metadata| metadata.duration_seconds) {
        fields.push(("Duration", format_duration(duration_seconds)));
    }
    if let Some(codec) = metadata.and_then(|metadata| metadata.codec.as_deref()) {
        fields.push(("Codec", codec.to_string()));
    }
    if let Some(bitrate_bps) = metadata.and_then(|metadata| metadata.bitrate_bps) {
        fields.push(("Bitrate", format_bitrate(bitrate_bps)));
    }
    if let Some(sample_rate_hz) = metadata.and_then(|metadata| metadata.sample_rate_hz) {
        fields.push(("Sample Rate", format_sample_rate(sample_rate_hz)));
    }
    if let Some(channels) = metadata.and_then(|metadata| metadata.channels) {
        fields.push(("Channels", format_channels(channels)));
    }
    fields.push(("File Size", crate::app::format_size(byte_size)));

    let label_width = fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(8);
    let mut lines = vec![preview_section_line("Audio", palette)];
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

fn format_bitrate(bits_per_second: u64) -> String {
    if bits_per_second >= 1_000_000 {
        format_decimal(bits_per_second as f64 / 1_000_000.0, "Mbps")
    } else if bits_per_second >= 1_000 {
        format_decimal(bits_per_second as f64 / 1_000.0, "kbps")
    } else {
        format!("{bits_per_second} bps")
    }
}

fn format_sample_rate(sample_rate_hz: u32) -> String {
    if sample_rate_hz >= 1_000 {
        format_decimal(sample_rate_hz as f64 / 1_000.0, "kHz")
    } else {
        format!("{sample_rate_hz} Hz")
    }
}

fn format_channels(channels: u32) -> String {
    match channels {
        1 => "1 (mono)".to_string(),
        2 => "2 (stereo)".to_string(),
        count => count.to_string(),
    }
}

fn format_decimal(value: f64, suffix: &str) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded.fract()).abs() < f64::EPSILON {
        format!("{rounded:.0} {suffix}")
    } else {
        format!("{rounded:.1} {suffix}")
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
