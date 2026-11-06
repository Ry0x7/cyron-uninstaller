# Cyron UI design system

A working specification of the current desktop interface, based on `src/styles/tokens.css`, `src/styles/app.css`, the shared React primitives, and the view styles. This documents the implemented product; it is not a promise of features that are not present.

## Product and mode

Cyron is an **Operate** tool for Windows users who need to inspect installed software, measure disk usage, and understand developer/AI cache footprints without accidentally removing data. The visual language should make measured facts and risk legible, keep familiar controls predictable, and avoid implying that read-only inspection can clean or uninstall anything.

## Color and themes

The light theme is warm paper with ink text and a restrained deep-teal signal. The dark theme is graphite with warm-white text and a brighter mint signal. Themes are selected through `data-theme` on the document root; `color-scheme` follows the selected theme. These are the values declared by the token sheet (the effective faint-text override in `app.css` is noted in the accessibility section).

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--bg` | `#f4f3ef` | `#0d1012` | Application background |
| `--bg-inset` | `#eae8e2` | `#090c0d` | Inset controls and tracks |
| `--surface` | `#fbfaf7` | `#15191c` | Panels and drawers |
| `--surface-2` | `#f5f3ee` | `#191e21` | Secondary panel surfaces |
| `--surface-hover` | `#efeee8` | `#1e2427` | Hovered rows and controls |
| `--line` | `#dedbd2` | `#262c30` | Quiet dividers |
| `--line-strong` | `#c8c4b9` | `#374046` | Control borders and scrollbar thumb |
| `--text-strong` | `#14181a` | `#eceae5` | Titles, primary values |
| `--text` | `#34393b` | `#c6c6c0` | Body text |
| `--text-muted` | `#6c6f6d` | `#949a9b` | Supporting copy |
| `--text-faint` | `#666966` | `#8b9294` | Faint metadata, kept at a ≥4.5:1 floor for 9–10px labels |
| `--accent` | `#0e7c66` | `#45c7a6` | Primary action and selection |
| `--accent-strong` | `#096352` | `#6adcbd` | High-contrast accent text/hover |
| `--accent-soft` | `#e0efea` | `#122f28` | Selected or informational surface |
| `--accent-contrast` | `#ffffff` | `#04120e` | Text on the primary accent fill |
| `--safe` / `--safe-soft` | `#2c7154` / `#e2efe7` | `#6ac69b` / `#122a20` | Regenerable data, safe status |
| `--caution` / `--caution-soft` | `#86590a` / `#f6ecd9` | `#d8a94e` / `#2b2314` | Re-download/reset cost, caution |
| `--danger` / `--danger-soft` | `#a3342b` / `#f6e2df` | `#e97b6f` / `#2e1917` | Errors and destructive-risk language |
| `--focus` | `#0e7c66` | `#45c7a6` | Keyboard focus ring |
| `--canvas` | `#12171a` | `#0b0f11` | Treemap surface (same in both themes) |
| `--canvas-grid` | `#1d2529` | `#1a2327` | Reserved grid token; currently unused |
| `--canvas-text` | `#f2f1ec` | `#f2f1ec` | Reserved canvas text token; currently unused |

Shadows: `--shadow-1` is a subtle border lift and short ambient shadow; `--shadow-2` is the deeper drawer shadow. Do not use shadows to turn every content block into a floating card.

## Typography and numbers

- UI: `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, sans-serif`.
- Display: `"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif` for brand, view titles, and numeric highlights.
- Monospace: `"Cascadia Mono", "Cascadia Code", Consolas, ui-monospace, monospace` for paths, dates, IDs, labels, and measurements.
- The root body is 13px with 1.5 line height. View titles are 21px; panel titles 13px; standard supporting copy is roughly 11–12.5px. Stats use 20px, or 27px for large values; analyzer summary values use 17px.
- Micro-labels are uppercase monospace, generally 9–10px, with measured letter spacing. Keep them short; do not use this small style for explanatory text.
- Numeric measurements use `font-variant-numeric: tabular-nums`; sizes and filesystem paths use mono where alignment or exact spelling matters.
- The stack is specified in CSS, not bundled font files; actual glyphs depend on the Windows installation and fallback chain.

## Layout, spacing, and shape

There is no formal spacing scale token yet; current layouts primarily use 4px increments and view-level gaps of 14–16px. Common panel padding is 15–16px; header/body horizontal padding is 30px, reducing to 22px at 1120px. Keep new rules aligned with these existing measurements instead of introducing a second scale.

| Token | Value | Intent |
| --- | --- | --- |
| `--radius-xs` | 3px | Risk badges |
| `--radius-sm` | 5px | Compact fields/details |
| `--radius` | 8px | Controls and inset blocks |
| `--radius-lg` | 11px | Main panels |
| `--sidebar-w` | 234px | Expanded navigation |
| `--sidebar-w-rail` | 64px | Collapsed icon rail |

Shapes are restrained: panels and controls have modest corners, risk labels are square, and treemap blocks are nearly square with 2px corners. Avoid pill shapes except for progress tracks and the switch thumb. The global canvas is a fixed-height desktop shell; feature grids stack at their documented breakpoints.

## Shared UI anatomy

- **Sidebar:** brand mark/name, six primary destinations, a single Settings entry at the foot, theme controls, and a collapse control. The rail collapses from 234px to 64px and keeps navigation discoverable with labels/tooltips.
- **View header:** sticky translucent header with one clear page title, short description, and contextual actions aligned to the right.
- **Panels:** bordered surface with optional heading/subtitle/actions and a padded or flush body. They group related work, not decoration.
- **Buttons:** primary for the main action, secondary for ordinary actions, ghost for low emphasis, danger for explicit warnings. Icon-only buttons require an accessible label.
- **Segmented control:** compact mutually exclusive controls with pressed state; counts are supplementary, not the only label.
- **Switch:** native button with `role="switch"`, checked state, label and optional description.
- **Badges:** compact uppercase mono labels. Risk is always text-coded (`safe`, `caution`, `protected`); color is reinforcing rather than the only signal.
- **Tables:** tabular numeric alignment, sticky headings, internal scrolling, quiet row dividers, and explicit keyboard focus on interactive rows.
- **Drawer:** right-side modal up to 470px wide (92vw maximum), scrim, labelled heading, optional description, scrollable body, and optional footer. It contains contextual detail rather than creating another navigation page.
- **Empty/error/loading states:** use the shared `EmptyState`, `Banner`, skeleton, and progress primitives. Errors should remain distinguishable from empty inventories and carry a retry when available.

## Treemap palette

The treemap remains a dark canvas in both themes. File fills come from `src/lib/categories.ts`; tile labels use dark ink `#0b1314`. Directory tiles use `#3d4c52` with paper text `#eef1ea`; aggregate groups use `#8a7a5e` with dark ink. Category labels are Code & data, Images, Video, Audio, Archives, Documents, Executables, and Other.

| Category | Fill | Dark-ink label contrast |
| --- | --- | ---: |
| Code & data | `#4fb79b` | 7.68:1 |
| Images | `#a7be55` | 9.07:1 |
| Video | `#d08360` | 6.35:1 |
| Audio | `#a291cf` | 6.70:1 |
| Archives | `#d3aa5c` | 8.66:1 |
| Documents | `#6ba1cc` | 6.80:1 |
| Executables | `#9aa6ac` | 7.54:1 |
| Other | `#b3a794` | 7.94:1 |
| Directory | `#3d4c52` | 7.82:1 for paper text `#eef1ea` |
| Aggregate group | `#8a7a5e` | 4.50:1 for dark ink `#0b1314` (borderline) |

A tile's area represents its measured logical size (allocated size is used when logical size is zero); color indicates file category, not risk. Small tiles may omit labels. Selecting highlights a tile; focus has an independent paper outline. Do not imply that the pruned tree is a complete file listing.

## Interaction and motion

- Navigation changes view while visited views remain mounted; avoid resetting scan state on navigation.
- Use clear hover, selected, disabled, focus-visible and loading states. Focus is a 2px accent outline with 2px offset (table rows use an inset outline); the search container owns focus styling for its borderless input.
- Motion is short and functional: control transitions 100–150ms, view entrance 180ms, drawer fade 140ms and slide 200ms, with the shared `cubic-bezier(0.2, 0.7, 0.3, 1)` easing. Progress and shimmer animations communicate active work.
- Honor `prefers-reduced-motion: reduce` globally by reducing animation and transition durations to 1ms and limiting iteration. Do not add essential information that is only conveyed through motion.
- Forced-colors mode restores system text/border and highlight colors for key controls.

## Accessibility and measured contrast

WCAG contrast ratios below were computed from the CSS hex colors using the WCAG 2.1 relative-luminance formula. Text pairs use 4.5:1 as the target; the table reports the smaller relevant ratio where a token is used on multiple ordinary surfaces.

| Pair | Light | Dark |
| --- | ---: | ---: |
| Strong text on app background | 16.09:1 | 15.88:1 |
| Body text on app background | 10.54:1 | 11.13:1 |
| Muted text on app background | 4.58:1 | 6.69:1 |
| Muted text on inset (`--bg-inset`) | 4.15:1 | 6.28:1 |
| Faint text on app background | 5.01:1 | 6.03:1 |
| Faint text on surface | 5.33:1 | 5.59:1 |
| Faint text on inset | 4.54:1 | 6.20:1 |
| Accent-strong on accent-soft | 6.07:1 | 8.60:1 |
| Accent-contrast on accent fill | 5.13:1 | 9.08:1 |
| Safe badge text on soft fill | 4.93:1 | 7.38:1 |
| Caution badge text on soft fill | 5.20:1 | 7.17:1 |
| Danger badge text on soft fill | 5.47:1 | 5.94:1 |

The earlier light-theme faint-text gap was fixed at the source: `--text-faint` is now `#666966` in `tokens.css` (5.01:1 on the page background, 4.54:1 on the inset) and the duplicate override in `app.css` is gone. The remaining known gap is muted text `#6c6f6d` at 4.15:1 on `--bg-inset`; it is used for supporting copy on normal surfaces, so keep it off inset fills or darken the token if that usage grows.

Treemap labels have adequate contrast at full opacity, but small tile-size text is rendered at 72% opacity, and the aggregate-group label is borderline at 4.50:1; do not treat those tiny labels as meeting normal body-text sizing/contrast without verifying their composited colors. Decorative borders and category distinctions should not carry meaning without their textual labels.

All interactive controls need visible keyboard focus and names. Keep status distinctions textual and maintain readable error/partial-result messaging. The app's read-only status is a safety affordance: never visually style an unimplemented destructive action as available.

## Anti-slop rules

- No gradients (except the existing neutral skeleton shimmer), glassy card stacks, decorative charts, or gratuitous dashboard stat tiles.
- No redundant navigation; use drawers, tabs, filters and contextual actions inside the six primary views.
- No large decorative illustrations or invented data. Show real measurements, clear empty states, and honest missing values.
- Keep micro-labels mono and purposeful, risk badges square and explicit, and color restrained to category/risk/status meaning.
- Preserve the paper/graphite identity and native desktop affordances; do not round every element into pills or animate for spectacle.
