use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Minimum time between intermediate progress results sent to the UI.
/// Only applies to permanent delete, which processes files one at a time.
/// Non-permanent trash is a single batched OS call with no intermediate
/// progress.
const PROGRESS_SEND_INTERVAL: Duration = Duration::from_millis(80);

pub(in crate::app::jobs) struct TrashPool {
    shared: Arc<TrashShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct TrashShared {
    state: Mutex<TrashState>,
    available: Condvar,
    cancelled: AtomicBool,
    cancel_token: AtomicU64,
}

struct TrashState {
    pending: Option<TrashRequest>,
    active: bool,
    closed: bool,
}

impl TrashPool {
    pub(in crate::app::jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(TrashShared {
            state: Mutex::new(TrashState {
                pending: None,
                active: false,
                closed: false,
            }),
            available: Condvar::new(),
            cancelled: AtomicBool::new(false),
            cancel_token: AtomicU64::new(0),
        });
        let shared_worker = Arc::clone(&shared);
        let worker = thread::spawn(move || {
            while let Some(request) = TrashShared::pop(&shared_worker) {
                TrashShared::set_active(&shared_worker, true);
                let (completed, errors, stopped_early) = run_trash(
                    &request,
                    &result_tx,
                    &shared_worker.cancelled,
                    &shared_worker.cancel_token,
                );
                TrashShared::set_active(&shared_worker, false);

                let verb = if request.permanent {
                    "Permanently deleted"
                } else {
                    "Trashed"
                };
                let total = request.targets.len();
                let single_name = (total == 1).then(|| request.targets[0].name.as_str());
                let status = if stopped_early && errors.is_empty() {
                    match completed {
                        0 => "Trash cancelled".to_string(),
                        1 => format!("Trash cancelled — {verb} 1 item"),
                        n => format!("Trash cancelled — {verb} {n} items"),
                    }
                } else if stopped_early {
                    // Cancelled but some items also errored — surface the errors.
                    // Errors can come from either the direct remove path or staged
                    // cleanup, so use a neutral label rather than "cleanup error".
                    let base = match completed {
                        0 => "Trash cancelled".to_string(),
                        1 => format!("Trash cancelled — {verb} 1 item"),
                        n => format!("Trash cancelled — {verb} {n} items"),
                    };
                    format!("{base}; {} error(s) — first: {}", errors.len(), errors[0])
                } else if errors.is_empty() {
                    match (completed, single_name) {
                        (0, _) => "Nothing was deleted".to_string(),
                        (1, Some(name)) => format!("{verb} \"{name}\""),
                        (n, _) => format!("{verb} {n} items"),
                    }
                } else if completed == 0 {
                    if errors.len() == 1 {
                        errors[0].clone()
                    } else {
                        format!("{} errors — first: {}", errors.len(), errors[0])
                    }
                } else {
                    format!(
                        "{verb} {completed} item(s); {} error(s) — first: {}",
                        errors.len(),
                        errors[0]
                    )
                };

                if result_tx
                    .send(JobResult::Trash(TrashBuild {
                        token: request.token,
                        completed,
                        done: true,
                        status: Some(status),
                    }))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            shared,
            workers: vec![worker],
        }
    }

    pub(in crate::app::jobs) fn submit(&self, request: TrashRequest) -> bool {
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    /// Signal the worker to stop after the current item if it is processing
    /// the trash request with the given token.  A concurrent or future request
    /// with a different token is unaffected.
    pub(in crate::app::jobs) fn cancel_trash(&self, token: u64) {
        self.shared.cancel_token.store(token, Ordering::Relaxed);
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active
    }
}

impl Drop for TrashPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            // Do NOT clear `pending` and do NOT set `cancelled`: the worker must
            // finish any in-flight and queued requests completely before exiting.
            // Setting `cancelled` here (as PastePool does) would abandon targets
            // mid-batch and leave them neither deleted nor untouched, which is
            // worse than a momentary delay on exit.
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl TrashShared {
    fn pop(shared: &Arc<Self>) -> Option<TrashRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            // Drain any queued request before honoring the close signal so
            // that a pending job submitted just before shutdown is not lost.
            if let Some(request) = state.pending.take() {
                return Some(request);
            }
            if state.closed {
                return None;
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn set_active(shared: &Arc<Self>, active: bool) {
        lock_unpoison(&shared.state).active = active;
    }
}

fn run_trash(
    request: &TrashRequest,
    result_tx: &mpsc::Sender<JobResult>,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) -> (usize, Vec<String>, bool) {
    if request.permanent {
        run_permanent_delete(request, result_tx, cancelled, cancel_token)
    } else {
        run_trash_batch(request, cancelled, cancel_token)
    }
}

/// Delete each target permanently.
///
/// Directories are first renamed into a staging area on the same filesystem
/// (O(1), atomic), counted as completed immediately, then deleted in parallel
/// background workers with bounded concurrency.  This makes large-directory
/// deletion appear instant to the user while the actual unlink work happens
/// concurrently.  Files are removed in-place with `remove_file` as before.
///
/// Cancellation stops processing new targets but always joins all staged
/// cleanup workers before returning, so no staging entries are orphaned by
/// a clean cancel.  If the process is killed before cleanup finishes, the
/// startup sweep will reclaim leftover staging entries on next launch.
///
/// Sends throttled intermediate progress results so the UI chip updates
/// during long operations.
fn run_permanent_delete(
    request: &TrashRequest,
    result_tx: &mpsc::Sender<JobResult>,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) -> (usize, Vec<String>, bool) {
    let staging = staging_dir();
    let mut staged: Vec<(String, PathBuf)> = Vec::new();
    let mut completed = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut stopped_early = false;
    let mut last_progress_at: Option<Instant> = None;

    for target in &request.targets {
        if cancelled.load(Ordering::Relaxed)
            || cancel_token.load(Ordering::Relaxed) == request.token
        {
            stopped_early = true;
            break;
        }

        if target.is_dir {
            // Try rename-to-staging first.  If staging is unavailable or the
            // rename fails (wrong filesystem, permissions), fall back to an
            // in-place remove_dir_all.
            match staging
                .as_ref()
                .and_then(|s| rename_into_staging(&target.path, s))
            {
                Some(staged_path) => {
                    staged.push((target.name.clone(), staged_path));
                    completed += 1;
                }
                None => match fs::remove_dir_all(&target.path) {
                    Ok(()) => completed += 1,
                    Err(e) => {
                        errors.push(format!("Could not delete \"{}\": {e}", target.name));
                    }
                },
            }
        } else {
            match fs::remove_file(&target.path) {
                Ok(()) => completed += 1,
                Err(e) => errors.push(format!("Could not delete \"{}\": {e}", target.name)),
            }
        }

        if !send_trash_progress(result_tx, request.token, completed, &mut last_progress_at) {
            break;
        }
    }

    // Drain all staged directories even when stopped early — they are already
    // gone from the user's view and must be fully reclaimed before we return.
    // Any cleanup failures are surfaced as errors and the item is no longer
    // counted as completed, so the final status accurately reflects reality.
    let cleanup_errors = run_staged_cleanup(staged);
    completed = completed.saturating_sub(cleanup_errors.len());
    for name in cleanup_errors {
        errors.push(format!("Could not delete \"{name}\": cleanup failed"));
    }

    (completed, errors, stopped_early)
}

/// Returns the path used as a staging area for rename-first directory deletion.
///
/// Uses a PID-scoped subdirectory of the XDG data dir:
/// `~/.local/share/elio/cleanup/{pid}/` on Linux.  Scoping by PID means the
/// startup sweep can identify and reclaim entries from *previous* sessions by
/// checking whether the subdirectory name matches the current PID, with no
/// risk of a false positive from a file whose base name happens to embed the
/// same number.  The parent dir (`cleanup/`) is on the same filesystem as the
/// home trash so `rename(2)` never crosses a device boundary.
fn staging_dir() -> Option<PathBuf> {
    let pid = std::process::id();
    dirs::data_dir().map(|d| d.join("elio").join("cleanup").join(pid.to_string()))
}

/// Atomically moves `path` into `staging`, giving it a unique name.
/// Returns `None` if the staging directory cannot be created or the rename
/// fails (e.g. cross-device — should not happen given `staging_dir()`'s
/// placement, but handled defensively).
fn rename_into_staging(path: &Path, staging: &Path) -> Option<PathBuf> {
    fs::create_dir_all(staging).ok()?;
    let base = path.file_name().and_then(|n| n.to_str()).unwrap_or("dir");
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dest = staging.join(format!("{base}-{pid}-{nanos}"));
    fs::rename(path, &dest).ok()?;
    Some(dest)
}

/// Deletes all staged directories using a bounded worker pool.
/// Cap: `min(available_parallelism, 4)`.
///
/// Returns the original names of any directories that could not be deleted,
/// so the caller can decrement `completed` and surface them as errors.
fn run_staged_cleanup(staged: Vec<(String, PathBuf)>) -> Vec<String> {
    if staged.is_empty() {
        return Vec::new();
    }
    let cap = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(4)
        .min(staged.len());

    let (tx, rx) = mpsc::channel::<(String, PathBuf)>();
    let rx = Arc::new(Mutex::new(rx));

    // Each worker sends back the name on failure.
    let (err_tx, err_rx) = mpsc::channel::<String>();

    let workers: Vec<_> = (0..cap)
        .map(|_| {
            let rx = Arc::clone(&rx);
            let err_tx = err_tx.clone();
            thread::spawn(move || {
                while let Ok((name, path)) = rx.lock().unwrap().recv() {
                    if fs::remove_dir_all(&path).is_err() {
                        let _ = err_tx.send(name);
                    }
                }
            })
        })
        .collect();
    drop(err_tx);

    for item in staged {
        let _ = tx.send(item);
    }
    drop(tx);

    for w in workers {
        let _ = w.join();
    }

    err_rx.iter().collect()
}

/// Spawns a background thread that sweeps any PID subdirectories left in the
/// staging root from sessions that were killed before cleanup could finish.
/// Best-effort: errors are silently ignored.
pub(in crate::app::jobs) fn sweep_staging_on_startup() {
    let current_pid = std::process::id();
    let Some(cleanup_root) = dirs::data_dir().map(|d| d.join("elio").join("cleanup")) else {
        return;
    };
    // If cleanup_root/{current_pid}/ already exists at startup it must be a
    // leftover from a previous session whose PID the OS reused.  Delete it
    // synchronously now, before this session ever calls rename_into_staging,
    // so the async sweep (which skips the current-pid subdir to avoid racing
    // with live staged deletes) cannot leave it behind.
    let current_pid_dir = cleanup_root.join(current_pid.to_string());
    if current_pid_dir.exists() {
        let _ = fs::remove_dir_all(&current_pid_dir);
    }
    thread::spawn(move || sweep_staging_dir(&cleanup_root, current_pid));
}

/// Sweeps PID subdirectories inside `cleanup_root` that do not match
/// `current_pid` and whose owner process is no longer running.
///
/// Each session writes into `cleanup_root/{pid}/`.  Before deleting a
/// directory whose name is a PID other than `current_pid`, the sweep checks
/// whether a process with that PID still exists.  If it does, the directory
/// belongs to a concurrently running instance and must not be touched.  Only
/// directories whose owning process is confirmed dead are removed.
///
/// The current-pid subdir is skipped entirely: it may be populated by a
/// concurrent permanent delete in this session, and any stale dir with the
/// same PID was already removed synchronously in `sweep_staging_on_startup`.
/// Best-effort: individual entry errors are silently ignored.
fn sweep_staging_dir(cleanup_root: &Path, current_pid: u32) {
    let current_pid_str = current_pid.to_string();
    let Ok(entries) = fs::read_dir(cleanup_root) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == current_pid_str {
            continue;
        }
        // Only remove directories whose owning process is dead.  This prevents
        // one instance from deleting another live instance's staged dirs.
        if let Ok(pid) = name.parse::<u32>()
            && pid_is_alive(pid)
        {
            continue;
        }
        if p.is_dir() {
            let _ = fs::remove_dir_all(&p);
        }
    }
}

/// Returns `true` if a process with `pid` is currently running.
///
/// On Unix, `kill(pid, 0)` probes for process existence without delivering a
/// signal: it returns 0 if the process exists (even if we lack permission to
/// signal it — `EPERM` still means the process is alive).
///
/// On non-Unix platforms, conservatively returns `true` so the sweep never
/// deletes a directory it cannot safely prove is stale.  Those directories
/// will be reclaimed on a future run once the OS has recycled the PID.
#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    // SAFETY: kill(2) is async-signal-safe and has no preconditions beyond a
    // valid pid_t value.  Signal 0 never delivers a signal.
    let ret = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if ret == 0 {
        return true;
    }
    // ESRCH means no such process; any other error (e.g. EPERM) means it exists.
    // Use std::io::Error::last_os_error() to read errno portably across all
    // Unix platforms (Linux, macOS, FreeBSD) rather than a glibc-specific symbol.
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    true
}

/// Move all targets to the OS trash in a single batched call to
/// `trash::delete_all`.
///
/// This is significantly faster than per-item trashing because the `trash`
/// crate amortizes expensive setup work — reading `/proc/mounts`, resolving
/// the trash directory, checking for name collisions — across the whole
/// batch instead of repeating it for every file.
///
/// Cancellation is checked once before the batch starts.  The batch itself
/// is treated as atomic from Elio's perspective: once the OS call is in
/// flight it cannot be interrupted mid-way.  No intermediate progress
/// results are sent; the UI chip stays at 0/N until the call returns.
fn run_trash_batch(
    request: &TrashRequest,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) -> (usize, Vec<String>, bool) {
    if cancelled.load(Ordering::Relaxed) || cancel_token.load(Ordering::Relaxed) == request.token {
        return (0, Vec::new(), true);
    }

    let paths: Vec<_> = request.targets.iter().map(|t| &t.path).collect();
    let total = paths.len();

    match ::trash::delete_all(paths) {
        Ok(()) => {
            #[cfg(target_os = "macos")]
            {
                let origins: Vec<(String, std::path::PathBuf)> = request
                    .targets
                    .iter()
                    .map(|t| (t.name.clone(), t.path.clone()))
                    .collect();
                crate::fs::save_restore_origins(&origins);
            }
            (total, Vec::new(), false)
        }
        Err(e) => (0, vec![e.to_string()], false),
    }
}

/// Send a throttled intermediate progress result for the permanent-delete
/// worker.  Returns `false` if the receiver has been dropped (loop should
/// break).
fn send_trash_progress(
    result_tx: &mpsc::Sender<JobResult>,
    token: u64,
    completed: usize,
    last_progress_at: &mut Option<Instant>,
) -> bool {
    let now = Instant::now();
    let due = last_progress_at.is_none_or(|t| now.duration_since(t) >= PROGRESS_SEND_INTERVAL);
    if due {
        *last_progress_at = Some(now);
        return result_tx
            .send(JobResult::Trash(TrashBuild {
                token,
                completed,
                done: false,
                status: None,
            }))
            .is_ok();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a unique temporary directory under the system temp dir and
    /// returns its path.  The caller is responsible for cleanup (or the OS
    /// will reclaim it on reboot).  We avoid a `tempfile` dependency by using
    /// a pid+nanos unique name — the same scheme used by `rename_into_staging`.
    fn make_tmp_dir(tag: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("elio-test-{tag}-{pid}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Simulates the startup logic: delete cleanup_root/{current_pid}/ if it
    /// exists (pre-existing = stale), then run the async sweep.  Mirrors what
    /// sweep_staging_on_startup does, but against a temp root for testing.
    fn startup_sweep(cleanup_root: &Path, current_pid: u32) {
        let current_pid_dir = cleanup_root.join(current_pid.to_string());
        if current_pid_dir.exists() {
            let _ = fs::remove_dir_all(&current_pid_dir);
        }
        sweep_staging_dir(cleanup_root, current_pid);
    }

    // ── sweep_staging_dir ──────────────────────────────────────────────────
    // The cleanup root contains one subdirectory per PID, e.g.:
    //   cleanup_root/
    //     1234/   ← previous session (should be swept)
    //     5678/   ← current session  (must be skipped)

    /// Returns the PID of a process that has already exited, guaranteed to be
    /// dead (and not yet recycled since we hold the Child handle until after
    /// we call this).
    #[cfg(unix)]
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[cfg(unix)]
    #[test]
    fn sweep_removes_dead_pid_subdirectory() {
        let cleanup_root = make_tmp_dir("sweep-dead");
        let current_pid = std::process::id();
        let dead = dead_pid();

        let stale_dir = cleanup_root.join(dead.to_string());
        fs::create_dir_all(&stale_dir).unwrap();
        fs::write(stale_dir.join("inner.txt"), b"hello").unwrap();

        sweep_staging_dir(&cleanup_root, current_pid);

        assert!(!stale_dir.exists(), "dead-pid subdir should be removed");
        let _ = fs::remove_dir_all(&cleanup_root);
    }

    #[cfg(unix)]
    #[test]
    fn sweep_skips_live_other_pid_subdirectory() {
        // Simulate a concurrently running instance: spawn a child that stays
        // alive while we run the sweep, then kill it.
        let cleanup_root = make_tmp_dir("sweep-live-other");
        let current_pid = std::process::id();

        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let other_pid = child.id();

        let live_dir = cleanup_root.join(other_pid.to_string());
        fs::create_dir_all(&live_dir).unwrap();
        fs::write(live_dir.join("staged.tmp"), b"in-flight").unwrap();

        sweep_staging_dir(&cleanup_root, current_pid);
        child.kill().ok();
        child.wait().ok();

        assert!(
            live_dir.exists(),
            "live other-instance dir must not be swept"
        );
        let _ = fs::remove_dir_all(&cleanup_root);
    }

    #[test]
    fn sweep_skips_current_pid_subdirectory() {
        let cleanup_root = make_tmp_dir("sweep-skip");
        let current_pid = std::process::id();

        // Live session subdir.
        let live_dir = cleanup_root.join(current_pid.to_string());
        fs::create_dir_all(&live_dir).unwrap();
        fs::write(live_dir.join("staged.tmp"), b"live").unwrap();

        sweep_staging_dir(&cleanup_root, current_pid);

        assert!(live_dir.exists(), "current-pid subdir must not be swept");
        let _ = fs::remove_dir_all(&cleanup_root);
    }

    #[test]
    fn startup_sweep_reclaims_stale_dir_with_reused_pid() {
        // Simulate the OS reusing a PID: a previous crashed session left
        // cleanup_root/{current_pid}/ behind, and the new session starts with
        // the same PID.  The startup sweep must remove it.
        let cleanup_root = make_tmp_dir("sweep-pid-reuse");
        let current_pid = std::process::id();

        let stale_same_pid_dir = cleanup_root.join(current_pid.to_string());
        fs::create_dir_all(&stale_same_pid_dir).unwrap();
        fs::write(stale_same_pid_dir.join("leftover.tmp"), b"stale").unwrap();

        startup_sweep(&cleanup_root, current_pid);

        assert!(
            !stale_same_pid_dir.exists(),
            "stale dir with reused PID should be removed at startup"
        );
        let _ = fs::remove_dir_all(&cleanup_root);
    }

    #[test]
    fn sweep_is_no_op_for_empty_cleanup_root() {
        let cleanup_root = make_tmp_dir("sweep-empty");
        sweep_staging_dir(&cleanup_root, std::process::id());
        assert!(cleanup_root.exists());
        let _ = fs::remove_dir_all(&cleanup_root);
    }

    #[test]
    fn sweep_is_no_op_when_cleanup_root_does_not_exist() {
        let parent = make_tmp_dir("sweep-absent-parent");
        let nonexistent = parent.join("no-such-dir");
        // Should not panic.
        sweep_staging_dir(&nonexistent, std::process::id());
        let _ = fs::remove_dir_all(&parent);
    }

    // ── run_staged_cleanup ─────────────────────────────────────────────────

    #[test]
    fn staged_cleanup_succeeds_and_returns_no_errors() {
        let staging = make_tmp_dir("cleanup-ok");
        let dir = staging.join("to-delete");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("file.txt"), b"x").unwrap();

        let errors = run_staged_cleanup(vec![("to-delete".to_string(), dir.clone())]);

        assert!(errors.is_empty());
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(&staging);
    }

    #[test]
    fn staged_cleanup_returns_name_on_failure() {
        // Pass a path that does not exist — remove_dir_all returns an error.
        let parent = make_tmp_dir("cleanup-fail-parent");
        let missing = parent.join("ghost-dir");

        let errors = run_staged_cleanup(vec![("ghost-dir".to_string(), missing)]);

        assert_eq!(errors, vec!["ghost-dir"]);
        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn staged_cleanup_reports_only_failed_entries() {
        let staging = make_tmp_dir("cleanup-mixed");
        let good = staging.join("good");
        fs::create_dir_all(&good).unwrap();

        let bad = staging.join("bad-ghost");
        // `bad` deliberately never created

        let errors = run_staged_cleanup(vec![
            ("good".to_string(), good.clone()),
            ("bad-ghost".to_string(), bad),
        ]);

        assert_eq!(errors, vec!["bad-ghost"]);
        assert!(!good.exists());
        let _ = fs::remove_dir_all(&staging);
    }
}
