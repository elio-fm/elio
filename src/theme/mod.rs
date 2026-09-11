mod item_styling;
mod theme_loading;

#[cfg(test)]
mod tests;

pub(crate) use self::{
    item_styling::{
        entry_color, entry_symbol, mix_color, path_color, path_color_with_symlink, path_symbol,
        path_symbol_with_symlink, resolve_browser_entry, resolve_entry, resolve_path,
        resolve_path_with_class,
    },
    theme_loading::{Palette, code_preview_palette, initialize, palette},
};
