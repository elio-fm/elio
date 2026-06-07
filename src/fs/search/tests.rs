use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-search-{label}-{unique}"))
}

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

#[test]
fn collect_candidates_respects_hidden_toggle() {
    let root = temp_path("hidden-toggle");
    fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create hidden dir");
    fs::create_dir_all(root.join("projects/needle")).expect("failed to create visible dir");

    let hidden_off =
        collect_candidates_with_limits(&root, false, SearchCandidateScope::Folders, 100, 1_000)
            .expect("failed to collect visible candidates")
            .candidates;
    assert!(
        hidden_off
            .iter()
            .any(|candidate| candidate.relative == "projects")
    );
    assert!(
        hidden_off
            .iter()
            .any(|candidate| candidate.relative == "projects/needle")
    );
    assert!(
        !hidden_off
            .iter()
            .any(|candidate| candidate.relative == ".hidden-root/needle")
    );

    let hidden_on =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Folders, 100, 1_000)
            .expect("failed to collect hidden candidates")
            .candidates;
    assert!(
        hidden_on
            .iter()
            .any(|candidate| candidate.relative == ".hidden-root/needle")
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_follow_stable_breadth_first_order_under_limit() {
    let root = temp_path("breadth-first-order");
    fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create target dir");
    fs::create_dir_all(root.join("alpha")).expect("failed to create alpha dir");
    fs::create_dir_all(root.join("beta")).expect("failed to create beta dir");
    fs::create_dir_all(root.join("gamma")).expect("failed to create gamma dir");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Folders, 6, 1_000)
            .expect("failed to collect candidates")
            .candidates;

    assert_eq!(candidates[0].relative, ".hidden-root");
    assert_eq!(candidates[1].relative, "alpha");
    assert_eq!(candidates[2].relative, "beta");
    assert_eq!(candidates[3].relative, "gamma");
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.relative == ".hidden-root/needle")
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_prune_known_dirs_without_hiding_the_directory_itself() {
    let root = temp_path("pruned-dirs");
    fs::create_dir_all(root.join("node_modules/package")).expect("failed to create node_modules");
    fs::create_dir_all(root.join("src/feature")).expect("failed to create src tree");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Folders, 100, 1_000)
            .expect("failed to collect candidates")
            .candidates;
    let names = candidates
        .iter()
        .map(|candidate| candidate.relative.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"node_modules"));
    assert!(!names.contains(&"node_modules/package"));
    assert!(names.contains(&"src"));
    assert!(names.contains(&"src/feature"));

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_still_descends_directories_when_searching_files() {
    let root = temp_path("file-search-descend");
    fs::create_dir_all(root.join("alpha")).expect("failed to create alpha");
    fs::write(root.join("alpha/needle.txt"), "needle").expect("failed to write needle");
    fs::write(root.join("top.txt"), "top").expect("failed to write top");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 100, 1_000)
            .expect("failed to collect file candidates")
            .candidates;
    let names = candidates
        .iter()
        .map(|candidate| candidate.relative.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"top.txt"));
    assert!(names.contains(&"alpha/needle.txt"));
    assert!(!names.contains(&"alpha"));

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_reports_node_limit_truncation() {
    let root = temp_path("node-limit");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "alpha").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "beta").expect("failed to write beta");
    fs::write(root.join("gamma.txt"), "gamma").expect("failed to write gamma");

    let index = collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 100, 2)
        .expect("failed to collect candidates");

    assert_eq!(index.stats.visited_nodes, 2);
    assert!(index.stats.node_limit_reached);
    assert!(!index.stats.candidate_limit_reached);
    assert!(index.stats.is_limited());
    assert_eq!(index.candidates.len(), 2);

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_reports_candidate_limit_truncation() {
    let root = temp_path("candidate-limit");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "alpha").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "beta").expect("failed to write beta");
    fs::write(root.join("gamma.txt"), "gamma").expect("failed to write gamma");

    let index = collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 2, 100)
        .expect("failed to collect candidates");

    assert_eq!(index.stats.visited_nodes, 3);
    assert!(!index.stats.node_limit_reached);
    assert!(index.stats.candidate_limit_reached);
    assert!(index.stats.is_limited());
    assert_eq!(index.candidates.len(), 2);

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_streaming_emits_batches_before_final_index() {
    let root = temp_path("streaming-batches");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..(SEARCH_CANDIDATE_BATCH_SIZE + 1) {
        fs::write(root.join(format!("file-{index:04}.txt")), "data").expect("failed to write file");
    }

    let mut batches = Vec::new();
    let index = collect_candidates_streaming(
        &root,
        true,
        SearchCandidateScope::Files,
        || false,
        |batch| {
            batches.push((batch.candidates.len(), batch.stats.visited_nodes));
            true
        },
    )
    .expect("failed to collect streaming candidates");

    assert_eq!(index.candidates.len(), SEARCH_CANDIDATE_BATCH_SIZE + 1);
    assert!(
        batches
            .iter()
            .any(|(candidate_count, _)| *candidate_count == SEARCH_CANDIDATE_BATCH_SIZE),
        "expected at least one full candidate batch, got {batches:?}"
    );
    assert!(
        batches
            .iter()
            .any(|(candidate_count, _)| *candidate_count == 1),
        "expected the trailing candidate to be flushed, got {batches:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_streaming_stops_after_cancellation() {
    use std::cell::Cell;

    let root = temp_path("streaming-cancel");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..10 {
        fs::write(root.join(format!("file-{index:04}.txt")), "data").expect("failed to write file");
    }

    let canceled = Cell::new(false);
    let mut batches = 0usize;
    let index = collect_candidates_with_limits_and_emitter(
        &root,
        true,
        SearchCandidateScope::Files,
        SearchCollectionLimits {
            candidate_limit: usize::MAX,
            node_visit_limit: 100,
            batch_size: 1,
        },
        || canceled.get(),
        |batch| {
            batches += 1;
            assert_eq!(batch.candidates.len(), 1);
            canceled.set(true);
            true
        },
    )
    .expect("failed to collect streaming candidates");

    assert_eq!(batches, 1);
    assert_eq!(index.candidates.len(), 1);

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[cfg(unix)]
#[test]
fn collect_candidates_includes_linked_directory_in_folder_search() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-dir-folder");
    fs::create_dir_all(root.join("real-dir/inner")).expect("failed to create real dir");
    symlink(root.join("real-dir"), root.join("linked-dir")).expect("failed to create dir symlink");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Folders, 100, 1_000)
            .expect("failed to collect folder candidates")
            .candidates;
    let linked = candidates
        .iter()
        .find(|candidate| candidate.relative == "linked-dir")
        .expect("linked-dir should appear in folder search");
    assert!(linked.is_dir, "linked dir should be classified as dir");
    assert_eq!(
        linked
            .symlink
            .as_ref()
            .and_then(|symlink| symlink.target_kind),
        Some(EntryKind::Directory)
    );

    let relatives = candidates
        .iter()
        .map(|candidate| candidate.relative.as_str())
        .collect::<Vec<_>>();
    assert!(
        !relatives.contains(&"linked-dir/inner"),
        "symlinked dir should not be descended into"
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[cfg(unix)]
#[test]
fn collect_candidates_includes_linked_file_in_file_search() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("real.txt"), "data").expect("failed to write real file");
    symlink(root.join("real.txt"), root.join("linked.txt")).expect("failed to create file symlink");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 100, 1_000)
            .expect("failed to collect file candidates")
            .candidates;
    let linked = candidates
        .iter()
        .find(|candidate| candidate.relative == "linked.txt")
        .expect("linked.txt should appear in file search");
    assert!(!linked.is_dir);
    assert_eq!(
        linked
            .symlink
            .as_ref()
            .and_then(|symlink| symlink.target_kind),
        Some(EntryKind::File)
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[cfg(unix)]
#[test]
fn collect_candidates_includes_broken_symlink_in_file_search() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-broken");
    fs::create_dir_all(&root).expect("failed to create temp root");
    symlink(root.join("missing-target"), root.join("dangling"))
        .expect("failed to create broken symlink");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 100, 1_000)
            .expect("failed to collect file candidates")
            .candidates;
    let broken = candidates
        .iter()
        .find(|candidate| candidate.relative == "dangling")
        .expect("broken symlink should appear in file search");
    assert!(!broken.is_dir);
    let symlink_info = broken
        .symlink
        .as_ref()
        .expect("broken candidate carries symlink info");
    assert!(symlink_info.is_broken());

    let folder_candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Folders, 100, 1_000)
            .expect("failed to collect folder candidates")
            .candidates;
    assert!(
        !folder_candidates
            .iter()
            .any(|candidate| candidate.relative == "dangling"),
        "broken symlink should not appear in folder search"
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[cfg(unix)]
#[test]
fn collect_candidates_handles_symlink_cycle() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-cycle");
    fs::create_dir_all(&root).expect("failed to create temp root");
    // A self-referential symlink: even with the read_link follow this should
    // be reported as a broken symlink (metadata resolution fails on cycles)
    // and must never be descended into.
    symlink(root.join("loop"), root.join("loop"))
        .expect("failed to create self-referential symlink");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 100, 1_000)
            .expect("failed to collect candidates")
            .candidates;
    let loop_entry = candidates
        .iter()
        .find(|candidate| candidate.relative == "loop")
        .expect("cycle symlink should appear in file search");
    assert!(!loop_entry.is_dir);
    let symlink_info = loop_entry
        .symlink
        .as_ref()
        .expect("cycle symlink should carry symlink info");
    assert!(symlink_info.is_broken());
    // The important invariant is that we returned at all (no infinite loop).
    assert!(candidates.len() < 50);

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[cfg(unix)]
#[test]
fn collect_candidates_hides_dot_prefixed_symlink_when_hidden_off() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-hidden");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("visible.txt"), "data").expect("failed to write visible");
    symlink(root.join("visible.txt"), root.join(".hidden-link"))
        .expect("failed to create hidden symlink");

    let visible =
        collect_candidates_with_limits(&root, false, SearchCandidateScope::Files, 100, 1_000)
            .expect("failed to collect non-hidden candidates")
            .candidates;
    assert!(
        !visible
            .iter()
            .any(|candidate| candidate.relative == ".hidden-link"),
        "dot-prefixed symlink must respect hidden toggle"
    );

    let all = collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 100, 1_000)
        .expect("failed to collect hidden candidates")
        .candidates;
    assert!(
        all.iter()
            .any(|candidate| candidate.relative == ".hidden-link"),
        "dot-prefixed symlink must appear when hidden toggle is on"
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn collect_candidates_uses_natural_name_order() {
    let root = temp_path("natural-order");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("chapter 10.txt"), "ten").expect("failed to write file");
    fs::write(root.join("chapter 2.txt"), "two").expect("failed to write file");
    fs::write(root.join("chapter 1.txt"), "one").expect("failed to write file");

    let candidates =
        collect_candidates_with_limits(&root, true, SearchCandidateScope::Files, 10, 1_000)
            .expect("failed to collect candidates")
            .candidates;
    let names = candidates
        .iter()
        .map(|candidate| candidate.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["chapter 1.txt", "chapter 2.txt", "chapter 10.txt"]
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
