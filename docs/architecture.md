# Architecture

## Overview

PC Diagnostics is a Tauri 2 desktop app for Windows and macOS. A Rust backend gathers
real system data and runs benchmarks. A React/TypeScript UI renders that data. The two
sides communicate only through typed Tauri commands and channels. The UI runs in the
system web view: WebView2 on Windows, WKWebView on macOS.

```
React UI ──invoke / Channel──▶ src-tauri (commands, state, validation)
                                    │
     ┌──────────┬───────────┬───────┴───┬────────────┬─────────────┐
  monitor    processes    drivers     bench      diagnostics    persistence
     │          │           │           │      (pure, no I/O)   (SQLite)
     └──────────┴─────┬─────┴───────────┘
        platform (OS access; windows/ and macos/ impls)
                      │
                    models (shared contracts, exported to TypeScript)
```

## Crates

| Crate | Role | Status |
|---|---|---|
| `crates/models` | Shared serializable types, `Metric<T>`, `AppError`, settings, benchmark state machine, `Finding`. Generates TS bindings. | Phase 1 |
| `crates/platform` | The only crate that calls OS APIs. Windows providers in `windows/`, macOS providers in `macos/` (see "Platform layer"). | Phase 1; macOS port 2026-11 |
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
- Data lives in the per-user app data dir (`%APPDATA%\com.pcdiagnostics.desktop` on
  Windows, `~/Library/Application Support/com.pcdiagnostics.desktop` on macOS). No admin
  rights are needed.
- The CSP is the same on both systems. Tauri serves the frontend from
  `http://tauri.localhost` on Windows and `tauri://localhost` on macOS (both are `'self'`),
  and IPC goes to `http://ipc.localhost` on Windows and `ipc://localhost` on macOS, which
  `connect-src ipc: http://ipc.localhost` covers.
- macOS bundle: ad-hoc signed, hardened runtime, no entitlements (no sandbox exceptions,
  JIT, or library-validation exceptions are requested). The only Info.plist addition is
  `NSDocumentsFolderUsageDescription`, because report export writes to `~/Documents`,
  which TCC protects.
- No telemetry. Network use (Windows Update driver search, phase 4) is gated by
  `allowOnlineChecks`, which defaults to `false`.

## Platform layer

`crates/platform` is the only crate that talks to the OS. Each module (`live`,
`inventory`, `process`, `system`, `credentials`, `bench_env`, `gpu_bench`, `devices`,
`locale`, `overhead`, `remediation`) exposes one OS-independent API that returns types
from `models`, and dispatches by target:

```
platform/src/<module>.rs      public API, shared logic, tests
  ├─ #[cfg(windows)]                              → windows/<provider>.rs
  ├─ #[cfg(target_os = "macos")]                  → macos/<provider>.rs
  └─ #[cfg(not(any(windows, target_os = "macos")))] → explained Unavailability
```

| | Windows (`windows/`) | macOS (`macos/`) |
|---|---|---|
| APIs | PDH, WMI, DXGI, D3DKMT, NVML, SetupAPI/CfgMgr32, WUA, Toolhelp/NtQuerySystemInformation, Credential Manager | sysctl, Mach host statistics, I/O Kit (`objc2-io-kit`), libproc (through `sysinfo`), Security framework |
| `Source` variants | `Pdh`, `Wmi`, `Dxgi`, `D3dkmt`, `Nvml`, `Kernel`, `PowerApi`, `ConfigManager`, `WindowsUpdate` | `IoKit` ("I/O Kit"), `Mach` ("Mach (host_statistics64)"); `Sysinfo` and `Benchmark` on both |
| Not provided | | Driver inventory and Windows Update (no equivalent); Windows-only concepts such as commit charge stay unavailable rather than being filled with a value of a different meaning |

Rules for macOS providers: no admin rights, privileged helpers or kernel extensions; no
shelling out to `powermetrics`, `system_profiler`, `ioreg`, `diskutil` or similar tools for
data; every `unsafe` block has a `// SAFETY:` comment. `AppInfo.platform` (`windows` |
`macos` | `linux` | `other`) tells the UI which system it runs on, so it can word texts
per platform. The per-item status of the port is in `docs/roadmap.md` ("macOS port").

## Packaging

`src-tauri/tauri.conf.json` is the shared config and builds the NSIS installer on Windows.
Tauri merges `src-tauri/tauri.macos.conf.json` on top of it on macOS: `app` + `dmg`
targets, the `.icns` icon, `minimumSystemVersion` 13.3 (the first macOS that ships the
WebKit Tailwind CSS 4 needs), ad-hoc signing and the localized `InfoPlist.strings`.
`src-tauri/Info.plist` is merged into the bundle's Info.plist. CI runs `npm run check` on
`windows-latest` and `macos-latest`; the Release workflow builds both bundles for a tag.

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
