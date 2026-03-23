use super::*;

impl App {
    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        let result = match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => Ok(()),
        };

        if let Err(error) = result {
            self.report_runtime_error("Action failed", &error);
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.trash.is_some() {
            return self.handle_trash_key(key);
        }

        if self.restore.is_some() {
            return self.handle_restore_key(key);
        }

        if self.create.is_some() {
            return self.handle_create_key(key);
        }

        if self.rename.is_some() {
            return self.handle_rename_key(key);
        }

        if self.bulk_rename.is_some() {
            return self.handle_bulk_rename_key(key);
        }

        if self.search.is_some() {
            return self.handle_search_key(key);
        }

        if self.should_debounce_navigation_key(key) {
            return Ok(());
        }

        if self.help_open {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                self.help_open = false;
                return Ok(());
            }
            if key.code == KeyCode::Esc {
                self.help_open = false;
            }
            if is_help_shortcut(key) {
                self.help_open = false;
            }
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            if let Some(prog) = &self.trash_progress {
                self.scheduler.cancel_trash(self.trash_token);
                if prog.permanent {
                    // Permanent delete can be stopped between items; clear chip immediately.
                    self.trash_progress = None;
                }
                // Non-permanent: the batch OS call is atomic and may already be
                // in flight.  Keep the chip visible; done=true will clear it.
            } else if self.paste_progress.is_some() {
                self.scheduler.cancel_paste(self.paste_token);
                self.paste_progress = None;
            } else {
                self.clear_selection();
                self.clipboard = None;
            }
            return Ok(());
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('f') => {
                    self.open_search_with_status(SearchScope::Files);
                    return Ok(());
                }
                KeyCode::Char('a') => {
                    self.select_all();
                    return Ok(());
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.adjust_zoom(1);
                    return Ok(());
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    self.adjust_zoom(-1);
                    return Ok(());
                }
                KeyCode::Char('0') => {
                    self.reset_zoom();
                    return Ok(());
                }
                _ => {}
            }
        }

        if self.wheel_profile == WheelProfile::HighFrequency
            && key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            match key.code {
                KeyCode::Left => {
                    if self.handle_horizontal_navigation_key(-1) {
                        return Ok(());
                    }
                }
                KeyCode::Right => {
                    if self.handle_horizontal_navigation_key(1) {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Left => return self.go_back(),
                KeyCode::Right => return self.go_forward(),
                _ => {}
            }
        }

        if !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Char('[') => {
                    if self.step_epub_section(-1)
                        || self.step_comic_page(-1)
                        || self.step_pdf_page(-1)
                    {
                        return Ok(());
                    }
                }
                KeyCode::Char(']') => {
                    if self.step_epub_section(1) || self.step_comic_page(1) || self.step_pdf_page(1)
                    {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if let Some(prog) = &self.trash_progress {
                    self.scheduler.cancel_trash(self.trash_token);
                    if prog.permanent {
                        self.trash_progress = None;
                    }
                } else if self.paste_progress.is_some() {
                    self.scheduler.cancel_paste(self.paste_token);
                    self.paste_progress = None;
                } else {
                    self.clear_selection();
                    self.clipboard = None;
                }
            }
            _ if is_help_shortcut(key) => {
                self.clear_wheel_scroll();
                self.help_open = true;
            }
            KeyCode::Tab => self.step_pinned_place(1)?,
            KeyCode::BackTab => self.step_pinned_place(-1)?,
            KeyCode::Up | KeyCode::Char('k') => self.move_vertical(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_vertical(1),
            KeyCode::Left | KeyCode::Char('h') => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(-1);
                } else {
                    self.go_parent()?;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(1);
                } else if self.selected_entry().is_some_and(Entry::is_dir) {
                    self.open_selected()?;
                } else {
                    self.status = "Press Enter to open files".to_string();
                }
            }
            KeyCode::PageUp => self.page(-1),
            KeyCode::PageDown => self.page(1),
            KeyCode::Home => self.select_index(0),
            KeyCode::End => self.select_last(),
            KeyCode::Char('g') => self.select_index(0),
            KeyCode::Char('G') => self.select_last(),
            KeyCode::Enter => self.open_selected()?,
            KeyCode::Backspace => self.go_parent()?,
            KeyCode::Char('v') => {
                self.toggle_view_mode();
            }
            KeyCode::Char('s') => self.cycle_sort_mode()?,
            KeyCode::Char('.') => self.toggle_hidden_files()?,
            KeyCode::Char(' ') => self.toggle_selection(),
            KeyCode::Char('a') => self.open_create_prompt(),
            KeyCode::Char('d') => self.open_trash_prompt(),
            KeyCode::Char('r') => {
                if self.in_trash {
                    self.open_restore_prompt();
                } else if !self.selected_paths.is_empty() {
                    self.open_bulk_rename_prompt();
                } else {
                    self.open_rename_prompt();
                }
            }
            KeyCode::F(2) => {
                if !self.in_trash {
                    if !self.selected_paths.is_empty() {
                        self.open_bulk_rename_prompt();
                    } else {
                        self.open_rename_prompt();
                    }
                }
            }
            KeyCode::Char('f') => self.open_search_with_status(SearchScope::Folders),
            KeyCode::Char('o') => self.open_in_system()?,
            KeyCode::Char('y') => self.yank(),
            KeyCode::Char('x') => self.cut(),
            KeyCode::Char('p') => self.paste()?,
            _ => {}
        }
        Ok(())
    }

    fn should_debounce_navigation_key(&mut self, key: KeyEvent) -> bool {
        let Some(navigation_key) = Self::navigation_repeat_key(key) else {
            return false;
        };

        let now = Instant::now();
        if self
            .last_navigation_key
            .is_some_and(|(previous_key, previous_at)| {
                previous_key == navigation_key
                    && now.duration_since(previous_at) < KEY_REPEAT_NAV_INTERVAL
            })
        {
            return true;
        }

        self.last_navigation_key = Some((navigation_key, now));
        false
    }

    fn navigation_repeat_key(key: KeyEvent) -> Option<NavigationRepeatKey> {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return None;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(NavigationRepeatKey::Up),
            KeyCode::Down | KeyCode::Char('j') => Some(NavigationRepeatKey::Down),
            KeyCode::Left | KeyCode::Char('h') => Some(NavigationRepeatKey::Left),
            KeyCode::Right | KeyCode::Char('l') => Some(NavigationRepeatKey::Right),
            KeyCode::PageUp => Some(NavigationRepeatKey::PageUp),
            KeyCode::PageDown => Some(NavigationRepeatKey::PageDown),
            KeyCode::Home | KeyCode::Char('g') => Some(NavigationRepeatKey::Home),
            KeyCode::End | KeyCode::Char('G') => Some(NavigationRepeatKey::End),
            _ => None,
        }
    }

    pub(in crate::app) fn open_selected(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };
        if entry.is_dir() {
            self.set_dir(entry.path.clone())
        } else {
            self.open_in_system()
        }
    }
}

fn is_help_shortcut(key: KeyEvent) -> bool {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return false;
    }

    matches!(key.code, KeyCode::Char('?'))
        || matches!(key.code, KeyCode::Char('/')) && key.modifiers.contains(KeyModifiers::SHIFT)
}
