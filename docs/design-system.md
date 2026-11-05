# Design system

The visual and motion language of PC Diagnostics. Direction: quiet, precise, desktop-native
(Linear / Raycast / macOS), graphite neutrals with one discreet indigo accent. Data first;
no decoration that does not carry meaning.

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
- Do not use raw hex or Tailwind palette colors (`zinc-500`, `black/40`...) in components.

## Typography

- Body: Segoe UI Variable Text, 14 px base. Display (page titles): Segoe UI Variable Display.
- Scale: `text-2xs` 11 (captions, table headers, sources), `text-xs` 12 (hints), `text-[13px]`
  (controls, nav, card titles, dense body), `text-sm` 14 (body), 15 (dialog titles),
  22 (page titles), `text-2xl` 24 (headline metrics).
- Numbers: always `tabular` so live values do not jitter.
- Section labels inside cards: `eyebrow` utility (11 px, uppercase, tracked, `fg-subtle`).
- Card titles: sentence case, 13 px medium, `fg-muted` (handled by `<Card title>`).

## Space, radius, elevation

- 4 px grid. Card padding 16 (`p-4`); grid gaps 16 (`gap-4`); section spacing 24–32.
- Radius: `rounded-md` (7) controls, `rounded-lg` (10) panels/tooltips/toasts, `rounded-xl` (12) cards, dialogs.
- Elevation: cards are flat (hairline + faint top highlight). Shadows only on floating
  surfaces: `shadow-md` tooltips/chart tooltips, `shadow-lg` dialogs and toasts.

## Components

| Component | Notes |
| --- | --- |
| `Button` | `primary` / `secondary` / `ghost` / `danger`, sizes `sm` / `md`. Press = 3 % scale. |
| `Card` | `title`, `description`, `aside`, `flush`. |
| `Segmented` | Radio group, sliding thumb, arrow keys. `size="sm"` for dense headers. |
| `Switch` | Spring thumb. Settings' `Toggle` delegates to it. |
| `Tooltip` | Label tooltips (delay 350 ms, instant on keyboard focus, Esc). Supplementary only. |
| `toast` / `<Toaster>` | `toast.success(title, description?)` for confirmations of a user action. Errors stay inline. |
| `Skeleton` / `SkeletonLines` | First-load placeholders. Never for refreshes of data already shown. |
| `Spinner` (in `StateView`) | Indeterminate activity. |
| `StatusBadge` | Tinted chip + dot + text. |
| `UsageBar` / `ProgressBar` | Transform-scaled fill. `UsageBar` (live) is not animated; `ProgressBar` (benchmark) eases. |
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
