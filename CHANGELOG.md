# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
