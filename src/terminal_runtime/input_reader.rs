use std::{io, time::Duration};

use anyhow::Result;
use crossterm::event::{self, Event};

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

    pub(super) fn read(&self, timeout: Duration) -> Result<Option<InputEvent>> {
        match self {
            Self::Crossterm => {
                if event::poll(timeout)? {
                    Ok(Some(InputEvent::Terminal(event::read()?)))
                } else {
                    Ok(None)
                }
            }
            #[cfg(unix)]
            Self::KittyDnd(reader) => Ok(reader.recv_timeout(timeout)?),
        }
    }

    pub(super) fn try_read(&self) -> Result<Option<InputEvent>> {
        match self {
            Self::Crossterm => {
                if event::poll(Duration::ZERO)? {
                    Ok(Some(InputEvent::Terminal(event::read()?)))
                } else {
                    Ok(None)
                }
            }
            #[cfg(unix)]
            Self::KittyDnd(reader) => Ok(reader.try_recv()?),
        }
    }

    pub(super) fn set_paused(&self, paused: bool) {
        #[cfg(unix)]
        if let Self::KittyDnd(reader) = self {
            reader.set_paused(paused);
        }
        #[cfg(not(unix))]
        let _ = paused;
    }

    pub(super) fn coalesce_resizes(
        &self,
        input: InputEvent,
        next_input: &mut Option<InputEvent>,
    ) -> Result<InputEvent> {
        let InputEvent::Terminal(Event::Resize(mut width, mut height)) = input else {
            return Ok(input);
        };

        loop {
            let candidate = if let Some(input) = next_input.take() {
                Some(input)
            } else {
                self.try_read()?
            };
            match candidate {
                Some(InputEvent::Terminal(Event::Resize(w, h))) => {
                    width = w;
                    height = h;
                }
                Some(other) => {
                    *next_input = Some(other);
                    break;
                }
                None => break,
            }
        }

        Ok(InputEvent::Terminal(Event::Resize(width, height)))
    }
}
