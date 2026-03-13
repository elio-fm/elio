use super::*;

impl App {
    pub(in crate::app) fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if self.search.is_some() {
            return self.handle_search_mouse(mouse);
        }

        if self.help_open {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.clear_wheel_scroll();
                self.help_open = false;
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.update_wheel_target_from_position(mouse.column, mouse.row);
                if let Some(rect) = self.frame_state.back_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_back();
                }
                if let Some(rect) = self.frame_state.forward_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_forward();
                }
                if let Some(rect) = self.frame_state.parent_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_parent();
                }
                if let Some(rect) = self.frame_state.hidden_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.toggle_hidden_files()?;
                    return Ok(());
                }
                if let Some(rect) = self.frame_state.view_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.toggle_view_mode();
                    return Ok(());
                }

                if let Some(target) = self
                    .frame_state
                    .sidebar_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    return self.set_dir(target.path);
                }

                if let Some(hit) = self
                    .frame_state
                    .entry_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    let Some(path) = self.entries.get(hit.index).map(|entry| entry.path.clone())
                    else {
                        return Ok(());
                    };
                    self.select_index(hit.index);
                    if self.is_double_click(&path) {
                        self.open_selected()?;
                    }
                    self.last_click = Some(ClickState {
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
                self.update_wheel_target_from_position(mouse.column, mouse.row);
            }
            _ => {}
        }
        Ok(())
    }

    fn panel_target_at(&self, column: u16, row: u16) -> Option<WheelTarget> {
        if self
            .frame_state
            .preview_panel
            .is_some_and(|rect| rect_contains(rect, column, row))
        {
            Some(WheelTarget::Preview)
        } else if self
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
            self.last_wheel_target = Some(target);
        }
    }

    pub(in crate::app) fn resolve_wheel_target(
        &mut self,
        column: u16,
        row: u16,
    ) -> Option<WheelTarget> {
        if let Some(target) = self.panel_target_at(column, row) {
            self.last_wheel_target = Some(target);
            return Some(target);
        }

        if let Some(preview) = self.frame_state.preview_panel
            && column >= preview.x
        {
            self.last_wheel_target = Some(WheelTarget::Preview);
            return self.last_wheel_target;
        }

        if let Some(entries) = self.frame_state.entries_panel
            && column >= entries.x
            && column < entries.x.saturating_add(entries.width)
        {
            self.last_wheel_target = Some(WheelTarget::Entries);
            return self.last_wheel_target;
        }

        self.last_wheel_target
    }

    fn is_double_click(&self, path: &Path) -> bool {
        self.last_click
            .as_ref()
            .is_some_and(|click| click.path == path && click.at.elapsed() <= DOUBLE_CLICK_WINDOW)
    }
}
