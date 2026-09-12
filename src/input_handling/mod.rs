mod duplicate_finder;
mod fuzzy_finder;
mod goto;
mod keyboard;
mod local_filter;
mod mouse;
mod open_with;
pub(crate) mod text_editing;
mod wheel_scrolling;

use crate::app::*;
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
