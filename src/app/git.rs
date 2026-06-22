use super::{
    App,
    jobs::{GitCommandBuild, GitCommandRequest, GitStatusBuild, GitStatusRequest},
    state::GitView,
};
use crate::core::Entry;
use crate::preview::{PreviewContent, PreviewKind};
use ratatui::{
    style::{Color, Style},
    text::Line,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
};

/// Working-tree status of a single path, derived from `git status --porcelain`.
/// Collapses git's two-axis (index/worktree) codes into one badge per entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GitFileStatus {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
}

impl GitFileStatus {
    /// Single-letter badge shown next to the file name.
    pub(crate) fn badge(self) -> char {
        match self {
            Self::Untracked => '?',
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Conflicted => 'U',
        }
    }

    /// Maps a porcelain `XY` code to a single status, preferring the most
    /// significant axis. Returns `None` for ignored or unrecognized entries.
    fn from_porcelain(code: &str) -> Option<Self> {
        let mut chars = code.chars();
        let x = chars.next()?;
        let y = chars.next().unwrap_or(' ');

        if x == '?' || y == '?' {
            return Some(Self::Untracked);
        }
        if x == '!' || y == '!' {
            return None;
        }
        if x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
            return Some(Self::Conflicted);
        }
        if x == 'R' || y == 'R' || x == 'C' || y == 'C' {
            return Some(Self::Renamed);
        }
        if x == 'A' {
            return Some(Self::Added);
        }
        if x == 'D' || y == 'D' {
            return Some(Self::Deleted);
        }
        if matches!(x, 'M' | 'T') || matches!(y, 'M' | 'T') {
            return Some(Self::Modified);
        }
        None
    }
}

/// A read-only git command the user can run from the git menu. The output is
/// captured off-thread and shown in the preview pane; nothing here mutates the
/// repository, so it is always safe to run without leaving the TUI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum GitCommand {
    Status,
    Log,
    Diff,
}

impl GitCommand {
    pub(in crate::app) fn args(self) -> &'static [&'static str] {
        match self {
            // `-c color.ui=never` keeps ANSI escapes out of the captured text
            // since git would otherwise color when it detects this is not a tty
            // in some configurations.
            Self::Status => &["-c", "color.ui=never", "status"],
            Self::Log => &[
                "-c",
                "color.ui=never",
                "log",
                "--max-count=200",
                "--graph",
                "--oneline",
                "--decorate",
            ],
            Self::Diff => &["-c", "color.ui=never", "diff"],
        }
    }

    pub(in crate::app) fn title(self) -> &'static str {
        match self {
            Self::Status => "git status",
            Self::Log => "git log",
            Self::Diff => "git diff",
        }
    }
}

/// A remote-syncing git command run from the menu. Output is shown in the
/// preview pane (so the user sees the fetch/merge/push result) and markers are
/// refreshed afterwards; `pull` also reloads the directory listing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum GitRemote {
    Pull,
    Push,
    Fetch,
}

impl GitRemote {
    fn args(self) -> &'static [&'static str] {
        match self {
            // `--no-edit` keeps a merge from blocking on $EDITOR, which we
            // cannot drive from the captured-output model.
            Self::Pull => &["-c", "color.ui=never", "pull", "--no-edit"],
            Self::Push => &["-c", "color.ui=never", "push"],
            Self::Fetch => &["-c", "color.ui=never", "fetch", "--all", "--prune"],
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Pull => "git pull",
            Self::Push => "git push",
            Self::Fetch => "git fetch",
        }
    }

    /// Whether the command can change tracked files and thus needs a directory
    /// reload (only `pull` updates the working tree).
    fn reloads_worktree(self) -> bool {
        matches!(self, Self::Pull)
    }
}

/// A git operation submitted to the background pool. Either a read-only view
/// command (output → preview pane), a working-tree mutation on a specific path,
/// a commit, or a remote sync.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) enum GitCommandKind {
    View(GitCommand),
    Stage(PathBuf),
    Unstage(PathBuf),
    Commit(String),
    Remote(GitRemote),
}

/// A choice in the git menu. Resolved to a [`GitCommandKind`] at confirm time
/// (or, for [`Self::Commit`], opens the message prompt); mutations bind to
/// whichever entry is focused then.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum GitMenuAction {
    View(GitCommand),
    Stage,
    Unstage,
    Commit,
    Remote(GitRemote),
}

impl App {
    pub(crate) fn git_branch(&self) -> Option<&str> {
        self.git.branch.as_deref()
    }

    pub(crate) fn git_dirty(&self) -> bool {
        self.git.dirty
    }

    pub(crate) fn refresh_git_branch(&mut self) {
        let cwd = self.navigation.cwd.clone();
        let cwd_changed = self.git.cwd != cwd;
        self.git.cwd = cwd.clone();
        if cwd_changed {
            self.git.branch = None;
            self.git.dirty = false;
            self.git.statuses.clear();
        }
        self.git.token = self.git.token.wrapping_add(1);
        let token = self.git.token;
        self.jobs
            .scheduler
            .submit_git_status(GitStatusRequest { token, cwd });
    }

    /// Git working-tree status for an entry, if any. Directories report
    /// `Modified` when they contain changed paths but are not themselves a
    /// tracked change (fully untracked directories match by path directly).
    pub(crate) fn git_entry_status(&self, entry: &Entry) -> Option<GitFileStatus> {
        if self.git.statuses.is_empty() {
            return None;
        }
        if let Some(&status) = self.git.statuses.get(&entry.path) {
            return Some(status);
        }
        if entry.is_dir()
            && self
                .git
                .statuses
                .keys()
                .any(|path| path.starts_with(&entry.path))
        {
            return Some(GitFileStatus::Modified);
        }
        None
    }

    /// Whether the current directory is inside a git repository.
    pub(crate) fn git_is_active(&self) -> bool {
        self.git.branch.is_some()
    }

    /// Title of the git output currently shown in the preview pane, if any.
    pub(crate) fn git_view_title(&self) -> Option<&str> {
        self.git.view.as_ref().map(|view| view.title.as_str())
    }

    pub(crate) fn git_view_is_active(&self) -> bool {
        self.git.view.is_some()
    }

    /// Submits a read-only git view command to run off-thread. The captured
    /// output replaces the preview pane once the result arrives.
    pub(in crate::app) fn run_git_command(&mut self, command: GitCommand) {
        self.submit_git_kind(GitCommandKind::View(command));
    }

    /// Stages (or unstages) the focused entry. No-op with a status message when
    /// nothing is focused. Markers refresh once the mutation completes.
    pub(in crate::app) fn run_git_stage(&mut self, unstage: bool) {
        let Some(path) = self.selected_entry().map(|entry| entry.path.clone()) else {
            self.status = "No file selected".to_string();
            return;
        };
        let kind = if unstage {
            GitCommandKind::Unstage(path)
        } else {
            GitCommandKind::Stage(path)
        };
        self.submit_git_kind(kind);
    }

    /// Commits the staged changes with `message`. Markers and status refresh
    /// once the commit completes.
    pub(in crate::app) fn run_git_commit(&mut self, message: String) {
        self.submit_git_kind(GitCommandKind::Commit(message));
    }

    /// Runs a remote sync (pull/push/fetch). Output is shown in the preview
    /// pane and markers refresh once it completes.
    pub(in crate::app) fn run_git_remote(&mut self, remote: GitRemote) {
        self.submit_git_kind(GitCommandKind::Remote(remote));
    }

    fn submit_git_kind(&mut self, kind: GitCommandKind) {
        if self.git_branch().is_none() {
            self.status = "Not a git repository".to_string();
            return;
        }
        let cwd = self.navigation.cwd.clone();
        self.git.command_token = self.git.command_token.wrapping_add(1);
        let token = self.git.command_token;
        self.status.clear();
        self.jobs
            .scheduler
            .submit_git_command(GitCommandRequest { token, cwd, kind });
    }

    pub(in crate::app) fn apply_git_command_result(&mut self, result: GitCommandBuild) -> bool {
        if result.token != self.git.command_token || result.cwd != self.navigation.cwd {
            return false;
        }

        match &result.kind {
            GitCommandKind::View(command) => {
                let lines = git_output_lines(*command, &result.output, result.success);
                self.show_git_output(command.title(), lines);
            }
            GitCommandKind::Remote(remote) => {
                let lines = output_to_lines(&result.output, result.success, "Done", false);
                self.show_git_output(remote.title(), lines);
                // A pull can change tracked files; reload the listing.
                if remote.reloads_worktree()
                    && let Err(error) = self.reload()
                {
                    self.report_runtime_error("Reload after git pull failed", &error);
                }
                self.refresh_git_branch();
            }
            GitCommandKind::Stage(path) | GitCommandKind::Unstage(path) => {
                let verb = if matches!(result.kind, GitCommandKind::Unstage(_)) {
                    "Unstaged"
                } else {
                    "Staged"
                };
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                self.status = if result.success {
                    format!("{verb} {name}")
                } else {
                    git_error_message(&result.output)
                };
                // Re-run the status probe so the per-file markers reflect the
                // new index state.
                self.refresh_git_branch();
            }
            GitCommandKind::Commit(_) => {
                self.status = if result.success {
                    git_commit_summary(&result.output)
                } else {
                    git_error_message(&result.output)
                };
                self.refresh_git_branch();
            }
        }
        true
    }

    /// Shows captured git output in the preview pane under `title`. Bumps the
    /// preview token so any in-flight file preview is treated as stale and does
    /// not overwrite the git output.
    fn show_git_output(&mut self, title: &str, lines: Vec<Line<'static>>) {
        self.preview.state.token = self.preview.state.token.wrapping_add(1);
        self.preview.state.deferred_refresh_at = None;
        self.preview.state.prefetch_ready_at = None;
        self.preview.state.load_state = None;
        self.preview.state.scroll = 0;
        self.preview.state.horizontal_scroll = 0;
        self.preview.state.content = PreviewContent::new(PreviewKind::Code, lines);
        self.git.view = Some(GitView {
            title: title.to_string(),
        });
    }

    pub(in crate::app) fn apply_git_status_result(&mut self, result: GitStatusBuild) -> bool {
        if result.token != self.git.token || result.cwd != self.git.cwd {
            return false;
        }
        let changed = self.git.branch != result.branch
            || self.git.dirty != result.dirty
            || self.git.statuses != result.statuses;
        self.git.branch = result.branch;
        self.git.dirty = result.dirty;
        self.git.statuses = result.statuses;
        changed
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

pub(in crate::app) fn current_status(
    cwd: &Path,
) -> (Option<String>, HashMap<PathBuf, GitFileStatus>) {
    if git_command(cwd, ["rev-parse", "--is-inside-work-tree"])
        .is_none_or(|output| output.trim() != "true")
    {
        return (None, HashMap::new());
    }

    let branch = git_command(cwd, ["branch", "--show-current"])
        .and_then(non_empty_trimmed)
        .or_else(|| git_command(cwd, ["rev-parse", "--short", "HEAD"]).and_then(non_empty_trimmed));

    let toplevel = git_command(cwd, ["rev-parse", "--show-toplevel"])
        .and_then(non_empty_trimmed)
        .map(PathBuf::from);
    let statuses = git_command(
        cwd,
        ["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
    )
    .map(|output| parse_porcelain(&output, toplevel.as_deref()))
    .unwrap_or_default();

    (branch, statuses)
}

/// Parses NUL-delimited `git status --porcelain=v1 -z` output into a map of
/// absolute path → status. Rename/copy records carry an extra trailing field
/// (the source path) which is consumed and ignored.
fn parse_porcelain(output: &str, toplevel: Option<&Path>) -> HashMap<PathBuf, GitFileStatus> {
    let mut statuses = HashMap::new();
    let mut fields = output.split('\0');
    while let Some(record) = fields.next() {
        if record.len() < 4 {
            continue;
        }
        let code = &record[..2];
        let path = &record[3..];
        let is_rename =
            matches!(code.as_bytes()[0], b'R' | b'C') || matches!(code.as_bytes()[1], b'R' | b'C');
        if is_rename {
            // The source path follows as its own NUL-delimited field.
            let _ = fields.next();
        }
        let Some(status) = GitFileStatus::from_porcelain(code) else {
            continue;
        };
        let absolute = match toplevel {
            Some(root) => root.join(path),
            None => PathBuf::from(path),
        };
        statuses.insert(absolute, status);
    }
    statuses
}

/// Runs a read-only git command in `cwd` and returns its captured output and
/// whether it exited successfully. On failure stderr is preferred so the user
/// sees the actual git error message.
pub(in crate::app) fn run_command(cwd: &Path, kind: &GitCommandKind) -> (String, bool) {
    let mut command = Command::new("git");
    command.arg("--no-optional-locks").arg("-C").arg(cwd);
    match kind {
        GitCommandKind::View(view) => {
            command.args(view.args());
        }
        // `--` guards against paths that look like options; the path is passed
        // as an OsStr so non-UTF-8 names work.
        GitCommandKind::Stage(path) => {
            command.arg("add").arg("--").arg(path);
        }
        GitCommandKind::Unstage(path) => {
            command.arg("restore").arg("--staged").arg("--").arg(path);
        }
        GitCommandKind::Commit(message) => {
            command.arg("commit").arg("-m").arg(message);
        }
        GitCommandKind::Remote(remote) => {
            command.args(remote.args());
        }
    }
    let output = command.output();

    match output {
        Ok(output) => {
            let success = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let text = if success {
                if stdout.trim().is_empty() && !stderr.trim().is_empty() {
                    stderr.into_owned()
                } else {
                    stdout.into_owned()
                }
            } else if stderr.trim().is_empty() {
                stdout.into_owned()
            } else {
                stderr.into_owned()
            };
            (text, success)
        }
        Err(error) => (format!("Failed to run git: {error}"), false),
    }
}

/// Builds styled preview lines from captured git output. `diff` output gets
/// light +/- coloring; other commands render as plain text. An empty
/// successful result shows a friendly placeholder instead of a blank pane.
fn git_output_lines(command: GitCommand, output: &str, success: bool) -> Vec<Line<'static>> {
    let empty_message = match command {
        GitCommand::Diff => "No changes",
        GitCommand::Log => "No commits yet",
        GitCommand::Status => "Working tree clean",
    };
    output_to_lines(
        output,
        success,
        empty_message,
        matches!(command, GitCommand::Diff),
    )
}

/// Shared captured-output → styled-lines conversion. `empty_message` is shown
/// when a successful command produced no output; `color_diff` enables the +/-
/// highlighting used for diffs.
fn output_to_lines(
    output: &str,
    success: bool,
    empty_message: &str,
    color_diff: bool,
) -> Vec<Line<'static>> {
    let trimmed = output.trim_end_matches(['\n', '\r']);
    if success && trimmed.trim().is_empty() {
        return vec![Line::from(empty_message.to_string())];
    }

    trimmed
        .split('\n')
        .map(|line| {
            let text = line.trim_end_matches('\r').to_string();
            let style = if color_diff {
                diff_line_style(&text)
            } else {
                Style::default()
            };
            Line::styled(text, style)
        })
        .collect()
}

/// One-line summary of a successful commit. Git prints `[branch hash] subject`
/// as its first line, which already reads well in the status bar.
fn git_commit_summary(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| format!("Committed {line}"))
        .unwrap_or_else(|| "Committed".to_string())
}

/// First meaningful line of git's output, for one-line status reporting after
/// a failed mutation. Falls back to a generic message when output is empty.
fn git_error_message(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "git command failed".to_string())
}

fn diff_line_style(line: &str) -> Style {
    if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(Color::Yellow)
    } else if line.starts_with('+') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    }
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
    use super::{
        App, GitCommand, GitFileStatus, current_status, diff_line_style, git_output_lines,
        parse_porcelain,
    };
    use crate::app::jobs::GitCommandBuild;
    use ratatui::{style::Color, text::Line};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn test_directory(path: PathBuf) -> crate::core::Entry {
        crate::core::Entry {
            path,
            kind: crate::core::EntryKind::Directory,
            ..Default::default()
        }
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
        // Canonicalize to match `git rev-parse --show-toplevel`, which resolves
        // symlinks (e.g. macOS /var -> /private/var). In real usage elio's cwd
        // comes from `env::current_dir()` (getcwd), which is already canonical.
        let root = fs::canonicalize(&root).expect("failed to canonicalize temp dir");

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

        let (branch, statuses) = current_status(&root);
        assert_eq!(branch, Some("main".to_string()));
        assert!(statuses.is_empty(), "clean tree should report no statuses");

        fs::write(root.join("untracked.txt"), "dirty").expect("failed to write dirty file");
        let (branch, statuses) = current_status(&root);
        assert_eq!(branch, Some("main".to_string()));
        assert_eq!(
            statuses.get(&root.join("untracked.txt")),
            Some(&GitFileStatus::Untracked),
            "untracked file should be reported, got: {statuses:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn porcelain_classifies_codes() {
        assert_eq!(
            GitFileStatus::from_porcelain("??"),
            Some(GitFileStatus::Untracked)
        );
        assert_eq!(
            GitFileStatus::from_porcelain(" M"),
            Some(GitFileStatus::Modified)
        );
        assert_eq!(
            GitFileStatus::from_porcelain("M "),
            Some(GitFileStatus::Modified)
        );
        assert_eq!(
            GitFileStatus::from_porcelain("A "),
            Some(GitFileStatus::Added)
        );
        assert_eq!(
            GitFileStatus::from_porcelain(" D"),
            Some(GitFileStatus::Deleted)
        );
        assert_eq!(
            GitFileStatus::from_porcelain("R "),
            Some(GitFileStatus::Renamed)
        );
        assert_eq!(
            GitFileStatus::from_porcelain("UU"),
            Some(GitFileStatus::Conflicted)
        );
        assert_eq!(GitFileStatus::from_porcelain("!!"), None);
    }

    #[test]
    fn parse_porcelain_builds_absolute_paths_and_skips_rename_source() {
        let root = Path::new("/repo");
        // " M file.txt\0R  old.txt\0new.txt\0?? extra/\0"
        let output = " M file.txt\0R  new.txt\0old.txt\0?? extra/\0";
        let map = parse_porcelain(output, Some(root));

        assert_eq!(
            map.get(&root.join("file.txt")),
            Some(&GitFileStatus::Modified)
        );
        assert_eq!(
            map.get(&root.join("new.txt")),
            Some(&GitFileStatus::Renamed)
        );
        assert_eq!(
            map.get(&root.join("extra")),
            Some(&GitFileStatus::Untracked)
        );
        // The rename source must not leak in as its own entry.
        assert!(!map.contains_key(&root.join("old.txt")));
    }

    #[test]
    fn directory_entries_aggregate_nested_changes() {
        let root = temp_path("dir-aggregate");
        fs::create_dir_all(root.join("src")).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some("main"));
        app.git
            .statuses
            .insert(root.join("src/main.rs"), GitFileStatus::Modified);

        let dir = test_directory(root.join("src"));
        assert_eq!(app.git_entry_status(&dir), Some(GitFileStatus::Modified));

        let unrelated = test_directory(root.join("docs"));
        assert_eq!(app.git_entry_status(&unrelated), None);

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn empty_successful_output_shows_placeholder() {
        let lines = git_output_lines(GitCommand::Diff, "", true);
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "No changes");
    }

    #[test]
    fn output_is_split_into_lines() {
        let lines = git_output_lines(
            GitCommand::Status,
            "On branch main\nnothing to commit\n",
            true,
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(line_text(&lines[0]), "On branch main");
        assert_eq!(line_text(&lines[1]), "nothing to commit");
    }

    #[test]
    fn diff_lines_are_colored() {
        assert_eq!(diff_line_style("+added").fg, Some(Color::Green));
        assert_eq!(diff_line_style("-removed").fg, Some(Color::Red));
        assert_eq!(diff_line_style("@@ -1 +1 @@").fg, Some(Color::Cyan));
        assert_eq!(diff_line_style(" context").fg, None);
    }

    #[test]
    fn failed_output_prefers_stderr_and_is_not_colored_for_status() {
        // Failure output is shown verbatim regardless of command.
        let lines = git_output_lines(GitCommand::Status, "fatal: not a git repository", false);
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "fatal: not a git repository");
        assert_eq!(lines[0].spans[0].style.fg, None);
    }

    #[test]
    fn apply_git_command_result_populates_preview_and_view() {
        let root = temp_path("apply-result");
        fs::create_dir_all(&root).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some("main"));

        app.run_git_command(GitCommand::Status);
        let token = app.git.command_token;
        let build = GitCommandBuild {
            token,
            cwd: app.navigation.cwd.clone(),
            kind: crate::app::git::GitCommandKind::View(GitCommand::Status),
            output: "On branch main\n".to_string(),
            success: true,
        };

        assert!(app.apply_git_command_result(build));
        assert!(app.git_view_is_active());
        assert_eq!(app.git_view_title(), Some("git status"));
        assert_eq!(app.preview_scroll_offset(), 0);
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line_text(line) == "On branch main"),
            "preview should contain the git output"
        );

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn stale_git_command_result_is_ignored() {
        let root = temp_path("apply-stale");
        fs::create_dir_all(&root).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some("main"));

        app.run_git_command(GitCommand::Status);
        let stale_token = app.git.command_token.wrapping_add(1);
        let build = GitCommandBuild {
            token: stale_token,
            cwd: app.navigation.cwd.clone(),
            kind: crate::app::git::GitCommandKind::View(GitCommand::Status),
            output: "stale".to_string(),
            success: true,
        };

        assert!(!app.apply_git_command_result(build));
        assert!(!app.git_view_is_active());

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn refresh_preview_clears_git_view() {
        let root = temp_path("clear-view");
        fs::create_dir_all(&root).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some("main"));

        app.run_git_command(GitCommand::Status);
        let token = app.git.command_token;
        let build = GitCommandBuild {
            token,
            cwd: app.navigation.cwd.clone(),
            kind: crate::app::git::GitCommandKind::View(GitCommand::Status),
            output: "On branch main".to_string(),
            success: true,
        };
        assert!(app.apply_git_command_result(build));
        assert!(app.git_view_is_active());

        app.refresh_preview();
        assert!(!app.git_view_is_active());

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }
}
