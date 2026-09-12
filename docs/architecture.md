# Architecture

This crate is organized around focused subsystems.

- `cli` and `shell_integration`: command-line behavior and shell setup.
- `fs` and `file_classification`: filesystem access and file, format, and code-language identification.
- `file_browser`: current-directory state, loading, navigation, selection, filtering, and item
  counts.
- `file_operations`: workflows that create, rename, copy, move, trash, restore, or archive items.
- `background_jobs`: shared job scheduling, workers, requests, and result messages.
- `fuzzy_finder` and `duplicate_finder`: feature state and behavior for finding items.
- `goto_menu`: configured Go To entries and destination resolution.
- `input_handling`: application-level keyboard, mouse, paste, and wheel interpretation.
- `archive` and `opening`: archive operations and launching items with applications.
- `preview`: preview construction plus document and image inspection, preparation, and rendering.
- `theme`: palettes and file appearance rules shared by rendered interfaces.
- `app`: remaining cross-subsystem state, preview coordination, actions, and application of
  background-job results.
- `elevated_session`: privileged filesystem operations through sudo or doas.
- `terminal_runtime`: application startup, terminal lifecycle, raw input acquisition, event loop,
  drawing, terminal-image protocols, and session output.
- `ui`: terminal rendering and layout.

Current boundary rules:

- Shared model types live in the narrowest subsystem that owns their responsibility rather than in
  a generic shared module.
- Filesystem mutations belong to `file_operations`; their background workers remain centralized in
  `background_jobs` with Elio's other workers.
- Current-directory browsing state and pure browser transitions belong to `file_browser`; places
  pane state remains in `places`. Git status for the current directory and the browser-item drag
  selection also belong to `file_browser`; job execution, mouse hit testing, and terminal drag
  protocols remain at their respective boundaries.
- Fuzzy-finder, duplicate-finder, Go To menu, and Open With state and pure transitions belong to
  their feature subsystems; `input_handling` interprets user interaction, while `app` coordinates
  background jobs, navigation, and side effects.
- Raw Crossterm and Kitty input acquisition belongs to `terminal_runtime`; mapping those events to
  Elio behavior belongs to `input_handling`.
- `fs` and `file_classification` should not depend on `app`.
- Code-language recognition belongs to `file_classification`, not to a preview renderer.
- Preview construction and media processing in `preview` should not depend on `app` or
  `background_jobs`.
- Active-preview state, caching, and format-specific sessions belong to `preview`; the remaining
  `app/preview` code coordinates that state with application navigation and terminal presentation.
- `preview` should not reach into `theme` directly. The explicit adapter boundary for theme
  access is `src/preview/appearance.rs`.
- `app` owns behavior; it should not be the home for generic data model types that other
  layers need.

These rules are enforced by the architecture guardrail test and CI. Keep this document focused on
rules that the codebase actually follows and that tooling can check.
