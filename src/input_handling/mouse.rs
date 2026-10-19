use super::*;
use crate::app::{ScrollbarDrag, ScrollbarHit, ScrollbarTarget};

const HELP_WHEEL_LINES: isize = 2;
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);

impl App {
    fn scrollbar_target_is_active(&self, target: ScrollbarTarget) -> bool {
        // Keep this precedence in sync with the overlay order in `ui::render`.
        if self.file_operations.trash_is_open() {
            return target == ScrollbarTarget::Trash;
        }
        if self.file_operations.restore_is_open() {
            return target == ScrollbarTarget::Restore;
        }
        if self.file_operations.archive_password_is_open()
            || self.chooser.save_as().is_some_and(|save| save.is_open())
        {
            return false;
        }
        if self.file_operations.archive_create_is_open() {
            return target == ScrollbarTarget::ArchiveCreate;
        }
        if self.file_operations.create_is_open() {
            return target == ScrollbarTarget::Create;
        }
        if self.file_operations.rename_is_open() {
            return false;
        }
        if self.file_operations.bulk_rename_is_open() {
            return target == ScrollbarTarget::BulkRename;
        }
        if self.file_operations.editor_rename_confirm_is_open() {
            return target == ScrollbarTarget::EditorRenameConfirm;
        }
        if self.goto_is_open() || self.file_operations.copy_is_open() {
            return false;
        }
        if self.open_with_is_open() {
            return target == ScrollbarTarget::OpenWith;
        }
        if self.search_is_open() {
            return target == ScrollbarTarget::Search;
        }
        if self.overlays.help {
            return target == ScrollbarTarget::Help;
        }
        if self.duplicates_is_open() {
            return matches!(
                target,
                ScrollbarTarget::DuplicateResults | ScrollbarTarget::DuplicatePreview
            );
        }

        matches!(target, ScrollbarTarget::Browser | ScrollbarTarget::Preview)
    }

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
        if self.input.scrollbar_drag_cancelled_until_release {
            match mouse.kind {
                MouseEventKind::Up(MouseButton::Left) => {
                    self.input.scrollbar_drag_cancelled_until_release = false;
                    self.clear_drag_state();
                    return Ok(());
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    // A fresh press starts a new gesture if the old release was lost
                    // while focus was away.
                    self.input.scrollbar_drag_cancelled_until_release = false;
                    self.clear_drag_state();
                }
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(_) => return Ok(()),
                _ => {}
            }
        }
        if matches!(mouse.kind, MouseEventKind::Down(_)) && self.blocks_file_drag_and_drop() {
            // An outside click can dismiss an overlay before Kitty sends the matching
            // drag offer. Keep that press from becoming a browser drag afterward.
            self.suppress_drag_until_button_up();
        }
        if self.handle_scrollbar_mouse(mouse) {
            return Ok(());
        }
        if self.chooser.save_as().is_some_and(|save| save.is_open()) {
            return self.handle_save_as_mouse(mouse);
        }
        if self.file_operations.trash_is_open() {
            return self.handle_trash_mouse(mouse);
        }

        if self.file_operations.restore_is_open() {
            return self.handle_restore_mouse(mouse);
        }

        if self.file_operations.archive_password_is_open() {
            return self.handle_archive_password_mouse(mouse);
        }

        if self.file_operations.archive_create_is_open() {
            return self.handle_archive_create_mouse(mouse);
        }

        if self.file_operations.create_is_open() {
            return self.handle_create_mouse(mouse);
        }

        if self.file_operations.rename_is_open() {
            return self.handle_rename_mouse(mouse);
        }

        if self.file_operations.bulk_rename_is_open() {
            return self.handle_bulk_rename_mouse(mouse);
        }

        if self.file_operations.editor_rename_confirm_is_open() {
            return self.handle_editor_rename_confirm_mouse(mouse);
        }

        if self.overlays.goto.is_some() {
            return self.handle_goto_mouse(mouse);
        }

        if self.file_operations.copy_is_open() {
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
                        if self.chooser_mode() && !self.save_as_mode() && !is_dir {
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

    fn handle_scrollbar_mouse(&mut self, mouse: MouseEvent) -> bool {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(hit) = self
                    .input
                    .screen_regions
                    .scrollbar_hits
                    .iter()
                    .rev()
                    .find(|hit| {
                        self.scrollbar_target_is_active(hit.target)
                            && hit.rect.contains((mouse.column, mouse.row).into())
                    })
                    .copied()
                else {
                    return false;
                };
                let (thumb_height, max_scroll, max_thumb_top, thumb_top) = scrollbar_metrics(hit);
                if max_scroll == 0 || max_thumb_top == 0 {
                    return true;
                }
                let pointer_top = mouse.row.saturating_sub(hit.track.y) as isize;
                let on_thumb = pointer_top >= thumb_top as isize
                    && pointer_top < (thumb_top + thumb_height) as isize;
                if !on_thumb {
                    return true;
                }
                self.suppress_drag_until_button_up();
                let grab_offset = pointer_top - thumb_top as isize;
                let cols = self.input.screen_regions.metrics.cols.max(1);
                let focus_row = match hit.target {
                    ScrollbarTarget::Browser => (self.file_browser.selected / cols)
                        .saturating_sub(hit.scroll_row)
                        .min(hit.visible_rows.saturating_sub(1)),
                    ScrollbarTarget::Create => self
                        .file_operations
                        .create_cursor_line()
                        .saturating_sub(hit.scroll_row)
                        .min(hit.visible_rows.saturating_sub(1)),
                    ScrollbarTarget::BulkRename => self
                        .file_operations
                        .bulk_rename_cursor_line()
                        .saturating_sub(hit.scroll_row)
                        .min(hit.visible_rows.saturating_sub(1)),
                    ScrollbarTarget::Search => self
                        .fuzzy_finder
                        .search
                        .as_ref()
                        .map_or(0, |search| search.selected)
                        .saturating_sub(hit.scroll_row)
                        .min(hit.visible_rows.saturating_sub(1)),
                    ScrollbarTarget::DuplicateResults => self
                        .duplicate_finder
                        .session
                        .as_ref()
                        .map_or(0, |duplicates| duplicates.selected)
                        .saturating_sub(hit.scroll_row)
                        .min(hit.visible_rows.saturating_sub(1)),
                    ScrollbarTarget::OpenWith => self
                        .open_with_selected_index()
                        .saturating_sub(hit.scroll_row)
                        .min(hit.visible_rows.saturating_sub(1)),
                    _ => 0,
                };
                let focus_col = if hit.target == ScrollbarTarget::Browser {
                    self.file_browser.selected % cols
                } else {
                    0
                };
                let drag = ScrollbarDrag {
                    track: hit.track,
                    max_scroll,
                    max_thumb_top,
                    grab_offset,
                    target: hit.target,
                    visible_rows: hit.visible_rows,
                    focus_row,
                    focus_col,
                };
                self.input.scrollbar_drag = Some(drag);
                true
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(drag) = self.input.scrollbar_drag else {
                    return false;
                };
                if !self.scrollbar_target_is_active(drag.target) {
                    self.cancel_scrollbar_drag();
                    return true;
                }
                let pointer_top = mouse.row.saturating_sub(drag.track.y) as isize;
                let target_top =
                    (pointer_top - drag.grab_offset).clamp(0, drag.max_thumb_top as isize) as usize;
                self.apply_scrollbar_position(&drag, scrollbar_top_to_scroll(target_top, &drag));
                true
            }
            MouseEventKind::Up(MouseButton::Left) if self.input.scrollbar_drag.is_some() => {
                self.input.scrollbar_drag = None;
                self.clear_drag_state();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn cancel_scrollbar_drag(&mut self) {
        if self.input.scrollbar_drag.take().is_some() {
            self.input.scrollbar_drag_cancelled_until_release = true;
            self.suppress_drag_until_button_up();
        }
    }

    fn apply_scrollbar_position(&mut self, drag: &ScrollbarDrag, scroll_row: usize) {
        let scroll_row = scroll_row.min(drag.max_scroll);
        match drag.target {
            ScrollbarTarget::Browser => {
                self.file_browser.scroll_row = scroll_row;
                let cols = self.input.screen_regions.metrics.cols.max(1);
                let target_row = scroll_row
                    .saturating_add(drag.focus_row)
                    .saturating_mul(cols);
                let candidate = target_row.saturating_add(drag.focus_col);
                let entries_len = self.file_browser.entries.len();
                let selected = if candidate < entries_len {
                    candidate
                } else if drag.focus_col < entries_len {
                    entries_len
                        .saturating_sub(1)
                        .saturating_sub(drag.focus_col)
                        .checked_div(cols)
                        .unwrap_or(0)
                        .saturating_mul(cols)
                        .saturating_add(drag.focus_col)
                } else {
                    entries_len.saturating_sub(1)
                };
                self.set_selected_from_scrollbar(selected);
            }
            ScrollbarTarget::Preview | ScrollbarTarget::DuplicatePreview => {
                self.preview.state.scroll = scroll_row;
                self.sync_preview_scroll();
            }
            ScrollbarTarget::Help => {
                self.overlays.help_scroll =
                    scroll_row.min(self.input.screen_regions.help_scroll_max);
            }
            ScrollbarTarget::Search => {
                let index = scroll_row
                    .saturating_add(drag.focus_row)
                    .min(self.search_displayed_match_count().saturating_sub(1));
                self.set_search_scrollbar_position(index, scroll_row, drag.visible_rows);
            }
            ScrollbarTarget::DuplicateResults => {
                let index = scroll_row
                    .saturating_add(drag.focus_row)
                    .min(self.duplicate_file_count().saturating_sub(1));
                self.set_duplicate_scrollbar_position(index, scroll_row, drag.visible_rows);
            }
            ScrollbarTarget::OpenWith => {
                let index = scroll_row
                    .saturating_add(drag.focus_row)
                    .min(self.open_with_row_count().saturating_sub(1));
                self.select_open_with_index_from_scrollbar(index, scroll_row);
            }
            ScrollbarTarget::Trash => {
                if let Some(overlay) = self.file_operations.trash_overlay_mut() {
                    overlay.scroll = scroll_row;
                }
            }
            ScrollbarTarget::Restore => {
                if let Some(overlay) = self.file_operations.restore_overlay_mut() {
                    overlay.scroll = scroll_row;
                }
            }
            ScrollbarTarget::Create => {
                if let Some(overlay) = self.file_operations.create_overlay_mut() {
                    overlay.scroll_top_override = Some(scroll_row);
                    overlay.cursor_line = scroll_row
                        .saturating_add(drag.focus_row)
                        .min(overlay.lines.len().saturating_sub(1));
                    overlay.cursor_col = overlay
                        .preferred_col
                        .min(overlay.lines[overlay.cursor_line].chars().count());
                }
            }
            ScrollbarTarget::BulkRename => {
                if let Some(overlay) = self.file_operations.bulk_rename_overlay_mut() {
                    overlay.scroll_top_override = Some(scroll_row);
                    overlay.cursor_line = scroll_row
                        .saturating_add(drag.focus_row)
                        .min(overlay.new_names.len().saturating_sub(1));
                    overlay.cursor_col = overlay
                        .preferred_col
                        .min(overlay.new_names[overlay.cursor_line].chars().count());
                }
            }
            ScrollbarTarget::EditorRenameConfirm => {
                if let Some(overlay) = self.file_operations.editor_rename_confirm_overlay_mut() {
                    overlay.scroll = scroll_row;
                }
            }
            ScrollbarTarget::ArchiveCreate => {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    overlay.source_scroll = scroll_row;
                }
            }
        }
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

fn scrollbar_metrics(hit: ScrollbarHit) -> (usize, usize, usize, usize) {
    let track_height = hit.track.height as usize;
    let visible_rows = hit.visible_rows.max(1);
    let thumb_height = ((visible_rows.saturating_mul(track_height)) / hit.total_rows.max(1))
        .max(1)
        .min(track_height);
    let max_scroll = hit.total_rows.saturating_sub(visible_rows);
    let max_thumb_top = track_height.saturating_sub(thumb_height);
    let thumb_top = hit
        .scroll_row
        .checked_mul(max_thumb_top)
        .and_then(|offset| offset.checked_div(max_scroll.max(1)))
        .unwrap_or(0);
    (thumb_height, max_scroll, max_thumb_top, thumb_top)
}

fn scrollbar_top_to_scroll(top: usize, drag: &ScrollbarDrag) -> usize {
    top.checked_mul(drag.max_scroll)
        .and_then(|offset| offset.checked_add(drag.max_thumb_top / 2))
        .and_then(|offset| offset.checked_div(drag.max_thumb_top.max(1)))
        .unwrap_or(0)
}
