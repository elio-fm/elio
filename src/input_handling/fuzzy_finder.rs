use super::*;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub(crate) fn handle_search_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.close_search_overlay();
            self.status.clear();
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.close_search_overlay();
                self.status.clear();
            }
            KeyCode::Enter => self.confirm_search_selection()?,
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(search) = &mut self.overlays.search {
                    search.move_cursor_to_previous_word();
                }
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(search) = &mut self.overlays.search {
                    search.move_cursor_to_next_word();
                }
            }
            KeyCode::Left => {
                if let Some(search) = &mut self.overlays.search {
                    search.move_cursor(-1);
                }
            }
            KeyCode::Right => {
                if let Some(search) = &mut self.overlays.search {
                    search.move_cursor(1);
                }
            }
            KeyCode::Up => self.move_search_selection(-1),
            KeyCode::Down => self.move_search_selection(1),
            KeyCode::PageUp => self.page_search(-1),
            KeyCode::PageDown => self.page_search(1),
            KeyCode::Home => {
                if let Some(search) = &mut self.overlays.search {
                    search.move_cursor_to(0);
                }
            }
            KeyCode::End => {
                if let Some(search) = &mut self.overlays.search {
                    search.move_cursor_to_end();
                }
            }
            _ if search_key_deletes_previous_word(key) => {
                if let Some(search) = &mut self.overlays.search {
                    search.delete_word_before_cursor();
                }
                self.sync_search_scroll();
            }
            KeyCode::Backspace => {
                if let Some(search) = &mut self.overlays.search {
                    search.delete_char_before_cursor();
                }
                self.sync_search_scroll();
            }
            _ if search_key_deletes_next_word(key) => {
                if let Some(search) = &mut self.overlays.search {
                    search.delete_word_at_cursor();
                }
                self.sync_search_scroll();
            }
            KeyCode::Delete => {
                if let Some(search) = &mut self.overlays.search {
                    search.delete_char_at_cursor();
                }
                self.sync_search_scroll();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(search) = &mut self.overlays.search {
                    search.insert_char(ch);
                }
                self.sync_search_scroll();
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_search_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self
                    .input
                    .frame_state
                    .search_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    self.select_search_index(hit.index);
                    self.confirm_search_selection()?;
                } else if self
                    .input
                    .frame_state
                    .search_panel
                    .is_none_or(|rect| !rect_contains(rect, mouse.column, mouse.row))
                {
                    self.close_search_overlay();
                    self.status.clear();
                }
            }
            MouseEventKind::ScrollDown => self.queue_search_wheel(1),
            MouseEventKind::ScrollUp => self.queue_search_wheel(-1),
            _ => {}
        }
        Ok(())
    }

    fn page_search(&mut self, direction: isize) {
        let visible = self.input.frame_state.search_rows_visible.max(1) as isize;
        self.move_search_selection(direction * visible);
    }
}

fn search_key_deletes_previous_word(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Backspace) && key.modifiers.contains(KeyModifiers::CONTROL)
        || matches!(key.code, KeyCode::Char('h' | 'w'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
}

fn search_key_deletes_next_word(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Delete) && key.modifiers.contains(KeyModifiers::CONTROL)
        || matches!(key.code, KeyCode::Char('d'))
            && key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
}
