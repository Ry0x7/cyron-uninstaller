# Design system and navigation rebuild — verification receipt

Date: 2026-11-17
Tree: uncommitted Phase 5 working tree based on `d5e5afc` (`main`)
Scope: presentation, information architecture, native window states and keyboard access. Safety
plans, IPC payloads, quarantine behaviour and scanner read-only rules were not changed.

## What was reviewed

The native desktop debug build (`npm run build:desktop`) was driven through WebView2 remote
debugging. The client area was measured at 1320×860 and 1000×680; these are client dimensions, not
the outer Win32 frame. Captures use `Page.captureScreenshot`, so they show the React surface without
the OS titlebar. Light and dark themes were both exercised.

The review covered every primary destination, both Storage & Cleanup subviews, both History &
Recovery tabs, Settings, the 1000×680 layouts, the Hunter confirmation, the uninstall wizard and the
permanent-deletion review. The table below lists the selected captures that ship with this receipt.
Earlier baseline captures of the old dashboard were reviewed locally and are not committed.

## Result

| Area | Observed |
|---|---|
| Navigation | Exactly five destinations: Home, Applications, Storage & Cleanup, Performance, History & Recovery. Settings is in the sidebar footer. Hunter Mode is **Find by window** inside Applications. No duplicate or dead navigation item was found. |
| Tokens | All reviewed surfaces use `src/styles/tokens.css` and `src/components/ui.tsx`. One blue accent, one neutral family, one radius scale and one type hierarchy. No decorative gradients, glass, neon or page-local palette. |
| Themes | Light and dark render the same hierarchy and state colours. Dark surfaces use the same token names; no hard-coded surface colour was found in the reviewed views. |
| Responsive layout | At 1000×680, page document `scrollWidth - innerWidth` was 0 for Applications, Storage & Cleanup, Activity and Recovery. The Activity table was 646/646 px (no inner scroll) after the Type column was hidden at that width. |
| Long values | Application names, publishers and install paths truncate with an ellipsis or mid-path shortening; the full value remains in `title`. The permanent-deletion review shows a long fixture path without clipping. |
| Modals | Hunter confirmation and the uninstall wizard use the shared modal/drawer primitives. The permanent-deletion review shows exact target counts, consequences and a disabled-until-checked action. |
| Motion | `prefers-reduced-motion: reduce` matched in the renderer; computed transition and animation durations were ≤ 0.05 s for all buttons, links, inputs and tab controls sampled. |
| Contrast | Token spots: light body 9.32:1, secondary 6.05:1, faint 4.82:1 on `#f3f4f6`; dark body 12.17:1, muted 7.34:1, faint 5.21:1 on `#0f1114`; accent on white 4.57:1. All sampled body text meets 4.5:1. |

## Defects found and fixed during the audit

1. **Stale Cleanup header on Projects.** Selecting **Projects** kept the **Caches & leftovers** heading,
   its Scan selected drives action and cache-only warnings. The cache header, error and warning
   banners are now rendered only on the Cache cleanup tab.
2. **Recovery log badge wrapped at minimum width.** The 1000 px media rule declared three grid
   columns for five children, pushing the status badge onto a second row. The row now uses four
   columns with the detail column intentionally hidden.
3. **Activity table scrolled internally at minimum width.** The Type column is hidden at ≤1000 px,
   preserving Result, Operation, Data, Items and When without horizontal overflow.
4. **Canonical path prefix in the deletion review.** The Analyzer review rendered `\\?\` verbatim
   paths. Display helpers now strip the prefix for selected paths, progress, report items and plan
   targets; the server-held plan and execution still use the canonical path.
5. **Legacy plural titles.** Persisted history entries could read `Cleanup · 1 locations`. The display
   helper singularizes that legacy title, and new cleanup and purge log entries use the correct
   singular/plural form.
6. **Mixed grammar in the purge log.** `1 files permanently removed` now reads `1 file permanently
   removed`, and the Activity fallback uses `plural(1, "file")`.

## Checks

| Check | Command | Observed |
|---|---|---|
| TypeScript | `npm run typecheck` | exit 0 |
| Frontend tests | `npm test` | 100 passed in 12 files |
| Production frontend | `npm run build` | exit 0; 562.28 kB JS / 127.63 kB CSS; Vite chunk-size warning only |
| Documentation | `npm run docs:check` | `DOCS_CHECK: PASSED`, 79 Markdown files |
| Full Rust suite | `node scripts/msvc-env.mjs cargo test --manifest-path src-tauri/Cargo.toml` | 253 passed, 4 ignored, 0 failed |
| Desktop build | `npm run build:desktop` | exit 0; `src-tauri/target/debug/cyron.exe` built |
| Navigation | Native debug app, DOM and screenshot review | five destinations opened; Settings footer; Hunter inside Applications |
| Keyboard | CDP real key events | Tab order reached brand → Home → Applications → Storage & Cleanup → Performance → History & Recovery → Settings; Enter opened Applications; `:focus-visible` outline was `2px solid rgb(47, 111, 235)` |
| Dialog focus | Hunter confirmation, 10 Tab presses, Escape | focus stayed inside the dialog; Escape removed it and returned focus to **Find by window** |
| Reduced motion | `Emulation.setEmulatedMedia` with `prefers-reduced-motion=reduce` | media query matched; no sampled control exceeded 0.05 s |
| Layout probe | `document.documentElement.scrollWidth - innerWidth` at 1000×680 | 0 on Applications, Storage and History views |
| Contrast | WCAG relative-luminance calculation over design tokens | sampled pairs met 4.5:1 |

## Screenshots

All captures are 1320×860 unless the name says `min`, which are 1000×680.

| File | Surface |
|---|---|
| [01-home-light.png](phase5/01-home-light.png) | Home, light, populated cache summary |
| [02-home-dark.png](phase5/02-home-dark.png) | Home, dark |
| [03-applications-light.png](phase5/03-applications-light.png) | Applications inventory and header actions |
| [04-storage-cleanup-light.png](phase5/04-storage-cleanup-light.png) | Cleanup → Caches & leftovers, measured risk summary |
| [05-storage-projects-light.png](phase5/05-storage-projects-light.png) | Cleanup → Projects, developer project evidence |
| [06-storage-disk-space-light.png](phase5/06-storage-disk-space-light.png) | Disk space volumes and folder scan |
| [07-performance-light.png](phase5/07-performance-light.png) | Performance, symptom guide, live sample and recommendation |
| [08-performance-dark.png](phase5/08-performance-dark.png) | Performance, dark |
| [09-activity-light.png](phase5/09-activity-light.png) | History & Recovery → Activity |
| [10-recovery-light.png](phase5/10-recovery-light.png) | History & Recovery → Recovery and cleanup log |
| [11-settings-dark.png](phase5/11-settings-dark.png) | Settings, dark, Appearance |
| [12-hunter-dialog.png](phase5/12-hunter-dialog.png) | Hunter confirmation from Applications |
| [13-uninstall-wizard.png](phase5/13-uninstall-wizard.png) | Uninstall wizard, reviewed 7-Zip plan |
| [14-force-delete-dialog.png](phase5/14-force-delete-dialog.png) | Permanent-deletion review for a disposable fixture file |
| [15-applications-min.png](phase5/15-applications-min.png) | Applications at 1000×680 |
| [16-storage-min.png](phase5/16-storage-min.png) | Storage & Cleanup at 1000×680 |
| [17-recovery-min.png](phase5/17-recovery-min.png) | Recovery cleanup log at 1000×680 |
| [18-performance-min.png](phase5/18-performance-min.png) | Performance at 1000×680 |

## Limitations

- Screen-reader traversal (Narrator/NVDA) was not run. Focus order, focus visibility, dialog trapping
  and Escape return were tested with real key events and DOM checks.
- Windows forced-colors mode and 200% OS text scaling were not exercised. The stylesheet contains a
  `forced-colors` block, but it was not visually verified.
- The native captures use this machine's real inventory (179 applications, three volumes). No real
  user cache or project was cleaned. The deletion review used `C:\Users\Public\CyronPhase5Fixture`,
  a disposable fixture whose `package.json` was not deleted.
- Non-Latin and emoji stress data were not injected into every table. The real inventory included
  long English names and paths; those truncated intentionally.
- The production build still emits Vite's >500 kB chunk-size advisory. It is a warning, not a build
  failure; code splitting was not part of this design scope.
- This receipt records presentation and keyboard verification, not a clean-VM installer run or
  screen-reader certification.
