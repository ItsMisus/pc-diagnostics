# Design system

The visual and motion language of PC Diagnostics (since 2026-11-12): the perceived quality
of a contemporary Apple app, "Liquid Glass" inspired, adapted to a desktop diagnostics tool
used with mouse and keyboard in a resizable window. It replaces the 2026-11-05 dashboard
look; the content rules of `docs/ui-dashboard-brief.md` (R1–R7: one home per value, no
subtitles, sources only in Advanced, one view-mode control, counts and explanations once)
still apply.

Sources of truth:

- Tokens: `src/styles/index.css` (CSS custom properties per theme, exposed to Tailwind via
  `@theme inline`; material utilities `glass`, `surface-primary`, …).
- Motion: `src/lib/motion.ts` (durations, easings, springs, variants for Motion for React).
- Primitives: `src/components/common/*`, charts in `src/components/charts/*`, shell in
  `src/components/layout/*`.
- Tests guard the tokens: identical dark blocks, every light token defined in dark, WCAG AA
  for the text pairs, `-webkit-backdrop-filter` next to every `backdrop-filter`, CSS
  durations equal to `motion.ts` (`src/styles/tokens.test.ts`); every icon name used
  resolves (`src/components/common/icons.test.ts`).

## Principles

1. **Composition before decoration.** Every screen has one focal element (a primary panel,
   the 3D device on Overview) and supporting content around it. No endless grid of identical
   cards: vary size and weight by importance, group related things, leave space.
2. **Data stays legible.** Numbers, charts and tables sit on solid surfaces. Glass is for
   navigation, toolbars, controls and surfaces that float over other content.
3. **Real data only.** A missing value shows "n/d" with its reason (`MetricValue`,
   `StateView kind="unavailable"`), never a placeholder number. No invented health score:
   the overall state on Overview is derived from the diagnosis findings and says so.
4. **Color means something.** Neutral greys carry the interface; the azure accent marks
   focus, selection, the one primary action and at most one highlighted series; green /
   amber / red only for health, warning and error, always with words or a glyph.
5. **Quiet motion.** Animations answer a user action or a navigation; live data never
   animates. Short, natural, interruptible, and gone with reduced motion.
6. **Short copy.** Titles of one or two words, buttons with a verb, hints of one sentence.

## Themes

`ThemePreference` (Settings: Sistema / Chiaro / Scuro) sets `data-theme` on `<html>`
(`hooks/useThemeSync.ts`); "Sistema" removes it and CSS follows `prefers-color-scheme`. The
dark palette is declared twice (OS preference and explicit choice) because the WebViews
(WebView2 Chromium 120+, WKWebView on macOS 13.3+) predate `light-dark()`. Code that needs
the theme as a value (the 3D scene) uses `useResolvedTheme()` and `readColorToken("--accent")`
(`src/lib/theme.ts`).

- **Dark**: graphite and soft black (`canvas` #0e0f11, surfaces #17181b → #2e2f35), smoked
  glass, ivory text (#f3f0e8 / #b4b1a9 / #8f8c85), azure #4ea5ff (solid #0a72e0).
- **Light**: pearl canvas (#eceef2), white and cool-grey surfaces, satin glass, delicate
  separators, near-black text (#14161a / #4b505a / #656a74), azure #0a6be0.
- Both: a faint, fixed ambient wash on the canvas (`--canvas-wash`, utility `ambient-canvas`)
  that the glass picks up. It is the only gradient in the app besides the glass sheen.

## Materials

| Material | Utility / component | Blur | Use |
| --- | --- | --- | --- |
| Canvas | `ambient-canvas` (AppShell) | – | Window background. |
| Primary surface | `surface-primary`, `Panel variant="primary"` | – | The focal panel of a screen: raised, 18 px radius, soft shadow, top highlight, 20 px padding. One or two per screen. |
| Secondary surface | `surface-secondary`, `Panel` (default), `Card` | – | Supporting panels: flat, hairline, 14 px radius, 16 px padding. |
| Inset | `surface-inset`, `Panel variant="inset"` | – | Groups inside a panel (metric tiles, code, comparison). 10 px radius. |
| Glass | `glass`, `Panel variant="glass"` | 24 px | Sidebar, floating strips, panels over the 3D stage. |
| Thick glass | `glass-thick` | 36 px | Tooltips, toasts, chart tooltips, popovers. |
| Glass panel | `glass-panel` (Modal) | 36 px | Modals and sheets: 90 % opaque, they hold reading content. |
| Glass bar | `glass-bar`, `.page-toolbar` | 24 px | The sticky page toolbar once content scrolls under it. |
| Satin | `satin` | none | Controls: secondary buttons, segmented thumb, selection pill, glyph discs. Translucent fill, luminous edge, top highlight. |

Rules: never put a backdrop filter on a repeated element (rows, buttons, chips): it costs GPU
time per element and adds nothing on a solid panel. Every glass utility falls back to an
opaque surface without backdrop-filter support and with `prefers-reduced-transparency`.
Don't stack glass on glass.

## Color tokens

Tailwind classes use the token names (`bg-surface`, `text-fg-muted`, `border-line`, …). No raw
hex or Tailwind palette colors in components (the one exception: the white switch thumb).

| Token | Use |
| --- | --- |
| `canvas`, `--canvas-wash` | Window background and ambient light. |
| `surface`, `surface-raised`, `surface-2`, `surface-3` | Secondary panels; primary panels, menus; hover fills and insets; tracks, skeletons, pressed fills. |
| `glass`, `glass-thick`, `glass-edge`, `--glass-highlight`, `--glass-sheen` | Glass fills, the luminous 1 px edge, the inner top highlight, the top reflection. |
| `control`, `control-hover`, `control-active`, `control-edge`, `control-track`, `--control-thumb`, `field` | Satin controls; segmented / switch tracks; selected thumb; text field fill. |
| `line`, `line-strong` | Hairlines (translucent, work on any surface); hover borders, chart baselines. |
| `fg`, `fg-muted`, `fg-subtle` | Primary text; secondary text; captions, axis labels, placeholders. All AA on every surface. |
| `accent`, `accent-hover`, `accent-solid`, `accent-solid-hover`, `accent-fg`, `accent-soft`, `accent-text` | Focus borders; primary button fill; text on it; selection fills; accent-colored text and icons. |
| `ok`, `warn`, `bad` (+ `-soft`) | Status only, always paired with text or a glyph. |
| `meter`, `gauge-track` | Fill and track of meters, gauges, progress (grey until a threshold applies). |
| `chart-1`, `chart-2`, `chart-accent`, `--chart-grid`, `--chart-fill` | Chart series (primary, comparison, highlighted), gridlines, area fill. |
| `--focus`, `--backdrop`, `--scrollbar*` | Focus ring, modal backdrop, scrollbars. |

Meters stay grey (`--meter`) until a value crosses a threshold where "high" means a problem
(`src/features/overview/levels.ts`, mirroring the diagnostics rules); then the fill takes
`warn` / `bad` and a status word sits next to the value. Load, power and temperature stay
grey: high load is normal.

## Typography

System fonts only (offline): `-apple-system` (San Francisco; WebKit picks the Text/Display
optical size) on macOS, Segoe UI Variable Text/Display on Windows. `font-display` for titles
and big numbers, `font-sans` for everything else, `font-mono` for codes and raw data.

| Class | Size / line | Use |
| --- | --- | --- |
| `text-2xs` | 11 / 14 | Captions, table headers, axis labels, eyebrows, badges. |
| `text-xs` | 12 / 16 | Hints, secondary lines, tooltips. |
| `text-sm` | 13 / 18 | **Body** (the default), controls, nav, table cells. |
| `text-base` | 14 / 20 | Reading text in dialogs and explanations. |
| `text-md` | 15 / 20 | Secondary panel titles, dialog titles. |
| `text-lg` | 17 / 22 | Primary panel titles, section headings. |
| `text-xl` | 20 / 26 | Small readouts (`Readout size="sm"`). |
| `text-2xl` | 24 / 30 | Page title (`PageHeader`), `Readout size="md"`. |
| `text-3xl` | 28 / 34 | Overview device name. |
| `text-4xl` | 34 / 40 | Headline readouts and gauges. |
| `text-5xl` | 44 / 48 | Hero number (one per screen at most). |

- Hierarchy: page title 24 semibold > section heading 17 semibold > primary panel title 17 /
  secondary panel title 15 semibold > body 13. Sizes from 17 up have slight negative tracking
  built into the token.
- Numbers: always `tabular`, display face, regular weight, unit smaller and muted
  (`Readout`). Labels never repeat the panel title.
- Decimals, percentages and byte units follow the language and the view mode: always
  `formatNumber`, `formatPercent`, `formatBytes`, `formatStorageBytes`, `formatMib`, … from
  `src/lib/format.ts` (never `toFixed` or a hand-written unit). Disk sizes on macOS
  "Semplice" use powers of 1000 like Finder; memory stays on 1024.
- `eyebrow` utility: 11 px uppercase tracked label above a title or a group.

## Space

4 px grid (Tailwind spacing). Scale in use: 4, 8, 12, 16, 20, 24, 32, 40, 48.

- Page column: `page-gutter` (max 1200 px, 24 px gutters, 32 from 1024 px).
- Inside panels: 20 (primary) / 16 (secondary) / 12 (inset); 12 between a header and its
  content; 8–12 between rows.
- Between panels in a group: 16. Between groups (sections): 32. Page toolbar to content: 16.
- Controls: 28 (sm), 32 (md, default), 40 (lg) high; 8 between buttons.

## Radii

One continuous-looking family; nested shapes are concentric (`inner = outer − inset`).

| Token | px | Use |
| --- | --- | --- |
| `rounded-xs` | 4 | Checkboxes, kbd, tiny marks. |
| `rounded-sm` | 6 | Tags, skeleton bars, the brand tile. |
| `rounded-md` | 10 | Insets, nav rows, inputs, tooltips, chart tooltips. |
| `rounded-lg` | 14 | Secondary panels, toasts. |
| `rounded-xl` | 18 | Primary panels, sidebar, modals, sheets. |
| `rounded-2xl` | 24 | Hero stage (3D device), the floating running strip. |
| `rounded-full` | – | Buttons (capsules), segmented controls, switches, badges, search field, meters, glyph discs. |

## Elevation

`--elev-0…4`, mapped to `shadow-xs` / `sm` / `md` / `lg` / `xl`. Soft, low-contrast, deeper
in dark mode.

| Level | Use |
| --- | --- |
| 0 | Hairline lift: inputs, satin controls. |
| 1 | Resting primary panels, selected thumbs. |
| 2 | Glass sidebar and floating strips. |
| 3 | Tooltips, toasts, popovers (`glass-thick`). |
| 4 | Modals and sheets (`glass-panel`). |

Secondary panels have no shadow.

## Motion

| Kind | Token | Value | Curve |
| --- | --- | --- | --- |
| Press | `duration.instant` / `--dur-instant` | 100 ms | ease-out, scale 0.97 (`pressable`) |
| Hover, toggles, colors | `micro` / `--dur-micro` | 140 ms | ease-out |
| Tooltips, menus | `fast` / `--dur-fast` | 180 ms | ease-out |
| Disclosure, toasts, phase changes | `component` / `--dur-component` | 240 ms | ease-out / `spring.gentle` |
| Modals, sheets | `panel` / `--dur-panel` | 300 ms | `spring.panel` (no bounce) |
| Pages, sidebar width | `nav` / `--dur-nav` | 300 ms | ease-out; page exit 90 ms |
| Exits | `exit` | 120 ms | `ease.in` |
| Selection indicators | `spring.indicator` | 520 / 42 | no visible overshoot |
| Switch thumbs | `spring.snappy` | 640 / 34 | a hint of bounce |

Variants: `pageVariants`, `backdropVariants`, `dialogVariants`, `sheetVariants`,
`popoverTransition`.

1. Every animation answers a user action or a navigation. Live values, meters and charts
   update in place (measured: a 500 ms transition on live meters quadrupled idle CPU).
2. Animate `transform` and `opacity` (and colors). Never animate layout per frame, except the
   sidebar width and `<details>` height (single, short, user-triggered).
3. Exits are faster than entrances; nothing waits for an animation to finish.
4. Use `m.*` components (LazyMotion strict); `motion.*` throws in development.
5. Reduced motion: `MotionConfig reducedMotion="user"` drops transform/layout animations,
   CSS transitions are zeroed globally. Outside Motion (the 3D scene) read `useReducedMotion()`
   from `motion/react` or `prefersReducedMotion()` from `lib/motion.ts`: no auto-rotation,
   no idle animation, camera moves jump.
6. No infinite animations except spinners and skeletons while something is loading. The 3D
   device may move only in response to the user (drag, hover) or once on entrance.

## Icons

One family, inline SVG (`src/components/common/iconPaths.ts`, rendered by `<Icon>`): 16-unit
grid, 1.5 stroke, round caps and joins, rounded rectangles. Sizes: 14 in small controls,
16 in body and buttons, 18 in the sidebar, 20 in hero states. Icons take `currentColor`.
Names: navigation (`overview`, `hardware`, `benchmarks`, `processes`, `drivers`,
`diagnostics`, `history`, `settings`), components (`cpu`, `gpu`, `memory`, `storage`,
`network`, `display`, `battery`, `temperature`, `fan`, `power`/`bolt`, `gauge`, `clock`),
form factors (`desktop`, `laptop`, `miniPc`), status (`success`, `warning`/`alert`, `error`,
`info`, `lock`, `shield`), actions (`refresh`, `play`, `stop`, `check`, `close`, `plus`,
`minus`, `more`, `search`, `filter`, `copy`, `share`, `download`, `external`, `key`,
`sparkle`), arrows (`chevronUp/Down/Left/Right`, `arrowUp/Down/Right`), appearance (`sun`,
`moon`, `globe`), shell (`sidebar`, `collapse`, `expand`, `viewSimple`, `viewAdvanced`).
No other icon set, no emoji. A new icon is drawn on the same grid in `iconPaths.ts`.

## Components

| Component | API | Rules |
| --- | --- | --- |
| `Button` | `variant` `primary` / `secondary` (default) / `ghost` / `danger`; `size` `sm` / `md` / `lg`; `icon`, `iconOnly`, `loading`; forwards ref and button props | Capsules. One primary per view. `iconOnly` needs `aria-label` and usually a `Tooltip`. `loading` keeps the label, shows a spinner, blocks clicks. |
| `Panel` (`Card` is the same) | `variant` `primary` / `secondary` (default) / `inset` / `glass`; `title`, `icon`, `eyebrow`, `description`, `aside`, `flush`, `level`, `titleId`, `as` | Title = the content's name in 1–2 words. `PanelHeader` and `Surface` for custom compositions. |
| `SectionHeading` | `children`, `count`, `aside`, `id` | Heading of a group of panels. |
| `Segmented` | `label`, `options` (`value`, `label`, `icon`, `disabled`), `value`, `onChange`, `size`, `fullWidth`, `iconOnly` | 2–5 short options; radio group, sliding thumb, arrow keys. Longer lists: native `select`. |
| `Switch` | `label`, `checked`, `onChange`, `disabled`, `describedBy` | Settings' `Toggle` delegates to it. |
| `Tooltip` | `content`, `side` (`right`, `left`, `top`, `top-end`, `bottom`, `bottom-end`), `enabled`, `multiline` | 350 ms hover delay, instant on keyboard focus, Esc. Supplementary only; never a native `title` for information the user needs. |
| `Modal` (+ `ModalBody`, `ModalFooter`) | `open`, `onClose`, `title`, `description`, `icon`, `footer`, `size` `sm`/`md`/`lg`, `presentation` `dialog` / `sheet`, `dismissible`, `closeOnBackdrop`, `closeDisabled`, `closeHint`, `hideClose`, `bare`, `header`, `labelledBy`, `initialFocusRef`, `onExitComplete` | Native modal `<dialog>`: inert background, focus in on open, Tab wraps, Esc via `dismissible`, focus returns to the trigger. `sheet` = trailing side panel for details. |
| `toast` / `<Toaster>` | `toast.success / info / error(title, description?)` | Confirmations of a user action only; errors stay inline. |
| `StateView` | `kind` `loading` / `empty` / `unavailable` / `unsupported` / `disconnected` / `permission` / `consent` / `notImplemented` / `error`; `title`, `description`, `error`, `onRetry`, `action`, `layout` `inline` / `block` / `hero` (`compact` = inline) | Every non-data state. `hero` for a whole page or a big empty panel. |
| `Skeleton` / `SkeletonLines` | `className`, `round`; `lines` | First load only, shaped like the content. |
| `Spinner` | `size` | Indeterminate activity. |
| `MetricValue`, `UnavailableReason`, `useUnavailabilityText` | `metric`, `format` | The value, or a focusable dotted "n/d" whose tooltip gives the reason. |
| `Readout` / `Stat` | `value`, `unit`, `size` `sm`/`md`/`lg`/`xl`; `Stat`: `label`, `footer` | Big tabular number + small unit; `Stat` adds a label and a line under it. |
| `RingGauge` | `percent`, `value`, `unit`, `caption`, `label`, `tone`, `size`, `stroke` | 270° open gauge, round caps, grey unless `tone` `warn`/`bad`. Not animated. |
| `UsageBar` / `ProgressBar` | `percent`, `label`, `valueText`, `tone` / `fraction`, `label`, `tone` `meter`/`accent` | Capsule meters, transform-scaled fill. `UsageBar` (live) never animates; `ProgressBar` eases. |
| `StatusBadge` | `tone` `ok`/`warn`/`bad`/`neutral`, `label`, `glyph`, `size` | Tinted capsule + dot or glyph + words. |
| `Tag` | `children`, `title` | Neutral classification label, not a status. |
| `Table`, `Th`, `SortHeader`, `Tr`, `Td`, `NotReported` | `caption`, `headers` / `head`, `stickyHeader`; `Tr interactive selected` | Solid surface, hairline rows, tabular figures, numbers right-aligned. |
| `SearchField` | `label`, `value`, `onChange`, `placeholder`, `className` | Capsule; Esc clears. |
| `DefinitionList` | `items`, `aligned`, `divided` | `divided` = spec sheet rows (label left, value right). |
| `Disclosure` | `summary`, `children`, `defaultOpen`, `variant` `inline` / `row` | Native `<details>`. |
| `Kbd` | `children` | Shortcut labels (`shortcutLabel()` for the text). |
| `SourceCaption` | `metric` / `source` | Advanced view only. |
| `LineChart` | `series` (`tone` `primary`/`secondary`/`accent`), `xDomain`, `yDomain`, `formatY`, `formatX`, `height`, `ariaLabel`, `yTickCount`, `axisWidth` | Fixed domains, unit in the y labels, crosshair + glass tooltip, gaps break the line. |
| `Sparkline` | `points`, `xDomain`, `yDomain`, `ariaLabel`, `height`, `tone`, `markLast` | Compact trend next to a written value; no axes, no hover. |
| Native `select`, text inputs, `textarea`, checkboxes | – | Styled globally; add only sizing utilities. |

## Shell

- **Sidebar**: a floating glass panel inset 8 px from the window, 236 px wide or 72 px
  collapsed; icons at a fixed x in both states, width animates, labels fade, a satin pill
  springs between rows, the active icon in accent. Collapsed items get tooltips with their
  shortcut. Footer: the global Semplice | Avanzata control and the collapse button.
- **macOS window**: overlay title bar (`tauri.macos.conf.json`: `titleBarStyle: "Overlay"`,
  `hiddenTitle`, `trafficLightPosition`), so the glass reaches the top. The sidebar's top row
  is empty, draggable space for the traffic lights; the page toolbar (64 px, title aligned
  with the lights) also drags the window (`data-tauri-drag-region="deep"`, capability
  `core:window:allow-start-dragging`). Windows keeps its native title bar.
- **Page toolbar** (`PageHeader`): sticky, part of the canvas at rest, a blurred glass band
  with a hairline once content scrolls under it (`data-scrolled` on `<main>`).
- **Running benchmark**: a glass strip floating at the bottom of the main area on every page
  (progress, elapsed time, Stop).
- **Shortcuts**: `⌘B` / `Ctrl+B` toggles the sidebar; `⌘1…7` (macOS, no Drivers) and
  `Ctrl+1…8` (Windows) open the sections in sidebar order; ignored while typing or while a
  dialog is open.
- Pages cross-fade (`PageTransition`, opacity + 4 px); the scroll resets after the old page
  leaves; the main area reserves its scrollbar gutter; focus is always visible (2 px accent
  ring).

## Screens

Each page starts with `PageHeader`: a 34 px large title with one line under it saying what the
page is for (`shell:subtitle_<section>`), at most one primary action. When content scrolls
under it the title settles to 20 px and the line folds away. Built from primitives only. One
focal element per screen; no column ends early leaving a hole (spec rows split evenly, panels
in a row share their height).

### Overview (start page)

1. **Hero** (focal, `rounded-2xl` stage, solid or very subtle glass): the procedural 3D model
   of the device's form factor (tower / laptop / mini PC / all-in-one, `src/features/device3d`; how to add a model: `types.ts`), with the
   **device identity** next to it (model or OS name, CPU, GPU, memory, form factor as
   reported; unknown parts are omitted, not guessed) and the **overall state**: a sentence
   derived from the latest diagnosis ("Nessun problema rilevato" / "2 avvisi" with the
   highest severity tone and the time of the run), or "Diagnosi non ancora eseguita" with a
   Run action. No score, no percentage of health.
2. **Components** as widgets (`ComponentWidgets.tsx`, `widget` material: dense frosted glass,
   24 px radius): CPU, memory, GPU, disk activity. Each has its value large, the last two
   minutes as an area trend next to it, and its secondary readings in a grid. A reading the
   platform cannot provide at all (`notSupported`) leaves the grid and is named once in a
   footnote ("Non rilevabili su questo computer: …"), so a widget never shows a column of
   "n/d". Storage follows full width (volumes, disk health).
3. **Findings and processes** side by side; no findings: a centered empty state.
4. **Per-core grid** (folded).

### Hardware

Inventory only (what the PC is): one panel per component (System, CPU, GPU, Memory, Storage,
Displays) with `DefinitionList divided` spec rows and the component glyph in the title. The
inventory time once in the toolbar area. No live values.

### Benchmarks

1. **Selection**: four component tiles (CPU, GPU, RAM, SSD) as a selectable group, each with
   an isometric illustration of the part (`Illustration.tsx`, theme tokens `--art-*`; it lifts
   on hover, fans spin and RAM chips light while that component's test runs; nothing moves with
   reduced motion), the detected device and availability (unsupported shows why). Below,
   the latest result of every test (seeded from storage at start), or a three-step "how it
   works" before the first run.
2. **Configuration** of the selected component: test type (`Segmented`), duration / size
   options, the start button (primary). One heavy benchmark at a time.
3. **Progress**: the floating running strip (global) plus interim readings on the page;
   Cancel always available (`danger` / secondary "Stop").
4. **Result**: headline `Readout size="xl"` with unit, secondary metrics as `Stat`s, the run
   chart, state badge (completed / cancelled / failed with reason).
5. **Comparison**: against previous runs of the same test on this machine (only comparable
   runs), as a small table or bars; link to History.

### Diagnostics

Toolbar action "Esegui diagnosi" (primary). Summary panel (primary): counts per severity once,
coverage line, time of the run. Findings list: each finding as a panel with a
`StatusBadge glyph` for severity, the headline, **evidence** (the measured values with units
and sources), **"Causa confermata" vs "Causa possibile"** stated explicitly, **suggested
actions** (and the "Risolvi" assistant entry where offered). Limitations and the export panel
last.

### Processes

Ranking `Segmented` + `SearchField` in one control row; the process `Table` (sortable,
`Tr interactive`) as the focal panel; details in a `Modal presentation="sheet"` or a side
panel with its own charts. The Impact Score explanation once.

### History

The list of past runs as a `Table` (newest first) with state badges; selecting a run opens
its result (same `ResultView` as Benchmarks) in a sheet or detail panel. Empty: `StateView
layout="hero"` with "Esegui un benchmark".

### Settings

Grouped rows in secondary panels (Aspetto, Monitoraggio, Privacy, Assistente, Info), each row
label + one short hint + control (`Switch`, `Segmented`, `select`). The view-mode row stays
(the explained version of the sidebar control).

### Drivers (Windows only)

Summary counts once (as filter tabs), problem devices first (warning panel), the device list
as a table with health and update badges, the update-check panel short.

### The "Risolvi / Fix it" dialog

`Modal size="lg"` (`FixDialog`), phases inside one dialog; Esc only when nothing would be
lost; the close button disabled while a step runs (tooltip says why).
