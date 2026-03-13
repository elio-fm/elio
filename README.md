# elio

`elio` is a mouse-capable terminal file manager with a soft folder-first presentation.

It opens the current working directory, keeps pinned places on the left, shows files in a grid or list in the center, and uses the right-hand pane for details and previews.

## Highlights

- Three-pane layout with places on the left, the main browser in the center, and details plus previews on the right
- Grid view by default, with a denser list view when you want it
- Keyboard and mouse support, including click, double click, wheel scroll, and preview scrolling
- Back/forward history, parent navigation, hidden-file toggle, and sort cycling
- Automatic reload when the current directory changes, with polling fallback when file watching is unavailable
- Fuzzy folder search with `f` and fuzzy file search with `Ctrl+F`, both scoped to the current directory tree
- Type-aware previews for directories, text, markdown, code, structured data, archives, documents, images, and PDFs
- Configurable behavior from `config.toml` and configurable appearance rules from `theme.toml`

## Quick Start

Run from the repository root:

```bash
cargo run --release
```

`elio` starts in your current working directory.

If you want a local install instead of `cargo run`, you can also do:

```bash
cargo install --path .
elio
```

## Optional Tools

`elio` works without extra setup, but a few integrations unlock better behavior:

- `gio open` or `xdg-open` enables external open with `o`
- `pdfinfo` and `pdftocairo` enable rendered PDF page previews
- Kitty graphics protocol support, or a working `kitten` backend, enables inline image and PDF surface previews

Image previews are detected automatically in terminals such as Kitty, Ghostty, and WezTerm. When those tools are missing, `elio` falls back to text or metadata-based previews instead of failing.

## Configuration

`elio` reads configuration from:

```bash
~/.config/elio/config.toml
```

Current supported config:

```toml
[ui]
show_top_bar = false
```

- `show_top_bar`: show or hide the optional toolbar at the top of the screen

If the config file does not exist, `elio` uses built-in defaults. The example file in [examples/config.toml](examples/config.toml) mirrors the current config surface.

## Theming

Theme overrides live at:

```bash
~/.config/elio/theme.toml
```

Theme loading behavior:

- If the file exists and parses, `elio` layers it on top of the built-in default theme
- Any key you omit falls back to the built-in default theme
- If the file does not exist, `elio` uses the built-in default theme
- If the file cannot be read or parsed, `elio` falls back to the built-in default theme and prints an error to `stderr`

Supported sections:

- `[palette]` for app-wide colors
- `[preview.code]` for code preview syntax colors
- `[classes.<name>]` for default icon/color per file class
- `[extensions.<ext>]` for file extension overrides
- `[files."<exact-name>"]` for exact file-name overrides
- `[directories."<exact-name>"]` for exact directory-name overrides

Rule matching is case-insensitive and trims surrounding whitespace. Resolution order is:

1. Exact file or directory name
2. File extension
3. Built-in file classification fallback

Built-in file class names:

- `directory`
- `code`
- `config`
- `document`
- `image`
- `audio`
- `video`
- `archive`
- `font`
- `data`
- `file`

Class aliases:

- `directory`, `dir`, `folder`
- `document`, `doc`, `text`
- `image`, `img`
- `archive`, `compressed`
- `file`, `plain`

Minimal example:

```toml
[palette]
bg = "#020304"
chrome = "#07090c"
panel = "#101419"
text = "#e7edf5"
muted = "#8c97a8"
accent = "#7aaeff"
selected_bg = "#243758"

[preview.code]
keyword = "#ff78c6"
function = "#36d7ff"
type = "#b38cff"
string = "#79e7d5"
comment = "#6f8399"

[classes.config]
icon = "󰒓"
color = "#b38cff"

[extensions.lock]
class = "data"
icon = "󰌾"
color = "#59de94"
```

The built-in default theme is mirrored in [examples/themes/default/theme.toml](examples/themes/default/theme.toml). The older blue-heavier variant is kept in [examples/themes/navi/theme.toml](examples/themes/navi/theme.toml).

## Controls

Main browser controls:

- `Enter`: open the selected folder or file
- `Backspace`: go to the parent directory
- `Left` / `h`: go to the parent directory
- `Right` / `l`: enter the selected folder
- `Up` / `Down` or `j` / `k`: move selection
- `Tab` / `Shift+Tab`: jump through pinned places
- `Alt+Left` / `Alt+Right`: go back or forward in history
- `v`: toggle grid/list view
- `.`: show or hide dotfiles
- `s`: cycle sort mode (`Name`, `Modified`, `Size`)
- `o`: open the selected item externally
- `f`: fuzzy-find folders in the current directory tree
- `Ctrl+F`: fuzzy-find files in the current directory tree
- `?`: open the help overlay
- `q` or `Esc`: quit

Mouse and preview behavior:

- Click selects an item
- Double click opens a folder or file
- Mouse wheel scrolls the browser or the details pane, depending on focus
- `Shift+Wheel` scrolls sideways in code previews
- The details pane keeps its own scroll position and reports real line counts where available

Inside the fuzzy finder:

- `Left` / `Right`: move the text cursor
- `Home` / `End`: jump to the start or end of the query
- `Backspace` / `Delete`: edit at the cursor position
- `Up` / `Down`: move through results
- `Enter`: open the selected result
- `Esc`: close the finder

## License

Licensed under the [MIT license](LICENSE-MIT).
