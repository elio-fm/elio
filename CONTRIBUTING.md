# Contributing to elio

Thank you for your interest in contributing to `elio`.

Contributions are welcome across bug fixes, platform fixes, preview improvements, UI polish, documentation, tests, themes, and general maintenance.

This guide explains how to get productive in the repository and what is expected before opening a pull request.

## Getting Started

Before you start:

- Install Rust using [rustup](https://rustup.rs/).
- Make sure Git is available locally.
- Read the main project overview in [`README.md`](README.md).

Clone your fork and create a branch for your work:

```bash
git clone https://github.com/<your-username>/elio.git
cd elio
git checkout -b your-branch-name
```

The repository includes [`rust-toolchain.toml`](rust-toolchain.toml), which pins Rust `1.93.0` and installs the required `clippy` and `rustfmt` components automatically.

## Project Structure

A brief overview of the repository layout:

```text
.
├── .github/                    # GitHub workflows and repository automation
├── assets/                     # Bundled assets such as the logo, themes, and syntax data
├── docs/                       # Project documentation, including architecture notes
├── examples/                   # Example config and theme files
├── src/
│   ├── app/                    # Runtime coordination, state, jobs, and user actions
│   ├── config/                 # Config and theme loading/parsing
│   ├── core/                   # Shared model types used across layers
│   ├── file_info/              # File classification and metadata discovery
│   ├── fs/                     # Filesystem access and path-level operations
│   ├── preview/                # Preview construction and preview-specific tests
│   ├── ui/                     # Terminal rendering, layout, theming, and interaction
│   ├── lib.rs                  # Library entry and application runtime wiring
│   └── main.rs                 # Binary entrypoint
├── tests/
│   └── architecture_guardrails.rs  # Enforced dependency-boundary checks
├── build.rs                    # Build-time asset preparation
├── CHANGELOG.md                # Release notes and unreleased user-facing changes
├── CONTRIBUTING.md             # Contributor guide
├── Cargo.toml                  # Package manifest and dependency configuration
└── README.md                   # Project overview and user documentation
```

If you are not sure where a change belongs, start by reading [`docs/architecture.md`](docs/architecture.md).

## Development Workflow

Typical local workflow:

```bash
cargo build
cargo run --release
```

`elio` starts in the current working directory, so running it from a test folder is often the easiest way to reproduce navigation, preview, and layout behavior.

For configuration and theme work, use the examples in [`examples/config.toml`](examples/config.toml) and [`examples/themes/`](examples/themes/).

## Local Checks

Before opening a pull request, run the same checks expected by CI:

```bash
cargo fmt --check
cargo test --locked --test architecture_guardrails
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
```

If your change touches preview behavior, file classification, platform integration, or optional-tool handling, also test the feature manually in the terminal where the issue occurs.

## Preview and Platform Changes

`elio` has a large platform and environment surface area. Preview behavior can vary based on:

- terminal image protocol support
- installed helper tools such as `ffmpeg`, `ffprobe`, `pdfinfo`, `pdftocairo`, `resvg`, `7z`, `bsdtar`, and `isoinfo`
- desktop integration differences across Linux, macOS, Windows, and BSD

When making changes in these areas:

- prefer narrow, explicit fallbacks over broad heuristic changes
- document any new external-tool assumptions in [`README.md`](README.md)
- add or update tests where practical
- mention the platform and terminal used when describing the change in a pull request

For the broadest local preview-test coverage, install the optional archive and PDF tools used by the test suite, especially `7z`, `bsdtar`, `isoinfo`, `pdfinfo`, `pdftocairo`, and `xz`.

## Architecture Expectations

`elio` enforces a small set of architectural boundaries. In particular:

- `src/core/` holds shared model types that multiple layers need
- `src/fs/` and `src/file_info/` may depend on `core`, but not on `app`
- `src/preview/` must not depend on `app`
- `src/preview/` must not reach into `ui::theme` directly; the adapter boundary is `src/preview/appearance.rs`

These rules are checked by [`tests/architecture_guardrails.rs`](tests/architecture_guardrails.rs). Treat those tests as intentional design constraints, not incidental test coverage.

## Pull Requests

Pull requests are easier to review and merge when they are focused and explicit.

Please aim to:

- keep each pull request scoped to a single change or closely related set of changes
- update documentation when behavior, configuration, controls, or optional dependencies change
- include screenshots when changing visible UI, layout, or theme behavior
- call out platform-specific behavior when the change affects Linux, macOS, Windows, or BSD differently
- add or update tests for regressions, parsing logic, preview logic, or boundary rules when appropriate

If you are proposing a larger feature or behavioral change, open an issue or discussion first so the approach can be aligned before substantial implementation work begins.

## Changelog

`CHANGELOG.md` is maintained on `main` through the `Unreleased` section.

When a pull request changes user-visible behavior, add a short entry under `## [Unreleased]` in the appropriate category such as `Added`, `Changed`, or `Fixed`.

Changelog updates are not required for purely internal refactors, test-only changes, or CI and documentation maintenance that does not affect users.

During release preparation, `Unreleased` entries are curated into a dated version section.

## Security Auditing

Run `cargo audit --deny unsound` to check for security advisories.

The project currently has known `unmaintained` advisories in transitive dependencies. Those are tracked, but they are not release blockers. `unsound` advisories are treated as hard blockers.

## Release Process

Releases are cut from a commit on `main` that has already passed CI.

Before tagging a release:

- update `version` in `Cargo.toml`
- regenerate `Cargo.lock` so the root package version matches
- move the upcoming notes from `Unreleased` into a new `## [x.y.z] - YYYY-MM-DD` section in `CHANGELOG.md`
- confirm the release commit is on `main` and CI is green

To publish a release:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

The release workflow validates the tag and crate metadata, extracts release notes from `CHANGELOG.md`, runs `cargo test --locked`, builds release artifacts for Linux, macOS, and Windows, creates or updates the GitHub release, publishes the crate to crates.io, and then updates downstream AUR, COPR, and Homebrew packaging.

If the crates.io publish step fails for a transient external reason, rerun the workflow for the same tag. If the tagged commit itself is wrong, fix the issue in a new commit, update the version and changelog as needed, and cut a new tag.
