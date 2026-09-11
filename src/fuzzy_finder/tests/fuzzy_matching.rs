use super::*;
use std::path::PathBuf;

#[test]
fn fuzzy_filter_prefers_tighter_name_match() {
    let candidates = vec![
        SearchCandidate {
            path: PathBuf::from("/tmp/src/main.rs"),
            name: "main.rs".to_string(),
            name_key: "main.rs".to_string(),
            relative: "src/main.rs".to_string(),
            relative_key: "src/main.rs".to_string(),
            is_dir: false,
            symlink: None,
        },
        SearchCandidate {
            path: PathBuf::from("/tmp/docs/readme.md"),
            name: "readme.md".to_string(),
            name_key: "readme.md".to_string(),
            relative: "docs/readme.md".to_string(),
            relative_key: "docs/readme.md".to_string(),
            is_dir: false,
            symlink: None,
        },
    ];

    let result = filter_candidates_in(&candidates, 0..candidates.len(), "mn", 10);
    assert_eq!(result.matches.first().copied(), Some(0));
}
