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
/// Prevents a per-file result flood from turning into a constant redraw storm
/// when pasting or trashing large numbers of small files.
const PROGRESS_SEND_INTERVAL: Duration = Duration::from_millis(80);

pub(in crate::app::jobs) struct PastePool {
    shared: Arc<PasteShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct PasteShared {
    state: Mutex<PasteState>,
    available: Condvar,
    /// Set by `Drop` — signals the worker to stop for shutdown.
    cancelled: AtomicBool,
    /// Token of the paste the user explicitly cancelled.  The worker stops
    /// when its request token matches this value.  A new paste carries a
    /// different token so it is never accidentally cancelled by an older
    /// cancellation request — no reset needed on new submits.
    cancel_token: AtomicU64,
}

struct PasteState {
    pending: Option<PasteRequest>,
    active: bool,
    closed: bool,
}

impl PastePool {
    pub(in crate::app::jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(PasteShared {
            state: Mutex::new(PasteState {
                pending: None,
                active: false,
                closed: false,
            }),
            available: Condvar::new(),
            cancelled: AtomicBool::new(false),
            cancel_token: AtomicU64::new(0), // 0 = "nothing cancelled" (tokens start at 1)
        });
        let shared_worker = Arc::clone(&shared);
        let worker = thread::spawn(move || {
            while let Some(request) = PasteShared::pop(&shared_worker) {
                PasteShared::set_active(&shared_worker, true);
                let (completed, already_here, errors, stopped_early, destination_paths) = run_paste(
                    &request,
                    &result_tx,
                    &shared_worker.cancelled,
                    &shared_worker.cancel_token,
                );
                PasteShared::set_active(&shared_worker, false);

                let verb = match request.op {
                    ClipOp::Yank => "Copied",
                    ClipOp::Cut => "Moved",
                };
                let status = if stopped_early {
                    match completed {
                        0 => "Paste cancelled".to_string(),
                        1 => format!("Paste cancelled — {verb} 1 item"),
                        n => format!("Paste cancelled — {verb} {n} items"),
                    }
                } else if errors.is_empty() {
                    match (completed, already_here) {
                        (0, 0) => "Nothing was pasted".to_string(),
                        (0, 1) => "Already here".to_string(),
                        (0, n) => format!("{n} items already here"),
                        (1, 0) => format!("{verb} 1 item"),
                        (n, 0) => format!("{verb} {n} items"),
                        (1, 1) => format!("{verb} 1 item; 1 already here"),
                        (1, n) => format!("{verb} 1 item; {n} already here"),
                        (n, 1) => format!("{verb} {n} items; 1 already here"),
                        (n, skipped) => format!("{verb} {n} items; {skipped} already here"),
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
                    .send(JobResult::Paste(PasteBuild {
                        token: request.token,
                        completed,
                        done: true,
                        status: Some(status),
                        destination_paths,
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

    pub(in crate::app::jobs) fn submit(&self, request: PasteRequest) -> bool {
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    /// Signal the worker to stop after the current item if it is processing
    /// the paste with the given token.  A concurrent or future paste with a
    /// different token is unaffected.
    pub(in crate::app::jobs) fn cancel_paste(&self, token: u64) {
        self.shared.cancel_token.store(token, Ordering::Relaxed);
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active
    }
}

impl Drop for PastePool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending = None;
        }
        self.shared.cancelled.store(true, Ordering::Relaxed);
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl PasteShared {
    fn pop(shared: &Arc<Self>) -> Option<PasteRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.take() {
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn set_active(shared: &Arc<Self>, active: bool) {
        lock_unpoison(&shared.state).active = active;
    }
}

/// Execute the paste operation, sending throttled intermediate progress
/// results through `result_tx`. Returns completed count, errors, cancellation
/// state, skipped same-location moves, and destination paths actually written.
///
/// `stopped_early` is `true` if the loop was cut short by a cancel flag rather
/// than running to completion.
fn run_paste(
    request: &PasteRequest,
    result_tx: &mpsc::Sender<JobResult>,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) -> (usize, usize, Vec<String>, bool, Vec<PathBuf>) {
    let mut completed = 0usize;
    let mut already_here = 0usize;
    let mut destination_paths = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut stopped_early = false;
    // Tracks when we last sent a progress result.  None = never sent, which
    // causes the first update to go through immediately.
    let mut last_progress_at: Option<Instant> = None;

    for src in &request.paths {
        if cancelled.load(Ordering::Relaxed)
            || cancel_token.load(Ordering::Relaxed) == request.token
        {
            stopped_early = true;
            break;
        }
        let Some(file_name) = src.file_name().and_then(|n| n.to_str()) else {
            errors.push(format!("Cannot determine name for {}", src.display()));
            if !send_paste_progress(result_tx, request.token, completed, &mut last_progress_at) {
                break;
            }
            continue;
        };

        if !src.exists() {
            errors.push(format!("\"{}\" no longer exists", file_name));
            if !send_paste_progress(result_tx, request.token, completed, &mut last_progress_at) {
                break;
            }
            continue;
        }

        // For cut: same-dir same-name is a no-op, not a move.
        if request.op == ClipOp::Cut {
            let natural = request.dest_dir.join(file_name);
            if natural == *src {
                already_here += 1;
                if !send_paste_progress(result_tx, request.token, completed, &mut last_progress_at)
                {
                    break;
                }
                continue;
            }
        }

        let dest = unique_dest(&request.dest_dir, file_name);

        if source_contains_destination(src, &dest) {
            errors.push(format!("\"{}\" cannot be pasted into itself", file_name));
            if !send_paste_progress(result_tx, request.token, completed, &mut last_progress_at) {
                break;
            }
            continue;
        }

        let ok = match request.op {
            ClipOp::Yank => match copy_recursive(src, &dest) {
                Ok(()) => true,
                Err(e) => {
                    errors.push(format!("\"{}\" could not be copied: {e}", file_name));
                    false
                }
            },
            ClipOp::Cut => match fs::rename(src, &dest) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                    match copy_recursive(src, &dest) {
                        Ok(()) => {
                            let del = if src.is_dir() {
                                fs::remove_dir_all(src)
                            } else {
                                fs::remove_file(src)
                            };
                            if let Err(de) = del {
                                errors.push(format!(
                                    "\"{}\" was copied but source could not be removed: {de}",
                                    file_name
                                ));
                            }
                            true
                        }
                        Err(ce) => {
                            let _ = if dest.is_dir() {
                                fs::remove_dir_all(&dest)
                            } else {
                                fs::remove_file(&dest)
                            };
                            errors.push(format!("\"{}\" could not be moved: {ce}", file_name));
                            false
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("\"{}\" could not be moved: {e}", file_name));
                    false
                }
            },
        };

        if ok {
            completed += 1;
            destination_paths.push(dest);
        }

        if !send_paste_progress(result_tx, request.token, completed, &mut last_progress_at) {
            break;
        }
    }

    (
        completed,
        already_here,
        errors,
        stopped_early,
        destination_paths,
    )
}

/// Send a throttled intermediate progress result for the paste worker.
/// Returns `false` if the receiver has been dropped (loop should break).
fn send_paste_progress(
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
            .send(JobResult::Paste(PasteBuild {
                token,
                completed,
                done: false,
                status: None,
                destination_paths: Vec::new(),
            }))
            .is_ok();
    }
    true
}

/// Return a destination path inside `dir` for an item named `name` that does
/// not collide with any existing file.
fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let base = Path::new(name);
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = base.extension().and_then(|s| s.to_str());
    next_literal_dest(dir, stem, ext)
}

fn next_literal_dest(dir: &Path, stem: &str, ext: Option<&str>) -> PathBuf {
    for i in 1u32.. {
        let path = dir.join(format_copy_name(stem, ext, Some(i)));
        if !path.exists() {
            return path;
        }
    }
    dir.join(format_copy_name(stem, ext, None))
}

fn format_copy_name(stem: &str, ext: Option<&str>, suffix: Option<u32>) -> String {
    let name = match suffix {
        Some(index) => format!("{stem}_{index}"),
        None => stem.to_string(),
    };
    match ext {
        Some(ext) => format!("{name}.{ext}"),
        None => name,
    }
}

/// Recursively copy `src` to `dest`.
fn copy_recursive(src: &Path, dest: &Path) -> anyhow::Result<()> {
    if source_contains_destination(src, dest) {
        anyhow::bail!("Cannot paste a folder into itself");
    }
    if src.is_dir() {
        fs::create_dir_all(dest)
            .map_err(|e| anyhow::anyhow!("Cannot create directory \"{}\": {e}", dest.display()))?;
        for entry_result in fs::read_dir(src)
            .map_err(|e| anyhow::anyhow!("Cannot read \"{}\": {e}", src.display()))?
        {
            let child = entry_result
                .map_err(|e| anyhow::anyhow!("Cannot read entry in \"{}\": {e}", src.display()))?;
            copy_recursive(&child.path(), &dest.join(child.file_name()))?;
        }
    } else {
        fs::copy(src, dest).map_err(|e| {
            anyhow::anyhow!(
                "Cannot copy \"{}\" to \"{}\": {e}",
                src.display(),
                dest.display()
            )
        })?;
    }
    Ok(())
}

fn source_contains_destination(src: &Path, dest: &Path) -> bool {
    src.is_dir() && dest.starts_with(src)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-paste-{label}-{unique}"))
    }

    #[test]
    fn duplicate_plain_name_uses_underscore_suffixes() {
        let dir = temp_path("plain");
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        fs::write(dir.join("example"), "data").expect("failed to write source");

        assert_eq!(unique_dest(&dir, "example"), dir.join("example_1"));

        fs::remove_dir_all(&dir).expect("failed to remove temp dir");
    }

    #[test]
    fn duplicate_original_skips_to_next_available_suffix() {
        let dir = temp_path("next-available");
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        fs::write(dir.join("example"), "base").expect("failed to write base file");
        fs::write(dir.join("example_1"), "copy").expect("failed to write suffixed file");

        assert_eq!(unique_dest(&dir, "example"), dir.join("example_2"));

        fs::remove_dir_all(&dir).expect("failed to remove temp dir");
    }

    #[test]
    fn duplicate_suffixed_name_stays_literal() {
        let dir = temp_path("nested-suffix");
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        fs::write(dir.join("aur_1"), "report").expect("failed to write source");

        assert_eq!(unique_dest(&dir, "aur_1"), dir.join("aur_1_1"));

        fs::remove_dir_all(&dir).expect("failed to remove temp dir");
    }

    #[test]
    fn duplicate_suffixed_name_with_extension_stays_literal() {
        let dir = temp_path("nested-suffix-ext");
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        fs::write(dir.join("aur_1.txt"), "copy").expect("failed to write source");

        assert_eq!(unique_dest(&dir, "aur_1.txt"), dir.join("aur_1_1.txt"));

        fs::remove_dir_all(&dir).expect("failed to remove temp dir");
    }

    #[test]
    fn copy_recursive_refuses_directory_into_itself() {
        let root = temp_path("copy-into-self");
        let source = root.join("source");
        let dest = source.join("source");
        fs::create_dir_all(&source).expect("failed to create source dir");
        fs::write(source.join("file.txt"), "data").expect("failed to write source file");

        let error = copy_recursive(&source, &dest).expect_err("copy should be rejected");

        assert!(
            error
                .to_string()
                .contains("Cannot paste a folder into itself"),
            "unexpected error: {error}"
        );
        assert!(!dest.exists());

        fs::remove_dir_all(&root).expect("failed to remove temp root");
    }

    #[test]
    fn source_contains_destination_only_blocks_directory_descendants() {
        let root = temp_path("source-dest-check");
        let source_dir = root.join("source");
        let source_file = root.join("file.txt");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        fs::write(&source_file, "data").expect("failed to write source file");

        assert!(source_contains_destination(
            &source_dir,
            &source_dir.join("child")
        ));
        assert!(!source_contains_destination(
            &source_dir,
            &root.join("source_copy")
        ));
        assert!(!source_contains_destination(
            &source_file,
            &source_file.join("child")
        ));

        fs::remove_dir_all(&root).expect("failed to remove temp root");
    }
}
