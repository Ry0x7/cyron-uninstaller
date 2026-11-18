# Cyron Design System

Phase 5 replaces the old ink-and-teal dashboard language with a calm Windows-native utility shell. Product truth, safety copy and backend behaviour are unchanged.

The [earlier design spec at cb8e919](https://github.com/Ry0x7/cyron-uninstaller/blob/cb8e919/docs/development/DESIGN.md) records the former palette, layout and contrast measurements.

## Direction

Cyron should feel like a well-made Windows 11 utility: quiet neutral surfaces, one blue accent, native Segoe UI Variable type, comfortable density, and no decorative dashboard furniture. The interface disappears into the task.

- **One accent:** `--accent` is the only interactive accent. Semantic success, warning and danger colors are used only for state.
- **One neutral family:** cool greys in light and dark. No warm/cool mix.
- **No card grids as page structure.** Use spacing, headings and hairlines first; panels group real work.
- **No mono labels.** Monospace is reserved for paths, sizes, identifiers and tabular numbers.
- **No uppercase eyebrows, neon teal, gradients, glass or glow.**
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
| Page title | 26px / 600 / -0.028em |
| Section title | 15px / 600 / -0.012em |
| Body | 13px / 1.5 |
| Secondary | 12px / 1.5 |
| Caption | 11.5px / 1.45 |
| Radius | 6 controls, 8 buttons/inputs, 10 banners, 12 panels, 16 dialogs |
| Spacing | 4, 8, 12, 16, 20, 24, 32, 48 |
| Shadow | `--shadow-1` for raised rows; `--shadow-2` only for dialogs |
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
