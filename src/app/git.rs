use super::App;
use crate::background_jobs::{job_requests::GitStatusRequest, job_results::GitStatusBuild};

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
