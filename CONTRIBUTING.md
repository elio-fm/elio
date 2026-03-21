# Contributing

## Toolchain

`rust-toolchain.toml` pins the project to Rust 1.93.0 and installs the required `rustfmt` and `clippy` components automatically when you work in the repository.

## Local Quality Checks

Before opening a PR, run the same checks enforced in CI:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## Optional Preview Tooling

For the broadest local preview-test coverage, install the optional archive and PDF tools used by the test suite, especially `7z`, `bsdtar`, `isoinfo`, `pdfinfo`, `pdftocairo`, and `xz`.
