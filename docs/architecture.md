# Architecture

This crate is organized around a small set of layers.

- `core`: shared, dependency-light model types that multiple layers need.
- `fs` and `file_info`: filesystem access, file classification, and metadata discovery.
- `preview`: preview construction and rendering-oriented preview data.
- `app`: runtime coordination, state, jobs, and user actions.
- `ui`: terminal rendering, layout, and theming.

Current boundary rules:

- Shared model types that multiple layers need, such as file-model and sidebar types, live in
  `src/core/`, not in `src/app/`.
- `fs` and `file_info` may depend on `core`, but should not depend on `app`.
- `preview` is presentation code, but it should not depend on `app`.
- `preview` should not reach into `ui::theme` directly. The explicit adapter boundary for theme
  access is `src/preview/appearance.rs`.
- `app` coordinates behavior; it should not be the home for generic data model types that other
  layers need.

These rules are enforced by the architecture guardrail test and CI. Keep this document focused on
rules that the codebase actually follows and that tooling can check.
