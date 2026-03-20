# elio

A terminal-native, mouse-capable file manager with a soft folder-first layout and rich previews.

![elio — default theme](examples/themes/default/screenshot.png)

Three panes, strong keyboard and mouse support, and built-in previews for the file types you hit most often.

---

## Why Elio

Elio is designed to feel terminal-native rather than like a GUI file manager squeezed into a TUI: keyboard and mouse navigation are both first-class, common previews are built in, and richer preview features layer on through optional external tools instead of forcing a heavy baseline install.

---

## Features

- **Terminal-native navigation** — keyboard and mouse are both first-class
- **Rich previews** — text, code, structured data, archives, EPUB, comics, images, and PDFs
- **Inline image support** — rendered images and PDF pages in supported terminals
- **Fast browsing** — grid/list views, fuzzy search, history, sorting, and hidden-file toggle
- **Live updates** — watches the current directory with a polling fallback
- **Configurable** — behavior via `config.toml`, appearance via `theme.toml`

---

<details>
<summary><strong>Screenshots</strong></summary>

| Catppuccin Mocha | Tokyo Night |
|---|---|
| ![Catppuccin Mocha](examples/themes/catppuccin-mocha/screenshot.png) | ![Tokyo Night](examples/themes/tokyo-night/screenshot.png) |

| Amber Dusk | Blush Light |
|---|---|
| ![Amber Dusk](examples/themes/amber-dusk/screenshot.png) | ![Blush Light](examples/themes/blush-light/screenshot.png) |
</details>


## Installation

Requirements:
- Rust (stable toolchain) to build from source
- A terminal that supports 24-bit color and mouse reporting

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

## Preview Notes

> Inline image and PDF page previews depend on terminal graphics protocol support. Some preview types also use optional external tools for richer coverage or better fallback behavior. `elio` still works without them, but preview coverage is reduced when they are missing.

---

## Optional Tools

`elio` works without extra setup. These tools unlock additional behavior when present:

### PDF

| Package / Tool | Commands | What it enables |
|---|---|---|
| Poppler utilities | `pdfinfo`, `pdftocairo` | PDF metadata and rendered PDF page previews |

### Images

| Package / Tool | Commands | What it enables |
|---|---|---|
| `ffmpeg` | `ffmpeg` | Broader raster image format rendering |
| `resvg` | `resvg` | Preferred SVG rasterization for image previews |
| ImageMagick | `magick` | SVG rasterization fallback and broader image format rendering |

### Archives

| Package / Tool | Commands | What it enables |
|---|---|---|
| 7-Zip | `7z` | Comic archive preview and broad edge-case archive fallback |
| libarchive / bsdtar | `bsdtar` | Rare archive-family listing and ISO fallback |
| An `isoinfo` provider | `isoinfo` | Additional ISO listing fallback |

### External Open

| Package / Tool | Commands | What it enables |
|---|---|---|
| Desktop opener | `gio open` or `xdg-open` | Open files externally with `o` |

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

<details>
<summary><strong>Controls and Navigation</strong></summary>

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
| `q` | Quit |

### File Actions

| Key / Action | Description |
|---|---|
| `Space` | Toggle selection |
| `Ctrl+A` | Select all visible items |
| `a` | Create file or folder |
| `d` | Move selected item(s) to trash; permanently delete when already in trash |
| `r` | Restore in trash, or rename / bulk rename outside trash depending on selection |
| `F2` | Rename current item or bulk rename selected items |
| `Enter` | Confirm create, rename, bulk rename, trash, or restore prompts |
| `Esc` | Cancel active prompt, clear selection, close overlays, or quit |
| `Alt+Enter` / `Ctrl+J` | Add another line in the create prompt |

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
</details>

---

## Code Preview Support

Code preview is now split into one central language registry plus multiple rendering backends:

- `src/preview/code/registry.rs` resolves extensions, exact filenames, shebangs, modelines, and Markdown fence aliases from one source of truth.
- `src/preview/code/render.rs` is the single preview entrypoint used by both file previews and Markdown fenced code blocks.
- `src/preview/code/backends/syntect.rs` handles generic syntax-highlighted languages with Elio theme colors and a shell-aware fallback renderer.
- `src/preview/code/custom/` keeps the semantic renderers that are better than generic syntax highlighting.

Structured and specialized previews take priority over generic syntax highlighting for Markdown, JSON, JSONC / JSON5, TOML, YAML, `.env`, logs, directive configs, and INI / Desktop Entry files.

The current curated syntect bundle explicitly supports these code syntaxes:

- `html`, `xml`, `css`, `scss`, `sass`, `less`
- `javascript`, `jsx`, `typescript`, `tsx`
- `sql`, `diff`, `dockerfile`, `hcl`, `terraform`
- `rust`, `go`, `c`, `cpp`, `cs`, `java`, `dart`, `zig`, `php`, `swift`, `kotlin`, `elixir`, `clojure`, `ruby`, `python`, `lua`
- `groovy`, `scala`, `perl`, `haskell`, `julia`, `r`
- `make`, `just`, `sh`, `bash`, `zsh`, `ksh`, `fish`, `powershell`, `nix`, `cmake`

Anything outside that matrix falls back to plain code preview instead of advertising unsupported highlighting.

`clojure` support also covers the common Clojure-family aliases and file shapes used in practice: `clj`, `cljs`, `cljc`, `edn`, `project.clj`, `deps.edn`, `bb.edn`, and `shadow-cljs.edn`.

---

## License

[MIT](LICENSE-MIT)
