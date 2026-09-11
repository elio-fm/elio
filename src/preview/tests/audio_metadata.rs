use super::*;
use std::{path::Path, time::Duration};

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
