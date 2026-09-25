use super::{GitStatusPool, GitStatusRequest, JobResult, current_status};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn expect_branch(rx: &mpsc::Receiver<JobResult>, token: u64, cwd: &PathBuf, branch: Option<&str>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("watch should deliver Git status without a directory refresh");
        if let JobResult::GitStatus(build) = result
            && build.token == token
            && &build.cwd == cwd
            && build.branch.as_deref() == branch
        {
            return;
        }
    }
}

#[test]
fn head_watch_refreshes_same_commit_switches_in_root_nested_and_linked_worktree() {
    if !git_available() {
        eprintln!("skipping Git watcher test because git is unavailable");
        return;
    }
    let root = temp_path("head-watch");
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(
        &root,
        &[
            "-c",
            "user.name=elio tests",
            "-c",
            "user.email=elio@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "initial",
        ],
    );
    git(&root, &["branch", "other"]);
    let linked = temp_path("linked-head-watch");
    git(
        &root,
        &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
    );
    let nested = root.join("nested");
    fs::create_dir(&nested).unwrap();
    let (tx, rx) = mpsc::channel();
    let pool = GitStatusPool::new(tx);
    for (index, cwd) in [&root, &nested, &linked].into_iter().enumerate() {
        let token = index as u64 + 1;
        let initial = if cwd == &linked { "linked" } else { "main" };
        let alternate = if cwd == &linked {
            "linked-other"
        } else {
            "other"
        };
        if cwd == &linked {
            git(cwd, &["branch", alternate]);
        }
        assert!(pool.submit(GitStatusRequest {
            token,
            cwd: cwd.clone()
        }));
        expect_branch(&rx, token, cwd, Some(initial));
        if cwd == &root {
            fs::write(root.join(".git/HEAD.lock"), "unrelated lock").unwrap();
            fs::remove_file(root.join(".git/HEAD.lock")).unwrap();
            fs::write(root.join(".git/unrelated"), "metadata").unwrap();
            assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
            assert!(!pool.has_pending_work());
        }
        // Each checkout atomically replaces HEAD; watching the old inode would
        // pass once and fail on subsequent switches. No further submits here.
        for branch in [alternate, initial, alternate, initial] {
            git(cwd, &["checkout", branch]);
            expect_branch(&rx, token, cwd, Some(branch));
        }
    }
    // Leaving the repository retires the old watch and its request/token.
    let outside = temp_path("outside-head-watch");
    fs::create_dir(&outside).unwrap();
    assert!(pool.submit(GitStatusRequest {
        token: 4,
        cwd: outside.clone()
    }));
    expect_branch(&rx, 4, &outside, None);
    while rx.try_recv().is_ok() {}
    git(&linked, &["checkout", "linked-other"]);
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    assert!(!pool.has_pending_work());
    drop(pool);
    fs::remove_dir_all(outside).unwrap();
    fs::remove_dir_all(linked).unwrap();
    fs::remove_dir_all(root).unwrap();
}
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
