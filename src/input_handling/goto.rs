use crate::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub(crate) fn handle_goto_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.goto = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.overlays.goto = None;
            }
            _ => {
                if let Some(index) = crate::config::normalized_plain_key_char(key)
                    .and_then(|ch| self.goto_row_index_for_shortcut(ch))
                {
                    self.confirm_goto_index(index)?;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn handle_goto_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .screen_regions
                .goto_panel
                .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
            if !inside {
                self.overlays.goto = None;
                return Ok(());
            }

            if let Some(hit) = self
                .input
                .screen_regions
                .goto_hits
                .iter()
                .find(|hit| hit.rect.contains((mouse.column, mouse.row).into()))
                .cloned()
            {
                self.confirm_goto_index(hit.index)?;
            }
        }

        Ok(())
    }
}
