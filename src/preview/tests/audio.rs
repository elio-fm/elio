use super::*;
use std::path::Path;

#[test]
fn supported_audio_extensions_route_to_audio_preview_kind() {
    let root = temp_path("audio-kind");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let cases = [
        ("track.mp3", "MP3 audio"),
        ("track.wav", "WAV audio"),
        ("track.flac", "FLAC audio"),
        ("track.ogg", "Ogg audio"),
        ("track.m4a", "M4A audio"),
    ];

    for (name, detail) in cases {
        let path = root.join(name);
        fs::write(&path, b"not-real-audio").expect("failed to write audio fixture");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Audio);
        assert_eq!(preview.section_label(), "Audio");
        assert_eq!(preview.detail.as_deref(), Some(detail));
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn audio_preview_falls_back_to_file_metadata_without_tools() {
    let root = temp_path("audio-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("track.flac");
    let contents = b"still-not-real-audio";
    fs::write(&path, contents).expect("failed to write audio fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Audio);
    assert_eq!(preview.detail.as_deref(), Some("FLAC audio"));
    assert!(preview.preview_visual.is_none());
    assert!(line_texts.iter().any(|line| line.contains("File Size")
        && line.contains(&crate::app::format_size(contents.len() as u64))));
    assert!(line_texts.iter().all(|line| !line.contains("Title")));
    assert!(
        line_texts
            .iter()
            .all(|line| !line.contains("Binary or unsupported file"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn audio_preview_shows_metadata_with_ffprobe_only() {
    if !audio_tools_available() {
        return;
    }

    let root = temp_path("audio-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("track.mp3");
    if !write_test_audio_fixture(&path) {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        default_code_preview_line_limit(),
        true,
        false,
        &|| false,
    );
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Audio);
    assert_eq!(preview.detail.as_deref(), Some("MP3 audio"));
    assert!(preview.preview_visual.is_none());
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Title") && line.contains("Elio Tune"))
    );
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Artist") && line.contains("Codex"))
    );
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Album") && line.contains("Preview Suite"))
    );
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Track") && line.contains("3"))
    );
    assert!(line_texts.iter().any(|line| line.contains("Duration")));
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Codec") && line.contains("mp3"))
    );
    assert!(line_texts.iter().any(|line| line.contains("Bitrate")));
    assert!(line_texts.iter().any(|line| line.contains("Sample Rate")));
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Channels") && line.contains("mono"))
    );
    assert!(line_texts.iter().any(|line| line.contains("File Size")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn audio_preview_skips_artwork_without_overlay_support_even_when_embedded_art_exists() {
    if !audio_tools_available() {
        return;
    }

    let root = temp_path("audio-no-overlay");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("track.m4a");
    if !write_test_audio_fixture_with_artwork(&path) {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        default_code_preview_line_limit(),
        true,
        false,
        &|| false,
    );
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Audio);
    assert_eq!(preview.detail.as_deref(), Some("M4A audio"));
    assert!(preview.preview_visual.is_none());
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Title") && line.contains("Cover Track"))
    );
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Album") && line.contains("Artwork Suite"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn audio_preview_attaches_inline_cover_when_tools_and_artwork_are_available() {
    if !audio_tools_available() {
        return;
    }

    let root = temp_path("audio-cover");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("track.m4a");
    if !write_test_audio_fixture_with_artwork(&path) {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        default_code_preview_line_limit(),
        true,
        true,
        &|| false,
    );
    let visual = preview
        .preview_visual
        .clone()
        .expect("audio preview should attach artwork");
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Audio);
    assert_eq!(preview.detail.as_deref(), Some("M4A audio"));
    assert_eq!(visual.kind, PreviewVisualKind::Cover);
    assert_eq!(visual.layout, PreviewVisualLayout::Inline);
    assert!(visual.path.exists());
    assert!(visual.size > 0);
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Title") && line.contains("Cover Track"))
    );
    assert!(line_texts.iter().any(|line| line.contains("Codec")));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

fn audio_tools_available() -> bool {
    Command::new("ffprobe").arg("-version").output().is_ok()
        && Command::new("ffmpeg").arg("-version").output().is_ok()
}

fn write_test_audio_fixture(path: &Path) -> bool {
    Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:a",
            "libmp3lame",
            "-metadata",
            "title=Elio Tune",
            "-metadata",
            "artist=Codex",
            "-metadata",
            "album=Preview Suite",
            "-metadata",
            "track=3",
            "-id3v2_version",
            "3",
        ])
        .arg(path)
        .status()
        .is_ok_and(|status| status.success())
}

fn write_test_audio_fixture_with_artwork(path: &Path) -> bool {
    let cover_path = path.with_extension("png");
    write_test_raster_image(&cover_path, ImageFormat::Png, 32, 32);
    let created = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=660:duration=1",
            "-i",
        ])
        .arg(&cover_path)
        .args([
            "-map",
            "0:a",
            "-map",
            "1:v",
            "-c:a",
            "aac",
            "-c:v",
            "mjpeg",
            "-disposition:v",
            "attached_pic",
            "-metadata",
            "title=Cover Track",
            "-metadata",
            "artist=Codex",
            "-metadata",
            "album=Artwork Suite",
            "-metadata",
            "track=7",
        ])
        .arg(path)
        .status()
        .is_ok_and(|status| status.success());
    let _ = fs::remove_file(cover_path);
    created
}
