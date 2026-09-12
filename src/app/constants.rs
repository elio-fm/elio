use std::time::Duration;

/// If the selection changed within this window, treat keyboard navigation as
/// "rapid" and defer the preview refresh until movement pauses.  Chosen to
/// be longer than a typical deliberate keypress interval so a single
/// intentional keypress still shows a preview immediately.
pub(super) const KEY_NAV_RAPID_THRESHOLD: Duration = Duration::from_millis(250);
pub(super) const IMAGE_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(120);
pub(crate) const HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY: Duration = Duration::from_millis(140);
pub(crate) const DIRECTORY_ITEM_COUNT_IDLE_DELAY: Duration = Duration::from_millis(120);
pub(super) const DIRECTORY_STATS_IDLE_DELAY: Duration = Duration::from_millis(180);
pub(super) const PREVIEW_PREFETCH_IDLE_DELAY: Duration = Duration::from_millis(200);
pub(super) const PREVIEW_PREFETCH_LIMIT: usize = 2;
pub(super) const AUTO_RELOAD_INTERVAL_SMALL: Duration = Duration::from_millis(500);
pub(super) const AUTO_RELOAD_INTERVAL_MEDIUM: Duration = Duration::from_secs(1);
pub(super) const AUTO_RELOAD_INTERVAL_LARGE: Duration = Duration::from_secs(2);
