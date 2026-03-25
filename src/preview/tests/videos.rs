use super::*;

#[test]
fn supported_video_extensions_route_to_video_preview_kind() {
    let root = temp_path("video-kind");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("clip.mp4");
    fs::write(&path, b"not-a-real-video").expect("failed to write video fixture");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Video);
    assert_eq!(preview.section_label(), "Video");
    assert_eq!(preview.detail.as_deref(), Some("MP4 video"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn video_preview_falls_back_to_file_metadata_without_tools() {
    let root = temp_path("video-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("clip.mkv");
    let contents = b"still-not-a-real-video";
    fs::write(&path, contents).expect("failed to write video fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Video);
    assert_eq!(preview.detail.as_deref(), Some("Matroska video"));
    assert!(line_texts.iter().any(|line| line.contains("File Size")
        && line.contains(&crate::app::format_size(contents.len() as u64))));
    assert!(preview.preview_visual.is_none());
    assert!(
        line_texts
            .iter()
            .all(|line| !line.contains("Binary or unsupported file"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn video_loading_preview_uses_empty_body() {
    let root = temp_path("video-loading");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("clip.webm");
    fs::write(&path, b"still-loading").expect("failed to write video fixture");

    let preview = loading_preview_for(&file_entry(path), &PreviewRequestOptions::Default);

    assert_eq!(preview.kind, PreviewKind::Video);
    assert_eq!(preview.detail.as_deref(), Some("WebM video"));
    assert!(preview.lines.is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn video_preview_attaches_inline_cover_when_tools_are_available() {
    if !video_tools_available() {
        return;
    }

    let root = temp_path("video-thumb");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("clip.mp4");
    if !write_test_video_fixture(&path) {
        fs::remove_dir_all(root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        default_code_preview_line_limit(),
        default_code_preview_line_limit(),
        true,
        true,
        &|| false,
    );
    let visual = preview
        .preview_visual
        .clone()
        .expect("video preview should attach a thumbnail");
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Video);
    assert_eq!(visual.kind, PreviewVisualKind::Cover);
    assert_eq!(visual.layout, PreviewVisualLayout::Inline);
    assert!(visual.path.exists());
    assert!(visual.size > 0);
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Dimensions") && line.contains("320x180"))
    );
    assert!(line_texts.iter().any(|line| line.contains("Duration")));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn video_preview_skips_thumbnail_without_ffprobe_even_if_ffmpeg_is_available() {
    let root = temp_path("video-no-ffprobe");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("clip.avi");
    let contents = b"still-not-a-real-video";
    fs::write(&path, contents).expect("failed to write video fixture");

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        default_code_preview_line_limit(),
        default_code_preview_line_limit(),
        false,
        true,
        &|| false,
    );
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Video);
    assert_eq!(preview.detail.as_deref(), Some("AVI video"));
    assert!(preview.preview_visual.is_none());
    assert!(line_texts.iter().any(|line| line.contains("File Size")
        && line.contains(&crate::app::format_size(contents.len() as u64))));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

fn video_tools_available() -> bool {
    Command::new("ffprobe").arg("-version").output().is_ok()
        && Command::new("ffmpeg").arg("-version").output().is_ok()
}

fn write_test_video_fixture(path: &std::path::Path) -> bool {
    Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x180:r=25",
            "-t",
            "2",
            "-c:v",
            "mpeg4",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .status()
        .is_ok_and(|status| status.success())
}
