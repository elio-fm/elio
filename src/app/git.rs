use super::{
    App,
    jobs::{GitStatusBuild, GitStatusRequest},
};
use std::{path::Path, process::Command};

impl App {
    pub(crate) fn git_branch(&self) -> Option<&str> {
        self.git.branch.as_deref()
    }

    pub(crate) fn git_dirty(&self) -> bool {
        self.git.dirty
    }

    pub(in crate::app) fn refresh_git_branch(&mut self) {
        let cwd = self.navigation.cwd.clone();
        let cwd_changed = self.git.cwd != cwd;
        self.git.cwd = cwd.clone();
        if cwd_changed {
            self.git.branch = None;
            self.git.dirty = false;
        }
        self.git.token = self.git.token.wrapping_add(1);
        let token = self.git.token;
        self.jobs
            .scheduler
            .submit_git_status(GitStatusRequest { token, cwd });
    }

    pub(in crate::app) fn apply_git_status_result(&mut self, result: GitStatusBuild) -> bool {
        if result.token != self.git.token || result.cwd != self.git.cwd {
            return false;
        }
        let dirty = self.git.branch != result.branch || self.git.dirty != result.dirty;
        self.git.branch = result.branch;
        self.git.dirty = result.dirty;
        dirty
    }

    #[cfg(test)]
    pub(crate) fn set_git_branch_for_test(&mut self, branch: Option<&str>) {
        self.git.branch = branch.map(str::to_string);
    }

    #[cfg(test)]
    pub(crate) fn set_git_dirty_for_test(&mut self, dirty: bool) {
        self.git.dirty = dirty;
    }
}

pub(in crate::app) fn current_status(cwd: &Path) -> (Option<String>, bool) {
    if git_command(cwd, ["rev-parse", "--is-inside-work-tree"])
        .is_none_or(|output| output.trim() != "true")
    {
        return (None, false);
    }

    let branch = git_command(cwd, ["branch", "--show-current"])
        .and_then(non_empty_trimmed)
        .or_else(|| git_command(cwd, ["rev-parse", "--short", "HEAD"]).and_then(non_empty_trimmed));
    let dirty = git_command(
        cwd,
        ["status", "--porcelain=v1", "--untracked-files=normal"],
    )
    .is_some_and(|output| !output.trim().is_empty());

    (branch, dirty)
}

fn git_command<const N: usize>(cwd: &Path, args: [&str; N]) -> Option<String> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn non_empty_trimmed(output: String) -> Option<String> {
    let branch = output.trim();
    (!branch.is_empty()).then(|| branch.to_string())
}

#[cfg(test)]
mod tests {
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
}
