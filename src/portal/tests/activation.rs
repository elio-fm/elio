use super::*;

#[test]
fn generated_metadata_has_the_exact_backend_contract() {
    assert_eq!(
        portal_contents(),
        "[portal]\nDBusName=io.github.elio_fm.elio.Portal\nInterfaces=org.freedesktop.impl.portal.FileChooser;\n"
    );
    assert_eq!(
        service_contents(Path::new("/usr/local/bin/elio")).unwrap(),
        "[D-BUS Service]\nName=io.github.elio_fm.elio.Portal\nExec=/usr/local/bin/elio --portal-service\n"
    );
}

#[test]
fn launcher_candidates_never_use_a_relative_path() {
    let candidates = launcher_candidates(&OsString::from("elio")).unwrap();
    assert!(candidates.iter().all(|candidate| candidate.is_absolute()));
}

#[test]
fn state_hashes_detect_drift() {
    assert_ne!(hash(b"original"), hash(b"modified"));
}

#[test]
fn owner_handoff_requires_the_old_owner_to_disappear() {
    assert_eq!(classify_owner_handoff(":1.4", None), OwnerHandoff::Released);
    assert_eq!(
        classify_owner_handoff(":1.4", Some(":1.4")),
        OwnerHandoff::StillOwned
    );
    assert_eq!(
        classify_owner_handoff(":1.4", Some(":1.9")),
        OwnerHandoff::UnexpectedOwner(":1.9".to_string())
    );
}

#[test]
fn replacement_owner_must_differ_from_the_previous_owner() {
    assert!(!is_new_owner(Some(":1.4"), Some(":1.4")));
    assert!(is_new_owner(Some(":1.4"), Some(":1.9")));
    assert!(is_new_owner(None, Some(":1.9")));
    assert!(!is_new_owner(Some(":1.4"), None));
}

#[test]
fn activation_must_start_the_service_instead_of_reusing_an_owner() {
    assert!(service_started(zbus::fdo::StartServiceReply::Success));
    assert!(!service_started(
        zbus::fdo::StartServiceReply::AlreadyRunning
    ));
}

#[test]
fn file_chooser_introspection_timeout_is_clear() {
    let error = file_chooser_introspection_result(Err(zbus::fdo::Error::ZBus(
        io::Error::new(io::ErrorKind::TimedOut, "reply timed out").into(),
    )))
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "error: FileChooser introspection timed out after 5 seconds"
    );
}

#[test]
fn owner_disappearing_between_presence_check_and_read_is_absent() {
    assert_eq!(
        get_name_owner_result::<String>(Err(zbus::fdo::Error::NameHasNoOwner(
            PORTAL_NAME.to_string()
        )))
        .unwrap(),
        None
    );
    assert!(
        get_name_owner_result::<String>(Err(zbus::fdo::Error::Failed("unavailable".to_string())))
            .unwrap_err()
            .to_string()
            .contains("could not read the elio portal owner")
    );
}

#[test]
fn introspection_retry_requires_the_activated_owner() {
    assert!(owner_matches(":1.9", Some(":1.9")));
    assert!(!owner_matches(":1.9", Some(":1.10")));
    assert!(!owner_matches(":1.9", None));
}

#[test]
fn portal_owner_must_match_the_effective_uid() {
    assert!(owner_uid_matches(1000, 1000));
    assert!(!owner_uid_matches(1000, 1001));
}

#[test]
fn owned_artifact_updates_atomically_but_unowned_artifact_is_preserved() {
    let root = temporary_root("materialize");
    let path = root.join("elio.portal");
    assert!(materialize(&path, b"first", None).unwrap());
    let owned = ArtifactState {
        path: path.clone(),
        hash: hash(b"first"),
        created: true,
    };
    assert!(!materialize(&path, b"second", Some(&owned)).unwrap());
    assert_eq!(fs::read(&path).unwrap(), b"second");

    let unowned = root.join("unowned.portal");
    fs::write(&unowned, b"user data").unwrap();
    assert!(materialize(&unowned, b"elio data", None).is_err());
    assert_eq!(fs::read(&unowned).unwrap(), b"user data");

    let externally_managed = root.join("externally-managed.portal");
    fs::write(&externally_managed, b"old elio metadata").unwrap();
    let external_record = ArtifactState {
        path: externally_managed.clone(),
        hash: hash(b"old elio metadata"),
        created: false,
    };
    assert!(
        materialize(
            &externally_managed,
            b"new elio metadata",
            Some(&external_record)
        )
        .is_err()
    );
    assert_eq!(fs::read(&externally_managed).unwrap(), b"old elio metadata");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn disable_terminates_the_backend_before_removing_owned_metadata() {
    let root = temporary_root("disable-order");
    let paths = owned_paths(&root);
    let mut backend_terminated = false;
    let result = disable_at(&paths, || {
        assert!(paths.portal.is_file());
        assert!(paths.service.is_file());
        backend_terminated = true;
        Ok(())
    })
    .unwrap();
    assert!(backend_terminated);
    assert!(result.removed_portal);
    assert!(result.removed_service);
    assert!(!paths.portal.exists());
    assert!(!paths.service.exists());
    assert!(!paths.state.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_backend_termination_preserves_owned_metadata_for_retry() {
    let root = temporary_root("disable-termination-failure");
    let paths = owned_paths(&root);
    assert!(disable_at(&paths, || anyhow::bail!("backend persists")).is_err());
    assert_eq!(load_state(&paths).unwrap().unwrap().phase, Phase::Committed);
    assert!(paths.portal.is_file());
    assert!(paths.service.is_file());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn modified_owned_artifact_is_never_removed() {
    let root = temporary_root("remove");
    let path = root.join("elio.portal");
    fs::write(&path, b"elio data").unwrap();
    let artifact = ArtifactState {
        path: path.clone(),
        hash: hash(b"elio data"),
        created: true,
    };
    fs::write(&path, b"user modification").unwrap();
    assert!(remove_owned(&artifact, false).is_err());
    assert!(path.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn externally_managed_artifact_drift_or_removal_fails_health_verification() {
    let root = temporary_root("external-health");
    let path = root.join("elio.portal");
    let artifact = ArtifactState {
        path: path.clone(),
        hash: hash(b"known metadata"),
        created: false,
    };
    fs::write(&path, b"user modification").unwrap();
    assert!(verify_artifact(&artifact).is_err());
    fs::remove_file(&path).unwrap();
    assert!(verify_artifact(&artifact).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resuming_removal_accepts_an_artifact_removed_before_the_journal_update() {
    let root = temporary_root("removing");
    let path = root.join("elio.portal");
    let artifact = ArtifactState {
        path: path.clone(),
        hash: hash(b"elio data"),
        created: true,
    };
    assert!(!remove_owned(&artifact, true).unwrap());
    assert!(remove_owned(&artifact, false).is_err());
    fs::remove_dir_all(root).unwrap();
}

fn owned_paths(root: &Path) -> Paths {
    let paths = Paths {
        portal: root.join("portal/elio.portal"),
        service: root.join("service/elio.service"),
        state: root.join("state/portal-metadata.json"),
    };
    let portal = b"portal metadata";
    let service = b"service metadata";
    fs::create_dir_all(paths.portal.parent().unwrap()).unwrap();
    fs::create_dir_all(paths.service.parent().unwrap()).unwrap();
    fs::write(&paths.portal, portal).unwrap();
    fs::write(&paths.service, service).unwrap();
    write_state(
        &paths,
        &MetadataState {
            version: STATE_VERSION,
            phase: Phase::Committed,
            launcher: PathBuf::from("/usr/local/bin/elio"),
            resolved_executable: PathBuf::from("/usr/local/bin/elio"),
            portal: ArtifactState {
                path: paths.portal.clone(),
                hash: hash(portal),
                created: true,
            },
            service: ArtifactState {
                path: paths.service.clone(),
                hash: hash(service),
                created: true,
            },
        },
    )
    .unwrap();
    paths
}

fn temporary_root(label: &str) -> PathBuf {
    let root = env::temp_dir().join(format!(
        "elio-portal-activation-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
