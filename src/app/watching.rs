use super::*;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

const DIRECTORY_WATCH_DEBOUNCE: Duration = Duration::from_millis(45);

pub(super) type DirectoryWatcher = RecommendedWatcher;

#[derive(Clone, Debug)]
pub(super) enum DirectoryWatchEvent {
    Changed(Vec<PathBuf>),
    Rescan,
}

pub(super) fn directory_watch_debounce() -> Duration {
    DIRECTORY_WATCH_DEBOUNCE
}

pub(super) fn start_directory_watcher(
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

pub(super) fn event_affects_visible_entries(paths: &[PathBuf], show_hidden: bool) -> bool {
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
mod tests {
    use super::*;

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
}
