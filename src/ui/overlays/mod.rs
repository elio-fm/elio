mod archive_create;
mod archive_password;
mod bulk_rename;
mod copy_to_clipboard;
mod create;
mod duplicate_finder;
mod editor_rename_confirm;
mod fuzzy_finder;
mod goto;
mod help;
mod open_with;
mod rename;
mod restore;
mod trash_delete;

pub(super) use self::{
    archive_create::render_archive_create_overlay,
    archive_password::render_archive_password_overlay, bulk_rename::render_bulk_rename_overlay,
    copy_to_clipboard::render_copy_to_clipboard_overlay, create::render_create_overlay,
    duplicate_finder::render_duplicate_finder_overlay,
    editor_rename_confirm::render_editor_rename_confirm_overlay,
    fuzzy_finder::render_fuzzy_finder_overlay, goto::render_goto_overlay,
    help::render_help_overlay, open_with::render_open_with_overlay, rename::render_rename_overlay,
    restore::render_restore_overlay, trash_delete::render_trash_delete_overlay,
};

#[cfg(test)]
mod tests;
