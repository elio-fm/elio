# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added RAR archive previews using the existing external archive listing backends, with `unrar` as an additional fallback when available.
- Added non-image comic archive previews for CBZ and CBR files, using embedded XML/comment metadata or conservative structured-name fallbacks instead of showing an empty pane.
- Added MOBI and AZW3 ebook classification, book icons, and native metadata previews for Kindle ebook files.

### Changed

- Improved fuzzy search indexing and filtering responsiveness for large directory trees.
- Simplified document metadata previews by keeping author, dates, application, and stats in the `Details` section.
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

[Unreleased]: https://github.com/elio-fm/elio/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/elio-fm/elio/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/elio-fm/elio/releases/tag/v1.0.0
