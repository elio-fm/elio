use super::*;

impl App {
    pub(in crate::app) fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if self.overlays.trash.is_some() {
            return self.handle_trash_mouse(mouse);
        }

        if self.overlays.restore.is_some() {
            return self.handle_restore_mouse(mouse);
        }

        if self.overlays.create.is_some() {
            return self.handle_create_mouse(mouse);
        }

        if self.overlays.rename.is_some() {
            return self.handle_rename_mouse(mouse);
        }

        if self.overlays.bulk_rename.is_some() {
            return self.handle_bulk_rename_mouse(mouse);
        }

        if self.overlays.goto.is_some() {
            return self.handle_goto_mouse(mouse);
        }

        if self.overlays.copy.is_some() {
            return self.handle_copy_mouse(mouse);
        }

        if self.overlays.search.is_some() {
            return self.handle_search_mouse(mouse);
        }

        if self.overlays.help {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.clear_wheel_scroll();
                self.overlays.help = false;
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.update_wheel_target_from_position(mouse.column, mouse.row);
                if let Some(rect) = self.input.frame_state.back_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_back();
                }
                if let Some(rect) = self.input.frame_state.forward_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_forward();
                }
                if let Some(rect) = self.input.frame_state.parent_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_parent();
                }
                if let Some(rect) = self.input.frame_state.hidden_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.toggle_hidden_files()?;
                    return Ok(());
                }
                if let Some(rect) = self.input.frame_state.view_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.toggle_view_mode();
                    return Ok(());
                }

                if let Some(target) = self
                    .input
                    .frame_state
                    .sidebar_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    return self.set_dir(target.path);
                }

                if let Some(hit) = self
                    .input
                    .frame_state
                    .entry_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    let Some(path) = self
                        .navigation
                        .entries
                        .get(hit.index)
                        .map(|entry| entry.path.clone())
                    else {
                        return Ok(());
                    };
                    self.select_index(hit.index);
                    if self.is_double_click(&path) {
                        self.open_selected()?;
                    }
                    self.input.last_click = Some(ClickState {
                        path,
                        at: Instant::now(),
                    });
                }
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

    fn panel_target_at(&self, column: u16, row: u16) -> Option<WheelTarget> {
        if self
            .input
            .frame_state
            .preview_panel
            .is_some_and(|rect| rect_contains(rect, column, row))
        {
            Some(WheelTarget::Preview)
        } else if self
            .input
            .frame_state
            .entries_panel
            .is_some_and(|rect| rect_contains(rect, column, row))
        {
            Some(WheelTarget::Entries)
        } else {
            None
        }
    }

    pub(in crate::app) fn update_wheel_target_from_position(&mut self, column: u16, row: u16) {
        if let Some(target) = self.panel_target_at(column, row) {
            self.input.last_wheel_target = Some(target);
        }
    }

    pub(in crate::app) fn resolve_wheel_target(
        &mut self,
        column: u16,
        row: u16,
    ) -> Option<WheelTarget> {
        if let Some(target) = self.panel_target_at(column, row) {
            self.input.last_wheel_target = Some(target);
            return Some(target);
        }

        if let Some(preview) = self.input.frame_state.preview_panel
            && column >= preview.x
        {
            self.input.last_wheel_target = Some(WheelTarget::Preview);
            return self.input.last_wheel_target;
        }

        if let Some(entries) = self.input.frame_state.entries_panel
            && column >= entries.x
            && column < entries.x.saturating_add(entries.width)
        {
            self.input.last_wheel_target = Some(WheelTarget::Entries);
            return self.input.last_wheel_target;
        }

        self.input.last_wheel_target
    }

    fn is_double_click(&self, path: &Path) -> bool {
        self.input
            .last_click
            .as_ref()
            .is_some_and(|click| click.path == path && click.at.elapsed() <= DOUBLE_CLICK_WINDOW)
    }
}
