# Gates: Cyron Uninstaller Phase 2

OWNS: src/**, src-tauri/**, scripts/**, .github/**, package.json, package-lock.json, tsconfig*.json,
vite.config.ts, index.html, README.md, ROADMAP.md, HANDOFF.md, docs/**, GATES.md

Scope: Phase 1 remediation plus Phase 2 disk intelligence — MFT scanning with a directory-walk
fallback, benchmark harness, persistent synchronized tree, filters, duplicate verification, storage
categorization and stress hardening — while preserving the read-only safety boundary.

## Foundation and tests

- [x] G1: Rust backend compiles for the Windows target, all targets, no warnings
  CHECK: node scripts/msvc-env.mjs cargo check --manifest-path src-tauri/Cargo.toml --all-targets
  EXPECT: exit 0
  EVIDENCE: `Finished dev profile` exit 0 (2026-11-06); no warning lines.

- [x] G2: Rust unit, integration and safety tests pass
  CHECK: node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml
  EXPECT: exit 0
  EVIDENCE: 75 lib tests → 73 passed, 2 ignored; read_only 5 passed; real_system 1 passed;
  stress_scan 5 passed, 1 ignored. 0 failed.

- [x] G3: live registry, cache and filesystem integration
  CHECK: ... --test real_system -- --nocapture
  EXPECT: exit 0 with discovered apps and a scanned fixture
  EVIDENCE: passed; live registry and cache discovery plus exact byte assertions.

- [x] G4: frontend typecheck
  CHECK: npm run typecheck
  EXPECT: exit 0
  EVIDENCE: pass.

- [x] G5: frontend unit tests
  CHECK: npm test
  EXPECT: exit 0
  EVIDENCE: 6 files, 51/51 passed.

- [x] G6: production bundle
  CHECK: node scripts/verify-dist.mjs
  EXPECT: DIST_OK
  EVIDENCE: `DIST_OK` — 2 assets, 408.6 KiB.

## Phase 2 features

- [x] G7: NTFS MFT scanning works on a real volume and returns real totals
  CHECK: elevated `examples/scan_bench.exe --target 'C:\' --method mft`
  EXPECT: `"method":"mft"`, non-zero files and bytes
  EVIDENCE: `C:\` → 2,211,292 files, 604,557 dirs, 356,801,993,820 bytes, 14,515 ms, 687,366,144 B
  peak working set. Elevated shell required; unelevated runs report a reason and fall back.

- [x] G8: method selection and fallback reasons are honest
  CHECK: analyzer method badge + `ScanResult.methodNote`; MFT-on-subfolder measurement
  EXPECT: `auto` uses MFT only at drive roots; fallback carries a reason
  EVIDENCE: System32 MFT 3.91 s vs walk 0.28 s; unelevated MFT request reports the elevation
  requirement and the walk runs; `methodNote` rendered in the analyzer after scans.

- [x] G9: duplicate detection verifies content and handles hard links
  CHECK: `cargo test --lib duplicates::`
  EXPECT: exit 0
  EVIDENCE: 8 duplicate tests pass (identical files grouped, equal-size different content
  rejected, equal-edge partial-hash collision separated by full hash, minimum size, hard links,
  cancellation, missing root, exclusions).

- [x] G10: storage categories, age buckets and insights are real
  CHECK: `cargo test --lib storage::`
  EXPECT: exit 0
  EVIDENCE: 9 storage tests pass, including known/inferred confidence, category order, age
  boundaries, mixed-confidence handling and insight thresholds.

- [x] G11: scanner memory/output bounds and stress cases
  CHECK: ... --test stress_scan -- --nocapture
  EXPECT: exit 0
  EVIDENCE: 5/5 pass: 20,000-file wide directory with exact totals and bounded children, deep
  nesting within the depth cap, access-denied folders report errors, cancellation, exclusions.
  (The separate 20k-file `#[ignore]`d measurement test is not part of this gate.)

- [x] G12: duplicate scanning is cancellable and scope-safe in the UI
  CHECK: `npm run typecheck`, `npm test`, code review of `DuplicatesPanel.tsx`
  EXPECT: no stale report after a root change
  EVIDENCE: panel cancels and resets on scope change, ignores late events from an old job id, and
  guards reveal after unmount. No automated test covers scope-change cancellation; native
  duplicate run was exercised in the analyzer.

## Native and visual

- [x] G13: debug desktop binary builds
  CHECK: npm run build:desktop && node scripts/verify-desktop-binary.mjs
  EXPECT: DESKTOP_BINARY_OK
  EVIDENCE: `DESKTOP_BINARY_OK`.

- [x] G14: built app launches
  CHECK: node scripts/smoke-desktop.mjs
  EXPECT: DESKTOP_SMOKE_OK
  EVIDENCE: `DESKTOP_SMOKE_OK`. Limitation: the smoke check verifies the window exists; the debug
  build loads the Vite dev server, so this check cannot prove the UI rendered without it.

- [x] G15: all six views reviewed in both themes with real data
  CHECK: native window captures with a running Vite dev server
  EXPECT: no overflow, clipped controls, invisible text or blank states
  EVIDENCE: Overview, Uninstaller, Analyzer (Map/Storage/Duplicates), Cleaner, Activity and Settings
  captured in light and dark; a real `C:\Windows\System32` scan was loaded for the analyzer views.
  Contrast and table-width defects found in the pass were fixed.

## Documentation and hygiene

- [x] G16: Phase 2 documentation is accurate and does not overclaim
  CHECK: manual review of README, ROADMAP, HANDOFF, ARCHITECTURE, IMPLEMENTATION_STATUS, BENCHMARKS
  EXPECT: measured numbers carry method, elevation, cache state and machine caveats
  EVIDENCE: numbers above appear with warm-cache/single-machine/one-run labels; no WizTree
  comparison; MFT record-vs-path counting difference documented.

- [x] G17: internal runtime data is untracked
  CHECK: git ls-files .chryypt | wc -l
  EXPECT: 0
  EVIDENCE: `0`; `.chryypt/`, `/screenshots/` and `src-tauri/*-bench.txt` are gitignored. No
  credentials were found in the inspected session/checkpoint files. History rewrite to purge them
  from the initial commit has **not** been done and requires approval.

## Not gated / unverified

- [ ] Windows CI remote run: `.github/workflows/ci.yml` exists and the same commands pass locally,
  but no GitHub Actions run was observed from this machine.
- [ ] Release MSI/NSIS packaging and signing: not produced.
- [ ] Standalone debug build without the Vite dev server: the debug executable loads `devUrl`.
- [ ] MFT on a volume above the 4,910,404-record budget: falls back by design; fallback timing is
  unmeasured.
- [ ] Cold-cache benchmark: not measured.
