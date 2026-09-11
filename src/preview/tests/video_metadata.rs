use super::*;
use std::{path::Path, time::Duration};

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
