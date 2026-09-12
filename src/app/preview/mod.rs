pub(super) mod comic;
pub(super) mod epub;
pub(crate) mod pdf;
mod prefetch;
mod preview_pane;
mod refresh;
mod requests;
pub(crate) mod static_images;
pub(super) mod terminal_image_previews;
mod visual_layout;

use super::*;
use crate::preview::{
    PreviewDirectoryStatsState, PreviewLineCountKey, PreviewLoadState, PreviewRefreshMode,
};
use std::time::Duration;

pub(super) const IMAGE_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(120);
pub(crate) const HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY: Duration = Duration::from_millis(140);

#[cfg(test)]
mod tests;
