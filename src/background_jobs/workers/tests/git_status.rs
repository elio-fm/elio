use super::current_status;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-git-{label}-{unique}"))
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn git(root: &PathBuf, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git command should run");
    assert!(status.success(), "git command should succeed: {args:?}");
}

#[test]
fn current_status_marks_untracked_files_dirty() {
    if !git_available() {
        eprintln!("skipping git dirty-status integration test because git is unavailable");
        return;
    }

    let root = temp_path("dirty");
    fs::create_dir_all(&root).expect("failed to create temp dir");

    git(&root, &["init", "-b", "main"]);
    fs::write(root.join("tracked.txt"), "tracked").expect("failed to write tracked file");
    git(&root, &["add", "tracked.txt"]);
    git(
        &root,
        &[
            "-c",
            "user.name=elio tests",
            "-c",
            "user.email=elio@example.invalid",
            "commit",
            "-m",
            "initial",
        ],
    );

    assert_eq!(current_status(&root), (Some("main".to_string()), false));

    fs::write(root.join("untracked.txt"), "dirty").expect("failed to write dirty file");
    assert_eq!(current_status(&root), (Some("main".to_string()), true));

    fs::remove_dir_all(root).expect("failed to remove temp dir");
}
