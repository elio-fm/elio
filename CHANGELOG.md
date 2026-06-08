# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added `--chooser-file FILE [PATH]` to run elio as a file chooser, writing the confirmed selection as absolute paths, one per line, to `FILE` or stdout with `-`; cancel exits silently with a nonzero status. ([#153])
- `elio <path>` now accepts file paths as well as directories, opening the parent directory and focusing the file entry, including hidden files, file symlinks, and broken symlinks.

### Changed

- Updated shell integration scripts to pass `--chooser-file` invocations directly to `elio`, so chooser mode does not change the parent shell directory. Re-run `elio shell install` after upgrading to refresh existing shell integration.

## [1.8.0] - 2026-06-06

### Added

- Added `restore_from_trash` and function-key bindings such as `F2`, letting restore and rename be configured separately while keeping the default `r` restore behavior in Trash and `r`/`F2` rename behavior elsewhere.
- Added support for assigning multiple bindings to each configurable `[keys]` action, while keeping existing single-string key config compatible.
- Added configurable navigation bindings for `nav_left`, `nav_down`, `nav_up`, and `nav_right`, including named arrow keys.
- Added `open_or_enter` as a configurable binding for the existing `Enter` behavior: entering folders or opening files. ([#141])
- Added `[]` support for unbinding configurable `[keys]` actions.
- Added support for modifier `[keys]` bindings such as `ctrl+o`, `alt+o`, and `shift+right`.
- Added named `[keys]` values for `space`, `tab`, `backtab` / `shift+tab`, `backspace`, `pageup`, `pagedown`, `home`, and `end`.
- Added configurable browser control bindings for `go_to`, `toggle_selection`, `cycle_places_next`, `cycle_places_previous`, `go_parent`, `page_up`, `page_down`, `jump_first`, and `jump_last`.
- Added configurable `[keys]` bindings for `quit_without_cd`, `search_files`, `select_all`, `history_back`, and `history_forward`, plus `[` / `]` defaults for the existing `scroll_preview_up` / `scroll_preview_down` actions.
- Added a configurable `D` (`delete_permanently`) shortcut for permanently deleting selected entries without first opening Trash. ([#140])

### Changed

- Updated the help overlay to reflect the new configurable bindings and clearer quit-without-cd wording.
- Warn when `[keys]` contains unknown action names, while still applying valid key bindings.

## [1.7.0] - 2026-05-30

### Added

- Added a `terminal-ansi` theme example and support for terminal ANSI colors in `theme.toml`. ([#84])
- Added `--cwd-file` and installable bash, zsh, fish, and Nushell integration for changing the parent shell directory to elio's final directory on quit, including quit-without-cd, current-shell detection, zsh `ZDOTDIR` support, symlink-aware config updates, and a matching uninstall command. ([#69], [#129])

### Changed

- Simplified directory preview headers by hiding the `Contents` label. ([#125])
- Updated the transparent theme example to make popups and path surfaces fully transparent, and removed comments from example themes.

### Fixed

- Improved mouse double-click behavior when a selection is active.
- Fixed `o` / `Enter` on Linux and BSD to open default terminal-based apps in the current terminal when opening a single file instead of a separate terminal window. ([#134])
- Updated `tar` from `0.4.45` to `0.4.46` to address upstream advisory `GHSA-3cv2-h65g-fgmm`. ([#138])
- Improved popup rendering over image previews, preventing preview content from showing through transparent overlay cells across Kitty/Ghostty, WezTerm/iTerm2, Foot, and Windows Terminal.
- Fixed WezTerm and iTerm2 inline image previews during terminal resize, keeping previews and popups correctly layered.

## [1.6.0] - 2026-05-22

### Added

- Added `!` to open the user's shell in the current folder and return to elio when the shell exits. ([#112])
- Added bulk opening for selected items: `Enter` and `o` now open the current selection instead of only the focused row. ([#111])
- Added symlink-aware rendering in the file browser, directory previews, and Places, with inline `-> target` details, dedicated icons for symlinked folders and broken links, and a broken-link preview. Symlinked files keep their normal file-type appearance.
- Added themable `symlink_directory` and `broken_symlink` classes, whose default colors track the `directory` class color and `preview.code.invalid` unless explicitly overridden.

### Fixed

- Fixed quit stalls after browsing special files or large archives by avoiding blocking special-file reads, canceling archive listing commands, and sending oversized ZIP/CBZ archives through the bounded external listing path. ([#121])
- Improved fuzzy search for large directory trees: matches now appear while scanning continues, stale scans are canceled automatically, the scan cap is much higher, and the overlay reports `scan limit reached` when the cap is hit.
- Fixed fuzzy search so symlinked folders, symlinked files, and broken symlinks appear as searchable entries. Linked directories are listed but not descended into.
- Fixed `Tab` / `Shift+Tab` cycling and the active highlight for symlinked Places entries, which previously reset to Home after opening the symlink target. ([#109])

## [1.5.1] - 2026-05-15

### Added

- Added Linux desktop entry metadata and hicolor application icons for packaged installs, allowing desktop launchers to discover elio as a terminal file manager. ([#67])
- Added `amd64` Debian package assets and official apt repository publishing for Debian/Ubuntu installs, including the Linux desktop entry and hicolor application icons in the package.

## [1.5.0] - 2026-05-14

### Added

- Added a configurable `z` shortcut for jumping to directories with `zoxide query -i`, excluding the current directory from the picker and reporting missing zoxide, empty history, or history containing only the current directory clearly. ([#103])
- Added `"none"` (alias: `"transparent"`) as accepted color values in `theme.toml`, resetting foreground or background colors to the terminal default. For background fields, this lets transparent terminals show through. See `examples/themes/transparent/theme.toml`. ([#86])
- Added a `chip_text` palette field that controls the foreground of toolbar status chips (yank, cut, selected, trash, restore). Defaults to `#0c0c0c` on all themes; previously this color was reused from `chrome`. ([#86])

### Changed

- Improved chip text contrast on the bundled light themes (`default-light`, `blush-light`) as a side effect of the new `chip_text` palette field — chips now use a dark default fg instead of the theme's light chrome color. ([#86])
- Improved Kitty and Ghostty image preview auto-detection inside tmux when tmux hides the usual terminal environment markers. ([#70])

### Fixed

- Auto-enable tmux `allow-passthrough` for image previews in supported terminals, so users no longer need to configure it manually.
- Fixed Sixel preview support inside tmux, including Foot and Windows Terminal detection from tmux client/session environment. Windows Terminal now uses tmux's native Sixel path to avoid corrupted alternate-screen rendering in WSL. ([#70])
- Fixed undersized Windows Terminal Sixel previews on WSL outside tmux by querying the terminal cell pixel size before falling back to default cell dimensions.
- Fixed iTerm inline preview transport and placement inside tmux, including correct pane-relative positioning and compact cached payloads for large JPEG/GIF previews that could otherwise lag or disappear. ([#70])
- Fixed Kitty direct-placement preview transport and placement inside tmux for Konsole and Warp. ([#70])

## [1.4.0] - 2026-05-03

### Added

- Added an optional startup directory argument, so `elio <directory>` opens that directory and invalid or non-directory paths return a clear error.
- Added keyboard-driven preview scrolling that mirrors `[` / `]`: `Shift+K` / `Shift+J` (configurable) step pages on PDF, comic, and EPUB previews and otherwise scroll the preview up / down. `[` / `]` now also scrolls text/code/log previews while keeping its page-step behavior on paged previews. ([#79])
- Added a dedicated Preview section to the help overlay (`?`) listing the new vertical scroll keys, `[` / `]` page-stepping, and the horizontal scroll keys, and rebalanced the overlay columns so both sides end at the same height.

### Changed

- Changed the default horizontal preview scroll bindings from `<` / `>` to `Shift+H` / `Shift+L` so all four preview scroll directions share a consistent vim-style modifier pattern. Existing `scroll_preview_left` / `scroll_preview_right` overrides in `config.toml` continue to work unchanged.

### Fixed

- Fixed Linux/BSD default-app dispatch and Open With MIME detection to prefer GLib's MIME resolution before falling back to `xdg-open` or `xdg-mime`, keeping system default launches aligned with desktop MIME associations. ([#77])
- Fixed Warp inline image and PDF previews by using Kitty direct placement instead of Kitty Unicode placeholders. ([#75])
- Fixed tmux relaying of Kitty Graphics preview sequences so inline image and PDF previews can render inside tmux when `KittyGraphics` is selected and `allow-passthrough` is enabled. Some tmux setups may still require preserved terminal markers or `ELIO_IMAGE_PREVIEWS=1`. ([#74], [#70])

## [1.3.0] - 2026-04-28

### Added

- Added Konsole inline image preview support with a dedicated backend and conservative popup clearing to avoid preview artifacts.

### Fixed

- Fixed `cargo install elio` from crates.io by upgrading `lofty` from the yanked `0.23` series to `0.24`. Thanks @jprobichaud for catching this in [#66].

## [1.2.0] - 2026-04-25

### Added

- Added QML source file support with syntax-highlighted code previews.
- Added a dedicated QML file icon in the built-in browser theme.
- Added metadata previews for TrueType, OpenType, WOFF, and WOFF2 font files, replacing the generic binary fallback.

### Changed

- Improved large-directory navigation responsiveness by deferring browser directory counts and recursive directory totals until navigation settles, canceling stale directory reload work, and scaling polling reload cadence with directory size.

### Fixed

- Fixed compact file browser rows so folder item counts and file sizes stay visually aligned across singular/plural counts and differing size units.
- Fixed browser license icons so canonical license files, SPDX-marked text files, and standalone license documents keep the license appearance during fast-path row rendering.
- Fixed truncated directory previews so sampled entry counts are no longer shown as exact totals before background directory stats finish loading.

## [1.1.0] - 2026-04-18

### Added

- Added RAR archive previews using the existing external archive listing backends, with `unrar` as an additional fallback when available.
- Added non-image comic archive previews for CBZ and CBR files, using embedded XML/comment metadata or conservative structured-name fallbacks instead of showing an empty pane.
- Added MOBI and AZW3 ebook classification, book icons, native metadata previews, and cover image previews for Kindle ebook files.

### Changed

- Improved fuzzy search indexing and filtering responsiveness for large directory trees.
- Simplified document metadata previews by keeping author, dates, application, stats, and extra metadata fields in the `Details` section.
- Made MOBI and AZW3 cover previews larger while preserving room for document details.
- Made full-height EPUB image sections advance on preview scroll without first scrolling hidden context lines.
- Kept RAR archive loading previews silent while archive contents are inspected in the background.
- Documented fuzzy search scope, hidden-file handling, pruning, refresh behavior, and large-tree caps.
- Documented Trash behavior across Linux, BSD, macOS, and Windows.
- Prefer `gio trash` on Linux before falling back to the Freedesktop Trash layout for desktop-compatible trashing.

### Fixed

- Fixed fuzzy search reusing stale indexes after directory reloads, so pasted, cut, deleted, or newly created entries are reflected after filesystem changes.
- Fixed Freedesktop Trash entries with collision-suffixed storage names, such as `photo.jpg.2`, so they display, preview, open, and restore using their original `.trashinfo` names.
- Fixed stacked browser layouts so the Preview pane expands in tall narrow terminals and respects configured Files/Preview pane weights.
- Fixed metadata previews for large ZIP-based office documents, including PPTX, PPTM, ODP, DOCX, XLSX, and Pages files.
- Clarified document metadata preview sections by replacing the repeated `Document` body heading with `Details` and keeping `People` for author fields.
- Fixed fixed-layout EPUB pages without extractable text so the preview shows page and book context instead of an empty pane.
- Clarified media and binary metadata previews by using `Details` instead of repeating `Video`, `Audio`, `Image`, or `Binary` as the first body section.
- Clarified archive metadata previews by using `Details` instead of `Summary`, `Image`, or `Torrent` for the first body section.
- Clarified SQLite database previews by using `Details` for the first metadata section.

## [1.0.1] - 2026-04-12

### Added

- Added `--help`/`-h` and `--version`/`-V` CLI flags.
- Added release packaging automation for AUR, Fedora COPR, and Homebrew, including Homebrew bottle publishing.

## [1.0.0] - 2026-04-10

### Added

- Initial public release of `elio`.
- Three-pane interface with dedicated Places, Files, and Preview columns.
- Rich preview support for text, code, structured data, documents, archives, media, directories, and binary metadata.
- Inline image previews for supported terminals through Kitty Graphics, iTerm2 Inline, and Sixel backends.
- Keyboard and mouse navigation, list and grid views, and fuzzy search for efficient browsing.
- Configurable Places, theme overrides, pane layout settings, and browser key bindings.
- Quick actions including Go-to, Open With, clipboard copy, and system opener integration.
- Trash and restore support for safer file management workflows.
- Optional external-tool integrations such as Poppler, ffmpeg, ffprobe, resvg, and 7-Zip for richer previews and metadata.

[Unreleased]: https://github.com/elio-fm/elio/compare/v1.8.0...HEAD
[1.8.0]: https://github.com/elio-fm/elio/compare/v1.7.0...v1.8.0
[1.7.0]: https://github.com/elio-fm/elio/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/elio-fm/elio/compare/v1.5.1...v1.6.0
[1.5.1]: https://github.com/elio-fm/elio/compare/v1.5.0...v1.5.1
[1.5.0]: https://github.com/elio-fm/elio/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/elio-fm/elio/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/elio-fm/elio/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/elio-fm/elio/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/elio-fm/elio/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/elio-fm/elio/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/elio-fm/elio/releases/tag/v1.0.0
[#86]: https://github.com/elio-fm/elio/issues/86
[#84]: https://github.com/elio-fm/elio/issues/84
[#79]: https://github.com/elio-fm/elio/issues/79
[#77]: https://github.com/elio-fm/elio/pull/77
[#75]: https://github.com/elio-fm/elio/issues/75
[#74]: https://github.com/elio-fm/elio/pull/74
[#70]: https://github.com/elio-fm/elio/issues/70
[#67]: https://github.com/elio-fm/elio/issues/67
[#66]: https://github.com/elio-fm/elio/pull/66
[#69]: https://github.com/elio-fm/elio/issues/69
[#103]: https://github.com/elio-fm/elio/issues/103
[#109]: https://github.com/elio-fm/elio/issues/109
[#111]: https://github.com/elio-fm/elio/issues/111
[#112]: https://github.com/elio-fm/elio/issues/112
[#121]: https://github.com/elio-fm/elio/issues/121
[#125]: https://github.com/elio-fm/elio/issues/125
[#129]: https://github.com/elio-fm/elio/issues/129
[#134]: https://github.com/elio-fm/elio/issues/134
[#138]: https://github.com/elio-fm/elio/pull/138
[#140]: https://github.com/elio-fm/elio/issues/140
[#141]: https://github.com/elio-fm/elio/issues/141
[#153]: https://github.com/elio-fm/elio/issues/153
