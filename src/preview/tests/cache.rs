use crate::{
    filesystem::{Entry, EntryKind},
    preview::{
        PREVIEW_CACHE_LIMIT, PREVIEW_LINE_COUNT_CACHE_LIMIT, PreviewContent, PreviewKind,
        PreviewRequestOptions, PreviewRuntime,
    },
};
use ratatui::text::Line;
use std::path::PathBuf;

fn entry(index: usize) -> Entry {
    Entry {
        path: PathBuf::from(format!("{index}.txt")),
        name: format!("{index}.txt"),
        name_key: format!("{index}.txt"),
        kind: EntryKind::File,
        symlink: None,
        size: index as u64 + 1,
        modified: None,
        readonly: false,
    }
}

#[test]
fn preview_result_cache_evicts_oldest_entry_at_limit() {
    let mut preview = PreviewRuntime::new();
    let variant = PreviewRequestOptions::Default;

    for index in 0..=PREVIEW_CACHE_LIMIT {
        let entry = entry(index);
        let content = PreviewContent::new(
            PreviewKind::Text,
            vec![Line::from(format!("preview {index}"))],
        );
        preview
            .state
            .remember_preview(&entry, &variant, 0, 0, false, &content);
    }

    assert_eq!(preview.state.result_cache.len(), PREVIEW_CACHE_LIMIT);
    assert!(!preview.state.has_cached_preview_for_path(&entry(0).path));
    assert!(
        preview
            .state
            .has_cached_preview_for_path(&entry(PREVIEW_CACHE_LIMIT).path)
    );
}

#[test]
fn preview_line_count_cache_evicts_oldest_entry_at_limit() {
    let mut preview = PreviewRuntime::new();

    for index in 0..=PREVIEW_LINE_COUNT_CACHE_LIMIT {
        let entry = entry(index);
        preview
            .state
            .remember_line_count(entry.path, entry.size, None, index + 1);
    }

    assert_eq!(
        preview.state.line_count_cache.len(),
        PREVIEW_LINE_COUNT_CACHE_LIMIT
    );
    assert!(
        !preview
            .state
            .line_count_cache
            .keys()
            .any(|key| key.path == entry(0).path)
    );
    assert!(
        preview
            .state
            .line_count_cache
            .keys()
            .any(|key| key.path == entry(PREVIEW_LINE_COUNT_CACHE_LIMIT).path)
    );
}
