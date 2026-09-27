use super::SaveAsState;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChooserExit {
    Confirmed(Vec<PathBuf>),
    Cancelled,
}

#[derive(Default)]
pub(crate) struct ChooserState {
    enabled: bool,
    save_as: Option<SaveAsState>,
    exit: Option<ChooserExit>,
}

impl ChooserState {
    pub(crate) fn enable(&mut self) {
        self.enabled = true;
    }
    pub(crate) fn enable_save_as(&mut self, name: String) {
        self.enabled = true;
        self.save_as = Some(SaveAsState::new(name));
    }
    pub(crate) fn save_as(&self) -> Option<&SaveAsState> {
        self.save_as.as_ref()
    }
    pub(crate) fn save_as_mut(&mut self) -> Option<&mut SaveAsState> {
        self.save_as.as_mut()
    }
    pub(crate) fn is_save_as(&self) -> bool {
        self.save_as.is_some()
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn confirm_selection(
        &mut self,
        cwd: &Path,
        focused_path: Option<&Path>,
        selected_paths: Vec<PathBuf>,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        self.exit = Some(ChooserExit::Confirmed(resolve_selection(
            cwd,
            focused_path,
            selected_paths,
        )));
        true
    }

    pub(crate) fn confirm_path(&mut self, cwd: &Path, path: &Path) -> bool {
        if !self.enabled {
            return false;
        }
        self.exit = Some(ChooserExit::Confirmed(vec![absolute_path(cwd, path)]));
        true
    }

    pub(crate) fn cancel(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        self.exit = Some(ChooserExit::Cancelled);
        true
    }

    pub(crate) fn take_exit(&mut self) -> Option<ChooserExit> {
        self.exit.take()
    }

    #[cfg(test)]
    pub(crate) fn exit(&self) -> Option<&ChooserExit> {
        self.exit.as_ref()
    }
}

fn resolve_selection(
    cwd: &Path,
    focused_path: Option<&Path>,
    selected_paths: Vec<PathBuf>,
) -> Vec<PathBuf> {
    if selected_paths.is_empty() {
        return focused_path
            .map(|path| vec![absolute_path(cwd, path)])
            .unwrap_or_default();
    }

    let mut paths = selected_paths
        .iter()
        .map(|path| absolute_path(cwd, path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn absolute_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}
