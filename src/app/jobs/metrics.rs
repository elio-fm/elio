use std::time::Duration;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SchedulerMetricsSnapshot {
    pub directory_jobs_submitted: u64,
    pub directory_jobs_completed: u64,
    pub search_jobs_submitted: u64,
    pub search_jobs_completed: u64,
    pub preview_jobs_submitted_high: u64,
    pub preview_jobs_submitted_low: u64,
    pub preview_jobs_completed: u64,
    pub preview_low_priority_evictions: u64,
    pub preview_promotions: u64,
    pub preview_avg_build_ms: u64,
    pub preview_max_build_ms: u64,
    pub preview_pending_high: usize,
    pub preview_pending_low: usize,
    pub preview_active: usize,
}

#[derive(Default)]
pub(super) struct SchedulerMetrics {
    pub(super) directory_jobs_submitted: u64,
    pub(super) directory_jobs_completed: u64,
    pub(super) search_jobs_submitted: u64,
    pub(super) search_jobs_completed: u64,
    pub(super) preview_jobs_submitted_high: u64,
    pub(super) preview_jobs_submitted_low: u64,
    pub(super) preview_jobs_completed: u64,
    pub(super) preview_low_priority_evictions: u64,
    pub(super) preview_promotions: u64,
    preview_total_build_time: Duration,
    preview_max_build_time: Duration,
}

impl SchedulerMetrics {
    pub(super) fn record_directory_completed(&mut self, _elapsed: Duration) {
        self.directory_jobs_completed += 1;
    }

    pub(super) fn record_search_completed(&mut self, _elapsed: Duration) {
        self.search_jobs_completed += 1;
    }

    pub(super) fn record_preview_completed(&mut self, elapsed: Duration) {
        self.preview_jobs_completed += 1;
        self.preview_total_build_time += elapsed;
        self.preview_max_build_time = self.preview_max_build_time.max(elapsed);
    }

    #[cfg(test)]
    pub(super) fn snapshot(&self) -> SchedulerMetricsSnapshot {
        let preview_avg_build_ms = if self.preview_jobs_completed == 0 {
            0
        } else {
            (self.preview_total_build_time.as_millis() as u64) / self.preview_jobs_completed
        };
        SchedulerMetricsSnapshot {
            directory_jobs_submitted: self.directory_jobs_submitted,
            directory_jobs_completed: self.directory_jobs_completed,
            search_jobs_submitted: self.search_jobs_submitted,
            search_jobs_completed: self.search_jobs_completed,
            preview_jobs_submitted_high: self.preview_jobs_submitted_high,
            preview_jobs_submitted_low: self.preview_jobs_submitted_low,
            preview_jobs_completed: self.preview_jobs_completed,
            preview_low_priority_evictions: self.preview_low_priority_evictions,
            preview_promotions: self.preview_promotions,
            preview_avg_build_ms,
            preview_max_build_ms: self.preview_max_build_time.as_millis() as u64,
            preview_pending_high: 0,
            preview_pending_low: 0,
            preview_active: 0,
        }
    }
}
