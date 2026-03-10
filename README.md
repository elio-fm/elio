# elio

`elio` is a mouse-capable terminal file manager with a GNOME Files / Nautilus-inspired layout and a soft folder-first presentation.

## Features

- Nautilus-like shell with a top toolbar, places sidebar, main file area, and details pane
- Grid view by default, plus a denser list view
- Mouse click, double click, and wheel support
- Directory navigation, back/forward history, hidden-file toggle, sort cycling, instant auto-reload, and external open via `xdg-open`
- Rich details pane with metadata plus folder, text, markdown, and code previews
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
- `[preview.code]` for code preview syntax colors
- `[classes.<name>]` for default icon/color per file class
- `[extensions.<ext>]` for file-extension overrides
- `[files."<exact-name>"]` for exact file-name overrides
- `[directories."<exact-name>"]` for exact directory-name overrides

How theme loading works:

- if `~/.config/elio/theme.toml` exists and parses, `elio` layers it on top of the built-in default theme
- any key you omit falls back to the built-in default theme
- if the file does not exist, `elio` uses the built-in default theme
- if the file exists but fails to read or parse, `elio` falls back to the built-in default theme and prints an error to `stderr`

The built-in default theme is mirrored in [examples/default/theme.toml](/home/regueiro/1Projects/elio/examples/default/theme.toml). The older blue-heavy variant is kept in [examples/navi/theme.toml](/home/regueiro/1Projects/elio/examples/navi/theme.toml).

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

Code preview syntax colors can be customized under `[preview.code]`. The available keys are:

- `fg`, `bg`
- `selection_bg`, `selection_fg`
- `caret`, `line_highlight`, `line_number`
- `comment`, `string`, `constant`, `keyword`
- `function`, `type`, `parameter`
- `tag`, `operator`, `macro`, `invalid`

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

The built-in theme already includes exact-name rules for many common files and folders, including:

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
- `.config`
- `.github`
- `node_modules`
- `src`
- `target`
- `Documents`
- `Downloads`
- `Pictures`
- `Music`
- `Videos`

Class names accept a few aliases:

- `directory`, `dir`, `folder`
- `document`, `doc`, `text`
- `image`, `img`
- `archive`, `compressed`
- `file`, `plain`

Example:

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

[files."Cargo.toml"]
class = "config"
icon = ""
color = "#ff8f40"
```

There are fuller examples in [examples/default/theme.toml](/home/regueiro/1Projects/elio/examples/default/theme.toml) and [examples/navi/theme.toml](/home/regueiro/1Projects/elio/examples/navi/theme.toml).

## Controls

- `Enter`: open the selected folder or file
- `Backspace`: go to the parent directory
- `Arrows` or `h/j/k/l`: navigate the main browser
- `Tab` / `Shift+Tab`: jump through pinned places
- `Alt+Left` / `Alt+Right`: go back or forward in history
- `v`: toggle grid/list view
- `.`: show or hide dotfiles
- `s`: cycle sort mode
- `o`: open the selected file with `xdg-open`
- `f`: fuzzy-find folders in the current directory tree
- `Ctrl+F`: fuzzy-find files in the current directory tree
- `?`: open the help overlay
- `q` or `Esc`: quit

The current directory reloads automatically when its contents change. Elio uses filesystem watching when available and falls back to throttled polling if watching is unavailable.

The details pane supports its own mouse-wheel scrolling. Text, markdown, and code previews report real source line counts, while folder previews show item counts and a compact folder/file breakdown.

## Fuzzy Finder

Inside the fuzzy finder:

- `Left` / `Right`: move the text cursor
- `Home` / `End`: jump to the start or end of the query
- `Backspace` / `Delete`: edit at the cursor position
- `Up` / `Down`: move through results
- `Enter`: open the selected result
- `Esc`: close the finder
