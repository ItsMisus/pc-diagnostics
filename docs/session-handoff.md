# Session handoff (2026-11-05)

Read this first in a new session. User is Italian: reply in Italian. Standing consent:
never ask before acting. Use worktree subagents with strict file ownership; the
orchestrator owns `crates/models`, `src-tauri/src/commands` (shared wiring), `src/app`,
`src/components` and `src/i18n/index.ts`.

## Direction

PC Diagnostics is an open-source app (MIT) for everyone, on any Windows PC. The "Risolvi"
assistant (Anthropic API) is optional and set aside: keep it working, don't push it, don't
list a live API test as a next step.

## State of the project

- Public repository: https://github.com/ItsMisus/pc-diagnostics (published 2026-11-05).
  `master` there starts from a single commit "PC Diagnostics 0.1.0" (fresh history, so the
  author's personal e-mail and old fixtures are not public). The full development history
  stays only in the local branch `archive/full-history`: never push it.
- This clone has a repo-local `user.email` set to the GitHub noreply address; keep commits
  on that identity.
- Installed release: `%LOCALAPPDATA%\PC Diagnostics` (built 2026-11-05 from the i18n work,
  installed silently with `/S`; unsigned).
- Full check: `npm run check`, last result Rust 461 passed / 6 ignored, web 282 passed,
  build OK.
- Details of every phase: `docs/roadmap.md`.

## Languages (done 2026-11-05)

- Setting "Lingua": Sistema / Italiano / English. "Sistema" = Windows display language
  (Italian for Italian, English for everything else).
- Frontend: `src/i18n` (no dependency). Texts in `src/i18n/locales/<lang>/<namespace>.ts`;
  Italian is the reference, English must have the same keys and placeholders (tsc + unit
  test). `useT(ns)`, `t.plural`, `t.rich`, `tr`, `translatorFor`, `localeTag()`. Never
  build translated strings at module scope.
- Backend: `Language::pick(it, en)` next to each other in the Rust code. The language is read
  from `AppState::language()` when text is produced (diagnosis run, benchmark start,
  capabilities, driver inventory, assistant preview). Technical `AppError` messages are
  English only.
- Stored data (benchmark results, latest diagnosis) keeps the language it was made in.
- English name of the assistant: "Fix it" (Italian keeps "Risolvi").
- How to add a language: CONTRIBUTING.md "Translations".

## GitHub readiness (done 2026-11-05)

- `.github/workflows/ci.yml` (checks on push/PR), `.github/workflows/release.yml` (on tag
  `v<version>`: checks, installer, SHA-256, draft release), issue templates (bug, hardware
  report), PR template, `SECURITY.md` (private vulnerability reporting), README "Download".
- Secondary pages are lazy-loaded and preloaded when idle (main chunk 665 → 405 kB).
- Private vulnerability reporting is enabled on the repository; topics and a `hardware`
  label are set.

## Open items

- First release: push tag `v0.1.0` to get a draft release with the installer; review and
  publish it on GitHub.
- Publicly trusted code signing (needs a certificate or Azure Trusted Signing); until then
  SmartScreen may warn.
- Testing on other machines: Windows 10, Intel GPUs, laptops on battery, older WebView2.
- Redesign items not exercised live: the running-benchmark strip and completion toast,
  reduced-motion behavior with Windows animations off.

## Tooling notes

- Before UI automation, check that the user is not using a full-screen app or game
  (2026-11-05: League of Legends was running). `shot.ps1` steals the foreground and clicks
  at screen positions; if the window is not laid out yet the click lands elsewhere.
- UI automation: `scripts/dev/shot.ps1` (screenshot + click at window-relative coordinates;
  sets the window topmost). Synthetic mouse wheel does not scroll WebView2; PageDown via
  keybd_event does. Clicks can hit controls that persist settings: re-check language, theme,
  view mode and sidebar after a session.
- Subagents running cargo in parallel: give each its own `CARGO_TARGET_DIR` and delete it
  afterwards (each is ~7.5 GB). After merging branches built in other target dirs, cargo
  may reuse a stale build of a path crate: `touch` the sources if the compiler reports
  fields or functions that are present in the code.
- Subagent worktrees with a `node_modules` junction: remove the junction with
  `(Get-Item node_modules -Force).Delete()` before `git worktree remove`.
- Don't edit files with PowerShell Set-Content (corrupts UTF-8); avoid bash heredocs for
  code (they turned `\b` into a backspace once).

## Hard project rules (from the original spec)

- Real data only, never fabricated.
- No automatic benchmarks; one heavy benchmark at a time; Stop always available.
- No admin for normal use.
- Offline and local; no telemetry.
- No registry, driver, BIOS or overclock changes.
- No arbitrary shell commands from the frontend; minimal Tauri capabilities.
- No global health score.
- Don't claim tests passed without running them.
