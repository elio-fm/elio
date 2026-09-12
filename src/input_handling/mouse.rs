use super::*;

const HELP_WHEEL_LINES: isize = 2;
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);

impl App {
    pub(crate) fn remember_drag_candidate(&mut self, path: PathBuf) {
        self.file_browser.remember_drag_candidate(path);
    }

    #[cfg(unix)]
    pub(crate) fn clear_drag_candidate(&mut self) {
        self.file_browser.clear_drag_candidate();
    }

    pub(crate) fn clear_drag_state(&mut self) {
        self.file_browser.clear_drag_state();
    }

    pub(crate) fn suppress_drag_until_button_up(&mut self) {
        self.file_browser.suppress_drag_until_button_up();
    }

    #[cfg(any(unix, test))]
    pub(crate) fn take_drag_export_paths_at(&mut self, column: u16, row: u16) -> Vec<PathBuf> {
        let fallback_candidate = self.entry_path_at(column, row);
        self.file_browser.take_drag_export_paths(fallback_candidate)
    }

    #[cfg(any(unix, test))]
    fn entry_path_at(&self, column: u16, row: u16) -> Option<PathBuf> {
        self.input
            .screen_regions
            .entry_hits
            .iter()
            .find(|hit| hit.rect.contains((column, row).into()))
            .and_then(|hit| self.file_browser.entries.get(hit.index))
            .map(|entry| entry.path.clone())
    }

    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if self.file_operations.trash.is_some() {
            return self.handle_trash_mouse(mouse);
        }

        if self.file_operations.restore.is_some() {
            return self.handle_restore_mouse(mouse);
        }

        if self.file_operations.archive_password.is_some() {
            return self.handle_archive_password_mouse(mouse);
        }

        if self.file_operations.archive_create.is_some() {
            return self.handle_archive_create_mouse(mouse);
        }

        if self.file_operations.create.is_some() {
            return self.handle_create_mouse(mouse);
        }

        if self.file_operations.rename.is_some() {
            return self.handle_rename_mouse(mouse);
        }

        if self.file_operations.bulk_rename.is_some() {
            return self.handle_bulk_rename_mouse(mouse);
        }

        if self.file_operations.editor_rename_confirm.is_some() {
            return self.handle_editor_rename_confirm_mouse(mouse);
        }

        if self.overlays.goto.is_some() {
            return self.handle_goto_mouse(mouse);
        }

        if self.file_operations.copy.is_some() {
            return self.handle_copy_mouse(mouse);
        }

        if self.overlays.open_with.is_some() {
            return self.handle_open_with_mouse(mouse);
        }

        if self.overlays.help {
            return self.handle_help_mouse(mouse);
        }

        if self.duplicate_finder.session.is_some() {
            return self.handle_duplicate_mouse(mouse);
        }

        if self.fuzzy_finder.search.is_some() {
            return self.handle_search_mouse(mouse);
        }

        if self.preview_fullscreen() {
            match mouse.kind {
                MouseEventKind::ScrollDown => self.handle_wheel_event(mouse, 1),
                MouseEventKind::ScrollUp => self.handle_wheel_event(mouse, -1),
                MouseEventKind::ScrollLeft => self.handle_horizontal_wheel_event(mouse, -1),
                MouseEventKind::ScrollRight => self.handle_horizontal_wheel_event(mouse, 1),
                MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                    self.input.hover_panel = self.panel_target_at(mouse.column, mouse.row);
                    self.update_wheel_target_from_position(mouse.column, mouse.row);
                }
                MouseEventKind::Down(_) | MouseEventKind::Up(_) => self.clear_drag_state(),
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.clear_drag_state();
                self.update_wheel_target_from_position(mouse.column, mouse.row);
                if let Some(rect) = self.input.screen_regions.back_button
                    && rect.contains((mouse.column, mouse.row).into())
                {
                    return self.go_back();
                }
                if let Some(rect) = self.input.screen_regions.forward_button
                    && rect.contains((mouse.column, mouse.row).into())
                {
                    return self.go_forward();
                }
                if let Some(rect) = self.input.screen_regions.parent_button
                    && rect.contains((mouse.column, mouse.row).into())
                {
                    return self.go_parent();
                }
                if let Some(rect) = self.input.screen_regions.hidden_button
                    && rect.contains((mouse.column, mouse.row).into())
                {
                    self.toggle_hidden_files()?;
                    return Ok(());
                }
                if let Some(rect) = self.input.screen_regions.view_button
                    && rect.contains((mouse.column, mouse.row).into())
                {
                    self.toggle_view_mode();
                    return Ok(());
                }

                if let Some(target) = self
                    .input
                    .screen_regions
                    .sidebar_hits
                    .iter()
                    .find(|hit| hit.rect.contains((mouse.column, mouse.row).into()))
                    .cloned()
                {
                    return self.set_dir(target.path);
                }

                if let Some(hit) = self
                    .input
                    .screen_regions
                    .entry_hits
                    .iter()
                    .find(|hit| hit.rect.contains((mouse.column, mouse.row).into()))
                    .cloned()
                {
                    let Some((path, is_dir)) = self
                        .file_browser
                        .entries
                        .get(hit.index)
                        .map(|entry| (entry.path.clone(), entry.is_dir()))
                    else {
                        return Ok(());
                    };
                    self.remember_drag_candidate(path.clone());
                    self.select_index(hit.index);
                    if self.is_double_click(&path) {
                        if self.chooser_mode() && !is_dir {
                            self.confirm_chooser_path(&path);
                        } else {
                            self.open_entry_at_index(hit.index)?;
                        }
                        self.suppress_drag_until_button_up();
                    }
                    self.input.last_click = Some(ClickState {
                        path,
                        at: Instant::now(),
                    });
                }
            }
            MouseEventKind::Up(_) => {
                self.clear_drag_state();
            }
            MouseEventKind::ScrollDown => {
                self.handle_wheel_event(mouse, 1);
            }
            MouseEventKind::ScrollUp => {
                self.handle_wheel_event(mouse, -1);
            }
            MouseEventKind::ScrollLeft => {
                self.handle_horizontal_wheel_event(mouse, -1);
            }
            MouseEventKind::ScrollRight => {
                self.handle_horizontal_wheel_event(mouse, 1);
            }
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                // Track hover panel from Moved events separately. These events come from
                // ?1003h (any-event tracking) and always carry the true cursor position,
                // making hover_panel a reliable routing source when scroll event coordinates
                // are inaccurate (observed in some Alacritty/Ghostty configurations).
                self.input.hover_panel = self.panel_target_at(mouse.column, mouse.row);
                self.update_wheel_target_from_position(mouse.column, mouse.row);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_help_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.clear_wheel_scroll();
                self.overlays.help = false;
            }
            MouseEventKind::ScrollDown => {
                self.scroll_help_by(HELP_WHEEL_LINES);
            }
            MouseEventKind::ScrollUp => {
                self.scroll_help_by(-HELP_WHEEL_LINES);
            }
            _ => {}
        }
        Ok(())
    }

    fn panel_target_at(&self, column: u16, row: u16) -> Option<WheelTarget> {
        if self
            .input
            .screen_regions
            .preview_panel
            .is_some_and(|rect| rect.contains((column, row).into()))
        {
            Some(WheelTarget::Preview)
        } else if self
            .input
            .screen_regions
            .entries_panel
            .is_some_and(|rect| rect.contains((column, row).into()))
        {
            Some(WheelTarget::Entries)
        } else {
            None
        }
    }

    pub(crate) fn update_wheel_target_from_position(&mut self, column: u16, row: u16) {
        if let Some(target) = self.panel_target_at(column, row) {
            self.input.last_wheel_target = Some(target);
        }
    }

    pub(crate) fn resolve_wheel_target(&mut self, column: u16, row: u16) -> Option<WheelTarget> {
        if let Some(target) = self.panel_target_at(column, row) {
            self.input.last_wheel_target = Some(target);
            return Some(target);
        }

        if let Some(preview) = self.input.screen_regions.preview_panel
            && column >= preview.x
        {
            self.input.last_wheel_target = Some(WheelTarget::Preview);
            return self.input.last_wheel_target;
        }

        if let Some(entries) = self.input.screen_regions.entries_panel
            && column >= entries.x
            && column < entries.x.saturating_add(entries.width)
        {
            self.input.last_wheel_target = Some(WheelTarget::Entries);
            return self.input.last_wheel_target;
        }

        self.input.last_wheel_target
    }

    pub(crate) fn is_double_click(&self, path: &Path) -> bool {
        self.input
            .last_click
            .as_ref()
            .is_some_and(|click| click.path == path && click.at.elapsed() <= DOUBLE_CLICK_WINDOW)
    }
}
