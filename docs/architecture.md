# Architecture

This crate is organized around focused subsystems.

- `cli` and `shell_integration`: command-line behavior and shell setup.
- `config`: configuration loading, validation, and key bindings.
- `filesystem` and `file_classification`: filesystem access and file, format, and code-language
  identification.
- `file_browser`: current-directory state, loading, navigation, selection, filtering, and item
  counts.
- `file_operations`: state, requests, and actions for creating, renaming, copying, moving,
  trashing, restoring, or archiving items.
- `system_clipboard`: platform-neutral access to native text and file-item clipboard formats,
  with private OS protocol backends.
- `background_jobs`: shared job scheduling, workers, requests, and result messages.
- `chooser`: chooser-mode selection, confirmed or cancelled outcome, and selected-path output.
- `portal`: XDG Desktop Portal FileChooser integration on Linux and FreeBSD, including backend
  activation metadata, desktop-specific routing, frontend refresh, D-Bus service handling, and
  private chooser transport.
- `fuzzy_finder` and `duplicate_finder`: feature state and behavior for finding items.
- `goto_menu`: configured Go To entries and destination resolution.
- `places`: configured places and mounted-device discovery.
- `input_handling`: application-level keyboard, mouse, paste, and wheel interpretation.
- `archive` and `opening`: archive operations and launching items with applications.
- `preview`: preview construction plus format-specific inspection, preparation, and rendering.
- `theme`: palettes and file appearance rules shared by rendered interfaces.
- `app`: remaining cross-subsystem state, preview coordination, actions, and application of
  background-job results.
- `elevated_session`: privileged filesystem operations through sudo or doas.
- `terminal_images`: terminal detection, image protocols, placement, clearing, and geometry.
- `terminal_runtime`: terminal lifecycle, raw input acquisition, event loop, drawing, and session
  output.
- `ui`: terminal rendering, panes, overlays, and layout.

Current boundary rules:

- Shared model types live in the narrowest subsystem that owns their responsibility rather than in
  a generic shared module.
- User-facing filesystem-mutation state and requests belong to `file_operations`; reusable
  filesystem primitives, including trash metadata and restoration, belong to `filesystem`.
  `file_operations` returns explicit outcomes instead of depending on `app`; `input_handling`
  supplies browser context and coordinates navigation or job submission. Background workers remain
  centralized in `background_jobs` with elio's other workers.
- The internal clipboard (the `y`/`x`/`p` file-operation selection) belongs to
  `file_operations`. Native system clipboard reads and writes belong to `system_clipboard`;
  it returns platform-neutral text or file-item values and must not depend on input handling,
  UI, file-operation state, or background jobs. `input_handling` translates a system file
  clipboard paste into a file-operation request. Drag-and-drop is a separate pointer-transfer
  path owned by terminal runtime/input handling: it may share file-operation validation and
  workers with clipboard paste, but keeps separate entry points, origins, statuses, and protocol
  semantics.
- Current-directory browsing state and pure browser transitions belong to `file_browser`; places
  pane state remains in `places`. Git status for the current directory and the browser-item drag
  selection also belong to `file_browser`; job execution, mouse hit testing, and terminal drag
  protocols remain at their respective boundaries.
- Fuzzy-finder, duplicate-finder, Go To menu, and Open With state and pure transitions belong to
  their feature subsystems; `input_handling` interprets user interaction, while `app` coordinates
  background jobs, navigation, and side effects.
- Chooser selection resolution and output belong to `chooser`; `app` only supplies the current
  browser selection and coordinates application exit.
- `portal` owns XDG portal lifecycle, routing, D-Bus protocol handling, and private chooser
  transport. It reuses `chooser` for selection semantics and `terminal_runtime` for the chooser
  UI; it should not depend on `app`.
- Top-level startup orchestration initializes `config` and `theme` before handing control to
  `terminal_runtime`.
- Raw Crossterm and Kitty input acquisition belongs to `terminal_runtime`; mapping those events to
  elio behavior belongs to `input_handling`.
- `filesystem` and `file_classification` should not depend on `app`.
- Code-language recognition belongs to `file_classification`, not to a preview renderer.
- Preview construction and media processing in `preview` should not depend on `app` or
  `background_jobs`.
- Active-preview state, caching, and format-specific sessions belong to `preview`; the remaining
  `app/preview` code coordinates that state with application navigation and terminal presentation.
- `preview` should not reach into `theme` directly. The explicit adapter boundary for theme
  access is `src/preview/appearance.rs`.
- `app` owns cross-subsystem coordination; generic data model types belong to the narrowest
  subsystem responsible for them.

Key dependency rules are enforced by the architecture guardrail test and CI. Keep this document
focused on boundaries the codebase actually follows.
