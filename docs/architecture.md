# Architecture

This crate is organized around a small set of layers.

- `core`: shared, dependency-light model types that multiple layers need.
- `fs` and `file_info`: filesystem access, file classification, and metadata discovery.
- `preview`: preview construction and rendering-oriented preview data.
- `app`: runtime coordination, state, jobs, and user actions.
- `ui`: terminal rendering, layout, and theming.

Current boundary rules:

- Shared file-model types live in `src/core/`, not in `src/app/`.
- Lower layers such as `fs` and `file_info` may depend on `core`, but should not depend on `app`.
- `preview` is presentation code, but it should consume stable contracts from lower layers instead of reaching upward into unrelated subsystems.
- `app` coordinates behavior; it should not be the home for generic data model types that other layers need.

This document is intentionally short. Keep it focused on real dependency rules, and update it as architectural seams become explicit in the code.
