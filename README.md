# PC Diagnostics

[![CI](https://github.com/ItsMisus/pc-diagnostics/actions/workflows/ci.yml/badge.svg)](https://github.com/ItsMisus/pc-diagnostics/actions/workflows/ci.yml)

A desktop app for Windows 11 and macOS (Apple Silicon) that monitors hardware, runs
benchmarks and produces diagnostics. It shows only real data: any value it cannot read is
labelled as unavailable, with the reason. It works offline, keeps all data local and needs
no admin rights for normal use.

Windows is the original platform. The macOS port is in progress: some values Windows
provides do not exist on macOS, and they stay unavailable with the reason. See
[macOS: what is different](#macos-what-is-different) and the "macOS port" table in
[docs/roadmap.md](docs/roadmap.md).

Optional, off by default: the **Risolvi** assistant asks Anthropic's Claude (with your
own API key) for a fix plan for one diagnostic problem. Nothing is sent before you see
the exact data and accept; the app runs only a small set of built-in actions, one step
at a time, each after its own confirmation. See [docs/assistant.md](docs/assistant.md).

Languages: Italian and English (follows the system language by default; switch in
Settings). Translations are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md#translations).

Stack: Tauri 2 · Rust · React 19 · TypeScript · Vite · Tailwind CSS 4 · SQLite.

Current status: **Phases 1–7 complete on Windows** (v0.1.0); macOS port in progress. See
[docs/roadmap.md](docs/roadmap.md) for what is done, partial or out of scope, and
[docs/architecture.md](docs/architecture.md).

## Download

Builds are on the [Releases page](https://github.com/ItsMisus/pc-diagnostics/releases),
each with its SHA-256:

- **Windows**: `PC Diagnostics_<version>_x64-setup.exe`. The installer is per user and
  needs no admin rights. It is not signed yet, so Windows SmartScreen may show "Windows
  protected your PC": choose **More info → Run anyway** only if the SHA-256 matches the
  one on the release page.
- **macOS** (Apple Silicon): `PC Diagnostics_<version>_aarch64.dmg`. Open it and drag
  **PC Diagnostics** to **Applications**. The app is not notarized yet: see below.

### macOS: opening an unsigned app

The app is ad-hoc signed but not signed with an Apple Developer ID or notarized (that needs
a paid Apple Developer account). Gatekeeper therefore blocks the first launch of a
downloaded copy. First check the download:

```sh
shasum -a 256 ~/Downloads/"PC Diagnostics_<version>_aarch64.dmg"
```

and compare it with the `.sha256` file on the release page. Then open the app once in one
of these ways (you do not need to change any security setting, and Gatekeeper stays on for
every other app):

- **macOS 15 Sequoia and later**: double-click the app. When macOS says it cannot verify
  the developer, choose **Done**. Open **System Settings → Privacy & Security**, scroll to
  **Security**, click **Open Anyway** next to the "PC Diagnostics was blocked" message and
  confirm with your password or Touch ID.
- **macOS 13 Ventura and 14 Sonoma**: in Finder, Control-click (or right-click) the app in
  Applications, choose **Open**, then **Open** again in the dialog. The **Open Anyway**
  button in System Settings → Privacy & Security works here too.

macOS remembers the choice; later launches open normally. A copy you build yourself is not
quarantined and opens directly.

## Requirements

Windows:

- Windows 11 x64 (tested). Windows 10 x64 should work but is untested: reports welcome.
- WebView2 (preinstalled on Windows 11; the installer downloads it if missing)
- To build: Rust stable (MSVC toolchain), Visual Studio Build Tools, Node.js 22+

macOS:

- An Apple Silicon Mac (M1 or later) with macOS 13.3 Ventura or later. Tested on an Apple
  M5 with macOS 27.2. Intel Macs are not built or tested.
- No extra runtime: the app uses the system WebKit (WKWebView).
- To build: Xcode Command Line Tools (`xcode-select --install`; the full Xcode is not
  needed), Rust stable (`rustup`, target `aarch64-apple-darwin`), Node.js 22+

macOS 13.3 is the minimum because the interface uses CSS that Tailwind CSS 4 needs
(cascade layers, `@property`, `color-mix()`), which the system WebKit has from Safari 16.4,
the version that ships with macOS 13.3.

## Commands

```sh
npm install
npm run tauri dev      # run the desktop app
npm run gen:types      # regenerate src/types/generated from Rust models
npm run check          # fmt, clippy, cargo test, typecheck, vitest, vite build
npx tauri build        # release build + installer (Windows) or .app + .dmg (macOS)
```

The same commands work on both systems; `npx tauri build` picks the bundle for the system
it runs on (`src-tauri/tauri.macos.conf.json` holds the macOS overrides).

### Windows build

The installer is written to `target/release/bundle/nsis/PC Diagnostics_<version>_x64-setup.exe`.
It installs per user (no admin) into `%LOCALAPPDATA%\PC Diagnostics`; `/S` runs it
silently.

Code signing is opt-in: set `PCDIAG_SIGN_THUMBPRINT` to the SHA-1 thumbprint of a
code-signing certificate in `Cert:\CurrentUser\My` before `npx tauri build`, and
`scripts/sign.ps1` signs the app, the uninstaller and the installer with signtool
(optional: `PCDIAG_TIMESTAMP_URL`, `PCDIAG_SIGNTOOL`). Without it the build is unsigned
and SmartScreen may warn on first run.

### macOS build

`npx tauri build` writes:

- `target/release/bundle/macos/PC Diagnostics.app`
- `target/release/bundle/dmg/PC Diagnostics_<version>_aarch64.dmg`

The app is ad-hoc signed (`signingIdentity: "-"`) with the hardened runtime and no
entitlements; no Apple account is needed. While the disk image is made, Tauri's DMG script
uses AppleScript to arrange the icons in the image window, so macOS may ask to let your
terminal control Finder; with `CI=true` that step is skipped (the image then opens with
Finder's default layout). `scripts/macos/check-bundle.sh` checks a built bundle
(signature, minimum macOS version, architecture). To uninstall, move the app to the Trash;
your data stays in the folder below until you delete it.

## Where your data lives

| | Windows | macOS |
|---|---|---|
| Database (settings, benchmark history, latest diagnosis) | `%APPDATA%\com.pcdiagnostics.desktop\pc-diagnostics.sqlite3` | `~/Library/Application Support/com.pcdiagnostics.desktop/pc-diagnostics.sqlite3` |
| Exported reports | `Documents\PC Diagnostics` | `~/Documents/PC Diagnostics` (macOS asks once for access to Documents) |
| Assistant API key | Windows Credential Manager | macOS Keychain (part of the port in progress) |
| App | `%LOCALAPPDATA%\PC Diagnostics` | wherever you put `PC Diagnostics.app`, usually `/Applications` |

Logs go to standard output only; nothing is written to a log file. To see them on macOS,
start the binary from a terminal: `"/Applications/PC Diagnostics.app/Contents/MacOS/pc-diagnostics"`.

## macOS: what is different

The macOS build reads data through native interfaces (sysctl, Mach, I/O Kit) without admin
rights, kernel extensions, privileged helpers or command-line tools such as `powermetrics`.
Some things do not exist on macOS, or only behind those barriers; they are shown as
unavailable with the reason, never estimated:

- **Drivers page and the Windows Update driver check**: Windows only. macOS has no
  user-installed driver inventory comparable to Device Manager; system drivers ship with
  macOS and are updated by Software Update.
- **Windows-specific values** (for example commit charge, PDH counters, DXGI/WDDM GPU data,
  NVIDIA NVML): these are Windows memory-manager or driver concepts. Where macOS has a value
  with a different meaning, the app does not put it in the Windows field.
- **Assistant actions** that open Windows tools (Device Manager, Disk Cleanup, `sfc`) are
  not offered on macOS.
- Sensors (temperatures, power), benchmarks and the other providers are being ported; the
  per-item status is in the "macOS port" table of [docs/roadmap.md](docs/roadmap.md).

## Layout

```
crates/models        shared contracts (exported to TypeScript)
crates/platform      OS access layer: windows/ (PDH, WMI, DXGI, NVML, SetupAPI, WUA) and
                     macos/ (sysctl, Mach, I/O Kit, libproc), key store, remediation actions
crates/monitor       live sampler (CPU, GPU, RAM, disks)
crates/processes     process snapshots and Impact Score
crates/drivers       read-only driver inventory and update check (Windows)
crates/bench         benchmark engines and safety controller
crates/diagnostics   deterministic rules, comparisons, report export
crates/persistence   SQLite storage
crates/assistant     Risolvi assistant: redacted payload, Claude client, plan validation
src-tauri            Tauri app shell (commands, capabilities, bundle config)
src                  React UI
scripts              signing hook (Windows), bundle check (macOS), dev tools
docs                 architecture, roadmap, methodology
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Hardware reports from PCs and Macs we have not
tested are especially welcome (there is an issue template for them). Security problems:
see [SECURITY.md](SECURITY.md).

Releases: push a tag `v<version>` matching `src-tauri/tauri.conf.json`; the Release
workflow runs the checks, builds the Windows installer and the macOS disk image and
attaches both, with their SHA-256, to a draft release.

## License

[MIT](LICENSE).
