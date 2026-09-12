use super::super::App;
use crate::goto_menu::{GotoDestination, build_goto_menu};
use anyhow::Result;

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

    pub(crate) fn goto_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        self.overlays
            .goto
            .as_ref()
            .and_then(|menu| menu.index_for_shortcut(ch))
    }

    pub(crate) fn confirm_goto_index(&mut self, index: usize) -> Result<()> {
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
