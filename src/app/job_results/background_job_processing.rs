use super::super::App;
use crate::background_jobs::job_results::JobResult;
use std::time::{Duration, Instant};

const JOB_RESULT_APPLY_MAX_PER_TICK: usize = 12;
const JOB_RESULT_APPLY_TIME_BUDGET: Duration = Duration::from_millis(2);

impl App {
    pub fn process_background_jobs(&mut self) -> bool {
        let mut dirty = false;
        let started_at = Instant::now();
        let mut processed = 0usize;

        while processed < JOB_RESULT_APPLY_MAX_PER_TICK
            && started_at.elapsed() < JOB_RESULT_APPLY_TIME_BUDGET
        {
            let Ok(job) = self.job_scheduler.try_recv() else {
                break;
            };
            processed += 1;
            dirty |= match job {
                JobResult::Directory(build) => self.apply_directory_job_result(build),
                JobResult::DirectoryFingerprint(build) => {
                    self.apply_directory_fingerprint_job_result(build)
                }
                JobResult::DirectoryItemCount(build) => {
                    self.apply_directory_item_count_job_result(build)
                }
                JobResult::DirectoryStats(build) => self.apply_directory_stats_job_result(build),
                JobResult::GitStatus(build) => self.apply_git_status_job_result(build),
                JobResult::PreviewLineCount(build) => {
                    self.apply_preview_line_count_job_result(build)
                }
                JobResult::PdfProbe(build) => self.apply_pdf_probe_job_result(build),
                JobResult::PdfRender(build) => self.apply_pdf_render_job_result(build),
                JobResult::ImagePrepare(build) => self.apply_image_prepare_job_result(build),
                JobResult::SearchBatch(build) => self.apply_search_batch_job_result(build),
                JobResult::Search(build) => self.apply_search_job_result(build),
                JobResult::DuplicateScanBatch(build) => {
                    self.apply_duplicate_scan_batch_job_result(build)
                }
                JobResult::DuplicateScan(build) => self.apply_duplicate_scan_job_result(build),
                JobResult::ArchiveCreate(build) => self.apply_archive_create_job_result(build),
                JobResult::ArchiveExtract(build) => self.apply_archive_extract_job_result(build),
                JobResult::Paste(build) => self.apply_paste_job_result(build),
                JobResult::Trash(build) => self.apply_trash_job_result(build),
                JobResult::Restore(build) => self.apply_restore_job_result(build),
                JobResult::Preview(build) => self.apply_preview_job_result(build),
            };
        }

        if (processed == JOB_RESULT_APPLY_MAX_PER_TICK
            || (processed > 0 && started_at.elapsed() >= JOB_RESULT_APPLY_TIME_BUDGET))
            && let Ok(job) = self.job_scheduler.try_recv()
        {
            self.job_scheduler.defer_result(job);
        }

        dirty
    }
}
