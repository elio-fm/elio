use super::{PreviewContent, PreviewKind};
use crate::app::Entry;

pub(super) fn build_video_preview(
    _entry: &Entry,
    type_detail: Option<&'static str>,
    _ffprobe_available: bool,
    _ffmpeg_available: bool,
) -> PreviewContent {
    PreviewContent::new(
        PreviewKind::Video,
        vec![ratatui::text::Line::from(
            "Video preview not yet implemented",
        )],
    )
    .with_detail(type_detail.unwrap_or("Video"))
}
