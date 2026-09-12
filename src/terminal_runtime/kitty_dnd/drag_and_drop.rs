use super::*;
use crate::{
    app::{App, ClipOp},
    filesystem::EntryKind,
    terminal_runtime::tui_drawing::AppTerminal,
    theme,
};
use anyhow::Result;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Default)]
pub(in crate::terminal_runtime) struct PendingDragOut {
    active: bool,
    uri_list: Vec<u8>,
}

impl PendingDragOut {
    fn reset(&mut self) {
        self.active = false;
        self.uri_list.clear();
    }
}

#[derive(Debug, Default)]
pub(in crate::terminal_runtime) struct PendingDropIn {
    op: Option<ClipOp>,
}

impl PendingDropIn {
    fn set(&mut self, op: ClipOp) {
        self.op = Some(op);
    }

    fn take(&mut self) -> Option<ClipOp> {
        self.op.take()
    }

    fn reset(&mut self) {
        self.op = None;
    }
}

struct DragIconLabel {
    icon: String,
    text: String,
    icon_color: ratatui::style::Color,
}

pub(in crate::terminal_runtime) fn handle_event(
    terminal: &mut AppTerminal,
    app: &mut App,
    event: KittyDndEvent,
    pending_drag_out: &mut PendingDragOut,
    pending_drop_in: &mut PendingDropIn,
) -> Result<()> {
    match event {
        KittyDndEvent::DropOffer {
            mime_index,
            operation,
            final_drop,
        } => {
            let chosen_op = choose_drop_op(operation);
            let sequence = if pending_drag_out.active && final_drop {
                pending_drag_out.reset();
                pending_drop_in.reset();
                app.clear_drag_state();
                format!(
                    "{}{}",
                    finish_drop_sequence(DropFinish::Reject),
                    cancel_drag_sequence()
                )
            } else if pending_drag_out.active {
                pending_drop_in.reset();
                reject_drop_sequence().to_string()
            } else if final_drop {
                pending_drop_in.set(chosen_op);
                request_drop_data_sequence(mime_index)
            } else {
                pending_drop_in.set(chosen_op);
                accept_drop_sequence(drop_op_to_dnd_operation(chosen_op))
            };
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;
        }
        KittyDndEvent::DropData {
            paths,
            unsupported_schemes,
            ..
        } => {
            let was_own_drag = pending_drag_out.active;
            let op = pending_drop_in.take();
            let mut finish = DropFinish::Reject;
            if was_own_drag {
                pending_drag_out.reset();
                app.clear_drag_state();
            } else if !unsupported_schemes.is_empty() {
                app.set_status_message(unsupported_drop_scheme_status(&unsupported_schemes));
            } else if let Some(op) = op {
                if app.drop_external_paths(paths, op)? {
                    finish = clip_op_to_drop_finish(op);
                }
            } else {
                app.set_status_message("Drop was not negotiated");
            }
            terminal
                .backend_mut()
                .write_all(finish_drop_sequence(finish).as_bytes())?;
            if was_own_drag {
                terminal
                    .backend_mut()
                    .write_all(cancel_drag_sequence().as_bytes())?;
            }
            terminal.backend_mut().flush()?;
        }
        KittyDndEvent::DropLeave => {}
        KittyDndEvent::DropDataError {
            mime_index: _,
            message,
        } => {
            let was_own_drag = pending_drag_out.active;
            if pending_drag_out.active {
                pending_drag_out.reset();
                app.clear_drag_state();
            }
            terminal
                .backend_mut()
                .write_all(finish_drop_sequence(DropFinish::Reject).as_bytes())?;
            if was_own_drag {
                terminal
                    .backend_mut()
                    .write_all(cancel_drag_sequence().as_bytes())?;
            }
            if !message.is_empty() {
                app.set_status_message(format!("Drop failed: {message}"));
            }
            terminal.backend_mut().flush()?;
        }
        KittyDndEvent::DropUnsupported { final_drop } => {
            pending_drop_in.reset();
            let sequence = if final_drop {
                finish_drop_sequence(DropFinish::Reject)
            } else {
                reject_drop_sequence()
            };
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;
        }
        KittyDndEvent::DragOffer { x, y } => {
            if pending_drag_out.active {
                return Ok(());
            }
            let paths = app.take_drag_export_paths_at(x, y);
            let uri_list = uri_list_payload(&paths);
            if uri_list.is_empty() {
                pending_drag_out.reset();
                app.clear_drag_candidate();
                terminal
                    .backend_mut()
                    .write_all(cancel_drag_sequence().as_bytes())?;
                terminal.backend_mut().flush()?;
                return Ok(());
            }

            let label = drag_icon_label(app, &paths);
            let mut sequence = agree_drag_sequence(DndOperation::Either);
            sequence.push_str(&present_drag_data_sequence(0, &uri_list));
            sequence.push_str(&drag_icon_sequence(&label));
            sequence.push_str(start_drag_sequence());
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;

            pending_drag_out.active = true;
            pending_drag_out.uri_list = uri_list;
        }
        KittyDndEvent::DragDataRequested { mime_index } => {
            let sequence = if pending_drag_out.active && mime_index == 0 {
                send_drag_data_sequence(mime_index, &pending_drag_out.uri_list)
            } else {
                drag_data_error_sequence(mime_index, "ENOENT")
            };
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;
        }
        KittyDndEvent::DragStarted
        | KittyDndEvent::DragAccepted { mime_index: _ }
        | KittyDndEvent::DragActionChanged { operation: _ }
        | KittyDndEvent::DragDropped => {}
        KittyDndEvent::DragEnded { cancelled: _ } => {
            pending_drag_out.reset();
            app.clear_drag_state();
        }
        KittyDndEvent::DragError { message } => {
            pending_drag_out.reset();
            app.clear_drag_state();
            if !message.is_empty() {
                app.set_status_message(format!("Kitty DND drag failed: {message}"));
            }
        }
    }
    Ok(())
}

fn unsupported_drop_scheme_status(schemes: &[String]) -> String {
    match schemes {
        [] => "Drop contains no local files".to_string(),
        [scheme] => format!("Unsupported drop URI scheme: {scheme}"),
        schemes => format!("Unsupported drop URI schemes: {}", schemes.join(", ")),
    }
}

fn choose_drop_op(operation: DndOperation) -> ClipOp {
    match operation {
        DndOperation::Copy => ClipOp::Yank,
        DndOperation::Move | DndOperation::Either => ClipOp::Cut,
    }
}

fn drop_op_to_dnd_operation(op: ClipOp) -> DndOperation {
    match op {
        ClipOp::Yank => DndOperation::Copy,
        ClipOp::Cut => DndOperation::Move,
    }
}

fn clip_op_to_drop_finish(op: ClipOp) -> DropFinish {
    match op {
        ClipOp::Yank => DropFinish::Copy,
        ClipOp::Cut => DropFinish::Move,
    }
}

fn drag_icon_sequence(label: &DragIconLabel) -> String {
    let palette = theme::palette();
    if let Some(image) = render_drag_image(
        &label.icon,
        &label.text,
        label.icon_color,
        palette.elevated,
        palette.text,
    ) {
        return present_drag_icon_png_sequence(image.width, image.height, &image.png);
    }

    present_drag_icon_sequence(&label.as_text())
}

impl DragIconLabel {
    fn as_text(&self) -> String {
        format!("{} {}", self.icon, self.text)
    }
}

fn drag_icon_label(app: &App, paths: &[PathBuf]) -> DragIconLabel {
    drag_icon_label_with(paths, |path| drag_icon_for_path(app, path))
}

fn drag_icon_label_with<F>(paths: &[PathBuf], mut icon_for_path: F) -> DragIconLabel
where
    F: FnMut(&Path) -> (String, ratatui::style::Color),
{
    const MAX_LABEL_CHARS: usize = 32;

    let (icon, text) = match paths {
        [path] => {
            let (icon, icon_color) = icon_for_path(path);
            let text = path
                .file_name()
                .map(|name| crate::app::sanitize_terminal_text(&name.to_string_lossy()))
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "1 item".to_string());
            return DragIconLabel {
                icon,
                text: truncate_drag_label_text(&text, MAX_LABEL_CHARS),
                icon_color,
            };
        }
        paths => (
            drag_icon_for_many(paths, &mut icon_for_path),
            format!("{} items", paths.len()),
        ),
    };
    let text = truncate_drag_label_text(&text, MAX_LABEL_CHARS);
    DragIconLabel {
        icon: icon.0,
        text,
        icon_color: icon.1,
    }
}

fn drag_icon_for_path(app: &App, path: &Path) -> (String, ratatui::style::Color) {
    if let Some(entry) = app
        .file_browser
        .entries
        .iter()
        .find(|entry| entry.path == path)
    {
        let appearance = theme::resolve_browser_entry(entry);
        return (appearance.icon.to_string(), appearance.color);
    }

    let appearance = theme::resolve_path(path, drag_entry_kind(path));
    (appearance.icon.to_string(), appearance.color)
}

fn drag_entry_kind(path: &Path) -> EntryKind {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => EntryKind::Directory,
        _ => EntryKind::File,
    }
}

fn drag_icon_for_many<F>(
    paths: &[PathBuf],
    icon_for_path: &mut F,
) -> (String, ratatui::style::Color)
where
    F: FnMut(&Path) -> (String, ratatui::style::Color),
{
    const MULTIPLE_FOLDERS_ICON: &str = "󰉓";
    const MULTIPLE_FILES_ICON: &str = "";

    let Some((first, rest)) = paths.split_first() else {
        let appearance = theme::resolve_path(Path::new("item"), EntryKind::File);
        return (MULTIPLE_FILES_ICON.to_string(), appearance.color);
    };

    let first_kind = drag_entry_kind(first);
    let (first_icon, first_color) = icon_for_path(first);
    let mut all_same_icon = true;
    let mut all_directories = first_kind == EntryKind::Directory;

    for path in rest {
        let kind = drag_entry_kind(path);
        let (icon, _) = icon_for_path(path);
        all_same_icon &= icon == first_icon;
        all_directories &= kind == EntryKind::Directory;
    }

    if all_same_icon {
        (first_icon, first_color)
    } else if all_directories {
        let appearance = theme::resolve_path(Path::new("folder"), EntryKind::Directory);
        (MULTIPLE_FOLDERS_ICON.to_string(), appearance.color)
    } else {
        let appearance = theme::resolve_path(Path::new("item"), EntryKind::File);
        (MULTIPLE_FILES_ICON.to_string(), appearance.color)
    }
}

fn truncate_drag_label_text(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let mut truncated: String = label.chars().take(max_chars - 3).collect();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
#[path = "tests/drag_and_drop.rs"]
mod tests;
