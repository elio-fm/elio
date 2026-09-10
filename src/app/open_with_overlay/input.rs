use super::super::App;
use crate::{config::Action, fs::rect_contains};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub(in crate::app) fn handle_open_with_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.open_with = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.overlays.open_with = None;
            }
            _ => match crate::config::keys().action_for_key_in_context(key, self.key_context()) {
                Some(Action::NavDown) => self.move_open_with_selection(1),
                Some(Action::NavUp) => self.move_open_with_selection(-1),
                Some(Action::OpenOrEnter) => self.confirm_selected_open_with_row()?,
                _ => match key.code {
                    KeyCode::Char(ch)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        if let Some(index) = self.open_with_row_index_for_shortcut(ch) {
                            self.confirm_open_with_index(index)?;
                        }
                    }
                    _ => {}
                },
            },
        }

        Ok(())
    }

    pub(in crate::app) fn handle_open_with_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .frame_state
                    .open_with_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.overlays.open_with = None;
                    return Ok(());
                }

                if let Some(hit) = self
                    .input
                    .frame_state
                    .open_with_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    self.confirm_open_with_index(hit.index)?;
                }
            }
            MouseEventKind::ScrollDown => self.move_open_with_selection(1),
            MouseEventKind::ScrollUp => self.move_open_with_selection(-1),
            _ => {}
        }

        Ok(())
    }

    fn open_with_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        let needle = ch.to_ascii_lowercase();
        self.overlays.open_with.as_ref().and_then(|overlay| {
            overlay.rows.iter().position(|row| {
                row.shortcut
                    .is_some_and(|shortcut| shortcut.to_ascii_lowercase() == needle)
            })
        })
    }
}
