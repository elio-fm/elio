use std::path::PathBuf;

pub(super) struct GitStatusState {
    token: u64,
    cwd: PathBuf,
    branch: Option<String>,
    dirty: bool,
}

impl GitStatusState {
    pub(super) fn new() -> Self {
        Self {
            token: 0,
            cwd: PathBuf::new(),
            branch: None,
            dirty: false,
        }
    }
}

impl super::FileBrowserState {
    pub(crate) fn begin_git_status_refresh(&mut self) -> (u64, PathBuf) {
        let cwd = self.cwd.clone();
        if self.git_status.cwd != cwd {
            self.git_status.cwd = cwd.clone();
            self.git_status.branch = None;
            self.git_status.dirty = false;
        }
        self.git_status.token = self.git_status.token.wrapping_add(1);
        (self.git_status.token, cwd)
    }

    pub(crate) fn apply_git_status(
        &mut self,
        token: u64,
        cwd: PathBuf,
        branch: Option<String>,
        dirty: bool,
    ) -> bool {
        if token != self.git_status.token || cwd != self.git_status.cwd {
            return false;
        }
        let changed = self.git_status.branch != branch || self.git_status.dirty != dirty;
        self.git_status.branch = branch;
        self.git_status.dirty = dirty;
        changed
    }

    pub(crate) fn git_branch(&self) -> Option<&str> {
        self.git_status.branch.as_deref()
    }

    pub(crate) fn git_dirty(&self) -> bool {
        self.git_status.dirty
    }

    #[cfg(test)]
    pub(crate) fn set_git_branch_for_test(&mut self, branch: Option<&str>) {
        self.git_status.branch = branch.map(str::to_string);
    }

    #[cfg(test)]
    pub(crate) fn set_git_dirty_for_test(&mut self, dirty: bool) {
        self.git_status.dirty = dirty;
    }
}
