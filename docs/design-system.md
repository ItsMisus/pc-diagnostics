# Design system

The visual and motion language of PC Diagnostics. Direction (since 2026-11-05): a
monitoring dashboard. Neutral dark greys, flat panels with a visible hairline, monochrome
meters, ring gauges and charts, big light numbers with small units, dense layouts with
almost no explanatory text. A muted violet accent only for focus, selection and the primary
action. See `docs/ui-dashboard-brief.md` for the rules (one home per value, no
redundancy).

Sources of truth:

- Tokens: `src/styles/index.css` (CSS custom properties, exposed to Tailwind via `@theme inline`).
- Motion: `src/lib/motion.ts` (durations, easings, springs, variants for Motion for React).
- Primitives: `src/components/common/*`, layout in `src/components/layout/*`.

## Color

| Token | Use |
| --- | --- |
| `canvas` | Window background behind content. |
| `sidebar` | Navigation rail background. |
| `surface` | Cards, dialogs, inputs. |
| `surface-2` | Hover fills, inset panels, chips, tooltips. |
| `surface-3` | Meter tracks, pressed fills, skeletons. |
| `line` / `line-strong` | Hairline borders; `line-strong` for hover borders and floating surfaces. |
| `fg` / `fg-muted` / `fg-subtle` | Primary text / secondary text / captions, axis labels, placeholders. |
| `accent` (+ `-hover`, `-fg`, `-soft`, `-text`) | Selection, focus, primary action, live data series. Never for status. |
| `ok` / `warn` / `bad` (+ `-soft`) | Status only, always paired with text. |

Rules:

- The accent appears in small doses: the active nav icon, the selected thumb, primary buttons,
  chart lines, meters, focus rings. One primary button per view at most.
- Status color is never the only signal (`StatusBadge` always has a label).
- Meters are grey (`--meter`) until a value crosses a threshold where "high" means a
  problem; then the fill takes `--warn` / `--bad` and a short status word ("Avviso",
  "Critico") sits next to the value. The thresholds live in `src/features/overview/levels.ts`
  and mirror the diagnostics rules (available memory, commit, VRAM, free space). Load, power
  and temperature stay grey: high load is normal, and there is no diagnostics threshold for
  temperature.
- Do not use raw hex or Tailwind palette colors (`zinc-500`, `black/40`...) in components.

## Typography

- Body: Segoe UI Variable Text, 14 px base. Display (page titles): Segoe UI Variable Display.
- Scale: `text-2xs` 11 (captions, table headers, sources), `text-xs` 12 (hints), `text-[13px]`
  (controls, nav, dense body), `text-sm` 14 (body), 15 (dialog titles), `text-lg` 18 (card
  titles), `text-xl` 20 (section headings), 26 (page titles); headline metrics are light
  numbers (`Readout`, `RingGauge`).
- Numbers: always `tabular` so live values do not jitter.
- Decimals and percentages follow the language, like the backend's `fmt::num` / `fmt::pct`:
  "19,4 GB", "6,2 %" in Italian; "19.4 GB", "6.2%" in English; no thousands separator
  ("4700 MHz"). Use `formatNumber` / `formatPercent` (`src/lib/format.ts`), never `toFixed`
  in a component.
- Byte sizes scale by 1024 and follow the view mode: "Semplice" uses the labels File
  Explorer and Task Manager use (KB, MB, GB, TB), so the numbers match Windows; "Avanzata"
  uses the exact IEC names (KiB, MiB, GiB, TiB). Always format them with `formatBytes` /
  `formatBytesPerSec` / `formatMib` (`src/lib/format.ts`), never a hand-written unit. The
  backend's texts follow the same rule (`fmt::bytes_plain` / `fmt::bytes`).
- Section labels inside cards: `eyebrow` utility (11 px, uppercase, tracked, `fg-subtle`).
- Hierarchy of headings: page title 26 px (`PageHeader`) > section heading 20 px
  (`SectionHeading`, a group of cards such as "Andamento recente" or "Avvisi") > card title
  18 px semibold `fg`, display font (handled by `<Card title>`, the component's name: "CPU").

## Space, radius, elevation

- 4 px grid. Card padding 16 (`p-4`); grid gaps 16 (`gap-4`); section spacing 24–32.
- Radius: one small radius, `rounded-md` (7), for cards, panels, dialogs, tooltips, toasts, controls and navigation rows; `rounded-sm` for tiny chips, `rounded-full` for pills and gauges.
- Elevation: cards are flat (hairline, no shadow). Shadows only on floating
  surfaces: `shadow-md` tooltips/chart tooltips, `shadow-lg` dialogs and toasts.

## Components

| Component | Notes |
| --- | --- |
| `Button` | `primary` / `secondary` / `ghost` / `danger`, sizes `sm` / `md`. Press = 3 % scale. |
| `Card` | Flat panel (`rounded-md`, hairline). `title` (component name, 18 px semibold), `description`, `aside`, `flush`. |
| `SectionHeading` | 20 px heading of a group of cards, optional muted `count`. |
| `Segmented` | Radio group, sliding thumb, arrow keys. `size="sm"` for dense headers. |
| `Switch` | Spring thumb. Settings' `Toggle` delegates to it. |
| `Tooltip` | Tooltips (delay 350 ms, instant on keyboard focus, Esc). Supplementary only. `multiline` wraps a sentence at 16 rem; `side="top-end"` for triggers near a right edge. Never a native `title` for information the user needs: it does not open from the keyboard. |
| `MetricValue` | A value, or a focusable "n/d" whose `Tooltip` gives the reason (plain + technical in "Semplice"). |
| `toast` / `<Toaster>` | `toast.success(title, description?)` for confirmations of a user action. Errors stay inline. |
| `Skeleton` / `SkeletonLines` | First-load placeholders. Never for refreshes of data already shown. |
| `Spinner` (in `StateView`) | Indeterminate activity. |
| `StatusBadge` | Tinted chip + dot + text. |
| `RingGauge` | Open 270° gauge for headline live values, grey unless `tone` (`warn` / `bad`) is set; number + small unit in the centre, caption in the opening. Not animated. |
| `Readout` | Big light number + small unit. |
| `UsageBar` / `ProgressBar` | Transform-scaled fill; `UsageBar` takes the same `tone` as `RingGauge`. `UsageBar` (live) is not animated; `ProgressBar` (benchmark) eases. |
| `Disclosure` | Native `<details>`; height animates via global CSS, chevron rotates. |
| Native `select`, text inputs, checkboxes | Styled globally in `index.css`; just use the element. Add only sizing utilities (`h-8`, `w-56`, `px-2.5`). |

## Motion

| Kind | Duration | Curve |
| --- | --- | --- |
| Micro (hover, press, toggle, checkbox) | 100–180 ms (`--dur-micro` 140) | `ease-out` |
| Component (tooltip, toast, disclosure, dialog) | 180–280 ms (`--dur-component` 220) | `ease-out` / `spring.gentle` |
| Navigation (page, sidebar width) | 220–350 ms (`--dur-nav` 280) | `ease-out`; exits 90 ms `ease.in` |
| Selection indicators | spring `indicator` (520 / 42) | no visible overshoot |

Principles:

1. Every animation answers a user action or a navigation. Live data never animates: values,
   meters and charts update in place. (Measured: a 500 ms transition on the live meters
   quadrupled idle CPU, 0.08 % to 0.32 % of the machine.)
2. Animate `transform` and `opacity` (and colors). Never animate width/height per frame,
   except the sidebar width and `<details>` height, which are single, short, user-triggered.
3. Exits are faster than entrances.
4. Use `m.*` components (LazyMotion strict); `motion.*` throws in development.
5. Reduced motion: `MotionConfig reducedMotion="user"` disables transform/layout animations;
   CSS transitions are zeroed by the global media query.
6. No infinite animations except indeterminate spinners and skeletons, which only exist while
   something is actually loading.

## Shell

- Sidebar: icons stay at a fixed x in both states; width animates; labels fade; selection pill
  springs between rows; collapsed items get tooltips with their shortcut.
- Shortcuts: `Ctrl+B` toggles the sidebar; `Ctrl+1…8` jumps to a section (ignored while typing
  or while a dialog is open).
- Pages cross-fade (`PageTransition`, `mode="wait"`); scroll resets after the old page leaves.
- The main area reserves its scrollbar gutter so content never shifts when a page starts scrolling.
