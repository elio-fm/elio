use super::super::App;
use crate::{
    fs::rect_contains,
    goto_menu::{GotoDestination, build_goto_menu},
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub fn goto_is_open(&self) -> bool {
        self.overlays.goto.is_some()
    }

    pub fn goto_title(&self) -> &str {
        self.overlays
            .goto
            .as_ref()
            .map(|menu| menu.title())
            .unwrap_or("")
    }

    pub fn goto_row_count(&self) -> usize {
        self.overlays
            .goto
            .as_ref()
            .map_or(0, crate::goto_menu::GotoMenu::len)
    }

    pub fn goto_row_label(&self, index: usize) -> &str {
        self.overlays
            .goto
            .as_ref()
            .and_then(|menu| menu.entry(index))
            .map(crate::goto_menu::GotoMenuEntry::label)
            .unwrap_or("")
    }

    pub fn goto_row_shortcut(&self, index: usize) -> Option<char> {
        self.overlays
            .goto
            .as_ref()
            .and_then(|menu| menu.entry(index))
            .map(crate::goto_menu::GotoMenuEntry::shortcut)
    }
}

impl App {
    pub(crate) fn open_goto_overlay(&mut self) {
        self.overlays.help = false;
        self.overlays.goto = Some(build_goto_menu(
            &crate::config::goto().entries,
            &self.places.rows,
        ));
        self.status.clear();
    }

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
                .frame_state
                .goto_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.overlays.goto = None;
                return Ok(());
            }

            if let Some(hit) = self
                .input
                .frame_state
                .goto_hits
                .iter()
                .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                .cloned()
            {
                self.confirm_goto_index(hit.index)?;
            }
        }

        Ok(())
    }

    fn goto_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        self.overlays
            .goto
            .as_ref()
            .and_then(|menu| menu.index_for_shortcut(ch))
    }

    fn confirm_goto_index(&mut self, index: usize) -> Result<()> {
        let Some(destination) = self
            .overlays
            .goto
            .as_ref()
            .and_then(|menu| menu.destination(index))
        else {
            return Ok(());
        };

        match destination {
            GotoDestination::Top => {
                self.overlays.goto = None;
                self.select_index(0);
            }
            GotoDestination::Path(path) => {
                self.overlays.goto = None;
                self.set_dir(path)?;
            }
            GotoDestination::Missing(status) => {
                self.status = status;
            }
        }

        Ok(())
    }
}
