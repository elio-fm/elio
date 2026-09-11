# Architecture

This crate is organized around focused subsystems.

- `cli` and `shell_integration`: command-line behavior and shell setup.
- `fs` and `file_classification`: filesystem access and file, format, and code-language identification.
- `file_operations`: workflows that create, rename, copy, move, trash, restore, or archive items.
- `archive` and `opening`: archive operations and launching items with applications.
- `preview`: preview construction and rendering-oriented preview data.
- `theme`: palettes and file appearance rules shared by rendered interfaces.
- `app`: application state, input dispatch, and centralized background-job infrastructure.
- `elevated_session`: privileged filesystem operations through sudo or doas.
- `terminal_runtime`: application startup, terminal lifecycle, event loop, drawing, and session output.
- `ui`: terminal rendering and layout.

Current boundary rules:

- Shared model types live in the narrowest subsystem that owns their responsibility rather than in
  a generic shared module.
- Filesystem mutations belong to `file_operations`; their background workers remain centralized in
  `app/jobs` with Elio's other workers.
- `fs` and `file_classification` should not depend on `app`.
- Code-language recognition belongs to `file_classification`, not to a preview renderer.
- `preview` is presentation code, but it should not depend on `app`.
- `preview` should not reach into `theme` directly. The explicit adapter boundary for theme
  access is `src/preview/appearance.rs`.
- `app` owns behavior; it should not be the home for generic data model types that other
  layers need.

These rules are enforced by the architecture guardrail test and CI. Keep this document focused on
rules that the codebase actually follows and that tooling can check.
