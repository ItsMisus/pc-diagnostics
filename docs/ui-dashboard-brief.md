# Dashboard redesign brief (2026-11-05)

Two goals, from the user:

1. **Monitoring-dashboard look.** The reference is a hardware-monitoring app the user runs
   (captured on their PC for inspiration). What defines that style, and what we adopt:
   - Neutral dark greys, monochrome: meters, gauges, bars and charts are grey/white, not
     coloured. Colour appears only for status (ok / warn / bad, always with text) and,
     sparingly, a muted violet for focus and the one primary action.
   - Flat panels on a slightly darker canvas, a visible hairline border, small radius (6 px),
     no shadows.
   - Each panel is titled with the component's name in bold ("CPU", "GPU", "RAM").
   - Headline values are big numbers in a light weight with a smaller unit ("10%",
     "5425 MHz"), or an open ring gauge (270°, opening at the bottom, caption such as "Load"
     in the opening).
   - Secondary values are compact rows: a thin bar above, label on the left, value on the
     right.
   - Dense: the whole system state fits on one screen (CPU, GPU, RAM, network/disk,
     storage, top processes). Almost no explanatory text.
   - Navigation: grey icon + label, the active item white and bold.

   This is our own implementation of that style: no third-party names, logos, icons, fonts or
   assets in the app or the repository.
2. **No redundancy.** Each piece of information lives in one place. The user found the app
   repetitive: the same value, count, explanation or control appeared several times.

The hard rules still apply: real data only, live data never animates, no global health
score, status colour never the only signal, everything in Italian and English.

## Foundation (orchestrator, already in place)

- Tokens (`src/styles/index.css`): neutral greys; `--meter` (fill of meters, gauges, chart
  secondary) and `--gauge-track`; muted violet accent kept for focus/primary/selection. Light
  theme kept with the same structure. Text pairs are WCAG AA.
- `RingGauge` (`src/components/common/RingGauge.tsx`): open 270° monochrome gauge; props
  `percent`, `value`, `unit`, `caption`, `label`, `size` (140 headline, ~96 secondary).
- `Readout` (`src/components/common/Readout.tsx`): big light number + small unit.
- `UsageBar`: thin rectangular grey bar (use it above label/value rows).
- `Card`: flat panel, radius 6, padding 16, bold component title. Use `Card` for every panel.
- `PageHeader`: title + actions only. The `description` prop is ignored and will be
  removed: delete it from every call site you own.
- `SourceCaption` renders only in the Advanced view.
- `LineChart` is monochrome.
- Sidebar: the **only** global "Semplice | Avanzata" control is in its footer (plus the
  explained row in Settings).

## Rules

- **R1. One home per value.** Live values (usage, temperature, clock, power, fan, memory and
  VRAM in use, disk throughput) live on **Overview** (the dashboard). **Hardware** shows what
  the PC *is* (inventory and specs), no live values. A running benchmark may show its own
  readings, but no page repeats the dashboard.
- **R2. No page subtitles, no "Live / In tempo reale" badges.** Live is the normal state.
  Show a state only when it is not live (paused, stale, unavailable), next to the affected
  data. Timestamps ("collected at …") at most once per page, where they matter.
- **R3. Sources are technical.** "fonte: PDH" only in the Advanced view (`SourceCaption`
  handles it; don't render source text any other way).
- **R4. One view-mode control.** Remove `ViewModeToggle` from page headers. Pages read the
  mode with `useIsSimple()` / `useViewMode()` as before.
- **R5. Counts once.** A number shown in a summary is not repeated on the same screen in a
  heading badge, a filter tab, a sentence or a second card. Keep it where it is most useful
  (usually the control that acts on it, e.g. a filter tab).
- **R6. Explanations once and short.** An explanation appears in one place, next to what it
  explains (short hint or tooltip), not also in a "how to read this page" box or a footer.
  If a sentence restates what the UI already shows, delete it.
- **R7. No echoing labels.** Don't repeat the card title in labels ("CPU · Utilizzo CPU"), and
  don't show the same field twice in different words.
- **R8. Dashboard visual language.** As described above: monochrome meters, `RingGauge` /
  `Readout` for headline values, compact bar rows for secondary values, bold component
  titles, dense layout. Accent only for focus, selection and the one primary action.

When you remove a text, delete its key from both `it` and `en` locale files. New text needs
both languages. Keep tests passing; update tests that asserted removed text.

## Page targets

- **Overview → the dashboard, one screen.** Row 1: CPU and GPU panels, each with a
  `RingGauge` for load (caption "Carico"/"Load") and compact bar rows for temperature, clock,
  power, fan (GPU: also VRAM). Unavailable values show "n/d" with the reason in the tooltip.
  Row 2: RAM (gauge for memory in use, used/total), disk activity (read/write `Readout`s,
  and the most active process if it is already available from the process data), storage
  (one bar per volume, used/total, plus the Windows disk status). Then the top processes
  (a compact table, a few rows, linking to Processes). The recent-trend charts may stay below
  but must not repeat the current values (no current value in chart headers). The
  per-logical-processor usage grid moves here from Hardware, collapsed by default. The
  diagnostics insights card stays, compact.
- **Hardware → specs.** Inventory only: one panel per component with its specs (label/value
  rows); no LIVE column, no live meters. Inventory time once.
- **Processes.** One short line of context at most; the Impact Score explanation once; counts
  once; notes as tooltips where they belong (e.g. the I/O column header). Table in the same
  compact style as the dashboard's top-processes table.
- **Drivers.** Summary counts once (cards *or* filter tabs); "Working ≠ up to date" and the
  other reading notes in one place; the update-check panel short.
- **Diagnostics.** Severity counts once; coverage compact (summary line + badges, notes only
  when they add something); finding cards without repeated fields. The short-window warning
  appears once, not again in every component's coverage note.
- **Benchmarks / History.** The same result list or details never twice; History is the list
  of past runs, Benchmarks is where you run tests and see the current/last result.
- **Settings.** Rows with one short hint each; no repeated explanations; the view-mode row
  stays (it is the explained version of the sidebar control).

## Ownership

- Orchestrator: `src/styles/**`, `src/components/**`, `src/app/**`, `src/hooks/**`,
  `src/i18n/index.ts`, `src/i18n/locales/*/{common,shell}.ts`, docs.
- Agent A: `src/features/{overview,hardware,monitoring}/**` + their locale files.
- Agent B: `src/features/{diagnostics,drivers,assistant}/**` + locale files, and
  `crates/diagnostics/**` only to remove redundant evidence or coverage notes.
- Agent C: `src/features/{processes,benchmarks,history,settings}/**` + locale files.
