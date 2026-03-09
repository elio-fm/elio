# elio

`elio` is a mouse-capable terminal file manager with a GNOME Files / Nautilus-inspired layout and a soft folder-first presentation.

## Features

- Nautilus-like shell with a top toolbar, places sidebar, main file area, and details pane
- Grid view by default, plus a denser list view
- Mouse click, double click, and wheel support
- Directory navigation, back/forward history, hidden-file toggle, sort cycling, refresh, and external open via `xdg-open`
- Lightweight text preview for readable files
- Folder search with `f` and file search with `Ctrl+F`, both scoped to the current directory tree
- Type-aware icons and colors for folders, config files, documents, code, archives, media, fonts, data files, and plain files
- Configurable appearance rules from `~/.config/elio/theme.toml`

## Run

```bash
cargo run
```

## Theme

`elio` ships with a built-in default theme, but you can override file icons, file colors, and the full app chrome palette by creating:

```bash
~/.config/elio/theme.toml
```

Supported sections:

- `[palette]` for app-wide TUI colors
- `[classes.<name>]` for default icon/color per file class
- `[extensions.<ext>]` for file-extension overrides
- `[files."<exact-name>"]` for exact file-name overrides
- `[directories."<exact-name>"]` for exact directory-name overrides

How theme loading works:

- if `~/.config/elio/theme.toml` exists and parses, `elio` layers it on top of the built-in default theme
- any key you omit falls back to the built-in default theme
- if the file does not exist, `elio` uses the built-in default theme
- if the file exists but fails to read or parse, `elio` falls back to the built-in default theme and prints an error to `stderr`

The current app UI colors all come from `[palette]`. That includes:

- `bg`, `text`, `muted`
- `chrome`, `chrome_alt`
- `panel`, `panel_alt`
- `surface`, `elevated`
- `border`
- `accent`, `accent_soft`, `accent_text`
- `selected_bg`, `selected_border`
- `sidebar_active`
- `button_bg`, `button_disabled_bg`
- `path_bg`

The built-in file classes you can override under `[classes.<name>]` are:

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

Rule matching is case-insensitive and trims surrounding whitespace. Resolution order is:

- exact file or directory name
- file extension
- built-in file classification fallback

The built-in theme already includes exact-name rules for a small set of common files:

- `Cargo.toml`
- `Cargo.lock`
- `package.json`
- `package-lock.json`
- `Dockerfile`
- `compose.yml`
- `compose.yaml`
- `README.md`
- `LICENSE`
- `.gitignore`
- `.env`

Class names accept a few aliases:

- `directory`, `dir`, `folder`
- `document`, `doc`, `text`
- `image`, `img`
- `archive`, `compressed`
- `file`, `plain`

Example:

```toml
[palette]
bg = "#02050c"
chrome = "#070d16"
panel = "#09101b"
text = "#edf4ff"
muted = "#8ea2bf"
accent = "#7ec4ff"
selected_bg = "#204064"

[classes.config]
icon = "󰒓"
color = "#90c6ff"

[extensions.lock]
class = "data"
icon = "󰌾"
color = "#d9b36c"

[files."Cargo.toml"]
class = "config"
icon = "󰣖"
```

There is also a fuller example in [examples/theme.toml](/home/regueiro/elio/examples/theme.toml).

## Controls

- `Enter`: open the selected folder or file
- `Backspace`: go to the parent directory
- `Arrows` or `h/j/k/l`: navigate the main browser
- `Alt+Left` / `Alt+Right`: go back or forward in history
- `v`: toggle grid/list view
- `.`: show or hide dotfiles
- `s`: cycle sort mode
- `r`: refresh the current directory
- `o`: open the selected file with `xdg-open`
- `f`: fuzzy-find folders in the current directory tree
- `Ctrl+F`: fuzzy-find files in the current directory tree
- `?`: open the help overlay
- `q` or `Esc`: quit

## Fuzzy Finder

Inside the fuzzy finder:

- `Left` / `Right`: move the text cursor
- `Home` / `End`: jump to the start or end of the query
- `Backspace` / `Delete`: edit at the cursor position
- `Up` / `Down`: move through results
- `Enter`: open the selected result
- `Esc`: close the finder
