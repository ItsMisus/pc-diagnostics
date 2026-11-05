# "Risolvi" assistant

A "Risolvi" button on a diagnostic problem asks an AI model (Anthropic Claude, through
the user's own API key) for a fix plan. The app then performs only a fixed set of
natively implemented actions, one step at a time, each after its own confirmation.

This is the only feature that sends data off the machine on the user's request, so it is
off by default and built around three rules:

1. **Nothing is sent without consent.** The user sees the exact payload, the destination
   and a cost estimate first (`preview_fix`). `request_fix` accepts only the one-shot
   token of that preview and sends exactly that payload.
2. **The model never runs anything.** Its answer is data, constrained by a JSON schema to
   the `RemediationAction` variants in `crates/models/src/assistant.rs`. The app
   re-validates every action against the current state of the PC (`platform::remediation::validate`)
   and turns anything it rejects into manual instructions, with the reason.
3. **Every action needs its own confirmation.** The UI describes exactly what will
   happen; the backend refuses to run a step twice or a step that was not validated.

## Allowed actions

| Action | What it does | Reversible | Admin |
|---|---|---|---|
| `openSettingsPage` | Opens a fixed `ms-settings:` page (Windows Update, optional updates, Storage Sense, display, sound, Bluetooth, network, power, troubleshooters, startup apps, installed apps, privacy) | yes (opens only) | no |
| `openDeviceManager` | Opens Device Manager | yes (opens only) | no |
| `openDiskCleanup` | Starts Windows Disk Cleanup for a fixed drive; the user picks what to delete | the tool asks | no |
| `emptyRecycleBin` | Empties the Recycle Bin with Windows' own confirmation | **no** | no |
| `cleanTempFiles` | Deletes files older than N days from the user's temp folder, skipping files in use, never following links | **no** | no |
| `closeProgram` | Sends a close request to a program's windows (like clicking X), waits up to 10 s; never force-kills, never system/protected/other-user processes or PC Diagnostics | yes (reopen it) | no |
| `runSystemFileCheck` | Runs `sfc /scannow` in a visible window | repairs system files | yes, through Windows' UAC prompt |

Still never done, with or without the assistant: registry edits, driver installation,
update or removal, BIOS changes, overclocking/undervolting, disabling Windows or security
components, force-killing processes, running arbitrary commands, downloading tools.

## Data sent

The finding (rule id, title, description, evidence, plain-language text), the component,
its coverage note, the OS name/version and relevant hardware model names. Redaction
removes the host name, the user name and file paths before the preview is built, so
the preview is exactly what leaves the machine. No API key is ever shown to the UI
after it is saved: it lives in Windows Credential Manager.

Language: the preview (destination text, the payload's privacy note) and the plan follow
the app language effective when the preview is built (Italian or English). The system
prompt has one version per language, identical except that it asks Claude to write every
user-visible field of the plan (summary, step titles, explanations, manual instructions,
warnings) in that language; each version is a constant, so prompt caching still works.
The notes the app adds to a plan (forced safety flags, actions turned into manual steps)
and the step outcome messages use the same language. The redaction placeholders
(`<utente>`, `<host>`, `<percorso>`, `<id dispositivo>`) are the same in every language.
Technical error messages are English; the UI translates the title per error code.

Destination: `https://api.anthropic.com/v1/messages`, model chosen in Settings:
`claude-sonnet-5-5` by default (the user's choice: good judgment and clear plain-language
explanations at about a fifth of Opus 4.5's price), `claude-opus-5-5` for hard cases,
`claude-opus-4-5`, and `claude-haiku-5-5` for the cheapest runs. Costs are charged to the
user's Anthropic account; the preview shows an estimate and the plan shows the real
token usage.

## Audit log

Every preview, request, plan, executed step and error is recorded locally in
`assistant_events` (SQLite, migration v3): time, rule id, model, bytes sent, token usage,
step id and outcome. The payload content and the API key are not stored.

## Testing without an API key

Debug builds only (`npx tauri build --debug`, `npm run tauri dev`): if
`PCDIAG_ASSISTANT_BASE_URL` is set to `http://127.0.0.1:<port>`, requests go to that local
server instead of Anthropic. Release builds do not contain this override (the code is
compiled out) and always use `https://api.anthropic.com`.

Verified on 2026-11-04 with a debug build and a local mock server:

- Settings: enable switch, key saved to Credential Manager and removed again.
- Preview: exact payload shown, with destination, model and cost estimate.
- Request: the captured body had no user name, host name, user paths or key. The headers
  were `anthropic-version: 2026-10-21` and `anthropic-beta: server-side-fallback-2026-07-01`,
  and the body had `fallbacks: "default"`, `output_config.format` json_schema and no
  sampling parameters.
- Plan: rendered with its steps. A `closeProgram` on PID 4 was turned into a manual step
  with the validator's reason.
- Step: `openSettingsPage storageSense` ran after the two-step confirmation and opened the
  page.
- Audit log: preview, request, plan and step rows, with no payload and no key.

Not yet verified: a real request to Anthropic. That needs the user's API key.
