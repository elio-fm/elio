use super::*;

#[test]
fn folder_sizes_cache_is_bounded_reused_and_incomplete_invalidates() {
    let mut sizes = FolderSizes::default();
    let paths = (0..=CACHE_LIMIT)
        .map(|index| PathBuf::from(index.to_string()))
        .collect::<Vec<_>>();
    sizes.begin(0, &paths);
    for (index, path) in paths.into_iter().enumerate() {
        sizes.apply(0, path, Some(index as u64));
    }
    assert_eq!(sizes.cache.len(), CACHE_LIMIT);
    assert_eq!(sizes.order.len(), CACHE_LIMIT);
    assert!(!sizes.cache.contains_key(&PathBuf::from("0")));
    let path = PathBuf::from("1");
    sizes.begin(1, std::slice::from_ref(&path));
    assert_eq!(sizes.current[&path], 1);
    assert_eq!(sizes.pending, 1);
    assert!(!sizes.apply(0, path.clone(), Some(999)));
    assert_eq!(sizes.pending, 1);
    assert!(!sizes.apply(1, PathBuf::from("unrequested"), Some(999)));
    assert_eq!(sizes.pending, 1);
    assert!(sizes.apply(1, path.clone(), None));
    assert!(!sizes.apply(1, path.clone(), Some(999)));
    assert_eq!(sizes.pending, 0);
    sizes.begin(2, std::slice::from_ref(&path));
    assert!(sizes.current.is_empty());
    assert_eq!(sizes.pending, 1);
}
