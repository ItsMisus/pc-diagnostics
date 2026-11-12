# Roadmap and feature status

Status values: **done** (implemented, wired to the UI, tested), **partial**, **planned**.
A feature counts as done only when it meets the acceptance criteria in the project spec.

## Phase 1: Foundation

| Item | Status | Notes |
|---|---|---|
| Cargo workspace + Tauri 2 shell | done | Tauri 2.12 stable (3.x is alpha, not used) |
| React 19 + TS 7 + Vite 8 + Tailwind 4 | done | |
| Design system (tokens, dark/light, Card, Button, StatusBadge, StateView, MetricValue, Icon) | done | |
| SVG LineChart + scale utilities | done | Tested. First used in phase 2 |
| Sidebar navigation, collapsible, persisted | done | |
| Shared contracts (`models`) + TS generation | done | ts-rs 12 |
| `AppError` model and IPC error normalization | done | |
| SQLite + migrations + settings repository | done | |
| Commands: `get_app_info`, `get_system_info`, `get_settings`, `update_settings` | done | |
| Settings page (theme, sampling interval, top N, online checks) | done | Interval, top-N and online-check values are stored now; their consumers arrive in phases 2–4 |
| Overview: real OS info | done | |

## Phase 2: Hardware Monitoring — done

| Item | Status | Notes |
|---|---|---|
| Hardware inventory (CPU, GPUs via DXGI, RAM modules via WMI, physical disks via `MSFT_PhysicalDisk`, volumes) | done | Cached; "Aggiorna inventario" recollects (~40 ms with a warm WMI service) |
| CPU live: total and per-core usage, current MHz, max/limit MHz | done | Busy % is computed from raw PDH counters. Current MHz uses the Task Manager method (`% Processor Performance` × `MaxMhz`) because `CurrentMhz` is static on Ryzen |
| CPU temperature / power | unavailable by design | `providerMissing`: needs a kernel-mode driver, which is not bundled |
| Memory live: used/available, commit, pages/sec | done | |
| GPU live: usage, dedicated VRAM (PDH, all vendors) | done | |
| NVIDIA temperature, clock, power (NVML) | done | Matched to DXGI adapters by PCI location. `nvml.dll` is loaded only from absolute system paths |
| AMD/Intel GPU sensors | planned | `notImplemented` until the ADLX/IGCL adapters exist |
| Virtual display adapters | done | Flagged `isVirtual`, never chosen as the primary GPU, no telemetry |
| Disk activity (read/write B/s, active time) | done | `\PhysicalDisk(_Total)` |
| Sampler: bounded ring buffer (300), interval from settings, applied live | done | Verified end to end (UI click → SQLite → sampler) |
| Overview cards and charts, Hardware page, app overhead card | done | |

Measured (release build, Ryzen 9 7900X, 24 threads, Overview open, 1 s interval, 60 s window):

| Metric | Value |
|---|---|
| `sample()` cost | 1.37 / 1.54 / 3.33 ms (min / median / max) |
| CPU, whole process tree | 0.043 % of the machine (~1 % of one core) |
| CPU, Rust core | 0.021 % of the machine |
| Working set, Rust core | 69.6 MiB |
| Working set, tree (core + 6 WebView2 processes) | 445 MiB. This is the sum of working sets, so shared pages are counted more than once; most of it is the WebView2 runtime's baseline |

Known limits: the DXGI adapter list is read when the sampler starts (a hot-plugged GPU
needs a restart). Systems with more than 64 logical processors are untested.

Dev-only note: on the first `tauri dev` run after adding npm dependencies, Vite reloads
the page while optimizing them. The pre-reload event listener then stays registered in
Rust and logs `Couldn't find callback id` on each sample. Restarting `tauri dev` clears
it. Production builds do not reload.

## Phase 3: Process Analysis — done

| Item | Status | Notes |
|---|---|---|
| Enumeration of every process via `NtQuerySystemInformation` (no handles, no admin) | done | 389 processes on the test machine; 182 are protected (path unreadable) but still have real metrics |
| CPU / RAM / logical I/O per process, 2 s cadence | done | ~6.7 ms per tick (release, median) |
| Impact Score v1 (0.45·CPU + 0.25·RAM + 0.30·I/O), instant and 60 s window rankings, peaks | done | Formula and limits in docs/impact-score.md |
| Categories (System / Service / Application / Background) | done | Heuristics documented |
| Processes page: sortable table, search, detail with history charts | done | Read-only, with no actions on processes |

## Phase 4: Driver Diagnostics — done

| Item | Status | Notes |
|---|---|---|
| Device inventory via SetupAPI/CfgMgr32 + `Win32_PnPSignedDriver` (signature) | done | 262 devices, ~1.1 s (WMI is about 85 % of that) |
| Health from CM problem codes (working / problem / driver missing / disabled) | done | Table in docs/driver-verification.md |
| Windows Update driver search (opt-in, read-only, one at a time) | done | Works without admin. No device ever becomes "verified" just because WU offered nothing |
| Drivers page: summary, filters, details, consent flow | done | Read-only: no install, remove or update |

## Phase 5: Benchmarks — done

| Item | Status | Notes |
|---|---|---|
| Safety controller: one run at a time, cooperative cancel, watchdog, cleanup on every path | done | Cancel to result takes 2–6 ms (release, 24 threads) |
| CPU single-core / multi-core / sustained (methodology v1, checksum-pinned workload) | done | Single-core 19.2 kop/s verified in the app |
| RAM bandwidth (read/write/copy) and latency (indicative) | done | 60.9 / 27.7 / 35.4 GB/s; 97.6 ns |
| Storage sequential and random (temporary file, `NO_BUFFERING`, `DELETE_ON_CLOSE`, explicit consent, write budget) | done | Samsung 980: 2.57 GB/s seq write. Free-space reserve is `clamp(10 %, 2 GiB, 20 GiB)` |
| Global running bar with an always-available stop | done | |
| GPU compute benchmark (wgpu DX12/Vulkan, isolated worker process, CPU-verified output, NVML temperature stop) | done | RTX 4070 SUPER: 21.8–23.2 TFLOP/s; cancel 171 ms; no child left. Only the primary physical GPU can be tested (no selector) |
| GPU identity and driver in `BenchmarkContext`, required for comparisons | done | |

Methodology, thresholds and ids: docs/benchmark-methodology.md.

## Phase 6: Diagnostics and Reports — done

| Item | Status | Notes |
|---|---|---|
| Deterministic rules v1 (CPU, RAM, GPU, storage, drivers, processes) with evidence, kind and confidence | done | docs/diagnostics-rules.md |
| Coverage per component; no global health score | done | |
| Benchmark history in SQLite (migration v2) and comparison of compatible runs, with caveats | done | |
| JSON/HTML export to Documents\PC Diagnostics with redaction | done | Verified: no hostname, username or scripts in the HTML |
| Diagnostics collect the driver inventory if it was not loaded yet | done | |

## Phase 7: Optimization and Packaging — done

| Item | Status | Notes |
|---|---|---|
| NSIS installer, per-user, no admin (`npx tauri build`) | done | 4.8 MiB (5 040 170 bytes). Silent install (`/S`), uninstall and reinstall verified: program files, Start menu link and the HKCU uninstall entry are removed; the SQLite history in `%APPDATA%` is kept and shown again after reinstall |
| Installed app runs every benchmark kind from the UI, including GPU in its worker | done | GPU 21.8 TFLOP/s from the installed app; the worker exits 4 s after the result |
| Stability (installed release, Processes page open, 1 s sampling) | done | 403 s window: CPU mean 0.057 % of the machine (max 0.081 % per 20 s step); process tree (7 processes, mostly WebView2) 476 → 487 MiB working set, flat over the last 2 minutes; Rust core 81.6 → 84.7 MiB, private 40.7 → 43.5 MiB (one step at 141 s, then flat); handles stable (~3 790 tree, ~505 core) |
| Idle overhead (release, Overview page) | done | 0.043 % of the machine, 70 MiB core, 445 MiB tree (measured before wgpu was added; the soak above includes it) |
| Release binary size | done | 18.8 MB with wgpu (13.2 MB without) |
| Code-signing pipeline (`scripts/sign.ps1` via `bundle.windows.signCommand`, opt-in with `PCDIAG_SIGN_THUMBPRINT`) | done | Verified with a temporary self-signed certificate (deleted afterwards): app exe, uninstaller, installer and NSIS plugins signed with an RFC 3161 timestamp. Without the variable the build stays unsigned |
| Publicly trusted signature | not done | Needs a purchased code-signing certificate or an Azure Trusted Signing account; until then SmartScreen may warn on first run |
| Multi-hour soak (installed release, idle) | done | 121 samples over 14 416 s (about 4 h). CPU mean 0.061 % of the machine (max 0.237 %). Rust core: working set 68.8–74.7 MiB, private 40.6–45.0 MiB, handles 492–508. From 30 min to 3 h 50 min the core was flat (72.8 MiB working set, 44.0 MiB private, 492 handles). In the last 8 minutes one step appeared together with CPU activity (0.13–0.24 %): core +1.9 MiB working set, +0.9 MiB private, handles 500–508; the cause was not identified (the app window may have been in use). Process tree (7 processes): working set 446–501 MiB, handles 3 696–3 869; tree private memory was flat at ~239 MiB, peaked at 372 MiB in that window (WebView2) and settled at ~292 MiB |
| Other machines (Intel GPU, laptops on battery) | not done | Only the development desktop was tested |

## Post-v1 additions (2026-11-04)

| Item | Status | Notes |
|---|---|---|
| Vendor-neutral GPU sensors via WDDM perf data (D3DKMT): temperature, clock, power share, fan, driver thresholds | done | Radeon iGPU now reports 49–54 °C and 600–2200 MHz; NVIDIA cross-check: 12.1 % power share = 26.6 W NVML at a 220 W limit. Intel uses the same path, untested |
| GPU selector in the GPU benchmark, per-target availability, stop temperature and max duration | done | iGPU from the UI: 449 GFLOP/s, no driver threshold so 30 s cap; RTX: 23.1 TFLOP/s, stop at 90 °C |
| `Unit::FlopsPerSecond`, `BenchmarkResult.notes` (neutral info, separate from warnings) | done | |
| Report times in local time with the UTC offset printed; local-time file names | done | DST-aware via `SystemTimeToTzSpecificLocalTime` |
| Comparison caveats list only warnings present in one of the two runs | done | |
| "Semplice / Avanzata" view (setting, default Semplice) | done | Plain-language text for every finding (written by the rules, jargon-checked by tests), plain driver problem codes, plain errors and unavailable reasons; technical details behind "Mostra dettagli tecnici". HTML reports gain an "In parole semplici" block |
| "Risolvi" assistant (off by default; user's Anthropic key; Sonnet 5.5 default, Opus 5.5 / Opus 4.5 / Haiku 5.5 selectable) | done, live API untested | Preview of the exact redacted payload, one-shot consent token, schema-constrained plan, whitelisted native actions (Settings pages, Device Manager, Disk Cleanup, Recycle Bin, temp files, polite program close, sfc via UAC) re-validated before each step, audit log (migration v3). Verified end to end against a local mock server (see docs/assistant.md); a real request needs the user's key |
| UI/UX redesign and motion system | done | Design system in `docs/design-system.md` (graphite neutrals, one indigo accent, AA contrast in both themes); Motion for React 14.5.0 (pinned) with central durations/easings/springs; animated sidebar (Ctrl+B, Ctrl+1…8), page cross-fade, sliding segmented thumbs, switches, tooltips, toasts, skeletons, dialog enter/exit; every page restyled. Release build, Overview idle 60 s: 0.08 % machine CPU (v0.1.0: 0.08 %), WebView2 382 MiB (v0.1.0: 370 MiB); JS bundle 122 → 175 kB gzip |

## Open source and languages (2026-11-04/05)

| Item | Status | Notes |
|---|---|---|
| MIT license, GitHub Actions CI (`npm run check` on Windows), CONTRIBUTING.md, personal names removed from test fixtures | done | Branch `chore/open-source` |
| Language setting: Sistema / Italiano / English ("Sistema" follows the Windows display language: Italian for Italian, English otherwise) | done | `LanguagePreference` in `AppSettings`; `get_language` keeps frontend and backend in agreement |
| Interface in Italian and English | done | Dependency-free `src/i18n`, one namespace per feature, Italian is the typed reference; key and placeholder parity checked by tsc and a unit test; switching language is immediate |
| Backend texts in Italian and English | done | Diagnostics (findings, plain text, coverage, comparisons, HTML/JSON reports with `<html lang>` and locale dates), benchmarks (labels, phases, notes, warnings), drivers, remediation outcomes, assistant prompt and plan notes. Italian output is byte-for-byte unchanged (verified by diffing reports). Technical `AppError` messages are English only; the UI titles them per `ErrorCode` |
| Stored data keeps its language | by design | Benchmark results and the latest diagnosis are not re-translated; re-running a diagnosis produces it in the current language. Comparing an Italian and an English run may list the same warning as a context difference |
| Verified in the real app (release build) | done | Every page in English and Italian, a new diagnosis in each language, language switch back to "Sistema". Checks: Rust 461 passed / 6 ignored, web 282 passed, build OK |

## Dashboard redesign (2026-11-06)

| Item | Status | Notes |
|---|---|---|
| Monitoring-dashboard style (neutral greys, monochrome gauges/bars/charts, flat panels, big light numbers) | done | Brief and rules in `docs/ui-dashboard-brief.md`; `RingGauge`, `Readout` primitives |
| No redundancy: one home per value, counts and explanations once, no page subtitles or live badges, sources only in Advanced, one global Simple/Advanced control | done | Overview is the one-screen dashboard; Hardware is specs only; Diagnostics/Drivers/Processes/Benchmarks/History/Settings trimmed; repeated coverage note and duplicate "problem code" evidence removed in Rust |

## macOS port (2026-11-11)

Target: Apple Silicon (arm64), macOS 13.3 or later. Development and test machine: Apple M5,
10 cores, 16 GB, macOS 27.2.1. Rules are the same as on Windows: real data only, no admin,
no privileged helper or kernel extension, native APIs (sysctl, Mach, I/O Kit, libproc), no
shelling out to `powermetrics`, `system_profiler`, `ioreg` and similar tools. A value macOS
does not expose stays unavailable with the reason; a Windows-specific field is not reused
for a macOS value with a different meaning.

| Item | Status | Notes |
|---|---|---|
| Workspace builds and the app launches on macOS | done | Windows-only providers return `notSupported`; providers are being ported (rows below) |
| Bundle: `.app` + `.dmg` (`src-tauri/tauri.macos.conf.json`, Windows config unchanged) | done | `npx tauri build`, release with LTO: 2 min 58 s from a cold release cache with 4 jobs. `PC Diagnostics.app` 12.19 MiB, binary 12 697 968 bytes (arm64 only), `PC Diagnostics_0.1.0_aarch64.dmg` 5 836 064 bytes. DMG mounts with the app and an Applications link |
| Signing | done (ad-hoc) | `signingIdentity: "-"`, hardened runtime, no entitlements. `codesign --verify --deep --strict` passes, also on the copy inside the DMG. Builds without an Apple account |
| `LSMinimumSystemVersion` 13.3 | done | Tailwind CSS 4 needs Safari 16.4 features (cascade layers, `@property`, `color-mix()`); macOS 13.3 is the first release that ships that WebKit. The binary's `minos` is 13.3 (Tauri sets `MACOSX_DEPLOYMENT_TARGET`). Only macOS 27.2.1 was actually tested |
| Info.plist additions | done | `NSDocumentsFolderUsageDescription` (report export writes to `~/Documents/PC Diagnostics`, which TCC protects), English and Italian `InfoPlist.strings`. The TCC prompt itself was not exercised (no export was run) |
| CSP and IPC on WKWebView (`tauri://localhost`, `ipc://localhost`) | done | No CSP change needed: `connect-src ipc: http://ipc.localhost` covers the macOS custom protocol. Verified on the release `.app` started with `open -n`: the Overview showed real CPU load, RAM 8.9 of 16.0 GB, volumes and the process list, and values changed between two window captures about 25 s apart (commands and live events both work); the log shows `hardware inventory collected`, which only a command from the UI triggers at startup. Process alive 41 s, then stopped by its PID |
| Data location | done | `~/Library/Application Support/com.pcdiagnostics.desktop/pc-diagnostics.sqlite3` (opened by the release build, `database ready schema=3`) |
| Gatekeeper instructions for the unsigned build | done | README "macOS: opening an unsigned app" (Open Anyway on macOS 15+, Control-click Open on 13/14). Not tested with a quarantined download |
| CI: `check-macos` job on `macos-latest` running `npm run check` | written, not run | YAML parsed locally (Ruby Psych); GitHub Actions cannot run here. `npm run check` passes locally on macOS (2026-11-12) |
| Release: macOS `.dmg` + SHA-256 added to the draft release | written, not run | Parallel `macos` job, then `macos-release-asset` uploads to the draft the Windows job creates |
| Process enumeration and categories | done | sysinfo (`proc_listallpids` + `proc_pidinfo`); system accounts (uid < 500) count as services. `private_bytes` is the physical footprint (`proc_pid_rusage` `ri_phys_footprint`, Activity Monitor's "Memory"), shown as "Impronta in memoria"; other users' processes are refused by the OS (on the M5: 516 readable, 221 refused, mostly root daemons) and shown as unavailable. About 2.5 µs per process per scan |
| CPU live and inventory | done | Usage via sysinfo; max 4464 MHz on the M5 from the performance-cluster DVFS table in the I/O Kit device tree (same value as sysinfo). Current/limit clock NotSupported, power PermissionDenied (needs root). Temperature: hottest SoC die sensor (`PMU tdie*`) via I/O Kit HID; CPU, GPU and NPU share that die |
| Memory live | done | Used = Activity Monitor "Memory Used", available = total − used. Paging = `vm_statistics64.pageins`/s (Mach); the high-paging rule uses 250 pages/s on macOS (16 KiB pages, same ≈ 4 MiB/s as 1000 pages/s on Windows). Commit used/limit NotSupported |
| GPU live and inventory | done | IOAccelerator `PerformanceStatistics` utilization; ids `ioreg:<registry id>` shared by inventory, live samples and the GPU benchmark; vendor Apple (0x106B, new `GpuVendor::Apple`); Metal working-set size as shared memory; dedicated VRAM 0 (unified memory). GPU temperature NotSupported (no sensor on the M5) |
| Storage live and inventory | done | IOBlockStorageDriver `Statistics` bytes/s (disk images excluded, list cached, rescanned every 30 ticks). Active time NotSupported (Total Time is a sum of service times, not a busy share). Volumes from sysinfo without `/System/Volumes/*`. Memory modules NotSupported. SMART/NVMe health NotImplemented |
| App overhead | done | Core process only; WKWebView helpers are launchd XPC services (ppid 1) and cannot be attributed without private API, so the interface row is NotSupported |
| `sample()` cost | measured | ~21 ms wall time per tick (24 serialized HID sensor reads, mostly waiting); ~1.6 ms CPU per second for the whole monitor |
| Benchmarks | done | CPU/RAM portable (single 21 277, multi 141 623 units/s; RAM read 128 GB/s on the M5). Storage: temp file unlinked right after open, `F_NOCACHE`, `F_FULLFSYNC`, same write budget and free-space reserve (seq write 4.64 / read 6.54 GB/s on the internal SSD). GPU via Metal: 3324 GFLOP/s FP32, cancel in ~60 ms; worker exits on parent death (no job objects on macOS). No GPU temperature: the 30 s unobserved cap applies |
| Display language, Keychain, assistant | done | Language from `CFLocaleCopyPreferredLanguages`; assistant key in the login Keychain (round-trip test passes without prompts); assistant proposes only macOS-safe actions (fixed System Settings panes, own `$TMPDIR` cleanup); Windows prompt and schema byte-for-byte unchanged |
| Diagnostics texts | done | Platform-aware wording where a rule can fire on macOS; Windows output unchanged (20 rendered reports compared). A full diagnosis completes on macOS with Drivers "not evaluated" |
| UI | done | Platform loaded at startup; Drivers page and its links hidden on macOS; ⌘ shortcuts (⌘1…7); macOS texts (Keychain, no Windows names); unified-memory GPU shown in words; Windows disk-status block hidden; build target `chrome120` + `safari16.4` |
| Swap usage and memory pressure | done | `MemorySample.swap_used_bytes` / `swap_total_bytes` (`vm.swapusage`) and `pressure` (`kern.memorystatus_vm_pressure_level`: normal / warning / critical), both read without root; `NotSupported` on Windows. RAM panel on macOS: "Pressione memoria" in both views (the status word "Avviso" / "Critico" above normal), "Swap in uso" in Advanced instead of the commit row. New rule `memory.pressure.high`; it also corroborates `memory.paging.high`. On the M5: swap 1.0 of 2.0 GiB, pressure normal |
| GPU memory fields as "unavailable" | done | `GpuInfo.device_id`, `dedicated_vram_bytes`, `shared_memory_bytes` are Metrics. Apple silicon: device id and dedicated VRAM `NotSupported` (the UI says "Nessuna: memoria unificata" from that, no name guessing); shared memory is Metal `recommendedMaxWorkingSetSize` (new source `Metal`). Windows: DXGI values as before. `gpu.vram.high` separates "unified memory" from "size unknown" |
| Storage sizes as in Finder | done | "Semplice" on macOS shows disk, volume and file sizes in powers of 1000 (`formatStorageBytes`, `fmt::storage_plain`): the internal SSD reads 994,6 GB like Finder instead of 926,3 GB. Memory stays binary like Activity Monitor; Windows and "Avanzata" unchanged |
| Windows build of the port changes | type-checked | `cargo clippy --target x86_64-pc-windows-msvc -D warnings` passes for models, platform, diagnostics, monitor, bench, processes, drivers (rust-std for the target installed with rustup). src-tauri, persistence and assistant cannot be cross-checked (C build scripts of aws-lc-sys and libsqlite3-sys). Tests not run on Windows: the CI `check` job on `windows-latest` does that on the pull request |
| Drivers page and Windows Update driver check | not applicable | No comparable user-visible driver inventory on macOS; drivers ship with the OS |
| Developer ID signing and notarization | not done | Needs a paid Apple Developer account; until then Gatekeeper blocks the first launch of a downloaded copy |
| Intel Macs (x86_64) / universal binary | not done | Not built or tested; would need `--target universal-apple-darwin` and an Intel test machine |

## Out of scope for v1 (by design)

Driver installation/update/removal, process kill/suspend, registry changes, BIOS,
overclocking or undervolting, telemetry, automatic report upload, a global health score.

## Known gaps

- CPU temperature: not observable without a kernel driver; this desktop exposes no ACPI
  thermal zone either (PDH `Thermal Zone Information` and `MSAcpi_ThermalZoneTemperature`
  are absent). Shown as unavailable.
- AMD/Intel GPU power is a share of the maximum, not watts (watts would need ADLX/IGCL).
- Intel GPUs: untested (no Intel GPU on the development machine).
- `Source` has no Win32 variant: the only candidate values (commit charge) come from PDH,
  which is what the UI labels them with, so none is needed.
- `Unavailability` has no free-text variant on purpose: every gap must fall into an
  explained category (`ProviderMissing` and `Failed` already carry details).
