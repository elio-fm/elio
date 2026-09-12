# Architecture

This crate is organized around focused subsystems.

- `cli` and `shell_integration`: command-line behavior and shell setup.
- `fs` and `file_classification`: filesystem access and file, format, and code-language identification.
- `file_operations`: workflows that create, rename, copy, move, trash, restore, or archive items.
- `background_jobs`: shared job scheduling, workers, requests, and result messages.
- `archive` and `opening`: archive operations and launching items with applications.
- `preview`: preview construction plus document and image inspection, preparation, and rendering.
- `theme`: palettes and file appearance rules shared by rendered interfaces.
- `app`: application state, input dispatch, preview coordination, and applying background-job
  results.
- `elevated_session`: privileged filesystem operations through sudo or doas.
- `terminal_runtime`: application startup, terminal lifecycle, event loop, drawing, terminal-image
  protocols, and session output.
- `ui`: terminal rendering and layout.

Current boundary rules:

- Shared model types live in the narrowest subsystem that owns their responsibility rather than in
  a generic shared module.
- Filesystem mutations belong to `file_operations`; their background workers remain centralized in
  `background_jobs` with Elio's other workers.
- `fs` and `file_classification` should not depend on `app`.
- Code-language recognition belongs to `file_classification`, not to a preview renderer.
- Preview construction and media processing in `preview` should not depend on `app` or
  `background_jobs`.
- Active-preview caching, navigation, and terminal-image presentation live in `app/preview`; the
  independent `preview` subsystem constructs preview content and prepares image and PDF assets.
- `preview` should not reach into `theme` directly. The explicit adapter boundary for theme
  access is `src/preview/appearance.rs`.
- `app` owns behavior; it should not be the home for generic data model types that other
  layers need.

These rules are enforced by the architecture guardrail test and CI. Keep this document focused on
rules that the codebase actually follows and that tooling can check.
