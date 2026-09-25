use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

const DIRECTORY_WATCH_DEBOUNCE: Duration = Duration::from_millis(45);

pub(crate) type DirectoryWatcher = RecommendedWatcher;

#[derive(Clone, Debug)]
pub(crate) enum DirectoryWatchEvent {
    Changed(Vec<PathBuf>),
    Rescan,
}

pub(crate) fn directory_watch_debounce() -> Duration {
    DIRECTORY_WATCH_DEBOUNCE
}

pub(crate) fn start_directory_watcher(
    path: &Path,
    tx: &mpsc::Sender<DirectoryWatchEvent>,
) -> notify::Result<DirectoryWatcher> {
    let event_tx = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |result: notify::Result<Event>| match result {
            Ok(event) => {
                if !should_schedule_reload(&event) {
                    return;
                }
                let message = if event.paths.is_empty() {
                    DirectoryWatchEvent::Rescan
                } else {
                    DirectoryWatchEvent::Changed(event.paths)
                };
                let _ = event_tx.send(message);
            }
            Err(_) => {
                let _ = event_tx.send(DirectoryWatchEvent::Rescan);
            }
        },
        Config::default(),
    )?;
    watcher.watch(path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

// Watch the directory, not HEAD itself: Git replaces HEAD by renaming HEAD.lock.
pub(crate) fn start_git_head_watcher(
    git_dir: &Path,
    mut changed: impl FnMut() + Send + 'static,
) -> notify::Result<DirectoryWatcher> {
    let head = git_dir.join("HEAD");
    let coarse = matches!(
        RecommendedWatcher::kind(),
        notify::WatcherKind::Kqueue | notify::WatcherKind::Fsevent
    );
    let mut previous = if coarse {
        std::fs::read(&head).ok()
    } else {
        None
    };
    let mut watcher = RecommendedWatcher::new(
        move |result: notify::Result<Event>| match result {
            Ok(event) if git_head_changed(&head, &event, coarse, &mut previous) => {
                changed();
            }
            // A backend error warrants one refresh, never recurring Git polling.
            Err(_) => changed(),
            _ => {}
        },
        Config::default(),
    )?;
    watcher.watch(git_dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

fn git_head_changed(
    head: &Path,
    event: &Event,
    coarse: bool,
    previous: &mut Option<Vec<u8>>,
) -> bool {
    if !should_schedule_reload(event) {
        return false;
    }
    if coarse {
        // Kqueue's nonrecursive directory watch may name the directory or an
        // arbitrary previously unwatched child, not HEAD. Compare its contents
        // on events only: unrelated metadata writes must not spawn Git jobs.
        // Keep watching the parent so repeated atomic replacements still work.
        let current = std::fs::read(head).ok();
        if current == *previous {
            return false;
        }
        *previous = current;
        return true;
    }
    event.need_rescan() || event.paths.is_empty() || event.paths.iter().any(|path| path == head)
}

pub(crate) fn event_affects_visible_entries(paths: &[PathBuf], show_hidden: bool) -> bool {
    show_hidden
        || paths.is_empty()
        || paths.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| !name.starts_with('.'))
                .unwrap_or(true)
        })
}

fn should_schedule_reload(event: &Event) -> bool {
    !matches!(event.kind, EventKind::Access(_))
}

#[cfg(test)]
#[path = "tests/directory_watching.rs"]
mod tests;
