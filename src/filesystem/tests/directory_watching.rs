use super::*;

#[test]
fn coarse_head_events_compare_contents_not_reported_paths() {
    use notify::event::{AccessKind, CreateKind, ModifyKind};
    use std::{fs, time::SystemTime};

    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("elio-coarse-head-{}-{unique}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let head = dir.join("HEAD");
    fs::write(&head, "ref: refs/heads/main\n").unwrap();
    let mut previous = fs::read(&head).ok();
    let access = Event::new(EventKind::Access(AccessKind::Any)).add_path(head.clone());
    // Kqueue can report the parent or any unwatched child; other backends may
    // deliver a pathless event. None should cause Git churn with unchanged HEAD.
    for (index, event) in [
        Event::new(EventKind::Modify(ModifyKind::Any)).add_path(dir.clone()),
        Event::new(EventKind::Create(CreateKind::File)).add_path(dir.join("config")),
        Event::new(EventKind::Any),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(!git_head_changed(&head, &event, true, &mut previous));
        fs::write(
            dir.join("HEAD.lock"),
            format!("ref: refs/heads/branch-{index}\n"),
        )
        .unwrap();
        fs::rename(dir.join("HEAD.lock"), &head).unwrap();
        assert!(!git_head_changed(&head, &access, true, &mut previous));
        assert!(git_head_changed(&head, &event, true, &mut previous));
        assert!(!git_head_changed(&head, &event, true, &mut previous));
    }
    // Precise backends keep their cheap path filter, without reading HEAD.
    let unrelated = Event::new(EventKind::Any).add_path(dir.join("config"));
    assert!(!git_head_changed(&head, &unrelated, false, &mut previous));
    let exact = Event::new(EventKind::Any).add_path(head.clone());
    assert!(git_head_changed(&head, &exact, false, &mut previous));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hidden_paths_are_ignored_when_dotfiles_are_hidden() {
    assert!(!event_affects_visible_entries(
        &[PathBuf::from("/tmp/.secret")],
        false,
    ));
}

#[test]
fn visible_paths_trigger_reload_when_dotfiles_are_hidden() {
    assert!(event_affects_visible_entries(
        &[PathBuf::from("/tmp/file.txt")],
        false,
    ));
}

#[test]
fn empty_path_events_force_rescan() {
    assert!(event_affects_visible_entries(&[], false));
}
