mod item_styling;
mod theme_loading;

use ratatui::style::{Color, Modifier, Style};

#[cfg(test)]
mod tests;

pub(crate) use self::{
    item_styling::{
        entry_color, entry_name_color, entry_symbol, path_color, path_color_with_symlink,
        path_symbol, path_symbol_with_symlink, resolve_browser_entry, resolve_entry, resolve_path,
        resolve_path_with_class,
    },
    theme_loading::{Palette, code_preview_palette, focused_entry_style, initialize, palette},
};

pub(crate) fn focused_entry_reverse_background(
    focused: bool,
    entry_text_color: Color,
    background: Color,
) -> Color {
    if focused && focused_entry_style().reversed {
        entry_text_color
    } else {
        background
    }
}

pub(crate) fn focused_entry_reverse_style(focused: bool, background: Color, style: Style) -> Style {
    focused_entry_reverse_style_with_reversal(
        focused,
        focused_entry_style().reversed,
        background,
        style,
    )
}

fn focused_entry_reverse_style_with_reversal(
    focused: bool,
    reversed: bool,
    background: Color,
    style: Style,
) -> Style {
    if focused && reversed {
        Style::default()
            .fg(background)
            .bg(Color::Reset)
            .add_modifier(style.add_modifier | Modifier::REVERSED)
            .remove_modifier(style.sub_modifier)
    } else {
        style
    }
}
