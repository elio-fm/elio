const PREVIEW_WORKER_COUNT: usize = 2;
const SEARCH_WORKER_COUNT: usize = 1;
const DIRECTORY_ITEM_COUNT_WORKER_COUNT: usize = 1;
const DIRECTORY_STATS_WORKER_COUNT: usize = 1;
const DIRECTORY_FINGERPRINT_WORKER_COUNT: usize = 1;
const PREVIEW_LINE_COUNT_WORKER_COUNT: usize = 1;
const IMAGE_PREPARE_WORKER_COUNT: usize = 3;
const PDF_PROBE_WORKER_COUNT: usize = 2;
const PDF_RENDER_WORKER_COUNT: usize = 2;
const PREVIEW_QUEUE_LIMIT: usize = 8;
const DIRECTORY_ITEM_COUNT_QUEUE_LIMIT: usize = 48;
const PREVIEW_LINE_COUNT_QUEUE_LIMIT: usize = 16;
const IMAGE_PREPARE_QUEUE_LIMIT: usize = 16;
const PDF_PROBE_QUEUE_LIMIT: usize = 16;
const PDF_RENDER_QUEUE_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug)]
pub(super) struct SchedulerConfig {
    pub(super) search_worker_count: usize,
    pub(super) preview_worker_count: usize,
    pub(super) preview_queue_limit: usize,
    pub(super) directory_item_count_worker_count: usize,
    pub(super) directory_item_count_queue_limit: usize,
    pub(super) directory_fingerprint_worker_count: usize,
    pub(super) preview_line_count_worker_count: usize,
    pub(super) preview_line_count_queue_limit: usize,
    pub(super) image_prepare_worker_count: usize,
    pub(super) image_prepare_queue_limit: usize,
    pub(super) pdf_probe_worker_count: usize,
    pub(super) pdf_probe_queue_limit: usize,
    pub(super) pdf_render_worker_count: usize,
    pub(super) pdf_render_queue_limit: usize,
}

impl SchedulerConfig {
    pub(super) fn production() -> Self {
        Self {
            search_worker_count: SEARCH_WORKER_COUNT,
            preview_worker_count: PREVIEW_WORKER_COUNT,
            preview_queue_limit: PREVIEW_QUEUE_LIMIT,
            directory_item_count_worker_count: DIRECTORY_ITEM_COUNT_WORKER_COUNT,
            directory_item_count_queue_limit: DIRECTORY_ITEM_COUNT_QUEUE_LIMIT,
            directory_fingerprint_worker_count: DIRECTORY_FINGERPRINT_WORKER_COUNT,
            preview_line_count_worker_count: PREVIEW_LINE_COUNT_WORKER_COUNT,
            preview_line_count_queue_limit: PREVIEW_LINE_COUNT_QUEUE_LIMIT,
            image_prepare_worker_count: IMAGE_PREPARE_WORKER_COUNT,
            image_prepare_queue_limit: IMAGE_PREPARE_QUEUE_LIMIT,
            pdf_probe_worker_count: PDF_PROBE_WORKER_COUNT,
            pdf_probe_queue_limit: PDF_PROBE_QUEUE_LIMIT,
            pdf_render_worker_count: PDF_RENDER_WORKER_COUNT,
            pdf_render_queue_limit: PDF_RENDER_QUEUE_LIMIT,
        }
    }

    #[cfg(test)]
    pub(super) fn for_tests(
        search_worker_count: usize,
        preview_worker_count: usize,
        preview_queue_limit: usize,
    ) -> Self {
        Self {
            search_worker_count,
            preview_worker_count,
            preview_queue_limit,
            directory_item_count_worker_count: DIRECTORY_ITEM_COUNT_WORKER_COUNT,
            directory_item_count_queue_limit: DIRECTORY_ITEM_COUNT_QUEUE_LIMIT,
            directory_fingerprint_worker_count: 0,
            preview_line_count_worker_count: 0,
            preview_line_count_queue_limit: PREVIEW_LINE_COUNT_QUEUE_LIMIT,
            image_prepare_worker_count: 0,
            image_prepare_queue_limit: IMAGE_PREPARE_QUEUE_LIMIT,
            pdf_probe_worker_count: 0,
            pdf_probe_queue_limit: PDF_PROBE_QUEUE_LIMIT,
            pdf_render_worker_count: 0,
            pdf_render_queue_limit: PDF_RENDER_QUEUE_LIMIT,
        }
    }

    pub(super) const fn directory_stats_worker_count(self) -> usize {
        DIRECTORY_STATS_WORKER_COUNT
    }
}
