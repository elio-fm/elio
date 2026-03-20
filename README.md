# elio

A mouse-capable terminal file manager with a soft folder-first layout.

![elio — default theme](examples/themes/default/screenshot.png)

Three-pane interface: pinned places on the left, a grid or list browser in the center, and a detail and preview pane on the right. Works entirely in the terminal with full keyboard and mouse support.

---

## Features

- **Grid and list views** — switch with `v`; grid by default, denser list on demand
- **Full mouse support** — click, double-click, scroll, preview scrolling, Shift+scroll for sideways panning
- **Type-aware previews** — directories, text, Markdown, code, structured data, archives, EPUB, comic archives, images, and PDFs
- **Inline image previews** — rendered images and PDF pages directly inside the terminal on supported terminals
- **Fuzzy search** — `f` to search folders, `Ctrl+F` to search files, both scoped to the current directory tree
- **Auto-reload** — watches the current directory for changes, with polling fallback
- **Back/forward history**, parent navigation, hidden-file toggle, sort cycling
- **Configurable** via `config.toml` for behavior and `theme.toml` for appearance

---

## Screenshots

| Default | Catppuccin Mocha | Tokyo Night |
|---|---|---|
| ![Default](examples/themes/default/screenshot.png) | ![Catppuccin Mocha](examples/themes/catppuccin-mocha/screenshot.png) | ![Tokyo Night](examples/themes/tokyo-night/screenshot.png) |

| Neon Cherry | Amber Dusk | Blush Light |
|---|---|---|
| ![Neon Cherry](examples/themes/neon-cherry/screenshot.png) | ![Amber Dusk](examples/themes/amber-dusk/screenshot.png) | ![Blush Light](examples/themes/blush-light/screenshot.png) |

---

## Requirements

- Rust (stable toolchain) — for building from source
- A terminal that supports 24-bit color and mouse reporting

---

## Installation

**Run directly from the repository:**

```bash
cargo run --release
```

**Install to your PATH:**

```bash
cargo install --path .
elio
```

`elio` starts in your current working directory.

---

## Optional Tools

`elio` works without extra setup. These tools unlock additional behavior when present:

| Tool | What it enables |
|---|---|
| `pdfinfo` + `pdftocairo` | Rendered PDF page previews |
| `gio open` or `xdg-open` | Open files externally with `o` |
| `ffmpeg` | Broader raster image format rendering |
| `resvg` | Preferred SVG rasterization for image previews |
| `magick` | SVG rasterization fallback and broader image format rendering |

---

## Image Previews

`elio` renders inline images and PDF pages directly in the terminal using the native graphics protocol of the detected terminal. Detection is automatic — no configuration needed in supported terminals.

### Terminal Compatibility

| Terminal | Protocol | Image previews |
|---|---|---|
| [Kitty](https://sw.kovidgoyal.net/kitty/) | Kitty Graphics Protocol | Enabled by default |
| [Ghostty](https://ghostty.org/) | Kitty Graphics Protocol | Enabled by default |
| [Warp](https://www.warp.dev/) | Kitty Graphics Protocol | Enabled by default |
| [WezTerm](https://wezfurlong.org/wezterm/) | iTerm2 Inline Protocol | Enabled by default |
| Alacritty | — | Not supported |
| Other | Kitty Graphics Protocol | Disabled by default (see below) |

### Environment Variables

| Variable | Effect |
|---|---|
| `ELIO_IMAGE_PREVIEWS=1` | Force-enable image previews on unrecognized terminals that support the Kitty Graphics Protocol |
| `ELIO_DEBUG_PREVIEW` | Log image preview activity to `/tmp/elio-preview.log` |
| `ELIO_LOG_MOUSE` | Log raw mouse events to `/tmp/elio-mouse.log` |

---

## Configuration

```bash
~/.config/elio/config.toml
```

```toml
[ui]
show_top_bar = false
```

| Key | Default | Description |
|---|---|---|
| `ui.show_top_bar` | `false` | Show or hide the toolbar at the top of the screen |

If the file does not exist, `elio` uses built-in defaults. See [examples/config.toml](examples/config.toml) for an annotated reference.

---

## Theming

```bash
~/.config/elio/theme.toml
```

Theme files layer on top of the built-in defaults — only the keys you set are overridden. If the file is missing or unparseable, `elio` falls back to the built-in theme silently (parse errors are reported to `stderr`).

### Supported Sections

| Section | Controls |
|---|---|
| `[palette]` | App-wide colors |
| `[preview.code]` | Syntax highlight colors |
| `[classes.<name>]` | Default icon and color per file class |
| `[extensions.<ext>]` | Overrides by file extension |
| `[files."<name>"]` | Overrides by exact filename |
| `[directories."<name>"]` | Overrides by exact directory name |

Rule resolution order: exact name → extension → class fallback. Matching is case-insensitive.

### Built-in File Classes

`directory` · `code` · `config` · `document` · `image` · `audio` · `video` · `archive` · `font` · `data` · `file`

Aliases: `dir`/`folder` → `directory`, `doc`/`text` → `document`, `img` → `image`, `compressed` → `archive`, `plain` → `file`

### Minimal Example

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

[extensions.lock]
class = "data"
icon = "󰌾"
color = "#59de94"
```

Ready-to-use themes are in [examples/themes/](examples/themes/). Copy any `theme.toml` to `~/.config/elio/theme.toml` to apply it.

---

## Controls

### Browser

| Key / Action | Description |
|---|---|
| `Enter` | Open selected folder or file |
| `Backspace` · `Left` · `h` | Go to parent directory |
| `Right` · `l` | Enter selected folder |
| `Up` / `Down` · `j` / `k` | Move selection |
| `Tab` / `Shift+Tab` | Jump through pinned places |
| `Alt+Left` / `Alt+Right` | Back / forward in history |
| `v` | Toggle grid / list view |
| `.` | Show or hide dotfiles |
| `s` | Cycle sort mode (Name → Modified → Size) |
| `o` | Open selected item externally |
| `f` | Fuzzy-find folders in the current tree |
| `Ctrl+F` | Fuzzy-find files in the current tree |
| `?` | Open help overlay |
| `q` · `Esc` | Quit |

### Mouse

| Action | Description |
|---|---|
| Click | Select item |
| Double-click | Open folder or file |
| Scroll | Scroll browser or preview pane depending on cursor position |
| `Shift+Scroll` | Scroll sideways in code previews |

### Fuzzy Finder

| Key | Description |
|---|---|
| `Left` / `Right` | Move text cursor |
| `Home` / `End` | Jump to start / end of query |
| `Backspace` / `Delete` | Edit at cursor |
| `Up` / `Down` | Move through results |
| `Enter` | Open selected result |
| `Esc` | Close finder |

---

## License

[MIT](LICENSE-MIT)
