# Cyron Design System

Phase 5 replaced the old ink-and-teal dashboard language with a calm utility shell. The 2026-11-27 overhaul moves that shell toward an Apple-style system tool (macOS System Settings / Activity Monitor feel) while staying a native Windows app. Product truth, safety copy and backend behaviour are unchanged.

The [earlier design spec at cb8e919](https://github.com/Ry0x7/cyron-uninstaller/blob/cb8e919/docs/development/DESIGN.md) records the former palette, layout and contrast measurements.

## Direction

Cyron should feel like a well-made Apple system tool running on Windows: soft neutral background, borderless content cards with hairline shadows, translucent fills for controls, a source-list sidebar, a translucent sticky header, one blue accent and native system type (Segoe UI Variable on Windows, SF Pro where present). The interface disappears into the task.

- **One accent:** `--accent` is the only interactive accent. Semantic success, warning and danger colors are used only for state.
- **One neutral family:** cool greys in light and dark. No warm/cool mix.
- **No card grids as page structure.** Use spacing, headings and hairlines first; panels group real work.
- **No mono labels.** Monospace is reserved for paths, sizes, identifiers and tabular numbers.
- **Cards, not borders.** Panels are `--surface` cards using `--shadow-card`; controls use `--fill` / `--fill-strong` instead of outlines. Primary buttons use `--accent-fill`.
- **Translucency only where Apple uses it:** the sticky view header blurs content beneath it, with an opaque fallback. No decorative glass, gradients, neon or glow; no uppercase eyebrows.
- **Native titlebar.** The OS window frame remains; the app does not fake traffic lights or window controls.

## Navigation

Five primary destinations, always in this order:

1. Home
2. Applications
3. Storage & Cleanup
4. Performance
5. History & Recovery

Settings lives in the sidebar footer. Hunter Mode is an action inside Applications, not a navigation page. Related tools live inside their destination:

- **Applications:** installed apps, app details, uninstall/repair, find-by-window.
- **Storage & Cleanup:** Cleanup (caches, removed-app evidence, developer projects) and Disk space (map, files, duplicates, categories).
- **History & Recovery:** Activity and Recovery (quarantine plus cleanup log).

## Tokens

Defined in `src/styles/tokens.css`.

| Role | Rule |
|---|---|
| Type | Segoe UI Variable Display for headings, Segoe UI Variable Text for UI, Cascadia Mono for paths and figures |
| Page title | 28px / 700 / -0.028em |
| Section title | 15px / 600 / -0.012em |
| Body | 13px / 1.5 |
| Secondary | 12px / 1.5 |
| Caption | 11.5px / 1.45 |
| Radius | 5 xs, 7 controls, 9 buttons/inputs, 14 cards/panels, 20 dialogs |
| Spacing | 4, 8, 12, 16, 20, 24, 32, 48 |
| Shadow | `--shadow-card` for cards and panels; `--shadow-1` for raised rows and segmented thumbs; `--shadow-2` only for dialogs |
| Fill | `--fill` for controls, search and segmented tracks; `--fill-strong` for hover/pressed |
| Motion | 120–190ms `--ease`; no page-load choreography |

Every color, radius and shadow comes from a token. Feature CSS must not introduce another palette.

## Components

Shared primitives live in `src/components/ui.tsx`:

- `ViewHeader` — top-level destination title, description and actions.
- `SectionHeader` — heading for a tool nested inside a destination.
- `PageTabs` — top-level tabs inside one destination.
- `Panel`, `Stat`, `Badge`, `Button`, `IconButton`, `SearchField`, `Segmented`, `Toggle`, `Select`, `Field`.
- `ProgressBar`, `Spinner`, `Skeleton`.
- `EmptyState`, `Banner`, `Drawer`, `Modal`, `KeyValue`.

Rules:

- Use `PageTabs` only for destination-level tabs. Use `Segmented` for filters and local modes.
- A destructive action names its exact target and consequence. Existing confirmation plans and safety copy are not shortened away.
- A panel gets a header only when it has a real title or action. Avoid nested panels.
- Tabular numbers use `.mono` or `.numeric`; body copy does not.
- Long names wrap or truncate intentionally, never overflow. Interactive actions never shrink to zero.

## Performance and Disk space

- **Performance** has four tabs: Monitor (Task Manager-style resource rail with sparklines and 60-second charts for CPU, memory, each disk, each network adapter and each GPU), Processes (grouped Apps / Background / Windows table with heat-tinted cells), Startup, and Diagnose. Live tabs poll once a second only while visible and not paused.
- **Disk space** draws a nested canvas treemap: folders are framed with a name/size header so siblings read as separate blocks, and summarized folders load their contents on demand.

## Screenshots and the website

- `npm run preview:screenshots` renders the React frontend in headless Chromium with `scripts/preview/mock-tauri.js` standing in for the Tauri IPC, and writes `docs/screenshots/*.png`. Every value is sample data. Label these images as previews, never as native captures; native evidence still comes from the desktop E2E harness.
- `npm run preview:screenshots -- --scale 2 --themes light,dark --out <dir>` produces the light and dark pairs used by the website. The landing page stores them as WebP in `landing/assets/ui/` and shows the light or dark capture to match the page's appearance.
- The website in `landing/` follows the same direction as the app: neutral surfaces, one blue accent, cards without heavy borders. Motion is progressive: scroll-linked effects (hero tilt, word-by-word manifesto, pinned horizontal tour, scrubbed showcase) and the simulated live monitor stop or flatten under reduced motion, and the page reads fully without JavaScript. The live monitor demo is simulated in the browser and says so; it never reads the visitor's computer.

## States

Every surface must cover the states it can actually reach: loading, empty, error/stale, disabled, in-progress, success, partial and recovery. Loading skeletons should match the final layout. Errors name the problem and the next action. Empty states explain how to fill the surface. A run that moved some files is **partial**; it is never described as completed or as a clean cancellation.

## Accessibility

- Visible `:focus-visible` ring on every control.
- Dialog and drawer focus trap, Escape to close, focus returned to the trigger.
- Minimum 32px control height; sidebar rows 36px.
- Body and placeholder text meet 4.5:1; large text 3:1.
- `prefers-reduced-motion` removes spatial movement.
- Forced-colors mode keeps borders and selected states visible.
- Icon-only controls have an accessible label.

## Verification

Native screenshots at 1320×860 and 1000×680, plus keyboard traversal and a layout probe for horizontal overflow. A build is not visual evidence.
