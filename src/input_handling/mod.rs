mod duplicate_finder;
mod file_operations;
mod fuzzy_finder;
mod goto;
mod keyboard;
mod local_filter;
mod mouse;
mod open_with;
pub(crate) mod text_editing;
mod wheel_scrolling;

use crate::app::*;
use crate::background_jobs::job_requests::PreviewPriority;
#[cfg(test)]
use crate::chooser::ChooserExit;
use crate::file_browser::{LocalFilter, ViewMode};
use crate::file_operations::ClipOp;
use crate::preview::PreviewRefreshMode;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[cfg(test)]
use self::{
    keyboard::KEY_REPEAT_NAV_INTERVAL,
    wheel_scrolling::{ENTRY_WHEEL_TUNING, PREVIEW_AUTO_FOCUS_DELAY},
};
#[cfg(test)]
use ratatui::layout::Rect;
#[cfg(test)]
const WHEEL_SCROLL_BURST_WINDOW: Duration = ScrollState::BURST_WINDOW;

#[cfg(test)]
mod tests;
