# PC Diagnostics

[![CI](https://github.com/ItsMisus/pc-diagnostics/actions/workflows/ci.yml/badge.svg)](https://github.com/ItsMisus/pc-diagnostics/actions/workflows/ci.yml)

A desktop app for Windows 11 that monitors hardware, runs benchmarks and produces
diagnostics. It shows only real data: any value it cannot read is labelled as
unavailable, with the reason. It works offline, keeps all data local and needs no
admin rights for normal use.

Optional, off by default: the **Risolvi** assistant asks Anthropic's Claude (with your
own API key) for a fix plan for one diagnostic problem. Nothing is sent before you see
the exact data and accept; the app runs only a small set of built-in actions, one step
at a time, each after its own confirmation. See [docs/assistant.md](docs/assistant.md).

Languages: Italian and English (follows Windows by default; switch in Settings).
Translations are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md#translations).

Stack: Tauri 2 · Rust · React 19 · TypeScript · Vite · Tailwind CSS 4 · SQLite.

Current status: **Phases 1–7 complete** (v0.1.0). See [docs/roadmap.md](docs/roadmap.md) for what
is done, partial or out of scope, and [docs/architecture.md](docs/architecture.md).

## Download

Installers are on the [Releases page](https://github.com/ItsMisus/pc-diagnostics/releases) (`PC Diagnostics_<version>_x64-setup.exe`, with its
SHA-256). The installer is per user and needs no admin rights. It is not signed yet, so
Windows SmartScreen may show "Windows protected your PC": choose **More info → Run anyway**
only if the SHA-256 matches the one on the release page.

## Requirements

- Windows 11 x64 (tested). Windows 10 x64 should work but is untested: reports welcome.
- WebView2 (preinstalled on Windows 11; the installer downloads it if missing)
- Rust stable (MSVC toolchain) and Visual Studio Build Tools
- Node.js 22+

## Commands

```sh
npm install
npm run tauri dev      # run the desktop app
npm run gen:types      # regenerate src/types/generated from Rust models
npm run check          # fmt, clippy, cargo test, typecheck, vitest, vite build
npx tauri build        # release build + NSIS installer
```

The installer is written to `target/release/bundle/nsis/PC Diagnostics_<version>_x64-setup.exe`.
It installs per user (no admin) into `%LOCALAPPDATA%\PC Diagnostics`; `/S` runs it
silently. Data lives in `%APPDATA%\com.pcdiagnostics.desktop\pc-diagnostics.sqlite3`;
exported reports go to `Documents\PC Diagnostics`.

Code signing is opt-in: set `PCDIAG_SIGN_THUMBPRINT` to the SHA-1 thumbprint of a
code-signing certificate in `Cert:\CurrentUser\My` before `npx tauri build`, and
`scripts/sign.ps1` signs the app, the uninstaller and the installer with signtool
(optional: `PCDIAG_TIMESTAMP_URL`, `PCDIAG_SIGNTOOL`). Without it the build is unsigned
and SmartScreen may warn on first run.

## Layout

```
crates/models        shared contracts (exported to TypeScript)
crates/platform      OS access layer (PDH, WMI, DXGI, NVML, SetupAPI, WUA), key store, remediation actions
crates/monitor       live sampler (CPU, GPU, RAM, disks)
crates/processes     process snapshots and Impact Score
crates/drivers       read-only driver inventory and update check
crates/bench         benchmark engines and safety controller
crates/diagnostics   deterministic rules, comparisons, report export
crates/persistence   SQLite storage
crates/assistant     Risolvi assistant: redacted payload, Claude client, plan validation
src-tauri            Tauri app shell (commands, capabilities)
src                  React UI
docs                 architecture, roadmap, methodology
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Hardware reports from PCs we have not tested are
especially welcome (there is an issue template for them). Security problems: see
[SECURITY.md](SECURITY.md).

Releases: push a tag `v<version>` matching `src-tauri/tauri.conf.json`; the Release
workflow runs the checks, builds the installer and attaches it to a draft release.

## License

[MIT](LICENSE).
