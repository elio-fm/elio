use std::time::Duration;

pub(super) const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);
pub(super) const KEY_REPEAT_NAV_INTERVAL: Duration = Duration::from_millis(28);
/// If the selection changed within this window, treat keyboard navigation as
/// "rapid" and defer the preview refresh until movement pauses.  Chosen to
/// be longer than a typical deliberate keypress interval so a single
/// intentional keypress still shows a preview immediately.
pub(super) const KEY_NAV_RAPID_THRESHOLD: Duration = Duration::from_millis(250);
pub(super) const WHEEL_SCROLL_INTERVAL_HORIZONTAL: Duration = Duration::from_millis(64);
pub(super) const WHEEL_SCROLL_INTERVAL_VERTICAL: Duration = Duration::from_millis(16);
pub(super) const WHEEL_SCROLL_INTERVAL_VERTICAL_HIGH_FREQUENCY: Duration =
    Duration::from_millis(12);
pub(super) const WHEEL_SCROLL_INTERVAL_PREVIEW: Duration = Duration::from_millis(12);
pub(super) const WHEEL_SCROLL_INTERVAL_PREVIEW_HORIZONTAL: Duration = Duration::from_millis(12);
pub(super) const WHEEL_SCROLL_INTERVAL_SEARCH: Duration = Duration::from_millis(72);
pub(super) const PREVIEW_AUTO_FOCUS_DELAY: Duration = Duration::from_millis(220);
pub(super) const IMAGE_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(120);
pub(super) const HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY: Duration = Duration::from_millis(140);
pub(super) const PREVIEW_PREFETCH_IDLE_DELAY: Duration = Duration::from_millis(200);
pub(super) const WHEEL_SCROLL_QUEUE_LIMIT: isize = 8;
pub(super) const WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL: isize = 3;
pub(super) const WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL: isize = 10;
pub(super) const WHEEL_SCROLL_QUEUE_LIMIT_SEARCH: isize = 2;
pub(super) const WHEEL_SCROLL_BURST_WINDOW: Duration = Duration::from_millis(150);
pub(super) const SEARCH_MATCH_LIMIT: usize = 250;
pub(super) const SEARCH_CACHE_LIMIT: usize = 32;
pub(super) const PREVIEW_CACHE_LIMIT: usize = 24;
pub(super) const PREVIEW_LINE_COUNT_CACHE_LIMIT: usize = 64;
pub(super) const PREVIEW_PREFETCH_LIMIT: usize = 2;
pub(super) const DIRECTORY_ITEM_COUNT_CACHE_LIMIT: usize = 128;
pub(super) const AUTO_RELOAD_INTERVAL: Duration = Duration::from_millis(250);
pub(super) const INCREMENTAL_RENDER_LOOKAHEAD: usize = 80;
