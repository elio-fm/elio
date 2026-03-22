use super::{PreviewContent, PreviewKind, PreviewVisual, PreviewVisualKind, PreviewVisualLayout};
use crate::app::Entry;
use crate::ui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use serde::Deserialize;
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, SystemTime},
};

const VIDEO_THUMBNAIL_CACHE_VERSION: usize = 1;
const VIDEO_FALLBACK_THUMBNAIL_TIMESTAMP_MS: [u64; 2] = [1_000, 0];
const CANCELLABLE_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(5);

static COMMAND_CAPTURE_ID: AtomicU64 = AtomicU64::new(0);

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

pub(super) fn build_video_preview<F>(
    entry: &Entry,
    type_detail: Option<&'static str>,
    ffprobe_available: bool,
    ffmpeg_available: bool,
    canceled: &F,
) -> PreviewContent
where
    F: Fn() -> bool,
{
    let detail = type_detail.unwrap_or("Video");
    let (byte_size, modified) = video_source_identity(entry);
    let metadata = if canceled() || !ffprobe_available {
        None
    } else {
        probe_video_metadata(entry, canceled)
    };
    let lines = render_video_metadata_lines(metadata.as_ref(), byte_size);
    let mut preview = PreviewContent::new(PreviewKind::Video, lines).with_detail(detail);
    if canceled() {
        return preview;
    }
    if ffmpeg_available
        && let Some(metadata) = metadata.as_ref()
        && let Some(visual) =
            extract_video_thumbnail(entry, byte_size, modified, metadata, canceled)
    {
        preview = preview.with_preview_visual(visual);
    }
    preview
}

fn probe_video_metadata<F>(entry: &Entry, canceled: &F) -> Option<VideoMetadata>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let mut command = Command::new("ffprobe");
    command
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
        .arg(&entry.path);
    let output = run_command_capture_stdout_cancellable(command, "video-ffprobe", canceled)?;
    if canceled() {
        return None;
    }

    parse_ffprobe_metadata(&String::from_utf8_lossy(&output))
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

fn render_video_metadata_lines(
    metadata: Option<&VideoMetadata>,
    byte_size: u64,
) -> Vec<Line<'static>> {
    let palette = theme::palette();
    let mut fields = vec![("File Size", crate::app::format_size(byte_size))];
    if let Some((width, height)) = metadata.and_then(|metadata| metadata.dimensions) {
        fields.insert(0, ("Dimensions", format!("{width}x{height}")));
    }
    if let Some(duration_seconds) = metadata.and_then(|metadata| metadata.duration_seconds) {
        fields.push(("Duration", format_duration(duration_seconds)));
    }
    if let Some(codec) = metadata.and_then(|metadata| metadata.codec.as_deref()) {
        fields.push(("Video Codec", codec.to_string()));
    }
    if let Some(fps) = metadata.and_then(|metadata| metadata.fps) {
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

fn extract_video_thumbnail<F>(
    entry: &Entry,
    size: u64,
    modified: Option<SystemTime>,
    metadata: &VideoMetadata,
    canceled: &F,
) -> Option<PreviewVisual>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    for timestamp_ms in thumbnail_candidate_timestamps_ms(metadata.duration_seconds) {
        if canceled() {
            return None;
        }
        if let Some(visual) =
            extract_video_thumbnail_at(entry, size, modified, timestamp_ms, canceled)
        {
            return Some(visual);
        }
    }
    None
}

fn extract_video_thumbnail_at<F>(
    entry: &Entry,
    size: u64,
    modified: Option<SystemTime>,
    timestamp_ms: u64,
    canceled: &F,
) -> Option<PreviewVisual>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let cache_path = video_thumbnail_cache_path(&entry.path, size, modified, timestamp_ms)?;
    if cache_path.exists() {
        return preview_visual_from_path(cache_path);
    }

    let temp_path = video_thumbnail_temp_path(&cache_path)?;
    if render_video_thumbnail_with_ffmpeg(&entry.path, &temp_path, timestamp_ms, canceled) {
        if canceled() {
            let _ = fs::remove_file(&temp_path);
            return None;
        }
        finalize_video_thumbnail(&temp_path, &cache_path)?;
        return preview_visual_from_path(cache_path);
    }

    let _ = fs::remove_file(temp_path);
    None
}

fn thumbnail_candidate_timestamps_ms(duration_seconds: Option<f64>) -> Vec<u64> {
    let mut timestamps = Vec::new();
    if let Some(duration_seconds) = duration_seconds {
        timestamps.push(clamp_thumbnail_timestamp_ms(duration_seconds));
    }
    for fallback in VIDEO_FALLBACK_THUMBNAIL_TIMESTAMP_MS {
        if !timestamps.contains(&fallback) {
            timestamps.push(fallback);
        }
    }
    timestamps
}

fn clamp_thumbnail_timestamp_ms(duration_seconds: f64) -> u64 {
    ((duration_seconds * 0.1).clamp(1.0, 30.0) * 1000.0).round() as u64
}

fn video_source_identity(entry: &Entry) -> (u64, Option<SystemTime>) {
    let metadata = fs::metadata(&entry.path).ok();
    let size = metadata
        .as_ref()
        .map(|metadata| metadata.len())
        .unwrap_or(entry.size);
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .or(entry.modified);
    (size, modified)
}

fn render_video_thumbnail_with_ffmpeg<F>(
    path: &Path,
    output_path: &Path,
    timestamp_ms: u64,
    canceled: &F,
) -> bool
where
    F: Fn() -> bool,
{
    if canceled() {
        return false;
    }

    let timestamp_arg = format_timestamp_arg(timestamp_ms);
    let mut command = Command::new("ffmpeg");
    command
        .args(["-loglevel", "error", "-y", "-ss", &timestamp_arg, "-i"])
        .arg(path)
        .args(["-frames:v", "1"])
        .arg(output_path);
    let success = run_command_status_cancellable(command, canceled).unwrap_or(false);
    if canceled() {
        let _ = fs::remove_file(output_path);
        return false;
    }
    success
}

fn format_timestamp_arg(timestamp_ms: u64) -> String {
    format!("{:.3}", timestamp_ms as f64 / 1000.0)
}

fn preview_visual_from_path(path: PathBuf) -> Option<PreviewVisual> {
    let metadata = fs::metadata(&path).ok()?;
    Some(PreviewVisual {
        kind: PreviewVisualKind::Cover,
        layout: PreviewVisualLayout::Inline,
        path,
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn video_thumbnail_cache_path(
    path: &Path,
    size: u64,
    modified: Option<SystemTime>,
    timestamp_ms: u64,
) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    VIDEO_THUMBNAIL_CACHE_VERSION.hash(&mut hasher);
    path.hash(&mut hasher);
    size.hash(&mut hasher);
    modified.and_then(system_time_key).hash(&mut hasher);
    timestamp_ms.hash(&mut hasher);
    let cache_dir =
        env::temp_dir().join(format!("elio-video-thumb-v{VIDEO_THUMBNAIL_CACHE_VERSION}"));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("thumb-{:016x}.png", hasher.finish())))
}

fn video_thumbnail_temp_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let stem = path.file_stem()?.to_string_lossy();
    Some(parent.join(format!(".{stem}.tmp-{}-{unique}.png", std::process::id())))
}

fn finalize_video_thumbnail(temp_path: &Path, cache_path: &Path) -> Option<()> {
    match fs::rename(temp_path, cache_path) {
        Ok(()) => Some(()),
        Err(_) if cache_path.exists() => {
            let _ = fs::remove_file(temp_path);
            Some(())
        }
        Err(_) => {
            let _ = fs::remove_file(temp_path);
            None
        }
    }
}

fn run_command_capture_stdout_cancellable<F>(
    mut command: Command,
    capture_label: &str,
    canceled: &F,
) -> Option<Vec<u8>>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let capture_path = command_capture_path(capture_label);
    let stdout = fs::File::create(&capture_path).ok()?;
    let mut child = match command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            let _ = fs::remove_file(&capture_path);
            return None;
        }
    };

    loop {
        if canceled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&capture_path);
            return None;
        }

        match child.try_wait().ok()? {
            Some(status) => {
                let output = status
                    .success()
                    .then(|| fs::read(&capture_path).ok())
                    .flatten();
                let _ = fs::remove_file(&capture_path);
                return output;
            }
            None => thread::sleep(CANCELLABLE_COMMAND_POLL_INTERVAL),
        }
    }
}

fn run_command_status_cancellable<F>(mut command: Command, canceled: &F) -> Option<bool>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    loop {
        if canceled() {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }

        match child.try_wait().ok()? {
            Some(status) => return Some(status.success()),
            None => thread::sleep(CANCELLABLE_COMMAND_POLL_INTERVAL),
        }
    }
}

fn command_capture_path(label: &str) -> PathBuf {
    let id = COMMAND_CAPTURE_ID.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!("elio-{label}-{}-{id}.tmp", std::process::id()))
}

fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };

    #[test]
    fn thumbnail_timestamp_clamps_to_supported_range() {
        assert_eq!(clamp_thumbnail_timestamp_ms(5.0), 1_000);
        assert_eq!(clamp_thumbnail_timestamp_ms(120.0), 12_000);
        assert_eq!(clamp_thumbnail_timestamp_ms(600.0), 30_000);
    }

    #[test]
    fn parse_ffprobe_metadata_extracts_dimensions_duration_codec_and_fps() {
        let metadata = parse_ffprobe_metadata(
            r#"{
                "streams": [{
                    "codec_name": "h264",
                    "width": 1920,
                    "height": 1080,
                    "avg_frame_rate": "24000/1001",
                    "r_frame_rate": "24000/1001"
                }],
                "format": {
                    "duration": "123.456"
                }
            }"#,
        )
        .expect("ffprobe payload should parse");

        assert_eq!(metadata.dimensions, Some((1920, 1080)));
        assert_eq!(metadata.codec.as_deref(), Some("h264"));
        assert_eq!(metadata.duration_seconds, Some(123.456));
        assert!(
            metadata
                .fps
                .is_some_and(|fps| (fps - 23.976_023_976).abs() < 0.001)
        );
    }

    #[test]
    fn video_thumbnail_cache_path_is_stable_for_same_input_and_changes_with_timestamp() {
        let modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(123));
        let path = Path::new("/tmp/demo.mp4");
        let current = video_thumbnail_cache_path(path, 42, modified, 1_000)
            .expect("cache path should be available");
        let same = video_thumbnail_cache_path(path, 42, modified, 1_000)
            .expect("cache path should be available");
        let different = video_thumbnail_cache_path(path, 42, modified, 12_000)
            .expect("cache path should be available");

        assert_eq!(current, same);
        assert_ne!(current, different);
    }

    #[test]
    fn thumbnail_candidate_timestamps_include_clamped_target_and_fallbacks() {
        assert_eq!(thumbnail_candidate_timestamps_ms(None), vec![1_000, 0]);
        assert_eq!(
            thumbnail_candidate_timestamps_ms(Some(120.0)),
            vec![12_000, 1_000, 0]
        );
    }

    #[test]
    fn cancellable_command_helper_stops_long_running_process_promptly() {
        let canceled = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::clone(&canceled);
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            cancel_flag.store(true, Ordering::Relaxed);
        });

        let mut command = Command::new("bash");
        command.arg("-lc").arg("sleep 1; printf late");
        let started_at = Instant::now();
        let output =
            run_command_capture_stdout_cancellable(command, "video-cancel-command", &|| {
                canceled.load(Ordering::Relaxed)
            });
        cancel_thread
            .join()
            .expect("cancel thread should finish cleanly");

        assert!(
            output.is_none(),
            "canceled command output should be discarded"
        );
        assert!(
            started_at.elapsed() < Duration::from_millis(500),
            "canceled command should stop promptly"
        );
    }
}
