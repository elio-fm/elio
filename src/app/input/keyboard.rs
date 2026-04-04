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
        if self.overlays.trash.is_some() {
            return self.handle_trash_key(key);
        }

        if self.overlays.restore.is_some() {
            return self.handle_restore_key(key);
        }

        if self.overlays.create.is_some() {
            return self.handle_create_key(key);
        }

        if self.overlays.rename.is_some() {
            return self.handle_rename_key(key);
        }

        if self.overlays.bulk_rename.is_some() {
            return self.handle_bulk_rename_key(key);
        }

        if self.overlays.goto.is_some() {
            return self.handle_goto_key(key);
        }

        if self.overlays.copy.is_some() {
            return self.handle_copy_key(key);
        }

        if self.overlays.open_with.is_some() {
            return self.handle_open_with_key(key);
        }

        if self.overlays.search.is_some() {
            return self.handle_search_key(key);
        }

        if self.should_debounce_navigation_key(key) {
            return Ok(());
        }

        if self.overlays.help {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                self.overlays.help = false;
                return Ok(());
            }
            if key.code == KeyCode::Esc {
                self.overlays.help = false;
            }
            if is_help_shortcut(key) {
                self.overlays.help = false;
            }
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            if let Some(prog) = &self.jobs.trash_progress {
                self.jobs.scheduler.cancel_trash(self.jobs.trash_token);
                if prog.permanent {
                    // Permanent delete can be stopped between items; clear chip immediately.
                    self.jobs.trash_progress = None;
                }
                // Non-permanent: the batch OS call is atomic and may already be
                // in flight.  Keep the chip visible; done=true will clear it.
            } else if self.jobs.restore_progress.is_some() {
                self.jobs.scheduler.cancel_restore(self.jobs.restore_token);
                self.jobs.restore_progress = None;
            } else if self.jobs.paste_progress.is_some() {
                self.jobs.scheduler.cancel_paste(self.jobs.paste_token);
                self.jobs.paste_progress = None;
                self.clear_queued_pastes();
            } else {
                self.clear_selection();
                self.jobs.clipboard = None;
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
                _ => {}
            }
        }

        if self.input.wheel_profile == WheelProfile::HighFrequency
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
            KeyCode::Esc => {
                if let Some(prog) = &self.jobs.trash_progress {
                    self.jobs.scheduler.cancel_trash(self.jobs.trash_token);
                    if prog.permanent {
                        self.jobs.trash_progress = None;
                    }
                } else if self.jobs.restore_progress.is_some() {
                    self.jobs.scheduler.cancel_restore(self.jobs.restore_token);
                    self.jobs.restore_progress = None;
                } else if self.jobs.paste_progress.is_some() {
                    self.jobs.scheduler.cancel_paste(self.jobs.paste_token);
                    self.jobs.paste_progress = None;
                    self.clear_queued_pastes();
                } else {
                    self.clear_selection();
                    self.jobs.clipboard = None;
                }
            }
            _ if is_help_shortcut(key) => {
                self.clear_wheel_scroll();
                self.overlays.help = true;
            }
            KeyCode::Tab => self.step_sidebar_place(1)?,
            KeyCode::BackTab => self.step_sidebar_place(-1)?,
            KeyCode::Up | KeyCode::Char('k') => self.move_vertical_keyboard(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_vertical_keyboard(1),
            KeyCode::Left | KeyCode::Char('h') => {
                if self.navigation.view_mode == ViewMode::Grid {
                    self.move_by_keyboard(-1);
                } else {
                    self.go_parent()?;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.navigation.view_mode == ViewMode::Grid {
                    self.move_by_keyboard(1);
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
            KeyCode::Char('g') => self.open_goto_overlay(),
            KeyCode::Char('G') => self.select_last(),
            KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r') => self.open_selected()?,
            KeyCode::Backspace => self.go_parent()?,
            KeyCode::Char(' ') => self.toggle_selection(),
            KeyCode::Char('+') | KeyCode::Char('=')
                if self.navigation.view_mode == ViewMode::Grid =>
            {
                self.adjust_zoom(1);
            }
            KeyCode::Char('-') | KeyCode::Char('_')
                if self.navigation.view_mode == ViewMode::Grid =>
            {
                self.adjust_zoom(-1);
            }
            KeyCode::F(2) => {
                if !self.navigation.in_trash {
                    if !self.navigation.selected_paths.is_empty() {
                        self.open_bulk_rename_prompt();
                    } else {
                        self.open_rename_prompt();
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(action) = crate::config::keys().action_for(c) {
                    self.dispatch_action(action)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app) fn dispatch_action(&mut self, action: crate::config::Action) -> Result<()> {
        use crate::config::Action;
        match action {
            Action::Quit => self.should_quit = true,
            Action::Yank => self.yank(),
            Action::Cut => self.cut(),
            Action::Paste => self.paste()?,
            Action::Trash => self.open_trash_prompt(),
            Action::Create => self.open_create_prompt(),
            Action::Rename => {
                if self.navigation.in_trash {
                    self.open_restore_prompt();
                } else if !self.navigation.selected_paths.is_empty() {
                    self.open_bulk_rename_prompt();
                } else {
                    self.open_rename_prompt();
                }
            }
            Action::CopyPath => self.open_copy_overlay(),
            Action::SearchFolders => self.open_search_with_status(SearchScope::Folders),
            Action::Open => self.open_in_system()?,
            Action::OpenWith => self.open_open_with_overlay(),
            Action::Sort => self.cycle_sort_mode()?,
            Action::ToggleView => self.toggle_view_mode(),
            Action::ToggleHidden => self.toggle_hidden_files()?,
            Action::ScrollPreviewLeft => {
                let _ = self.scroll_preview_columns(-1);
            }
            Action::ScrollPreviewRight => {
                let _ = self.scroll_preview_columns(1);
            }
        }
        Ok(())
    }

    fn should_debounce_navigation_key(&mut self, key: KeyEvent) -> bool {
        let Some(navigation_key) = Self::navigation_repeat_key(key) else {
            return false;
        };

        let now = Instant::now();
        if self
            .input
            .last_navigation_key
            .is_some_and(|(previous_key, previous_at)| {
                previous_key == navigation_key
                    && now.duration_since(previous_at) < KEY_REPEAT_NAV_INTERVAL
            })
        {
            return true;
        }

        self.input.last_navigation_key = Some((navigation_key, now));
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
            KeyCode::Home => Some(NavigationRepeatKey::Home),
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
            self.dispatch_action(crate::config::Action::Open)
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
