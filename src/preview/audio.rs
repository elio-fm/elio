use super::{
    PreviewContent, PreviewKind, PreviewVisual, PreviewVisualKind, PreviewVisualLayout,
    appearance as theme,
};
use crate::core::Entry;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, SystemTime},
};

const AUDIO_ARTWORK_CACHE_VERSION: usize = 2;
const NATIVE_ARTWORK_STREAM_INDEX: u32 = u32::MAX;
const CANCELLABLE_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(5);

static COMMAND_CAPTURE_ID: AtomicU64 = AtomicU64::new(0);

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

#[derive(Clone, Debug, Default, PartialEq)]
struct AudioProbeResult {
    metadata: AudioMetadata,
    artwork_stream_index: Option<u32>,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    index: Option<u32>,
    codec_type: Option<String>,
    codec_name: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    bit_rate: Option<String>,
    #[serde(default)]
    disposition: FfprobeDisposition,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Default, Deserialize)]
struct FfprobeDisposition {
    #[serde(default)]
    attached_pic: u8,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    bit_rate: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

pub(super) fn build_audio_preview<F>(
    entry: &Entry,
    type_detail: Option<&'static str>,
    ffprobe_available: bool,
    ffmpeg_available: bool,
    canceled: &F,
) -> PreviewContent
where
    F: Fn() -> bool,
{
    let detail = type_detail.unwrap_or("Audio");
    let (byte_size, modified) = audio_source_identity(entry);

    // Fast path: native in-process reading — no subprocess, completes in < 1ms.
    if let Some(preview) = build_audio_preview_native(
        entry,
        type_detail,
        byte_size,
        modified,
        ffmpeg_available,
        canceled,
    ) {
        return preview;
    }

    // Slow fallback: ffprobe + ffmpeg for formats lofty does not support.
    let probe = if canceled() || !ffprobe_available {
        None
    } else {
        probe_audio_metadata(entry, canceled)
    };
    let lines = render_audio_metadata_lines(probe.as_ref().map(|probe| &probe.metadata), byte_size);
    let mut preview = PreviewContent::new(PreviewKind::Audio, lines).with_detail(detail);
    if canceled() {
        return preview;
    }
    if ffmpeg_available
        && let Some(probe) = probe.as_ref()
        && let Some(stream_index) = probe.artwork_stream_index
        && let Some(visual) =
            extract_audio_artwork(entry, byte_size, modified, stream_index, canceled)
    {
        preview = preview.with_preview_visual(visual);
    }
    preview
}

fn build_audio_preview_native<F>(
    entry: &Entry,
    type_detail: Option<&'static str>,
    byte_size: u64,
    modified: Option<SystemTime>,
    ffmpeg_available: bool,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    use lofty::file::TaggedFileExt;
    use lofty::prelude::*;

    let tagged_file = lofty::read_from_path(&entry.path).ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());
    let props = tagged_file.properties();

    let metadata = AudioMetadata {
        title: tag.and_then(|t| t.title()).map(|s| s.into_owned()),
        artist: tag.and_then(|t| t.artist()).map(|s| s.into_owned()),
        album: tag.and_then(|t| t.album()).map(|s| s.into_owned()),
        track: tag.and_then(|t| t.track()).map(|n| n.to_string()),
        duration_seconds: {
            let d = props.duration().as_secs_f64();
            (d > 0.0).then_some(d)
        },
        codec: codec_from_lofty_file_type(tagged_file.file_type()),
        bitrate_bps: props.audio_bitrate().map(|kbps| kbps as u64 * 1000),
        sample_rate_hz: props.sample_rate(),
        channels: props.channels().map(|c| c as u32),
    };

    let lines = render_audio_metadata_lines(Some(&metadata), byte_size);
    let detail = type_detail.unwrap_or("Audio");
    let mut preview = PreviewContent::new(PreviewKind::Audio, lines).with_detail(detail);

    if !canceled()
        && ffmpeg_available
        && let Some(tag) = tag
    {
        let picture = tag
            .pictures()
            .iter()
            .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
            .or_else(|| tag.pictures().first());
        if let Some(pic) = picture
            && let Some(visual) =
                extract_audio_artwork_native(entry, byte_size, modified, pic.data(), canceled)
        {
            preview = preview.with_preview_visual(visual);
        }
    }

    Some(preview)
}

fn codec_from_lofty_file_type(file_type: lofty::file::FileType) -> Option<String> {
    use lofty::file::FileType;
    Some(
        match file_type {
            FileType::Mpeg => "mp3",
            FileType::Flac => "flac",
            FileType::Mp4 => "aac",
            FileType::Wav => "pcm",
            FileType::Aiff => "pcm",
            FileType::Opus => "opus",
            FileType::Vorbis => "vorbis",
            FileType::Speex => "speex",
            _ => return None,
        }
        .to_string(),
    )
}

fn extract_audio_artwork_native<F>(
    entry: &Entry,
    size: u64,
    modified: Option<SystemTime>,
    picture_data: &[u8],
    canceled: &F,
) -> Option<PreviewVisual>
where
    F: Fn() -> bool,
{
    let ext = picture_data_extension(picture_data);
    let cache_path = audio_artwork_cache_path(
        &entry.path,
        size,
        modified,
        NATIVE_ARTWORK_STREAM_INDEX,
        ext,
    )?;
    if cache_path.exists() {
        return preview_visual_from_path(cache_path);
    }
    if canceled() {
        return None;
    }
    let temp_path = audio_artwork_temp_path(&cache_path)?;
    fs::write(&temp_path, picture_data).ok()?;
    if canceled() {
        let _ = fs::remove_file(&temp_path);
        return None;
    }
    finalize_audio_artwork(&temp_path, &cache_path)?;
    preview_visual_from_path(cache_path)
}

fn picture_data_extension(data: &[u8]) -> &'static str {
    if data.starts_with(b"\xFF\xD8\xFF") {
        "jpg"
    } else if data.starts_with(b"\x89PNG\r\n\x1A\n") {
        "png"
    } else {
        "jpg"
    }
}

fn probe_audio_metadata<F>(entry: &Entry, canceled: &F) -> Option<AudioProbeResult>
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
            "-show_entries",
            "stream=index,codec_type,codec_name,sample_rate,channels,bit_rate:stream_disposition=attached_pic:stream_tags=title,artist,album,track,tracknumber:format=duration,bit_rate:format_tags=title,artist,album,track,tracknumber",
            "-of",
            "json",
        ])
        .arg(&entry.path);
    let output = run_command_capture_stdout_cancellable(command, "audio-ffprobe", canceled)?;
    if canceled() {
        return None;
    }

    parse_ffprobe_metadata(&String::from_utf8_lossy(&output))
}

fn parse_ffprobe_metadata(raw: &str) -> Option<AudioProbeResult> {
    let parsed = serde_json::from_str::<FfprobeOutput>(raw).ok()?;
    let stream = parsed
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("audio"))?;
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
    let artwork_stream_index = parsed
        .streams
        .iter()
        .find(|stream| stream.disposition.attached_pic == 1)
        .and_then(|stream| stream.index);

    Some(AudioProbeResult {
        metadata: AudioMetadata {
            title,
            artist,
            album,
            track,
            duration_seconds,
            codec,
            bitrate_bps,
            sample_rate_hz,
            channels,
        },
        artwork_stream_index,
    })
}

fn extract_audio_artwork<F>(
    entry: &Entry,
    size: u64,
    modified: Option<SystemTime>,
    artwork_stream_index: u32,
    canceled: &F,
) -> Option<PreviewVisual>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let cache_path =
        audio_artwork_cache_path(&entry.path, size, modified, artwork_stream_index, "png")?;
    if cache_path.exists() {
        return preview_visual_from_path(cache_path);
    }

    let temp_path = audio_artwork_temp_path(&cache_path)?;
    if extract_audio_artwork_with_ffmpeg(&entry.path, &temp_path, artwork_stream_index, canceled) {
        if canceled() {
            let _ = fs::remove_file(&temp_path);
            return None;
        }
        finalize_audio_artwork(&temp_path, &cache_path)?;
        return preview_visual_from_path(cache_path);
    }

    let _ = fs::remove_file(temp_path);
    None
}

fn extract_audio_artwork_with_ffmpeg<F>(
    path: &Path,
    output_path: &Path,
    artwork_stream_index: u32,
    canceled: &F,
) -> bool
where
    F: Fn() -> bool,
{
    if canceled() {
        return false;
    }

    let map_arg = format!("0:{artwork_stream_index}");
    let mut command = Command::new("ffmpeg");
    command
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(path)
        .args(["-map", &map_arg, "-frames:v", "1", "-c:v", "png"])
        .arg(output_path);
    let success = run_command_status_cancellable(command, canceled).unwrap_or(false);
    if canceled() {
        let _ = fs::remove_file(output_path);
        return false;
    }
    success
}

fn audio_artwork_cache_path(
    path: &Path,
    size: u64,
    modified: Option<SystemTime>,
    artwork_stream_index: u32,
    extension: &str,
) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    AUDIO_ARTWORK_CACHE_VERSION.hash(&mut hasher);
    path.hash(&mut hasher);
    size.hash(&mut hasher);
    modified.and_then(system_time_key).hash(&mut hasher);
    artwork_stream_index.hash(&mut hasher);
    let cache_dir =
        env::temp_dir().join(format!("elio-audio-cover-v{AUDIO_ARTWORK_CACHE_VERSION}"));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("cover-{:016x}.{extension}", hasher.finish())))
}

fn audio_artwork_temp_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let stem = path.file_stem()?.to_string_lossy();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    Some(parent.join(format!(".{stem}.tmp-{}-{unique}.{ext}", std::process::id())))
}

fn finalize_audio_artwork(temp_path: &Path, cache_path: &Path) -> Option<()> {
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

        match child.try_wait() {
            Ok(Some(status)) => {
                let output = status
                    .success()
                    .then(|| fs::read(&capture_path).ok())
                    .flatten();
                let _ = fs::remove_file(&capture_path);
                return output;
            }
            Ok(None) => thread::sleep(CANCELLABLE_COMMAND_POLL_INTERVAL),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&capture_path);
                return None;
            }
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

        match child.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) => thread::sleep(CANCELLABLE_COMMAND_POLL_INTERVAL),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn command_capture_path(label: &str) -> PathBuf {
    let id = COMMAND_CAPTURE_ID.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!("elio-{label}-{}-{id}.tmp", std::process::id()))
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

fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
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

fn audio_source_identity(entry: &Entry) -> (u64, Option<SystemTime>) {
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
    fn artwork_detection_requires_attached_pic_disposition() {
        let without_attached_pic = parse_ffprobe_metadata(
            r#"{
                "streams": [
                    {
                        "index": 0,
                        "codec_type": "audio",
                        "codec_name": "mp3",
                        "sample_rate": "44100",
                        "channels": 2,
                        "bit_rate": "192000",
                        "tags": {
                            "title": "Signal",
                            "track": "5"
                        }
                    },
                    {
                        "index": 1,
                        "codec_type": "video",
                        "codec_name": "mjpeg",
                        "disposition": {
                            "attached_pic": 0
                        }
                    }
                ],
                "format": {
                    "duration": "123.456",
                    "bit_rate": "256000",
                    "tags": {
                        "artist": "Elio",
                        "album": "Preview Suite"
                    }
                }
            }"#,
        )
        .expect("ffprobe payload should parse");
        let with_attached_pic = parse_ffprobe_metadata(
            r#"{
                "streams": [
                    {
                        "index": 0,
                        "codec_type": "audio",
                        "codec_name": "mp3",
                        "sample_rate": "44100",
                        "channels": 2,
                        "bit_rate": "192000"
                    },
                    {
                        "index": 1,
                        "codec_type": "video",
                        "codec_name": "png",
                        "disposition": {
                            "attached_pic": 1
                        }
                    }
                ],
                "format": {
                    "duration": "123.456",
                    "bit_rate": "256000"
                }
            }"#,
        )
        .expect("ffprobe payload should parse");

        assert_eq!(without_attached_pic.artwork_stream_index, None);
        assert_eq!(
            without_attached_pic.metadata.title.as_deref(),
            Some("Signal")
        );
        assert_eq!(
            without_attached_pic.metadata.artist.as_deref(),
            Some("Elio")
        );
        assert_eq!(
            without_attached_pic.metadata.album.as_deref(),
            Some("Preview Suite")
        );
        assert_eq!(without_attached_pic.metadata.track.as_deref(), Some("5"));
        assert_eq!(with_attached_pic.artwork_stream_index, Some(1));
    }

    #[test]
    fn audio_artwork_cache_path_is_stable_for_same_input_and_changes_with_stream() {
        let modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(123));
        let path = Path::new("/tmp/demo.mp3");
        let current = audio_artwork_cache_path(path, 42, modified, 1, "jpg")
            .expect("cache path should be available");
        let same = audio_artwork_cache_path(path, 42, modified, 1, "jpg")
            .expect("cache path should be available");
        let different = audio_artwork_cache_path(path, 42, modified, 2, "jpg")
            .expect("cache path should be available");

        assert_eq!(current, same);
        assert_ne!(current, different);
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
            run_command_capture_stdout_cancellable(command, "audio-cancel-command", &|| {
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
