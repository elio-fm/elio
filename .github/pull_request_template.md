## Summary

<!-- What does this PR change and why? Keep it concise. -->

## What Changed

<!-- Use bullets for the main implementation changes. Focus on key points. -->

## User Impact

<!-- Describe the visible behavior change. For internal-only changes, use "No user-visible behavior change." -->

## Notes

<!-- Optional: mention design decisions, follow-up work, compatibility details, or platform-specific behavior. Delete this section if it is not needed. -->

## Testing

Verified with:

- [ ] `cargo fmt --check`
- [ ] `cargo test --locked --test architecture_guardrails`
- [ ] `cargo clippy --locked --all-targets -- -D warnings`
- [ ] `cargo test --locked`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`

Manual checks:

<!-- If this affects preview behavior, file classification, platform integration, optional tools, UI/layout, or themes:

- Mention OS, terminal, and any relevant tools
- Describe what you tested
- Include screenshots or recordings when useful
-->

Result:

<!-- Example: All checks passed locally. -->

## Documentation and Changelog

- [ ] Updated docs when behavior, configuration, controls, or optional dependencies changed
- [ ] Added a `CHANGELOG.md` entry for user-visible changes
- [ ] Docs and changelog are not needed
