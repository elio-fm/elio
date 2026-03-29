<h1 align="left"><img src="assets/logo.png" width="75" alt="elio logo" align="absmiddle" />&nbsp;elio</h1>

A terminal-native, mouse-capable file manager with rich previews and inline images.

![elio — default theme](examples/themes/default/screenshot.webp)

| Catppuccin Mocha | Tokyo Night |
|---|---|
| ![Catppuccin Mocha](examples/themes/catppuccin-mocha/screenshot.png) | ![Tokyo Night](examples/themes/tokyo-night/screenshot.png) |

| Amber Dusk | Blush Light |
|---|---|
| ![Amber Dusk](examples/themes/amber-dusk/screenshot.png) | ![Blush Light](examples/themes/blush-light/screenshot.png) |

---

## Features

- **Three-pane layout** — Places, Files, and Preview side by side
- **Keyboard and mouse** — both are first-class; navigate however you prefer
- **Rich previews** — text, code with syntax highlighting, structured data, archives, EPUB, comics, images, and PDFs
- **Inline images** — rendered directly in the terminal on supported terminals
- **Grid and list views** — switch with `v`, zoom the grid with `+` / `-`
- **Fuzzy search** — find folders and files quickly
- **Live updates** — watches the current directory with a polling fallback
- **Theming** — full palette and file-class control via `theme.toml`

---

## Installation

Requires a Rust stable toolchain and a terminal with 24-bit color and mouse support.

```bash
cargo run --release
```

`elio` starts in your current working directory.

---

## Image Previews

Inline image and PDF previews work automatically on supported terminals — no configuration needed.

| Terminal | Protocol | Status |
|---|---|---|
| [Kitty](https://sw.kovidgoyal.net/kitty/) | Kitty Graphics Protocol | ✓ Auto-detected |
| [Ghostty](https://ghostty.org/) | Kitty Graphics Protocol | ✓ Auto-detected |
| [Warp](https://www.warp.dev/) | Kitty Graphics Protocol | ✓ Auto-detected |
| [WezTerm](https://wezfurlong.org/wezterm/) | iTerm2 Inline Protocol | ✓ Auto-detected |
| Alacritty | — | Not supported |
| Other | Kitty Graphics Protocol | Set `ELIO_IMAGE_PREVIEWS=1` to enable |

| Variable | Effect |
|---|---|
| `ELIO_IMAGE_PREVIEWS=1` | Force-enable on unrecognized terminals that support the Kitty Graphics Protocol |
| `ELIO_DEBUG_PREVIEW` | Log image preview activity to `elio-preview.log` in the system temp directory |
| `ELIO_LOG_MOUSE` | Log raw mouse events to `elio-mouse.log` in the system temp directory |

---

## Optional Tools

`elio` works without any extra setup. These tools unlock richer previews and additional features when installed:

| Category | Tool | Command(s) | What it enables |
|---|---|---|---|
| PDF | Poppler | `pdfinfo`, `pdftocairo` | PDF metadata and rendered page previews |
| Images | ffmpeg | `ffmpeg` | Broader raster image format support |
| Images | resvg | `resvg` | SVG rasterization (preferred) |
| Images | ImageMagick | `magick` | SVG rasterization fallback |
| Archives | 7-Zip | `7z` | Comic archive preview and edge-case archive fallback |
| Archives | libarchive | `bsdtar` | Rare archive types and ISO fallback |
| Archives | isoinfo | `isoinfo` | Additional ISO listing fallback |
| Clipboard | Wayland | `wl-copy` | Copy file metadata to clipboard with `c` |
| Clipboard | X11 | `xclip` or `xsel` | Copy file metadata to clipboard with `c` |
| Clipboard | macOS | `pbcopy` | Copy file metadata to clipboard with `c` |
| Clipboard | Windows | `clip` | Copy file metadata to clipboard with `c` |

Opening files externally (`o` / `Enter`) uses the system launcher: `open` on macOS, `cmd /c start` on Windows, and `xdg-open` or `gio` on Linux and BSD desktop sessions.

---

## Configuration

| Platform | Config file |
|---|---|
| Linux / BSD | `~/.config/elio/config.toml` (or `$XDG_CONFIG_HOME/elio/config.toml`) |
| macOS | `~/Library/Application Support/elio/config.toml` |
| Windows | `%APPDATA%\elio\config.toml` |

```toml
[ui]
show_top_bar = false
# grid_zoom = 1   # starting grid zoom: 0, 1, or 2

# [layout.panes]
# places  = 10
# files   = 45
# preview = 45
```

| Key | Default | Description |
|---|---|---|
| `ui.show_top_bar` | `false` | Show or hide the toolbar at the top of the screen |
| `ui.grid_zoom` | `1` | Starting grid zoom level (`0`, `1`, or `2`; values outside range are clamped) |
| `layout.panes.places` | unset | Relative width weight for the Places pane; `0` hides it |
| `layout.panes.files` | unset | Relative width weight for the Files pane |
| `layout.panes.preview` | unset | Relative width weight for the Preview pane; `0` hides it |

Pane weights are relative — `10/45/45` and `20/90/90` produce the same split. If `[layout.panes]` is omitted, elio uses a built-in responsive layout. See [examples/config.toml](examples/config.toml) for an annotated reference.

---

## Theming

| Platform | Theme file |
|---|---|
| Linux / BSD | `~/.config/elio/theme.toml` (or `$XDG_CONFIG_HOME/elio/theme.toml`) |
| macOS | `~/Library/Application Support/elio/theme.toml` |
| Windows | `%APPDATA%\elio\theme.toml` |

Theme files layer on top of the built-in defaults — only the keys you provide are overridden. If the file is missing or cannot be parsed, elio falls back silently (parse errors are reported to stderr).

| Section | Controls |
|---|---|
| `[palette]` | App-wide colors |
| `[preview.code]` | Syntax highlight colors |
| `[classes.<name>]` | Default icon and color per file class |
| `[extensions.<ext>]` | Overrides by file extension |
| `[files."<name>"]` | Overrides by exact filename |
| `[directories."<name>"]` | Overrides by exact directory name |

Rule resolution order: exact name → extension → class fallback. Matching is case-insensitive.

**Built-in file classes:** `directory` · `code` · `config` · `document` · `image` · `audio` · `video` · `archive` · `font` · `data` · `file`

```toml
[palette]
bg = "#020304"
accent = "#7aaeff"
selected_bg = "#243758"

[preview.code]
keyword = "#ff78c6"
function = "#36d7ff"
string  = "#79e7d5"

[extensions.lock]
class = "data"
icon  = "󰌾"
color = "#59de94"
```

The full default theme is at [`assets/themes/default/theme.toml`](assets/themes/default/theme.toml). Ready-to-use themes are in [`examples/themes/`](examples/themes/) — copy any `theme.toml` to the theme file path for your platform (see table above) to apply it.

---

<details>
<summary><strong>Controls</strong></summary>

### Navigation

| Key | Action |
|---|---|
| `↑` / `↓` · `j` / `k` | Move selection |
| `←` · `h` · `Backspace` | Go to parent directory |
| `→` · `l` · `Enter` | Enter folder / open file |
| `g` | Go-to menu (`g` top, `d` downloads, `h` home, `c` .config, `t` trash) |
| `G` | Jump to last item |
| `PageUp` / `PageDown` | Page up / down |
| `Tab` / `Shift+Tab` | Cycle places |
| `Alt+←` / `Alt+→` | Back / forward in history |

### View

| Key | Action |
|---|---|
| `v` | Toggle grid / list view |
| `+` / `-` | Grid zoom in / out |
| `.` | Show / hide dotfiles |
| `s` | Cycle sort (Name → Modified → Size) |
| `<` / `>` | Scroll preview left / right |

### Files and Clipboard

| Key | Action |
|---|---|
| `Space` | Toggle selection |
| `Ctrl+A` | Select all |
| `y` | Yank (copy) |
| `x` | Cut |
| `p` | Paste |
| `a` | Create file or folder |
| `d` | Trash; permanently delete if already in trash |
| `r` | Rename / bulk rename / restore from trash |
| `F2` | Rename / bulk rename |
| `o` | Open externally |
| `c` | Copy path details to clipboard |

### Search

| Key | Action |
|---|---|
| `f` | Fuzzy-find folders in the current tree |
| `Ctrl+F` | Fuzzy-find files in the current tree |

### Mouse

| Action | Description |
|---|---|
| Click | Select item |
| Double-click | Open item |
| Scroll | Scroll browser or preview |
| `Shift+Scroll` | Scroll preview sideways |

### General

| Key | Action |
|---|---|
| `?` | Open help overlay |
| `Esc` | Cancel / clear selection / close overlay |
| `q` | Quit |

</details>

---

## License

[MIT](LICENSE-MIT)
