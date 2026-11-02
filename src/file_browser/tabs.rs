use super::FileBrowserState;
use std::path::Path;

/// The number of simultaneously open browser tabs is deliberately bounded so
/// that every tab can keep its own directory view and watcher state.
pub(crate) const MAX_TABS: usize = 10;

/// Positional tab storage for the application-owned active browser.
///
/// `App` keeps the active `FileBrowserState` directly because the rest of the
/// application is intentionally written against the active browser. The
/// active slot is represented by `None`; switching tabs swaps that state with
/// the stored state. This keeps tab ordering independent from the active tab
/// and makes a close renumbering a simple vector removal.
pub(crate) struct BrowserTabs {
    tabs: Vec<Option<FileBrowserState>>,
    active: usize,
}

impl BrowserTabs {
    pub(crate) fn new() -> Self {
        Self {
            tabs: vec![None],
            active: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active
    }

    pub(crate) fn is_active(&self, index: usize) -> bool {
        self.active == index
    }

    /// Returns the directory displayed by a tab, including the active tab
    /// whose browser state is owned by `App`.
    pub(crate) fn cwd_at<'a>(
        &'a self,
        active: &'a FileBrowserState,
        index: usize,
    ) -> Option<&'a Path> {
        if index == self.active {
            return Some(&active.cwd);
        }
        self.tabs
            .get(index)
            .and_then(Option::as_ref)
            .map(|browser| browser.cwd.as_path())
    }

    /// Returns whether a tab is at the Trash root rather than its physical
    /// storage directory.
    pub(crate) fn is_trash_at(&self, active: &FileBrowserState, index: usize) -> Option<bool> {
        if index == self.active {
            return Some(active.in_trash);
        }
        self.tabs
            .get(index)
            .and_then(Option::as_ref)
            .map(|browser| browser.in_trash)
    }

    pub(crate) fn can_open(&self) -> bool {
        self.len() < MAX_TABS
    }

    pub(crate) fn open_after_active(
        &mut self,
        active: &mut FileBrowserState,
        new: FileBrowserState,
    ) {
        debug_assert!(self.can_open());
        self.tabs[self.active] = Some(std::mem::replace(active, new));
        self.active += 1;
        self.tabs.insert(self.active, None);
    }

    pub(crate) fn activate(&mut self, active: &mut FileBrowserState, index: usize) -> bool {
        if index >= self.len() || index == self.active {
            return false;
        }
        let next = self.tabs[index]
            .take()
            .expect("inactive tab slots always contain browser state");
        self.tabs[self.active] = Some(std::mem::replace(active, next));
        self.active = index;
        true
    }

    /// Closes `index`, returning whether the active browser changed.
    pub(crate) fn close(&mut self, active: &mut FileBrowserState, index: usize) -> bool {
        if self.len() == 1 || index >= self.len() {
            return false;
        }

        if index != self.active {
            self.tabs.remove(index);
            if index < self.active {
                self.active -= 1;
            }
            return false;
        }

        let replacement = if self.active > 0 { self.active - 1 } else { 1 };
        let next = self.tabs[replacement]
            .take()
            .expect("inactive replacement tab always contains browser state");
        self.tabs.remove(self.active);
        self.active = if replacement < self.active {
            replacement
        } else {
            replacement - 1
        };
        *active = next;
        true
    }

    pub(crate) fn cycle(&mut self, active: &mut FileBrowserState, delta: isize) -> bool {
        let len = self.len();
        if len < 2 {
            return false;
        }
        let index = (self.active as isize + delta).rem_euclid(len as isize) as usize;
        self.activate(active, index)
    }
}
