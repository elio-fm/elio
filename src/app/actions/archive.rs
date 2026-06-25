use super::*;

impl App {
    pub fn archive_extract_progress(&self) -> Option<(usize, Option<usize>)> {
        self.jobs
            .archive_extract_progress
            .as_ref()
            .map(|progress| (progress.completed, progress.total))
    }

    pub(in crate::app) fn extract_focused_archive(&mut self) -> Result<()> {
        if self.jobs.archive_extract_progress.is_some() {
            self.status = "Extraction already in progress".to_string();
            return Ok(());
        }

        let Some(entry) = self.selected_entry() else {
            self.status = "Select an archive to extract".to_string();
            return Ok(());
        };
        if entry.is_dir() {
            self.status = "Select an archive to extract".to_string();
            return Ok(());
        }

        let archive_path = entry.path.clone();
        if let Err(error) = crate::archive::plan_extract(&archive_path) {
            self.status = error.to_string();
            return Ok(());
        }

        let token = self.jobs.archive_extract_token.wrapping_add(1);
        self.jobs.archive_extract_token = token;
        self.jobs.archive_extract_progress = Some(ArchiveExtractProgress {
            completed: 0,
            total: None,
        });
        self.jobs.archive_extract_source_cwd = Some(self.navigation.cwd.clone());
        self.status.clear();

        let submitted = self
            .jobs
            .scheduler
            .submit_archive_extract(ArchiveExtractRequest {
                token,
                archive_path,
            });
        if !submitted {
            self.jobs.archive_extract_progress = None;
            self.jobs.archive_extract_source_cwd = None;
            self.status = "Extraction already in progress".to_string();
        }
        Ok(())
    }
}
