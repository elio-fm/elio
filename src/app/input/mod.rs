mod keyboard;
mod mouse;
mod wheel;

use super::*;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(test)]
use self::wheel::ENTRY_WHEEL_TUNING;

#[cfg(test)]
mod tests;
