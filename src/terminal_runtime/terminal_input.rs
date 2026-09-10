use std::io;

use crossterm::event::Event;

#[cfg(unix)]
use super::kitty_dnd;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InputEvent {
    Terminal(Event),
    #[cfg(unix)]
    KittyDnd(kitty_dnd::KittyDndEvent),
}

pub(super) enum InputReader {
    Crossterm,
    #[cfg(unix)]
    KittyDnd(kitty_dnd::InputReader),
}

impl InputReader {
    pub(super) fn new(use_kitty_dnd_input: bool) -> io::Result<Self> {
        if use_kitty_dnd_input {
            #[cfg(unix)]
            {
                kitty_dnd::InputReader::spawn().map(Self::KittyDnd)
            }
            #[cfg(not(unix))]
            {
                Ok(Self::Crossterm)
            }
        } else {
            Ok(Self::Crossterm)
        }
    }
}
