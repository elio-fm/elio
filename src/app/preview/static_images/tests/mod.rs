use super::*;
use crate::file_browser::ViewMode;
use crate::filesystem::{Entry, EntryKind};
use crate::preview::OverlayPresentState;
use crate::terminal_images::{
    ImageProtocol, RenderedImageDimensions, TerminalIdentity, TerminalWindowSize,
};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use ratatui::{buffer::Buffer, layout::Rect};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod helpers;
mod popup_interactions;
mod preloading;
mod preparation;
mod presentation;
mod preview_selection;
mod protocols;
mod resizing;

use helpers::*;
