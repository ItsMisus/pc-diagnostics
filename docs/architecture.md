# Architecture

## Overview

PC Diagnostics is a Tauri 2 desktop app. A Rust backend gathers real system data and
runs benchmarks. A React/TypeScript UI renders that data. The two sides communicate
only through typed Tauri commands and channels.

```
React UI ──invoke / Channel──▶ src-tauri (commands, state, validation)
                                    │
     ┌──────────┬───────────┬───────┴───┬────────────┬─────────────┐
  monitor    processes    drivers     bench      diagnostics    persistence
     │          │           │           │      (pure, no I/O)   (SQLite)
     └──────────┴─────┬─────┴───────────┘
                  platform (OS access; windows/ impl)
                      │
                    models (shared contracts, exported to TypeScript)
```

## Crates

| Crate | Role | Status |
|---|---|---|
| `crates/models` | Shared serializable types, `Metric<T>`, `AppError`, settings, benchmark state machine, `Finding`. Generates TS bindings. | Phase 1 |
| `crates/platform` | The only crate that calls OS APIs. Currently returns OS facts via `sysinfo`. | Phase 1 (minimal) |
| `crates/persistence` | SQLite access, append-only migrations, settings repository. | Phase 1 |
| `src-tauri` | Thin app shell: command handlers, managed state, input validation. | Phase 1 |
| `crates/monitor` | Sampler, ring buffers, provider scheduling. | Phase 2 |
| `crates/processes` | Process sampling and the Impact Score. | Phase 3 |
| `crates/drivers` | Device and driver inventory, update verification. | Phase 4 |
| `crates/bench` | Safety controller and the CPU/RAM/SSD benchmarks; client for the GPU worker. | Phase 5 |
| `crates/diagnostics`, `crates/report` | Rule engine, report export. | Phase 6 |

## Core contracts

- **`Metric<T>`** holds either a measured value (`available`: value, unit, source,
  timestamp) or an explained absence (`unavailable`: reason). The reasons are
  `notSupported`, `providerMissing`, `permissionDenied`, `notImplemented` and `failed`.
  Nothing is ever fabricated, and there is no "unknown".
- **`AppError`** is `{ code, message, detail, retriable }`. Every command rejects only
  with this type. The UI maps `code` to localized copy and shows `message`/`detail` in
  a "technical details" disclosure.
- **`BenchmarkState`** is the run lifecycle. Legal transitions are defined once, in
  `BenchmarkState::can_transition_to`, and unit-tested.
- **`Finding`** is a diagnostic result. It carries an explicit epistemic status
  (`observation` / `hypothesis` / `confirmed`), a confidence, evidence and limitations.

### Type generation

Rust is the single source of truth. `npm run gen:types` runs `cargo test -p models`,
which makes ts-rs write `src/types/generated/*.ts`. These files are committed and must
never be edited by hand.

## IPC rules

- Command names are listed in two places: `generate_handler!` (`src-tauri/src/lib.rs`)
  and the `CommandName` union (`src/services/ipc.ts`). Keep them in sync.
- Blocking work runs on `spawn_blocking` (helper `commands::blocking`) and never on the
  IPC thread.
- All input is validated in Rust (for example `AppSettings::validate`). The UI is
  never trusted.
- Streams use `tauri::ipc::Channel`, one per invocation, which keeps messages ordered
  and avoids cross-run races. Global events are reserved for the live sampler.
- When the UI runs outside the Tauri runtime (plain browser), every call rejects with
  `notSupported`. The UI shows that state and never substitutes demo data.

## Security

- Capabilities (`src-tauri/capabilities/default.json`) grant only
  `core:app|event|window:default`. No shell, fs or http plugins are installed, so the
  frontend cannot run commands or read files.
- A strict CSP is in place: `script-src 'self'`, no remote origins.
- Data lives in the per-user app data dir (`%APPDATA%\com.pcdiagnostics.desktop`). No
  admin rights are needed.
- No telemetry. Network use (Windows Update driver search, phase 4) is gated by
  `allowOnlineChecks`, which defaults to `false`.

## Persistence

- SQLite (bundled) is opened in WAL mode with `foreign_keys=ON` and a 5 s busy timeout.
- Migrations are run by `rusqlite_migration` (`user_version`). They are append-only:
  never edit a released migration.
- Corrupted or invalid stored settings fall back to defaults with a warning instead of
  blocking startup.
- The connection sits behind a `Mutex` in `AppState`. It will be replaced by a
  dedicated writer thread when history writes arrive (phase 5/6).

## Frontend

- Navigation uses Zustand state, with no router. Components subscribe through narrow
  selectors to limit re-renders.
- Design tokens are CSS variables in `src/styles/index.css` and are exposed to
  Tailwind v4 through `@theme inline`. Dark mode follows `prefers-color-scheme`
  unless `<html data-theme>` overrides it.
- `StateView` covers every non-data state: loading, empty, unsupported, permission,
  notImplemented and error. `notImplemented` (our gap) and `unsupported` (the
  machine's limit) are deliberately different.
- Charts are custom SVG (`components/charts`) with a fixed y-domain, so there is no
  auto-zoom distortion. A gap (`v: null`) breaks the line instead of interpolating.

## Multi-agent ownership (from phase 2)

| Agent | Owns | Read-only |
|---|---|---|
| 1 Hardware & Bench | `platform/windows/{pdh,dxgi,power,nvml}`, `monitor`, `bench` | `models` |
| 2 Process | `processes` | `models`, `platform` traits |
| 3 UI/UX | `src/**` except `src/types/generated` | generated types |
| 4 Diagnostics | `diagnostics`, `report`, `persistence` | `models` |
| 5 Driver | `drivers`, `platform/windows/{cfgmgr,wua}` | `models` |

Only the orchestrator changes `models` and `src-tauri/src/commands`. Each agent works in
its own git worktree, and the orchestrator merges and runs the full check suite.
