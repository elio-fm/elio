<h1 align="left"><img src="assets/logo.png" width="64" alt="elio logo" align="absmiddle" />&nbsp;elio</h1>

Snappy, batteries-included terminal file manager with rich previews, inline images, bulk actions, and trash support.

![elio — default theme](examples/themes/default/screenshot.webp)

## Documentation

- Installation: https://elio-fm.github.io/install/
- Usage: https://elio-fm.github.io/docs/

---

## Features

- **Three-pane layout** — Places, Files, and Preview side by side
- **Rich previews** — text, code, documents, archives, media, and more; see [Preview Coverage](#preview-coverage)
- **Inline images** — rendered directly in supported terminals
- **Customizable Places and devices** — pinned folders plus auto-detected drives and mounts
- **Quick actions** — Go-to, Open With, and copy-to-clipboard
- **Trash management** — trash, restore, or permanently delete files
- **Keyboard and mouse navigation** — browse comfortably either way
- **Grid and list views** — switch with `v`, zoom the grid with `+` / `-`
- **Fuzzy search** — find folders and files quickly
- **Zoxide jumps** — jump to frequent directories from your zoxide history
- **Shell integration** — install cd-on-exit wrappers for bash, zsh, fish, and Nushell
- **Theming** — full palette and file-class control via `theme.toml`

---

## Installation

### Arch Linux

Install from the AUR with your preferred AUR helper:

```bash
paru -S elio
```

### Fedora

Enable the COPR repository and install with `dnf`:

```bash
sudo dnf copr enable miguelregueiro/elio
sudo dnf install elio
```

### Debian and Ubuntu-based Linux

Configure the official apt repository and install with `apt`:

```bash
curl -fsSL https://elio-fm.github.io/elio-apt/install.sh | sudo sh
sudo apt install elio
```

Manual repository setup is available in [`elio-apt`](https://github.com/elio-fm/elio-apt). To install without adding a repository, download `elio_amd64.deb` from the [latest release](https://github.com/elio-fm/elio/releases/latest).

The apt repository currently publishes `amd64` packages.

### Homebrew

Install from the Homebrew tap:

```bash
brew install elio-fm/elio/elio
```

### Cargo

Install from crates.io:

```bash
cargo install elio
```

`elio` starts in your current working directory by default. Pass a directory path to start there instead, for example `elio path/to/directory`.

> [!TIP]
> Recommended: use a Nerd Font in your terminal so icons display correctly.

<details>
<summary><strong>Running From Source</strong></summary>

```bash
cargo run --release
```

</details>

---

## Example Themes

A few bundled themes are shown below. More are available in [`examples/themes/`](examples/themes/). See [Theming](#theming) for theme paths and docs.

| Catppuccin Mocha | Navi |
|---|---|
| <p align="center"><img src="examples/themes/catppuccin-mocha/screenshot.webp" alt="Catppuccin Mocha" width="440"></p> | <p align="center"><img src="examples/themes/navi/screenshot.webp" alt="Navi" width="440"></p> |

| Amber Dusk | Blush Light |
|---|---|
| <p align="center"><img src="examples/themes/amber-dusk/screenshot.webp" alt="Amber Dusk" width="440"></p> | <p align="center"><img src="examples/themes/blush-light/screenshot.webp" alt="Blush Light" width="440"></p> |

---

## Image Previews

Inline visual previews, including images, covers, thumbnails, and rendered pages, work automatically on supported terminals.

| Terminal | Protocol | Status |
|---|---|---|
| [Kitty](https://sw.kovidgoyal.net/kitty/) | Kitty Graphics Protocol | ✓ Auto-detected |
| [Ghostty](https://ghostty.org/) | Kitty Graphics Protocol | ✓ Auto-detected |
| [Warp](https://www.warp.dev/) | Kitty direct-placement protocol | ✓ Auto-detected |
| [WezTerm](https://wezfurlong.org/wezterm/) | iTerm2 Inline Protocol | ✓ Auto-detected |
| [iTerm2](https://iterm2.com/) | iTerm2 Inline Protocol | ✓ Auto-detected |
| [Konsole](https://konsole.kde.org/) | Kitty direct-placement protocol | ✓ Auto-detected |
| [foot](https://codeberg.org/dnkl/foot) | Sixel | ✓ Auto-detected |
| [Windows Terminal](https://github.com/microsoft/terminal) | Sixel | ✓ Auto-detected |
| Alacritty | — | Not supported |
| Other | Kitty Graphics Protocol | Set `ELIO_IMAGE_PREVIEWS=1` to enable |

> Sixel terminals can render large or first-time previews more slowly than Kitty Graphics or iTerm2 Inline backends.
>
> In Konsole, inline previews are temporarily cleared while modal popups are open to avoid rendering artifacts.

Useful environment variables:

<details>
<summary><strong>Environment Variables</strong></summary>

| Variable | Effect |
|---|---|
| `ELIO_IMAGE_PREVIEWS=1` | Force-enable on unrecognized terminals that support the Kitty Graphics Protocol |
| `ELIO_ZOXIDE_OPTS` | Extra options appended to the zoxide interactive picker options |
| `ELIO_DEBUG_PREVIEW` | Log image preview activity to `elio-preview.log` in the system temp directory |
| `ELIO_LOG_MOUSE` | Log raw mouse events to `elio-mouse.log` in the system temp directory |

</details>

---

## Preview Coverage

elio previews many file types directly in the Preview pane:

- Text, source code, Markdown, logs, and structured data such as JSON, YAML, TOML, CSV/TSV, and SQLite
- Documents such as PDFs, ebooks, Office files, OpenDocument files, and Apple Pages
- Images, audio, and video metadata, with inline images, covers, and thumbnails when supported
- Folders, archives, comic archives, torrents, ISO images, and other disk-image-style containers
- Binary files with useful metadata when no richer preview is available

Some richer previews require optional system tools such as Poppler, FFmpeg, resvg, 7-Zip, unar, or FontForge.

See the preview docs:
https://elio-fm.github.io/docs/previews/

---

## Optional Preview Tools

elio works out of the box, but a few external tools enable richer previews for specific file types:

- `poppler` for PDF previews
- `ffmpeg` / `ffprobe` for media metadata and thumbnails
- `resvg` for SVG previews
- `7z` / `unar` for archive previews
- `fontforge` for font previews

See the full optional tools list and package names in the docs:
https://elio-fm.github.io/docs/optional-tools/

---

## Using elio over SSH

elio works well over SSH for navigation, file operations, text/code previews, and terminal-based workflows.

Rich visual previews depend on the remote host and the local terminal:

- Text and code previews work normally.
- Images, PDF pages, video thumbnails, album art, SVG previews, and archive extras need terminal image support plus optional preview tools installed on the remote host.
- Terminal apps opened through `Open With` run inside the SSH session.
- System openers such as `Enter`, `o`, `open`, `gio`, `xdg-open`, or `cmd /c start` use the remote host's opener stack. From an SSH session, they may open on the remote host, fail, or do nothing useful.

---

## Workflow

elio supports common file-manager actions directly from the keyboard:

- `Enter` opens folders or selected files with the system default application.
- `O` opens files through the Open With flow when supported, including terminal apps.
- `c` copies names, paths, or directory paths using OSC52 or the platform clipboard.
- `g` opens quick jumps for common locations such as Home, Downloads, config, and Trash.
- `d` moves files to Trash; `D` deletes permanently; in the Trash view, `d` also deletes permanently and `r` restores.
- `f` searches folders, while `Ctrl+F` searches files in the current tree.
- `z` jumps through zoxide history when `zoxide` is installed.
- `!` opens a shell in the current folder and returns to elio when the shell exits.

Platform details vary for clipboard tools, trash backends, file openers, and shell selection. See the workflow docs:
https://elio-fm.github.io/docs/workflow/

---

## Change Directory on Quit

elio can leave your shell in the directory you were browsing when you quit:

```bash
elio shell install
```

Restart your shell, then run `elio` normally. Press `q` to quit and move your shell to elio's final directory, or `Q` to quit without changing directories.

See the shell integration docs for uninstall steps, supported shells, and manual setup:
https://elio-fm.github.io/docs/shell-integration/

---

## Configuration

elio reads configuration from:

| Platform | Config file |
|---|---|
| Linux / BSD | `~/.config/elio/config.toml` or `$XDG_CONFIG_HOME/elio/config.toml` |
| macOS | `~/Library/Application Support/elio/config.toml` |
| Windows | `%APPDATA%\elio\config.toml` |

See [`examples/config.toml`](examples/config.toml) for an annotated example, or the configuration docs:
https://elio-fm.github.io/docs/configuration/

---

## Theming

elio themes are TOML files layered on top of the built-in defaults, so you only need to set the keys you want to change.

| Platform | Theme file |
|---|---|
| Linux / BSD | `~/.config/elio/theme.toml` or `$XDG_CONFIG_HOME/elio/theme.toml` |
| macOS | `~/Library/Application Support/elio/theme.toml` |
| Windows | `%APPDATA%\elio\theme.toml` |

See [`assets/themes/default/theme.toml`](assets/themes/default/theme.toml) for the full default theme and [`examples/themes/`](examples/themes/) for ready-made themes.

For transparent or terminal-palette setups, see [`examples/themes/transparent/theme.toml`](examples/themes/transparent/theme.toml) and [`examples/themes/terminal-ansi/theme.toml`](examples/themes/terminal-ansi/theme.toml).

Theme docs:
https://elio-fm.github.io/docs/themes/

---

<details>
<summary><strong>Controls</strong></summary>

Keys marked with `*` are configurable in `[keys]` in `config.toml`; the defaults are shown here. Configurable actions accept either one key or a list, such as `open_with = ["O", "w"]`. Navigation actions also accept `left`, `right`, `up`, and `down`. Setting an action replaces its full default key list.

### Navigation

| Key | Action |
|---|---|
| `k` / `↑` `*` | Move up |
| `j` / `↓` `*` | Move down |
| `h` / `←` `*` / `Backspace` | Go to parent directory |
| `l` / `→` `*` | Enter folder |
| `Enter` | Enter folder / open file or selection |
| `g` | Go-to menu (`g` top, `d` downloads, `h` home, `c` config folder, `t` trash) |
| `G` | Jump to last item |
| `PageUp` / `PageDown` | Page up / down |
| `Tab` / `Shift+Tab` | Cycle places |
| `Alt+←` / `Alt+→` | Back / forward in history |

### Search

| Key | Action |
|---|---|
| `f` `*` | Fuzzy-find folders in the current tree |
| `Ctrl+F` | Fuzzy-find files in the current tree |
| `z` `*` | Jump with zoxide directory history |

### File Actions

| Key | Action |
|---|---|
| `o` `*` | Open focused item or selection with the system default application |
| `O` `*` | Open With chooser |
| `!` `*` | Open shell in current folder |
| `a` `*` | Create file or folder |
| `d` `*` | Trash; permanently delete if already in trash |
| `D` `*` | Delete permanently |
| `r` `*` | Rename / bulk rename / restore from trash |
| `F2` | Rename / bulk rename |

### View

| Key | Action |
|---|---|
| `v` `*` | Toggle grid / list view |
| `+` / `-` | Grid zoom in / out |
| `.` `*` | Show / hide dotfiles |
| `s` `*` | Cycle sort (Name → Modified → Size) |

### Preview

| Key | Action |
|---|---|
| `Shift+K` / `Shift+J` `*` | Step page (PDF, comic, EPUB) or scroll preview up / down |
| `Shift+H` / `Shift+L` `*` | Scroll preview left / right |
| `[` / `]` | Step page (PDF, comic, EPUB) or scroll text/code |

### Selection and Clipboard

| Key | Action |
|---|---|
| `Space` | Toggle selection |
| `Ctrl+A` | Select all |
| `y` `*` | Yank (copy) |
| `x` `*` | Cut |
| `p` `*` | Paste |
| `c` `*` | Copy path details to clipboard |

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
| `q` `*` | Quit |
| `Q` `*` | Quit without changing the shell directory |

</details>

---

## License

[MIT](LICENSE-MIT)
