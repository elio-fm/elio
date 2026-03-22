mod access;
mod cache;
mod headers;
mod prefetch;
mod refresh;
mod request;

use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{PreviewContent, PreviewKind, PreviewRequestOptions};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-preview-actions-{label}-{unique}"))
    }

    #[test]
    fn refresh_preview_reuses_stale_cached_preview_while_refreshing() {
        let root = temp_path("stale-refresh");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let source = root.join("main.rs");
        fs::write(&source, "fn main() {}\n").expect("failed to write source file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let entry = app
            .selected_entry()
            .cloned()
            .expect("source entry should be selected");
        let variant = app.current_preview_request_options();
        let preview = PreviewContent::new(PreviewKind::Code, vec![Line::from("stale preview")])
            .with_detail("Rust source file");
        app.cache_preview_result(&entry, &variant, &preview);
        app.entries[app.selected].size += 1;

        app.refresh_preview();

        assert_eq!(
            app.preview_state.load_state,
            Some(PreviewLoadState::Refreshing(entry.path.clone()))
        );
        assert_eq!(
            app.preview_header_detail(8).as_deref(),
            Some("Rust source file  •  Refreshing in background")
        );
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string() == "stale preview")
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_result_cache_evicts_oldest_entry_at_limit() {
        let root = temp_path("result-cache-limit");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let variant = PreviewRequestOptions::Default;
        let oldest_path = root.join("0.txt");
        let newest_path = root.join(format!("{PREVIEW_CACHE_LIMIT}.txt"));

        for index in 0..=PREVIEW_CACHE_LIMIT {
            let path = root.join(format!("{index}.txt"));
            let entry = Entry {
                path: path.clone(),
                name: format!("{index}.txt"),
                name_key: format!("{index}.txt"),
                kind: EntryKind::File,
                size: index as u64 + 1,
                modified: None,
                readonly: false,
            };
            let preview = PreviewContent::new(
                PreviewKind::Text,
                vec![Line::from(format!("preview {index}"))],
            );
            app.cache_preview_result_with_code_line_limit(&entry, &variant, 0, &preview);
        }

        assert_eq!(app.preview_state.result_cache.len(), PREVIEW_CACHE_LIMIT);
        assert!(!app.has_cached_preview_for_path(&oldest_path));
        assert!(app.has_cached_preview_for_path(&newest_path));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn apply_preview_line_count_result_updates_current_entry_header() {
        let root = temp_path("line-count-update");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let source = root.join("main.rs");
        fs::write(&source, "fn main() {}\n").expect("failed to write source file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let entry = app
            .selected_entry()
            .cloned()
            .expect("source entry should be selected");
        let key = PreviewLineCountKey {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        };
        app.preview_state.content =
            PreviewContent::new(PreviewKind::Code, vec![Line::from("fn main() {}")])
                .with_line_coverage(240, None, true);
        app.preview_state.content.set_total_line_count_pending(true);
        app.preview_state.pending_line_counts.insert(key.clone());

        assert!(app.apply_preview_line_count_result(
            &entry.path,
            entry.size,
            entry.modified,
            Some(1_500)
        ));
        assert!(!app.preview_state.pending_line_counts.contains(&key));
        assert_eq!(
            app.preview_header_detail_for_width(8, 40).as_deref(),
            Some("240 / 1,500 lines shown")
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn apply_preview_line_count_result_clears_pending_state_for_current_entry_without_total() {
        let root = temp_path("line-count-clear");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let source = root.join("main.rs");
        fs::write(&source, "fn main() {}\n").expect("failed to write source file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let entry = app
            .selected_entry()
            .cloned()
            .expect("source entry should be selected");
        let key = PreviewLineCountKey {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        };
        app.preview_state.content =
            PreviewContent::new(PreviewKind::Code, vec![Line::from("fn main() {}")])
                .with_line_coverage(240, None, true);
        app.preview_state.content.set_total_line_count_pending(true);
        app.preview_state.pending_line_counts.insert(key.clone());

        assert!(app.apply_preview_line_count_result(&entry.path, entry.size, entry.modified, None));
        assert!(!app.preview_state.pending_line_counts.contains(&key));
        assert_eq!(
            app.preview_header_detail_for_width(8, 40).as_deref(),
            Some("240 lines shown")
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_line_count_cache_evicts_oldest_entry_at_limit() {
        let root = temp_path("line-count-cache-limit");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let oldest_key = PreviewLineCountKey {
            path: root.join("0.txt"),
            size: 1,
            modified: None,
        };
        let newest_key = PreviewLineCountKey {
            path: root.join(format!("{PREVIEW_LINE_COUNT_CACHE_LIMIT}.txt")),
            size: PREVIEW_LINE_COUNT_CACHE_LIMIT as u64 + 1,
            modified: None,
        };

        for index in 0..=PREVIEW_LINE_COUNT_CACHE_LIMIT {
            app.cache_preview_line_count(
                root.join(format!("{index}.txt")),
                index as u64 + 1,
                None,
                index + 1,
            );
        }

        assert_eq!(
            app.preview_state.line_count_cache.len(),
            PREVIEW_LINE_COUNT_CACHE_LIMIT
        );
        assert!(!app.preview_state.line_count_cache.contains_key(&oldest_key));
        assert!(app.preview_state.line_count_cache.contains_key(&newest_key));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn build_preview_request_disables_video_thumbnails_without_image_overlay_support() {
        let root = temp_path("video-request-gating");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("clip.mp4");
        fs::write(&path, b"video").expect("failed to write video fixture");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let entry = app
            .selected_entry()
            .cloned()
            .expect("video entry should be selected");
        app.set_ffprobe_available_for_tests(true);
        app.set_video_ffmpeg_available_for_tests(true);

        let request = app.build_preview_request(
            entry,
            PreviewRequestOptions::Default,
            PreviewPriority::High,
            crate::preview::PreviewWorkClass::Heavy,
        );

        assert!(request.ffprobe_available);
        assert!(!request.ffmpeg_available);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
