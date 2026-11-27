## Summary

<!-- What does this change and why? Link the issue it closes, e.g. "Closes #12". -->

## Safety

<!-- Cyron's safety rules are in AGENTS.md and CONTRIBUTING.md. Tick what applies. -->

- [ ] Scanning and inventory code stays read-only.
- [ ] Any destructive action goes through a server-held, single-use confirmation plan.
- [ ] Removed files go to quarantine; nothing is deleted in place without a reviewed plan.
- [ ] No new network calls, telemetry or shell invocations.
- [ ] Not applicable: this change does not touch files, the registry, processes or the network.

## Verification

<!-- List what you ran and the result. Say what you could not check. -->

- [ ] `npm run typecheck`
- [ ] `npm test`
- [ ] `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` (Rust changes)
- [ ] `npm run docs:check` (documentation changes)
- [ ] `node scripts/verify-landing.mjs` (website changes)
- [ ] Checked the change in the desktop app on Windows

## Screenshots

<!-- For UI changes, add before/after screenshots in light and dark. -->
