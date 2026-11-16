# Capability Verification Receipt — 2026-11-09

Recorded during the capability work that produced commit `e38ea5c`
("analyzer action plans, cleaner drive scope, and truthful uninstaller
outcomes"). Transcribed into the repository on 2026-11-16 by the Phase 1 audit
from the local `.unlazy/capabilities/` runtime directory, which is gitignored.
This receipt records what was observed on 2026-11-09; it is not a re-run.

Windows 11 development machine, MSVC toolchain through `scripts/msvc-env.mjs`.
All destructive operations used disposable fixtures under `%TEMP%`.

## Acceptance results

| Gate | Scope | Command | Observed |
|---|---|---|---|
| G57 | Analyzer reviewed recycle/permanent actions, expiring single-use plans, protected/reparse/stale refusal, progress, cancellation, partial results | `cargo test --manifest-path src-tauri/Cargo.toml --test analyzer_actions` | Pass. Module tests cover expiration/reuse, protected and reparse paths, altered targets and partial failures. The interactive Recycle Bin marker test was invoked separately and passed. Desktop review permanently removed only the disposable 8 KB marker and preserved its sibling. |
| G58 | Cleaner drive scope, evidence/age/risk, quarantine journaling, limits, cross-volume no-overwrite restore | `cargo test --manifest-path src-tauri/Cargo.toml --lib cleanup::tests` | Pass, including changed-file refusal, unrelated single-file siblings, hard links, recovery and no-overwrite conflicts. The cross-volume fixture passed where a second writable local volume existed. |
| G59 | Uninstaller live command review, empty process step, truthful single/batch outcomes | `npx vitest run src/features/uninstaller/uninstallerFlow.test.ts src/lib/capabilities.test.ts` | Pass. Rust reconciliation and changed-command tests passed. No real application was uninstalled. |
| G60 | Frontend, Rust, docs, desktop build, isolated integration | See the command ledger below | Pass for the scoped change. Signing, installer bundles and release metadata were not rerun. |
| G61 | Visual/interaction review at 1320x860 and 1000x680 client sizes, plus expanded Uninstaller | Desktop inspection, eight scoped checks | Pass for the covered states; see limitations. |

## Command ledger (as recorded 2026-11-09)

| Check | Command | Result |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 92 tests passed across 11 files |
| Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 207 passed, 4 ignored; the ignored interactive Recycle Bin fixture was invoked separately and passed |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED` |
| Desktop debug build | `npm run build:desktop` | `Built application at: src-tauri\target\debug\cyron.exe` |
| Cleanup fixture | `node scripts/verify-cleanup.mjs` | `CLEANUP_FIXTURE_OK` |
| Uninstall/process/registry/recovery fixture | `node scripts/verify-e2e.mjs` | `RC1_E2E_FIXTURE_OK`, eight scenarios |

## Visual checks

The local receipt held 17 PNG captures. They remain on the recording machine and
are intentionally not committed. Each check below was marked `pass` at review
time, with no unresolved findings recorded.

| Check | State | Observed |
|---|---|---|
| `analyzer-normal` | Read-only fixture scan, normal size | Controls matched existing tokens; actions stayed disabled until selection. |
| `analyzer-protected-error` | Protected-name refusal and dismissal | No plan offered for a disposable `ntuser.dat`; refusal readable; permanent-action title correct; Escape dismissed. |
| `analyzer-progress-cancellation` | Reviewed permanent progress and partial completion | 2115 of 4000 disposable files deleted, zero folders removed, 1885 files and an unrelated sibling preserved; Close unavailable during execution. |
| `analyzer-minimum` | Minimum-size result, dismissal, scan invalidation | Long paths wrapped; footer reachable; stale selection discarded; rescan reported the remaining fixture files. |
| `cleaner-normal-loading` | Normal-size scope and loading | Drive scope usable before inventory finished; protected-data and quarantine copy stayed visible. |
| `cleaner-minimum-scope` | C-only read-only scan at minimum size | 30 locations versus 32 for all local drives; C-only scope visible; inaccessible Windows Temp warning shown; no cleanup executed. |
| `uninstaller-minimum-search` | Minimum-size empty search and recovery | Controls fit; clear-filter restored 207 registered items. |
| `uninstaller-store-guidance` | Expanded inventory, guidance-only Store flow | Store app offered Settings/winget guidance and a read-only leftover scan; no direct package removal. |

## Source binding

The local receipt contained 59 `path:sha256` source rows. On 2026-11-16 the
audit recomputed every hash against the working tree: 59 matched, 0 differed,
0 were missing. Normalized sorted-row digest:
`4f6f4877f27a9cd44117cb1d665a9bbf5262ecd71d49a3201e39935e7b48f684`.
Artifact digest at recording time:
`c6c42ccea462e3840631a6915c603501b03a31c01be2b777a5c50b0805379998`.

The local receipt's `environment.width`/`height` fields are invalid
(large integers, not pixel dimensions); the real window sizes recorded in the
steps were 1322x892 outer / 1320x860 client and 1002x712 outer / 1000x680
client. Those fields were not copied here.

## Limitations

- Real user caches were measured read-only. Cleanup, quarantine, uninstall and
  registry mutation were exercised with disposable fixtures only.
- No real vendor uninstaller, Microsoft Store removal, full screen-reader
  traversal, or exhaustive provider/window/state matrix was exercised.
- Removed-app remnants are session-scoped; candidate age is unavailable and
  unknown stale installers are intentionally unsupported.
- Windows shell recycling keeps the documented final same-name pathname race.
- Signing, installer bundles and release metadata were not rerun for this
  change; their earlier evidence is recorded in `GATES.md`.
