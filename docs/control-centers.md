# Control centers (Performance, Cooling, Lighting, Gaming)

Four sections added on 2026-11-13 next to the monitoring tools. They *change* things (on
request only), where the rest of the app observes. Sidebar order: Overview, then the four
centers, then a divider and the monitoring sections.

## Rules (from the project spec, binding)

- Real controls only. A control the OS does not let the app change without admin rights, a
  driver, a firmware/BIOS change or a registry write is `ReadOnly` or `Unsupported`
  (`models::control::Support`) with a reason in the user's language, and the UI says so
  (`SupportBadge`, `SupportReason`). Never simulate an effect: a disabled control looks
  disabled, and a preview is labelled as a preview.
- Nothing changes without a user action. Startup never applies a profile, except one the
  user explicitly left active (and even then only through the documented API of the OS).
- Before the first change a controller stores the original system state
  (`<center>.original` document); "Ripristina" puts it back, also after a restart.
- No admin, no elevation prompt, no helper tool, no kernel extension, no private
  framework unless it needs no entitlement and degrades to `Unsupported` when missing.
- Offline, no telemetry, nothing sent anywhere. Game covers come from local launcher caches.
- One center failing (an API missing, a device unplugged) never breaks the others: errors
  stay inside the center and become an `Unsupported` support or an `AppError`.
- Windows and macOS share the UI; platform code lives behind `cfg` with a portable fallback.

## Code layout (one owner per path)

| Path | What |
| --- | --- |
| `crates/models/src/control.rs` | Shared vocabulary: `Support`, `SupportState`, `ProfileRef`, `ControlDomainId`, `PowerSupply`, `validate_profile_name`. |
| `crates/models/src/control/<center>.rs` | IPC contracts of one center (ts-rs exports to `src/types/generated`). Prefix type names with the center (`PerformanceProfile`, `CoolingFan`, `LightingDevice`, `GameEntry`). |
| `crates/platform/src/control/<center>.rs` (+ `<center>/`) | OS access of one center. |
| `crates/control/src/lib.rs` | `DocumentStore`, `load_json` / `save_json`, `ProfileDomain` (the contract Gaming uses), `MemoryStore` for tests. |
| `crates/control/src/<center>.rs` (+ `<center>/`) | Controller: profiles, selection, original state, background work. |
| `crates/persistence` v4 | `control_documents(key, json, updated_at)`; keys `^[A-Za-z0-9._-]{1,64}$`, prefixed by the center name. |
| `src-tauri/src/control_store.rs` | `DbDocumentStore`: `DocumentStore` on its own connection to the app database. |
| `src-tauri/src/commands/<center>.rs` | Tauri commands of one center (thin: validate, call the controller on `blocking`). Each controller is Tauri managed state of its own (`app.manage(Arc<...>)`). |
| `src/features/<center>/` | Page (`<Center>Page.tsx`), store, `api.ts` (the center's `invoke` wrappers via `services/ipc.ts` `call`), 3D scenes. |
| `src/i18n/locales/{it,en}/<center>.ts` | Texts. |
| `src/components/control/` | Shared UI: `ProfileSwitcher` (profile tiles), `SupportBadge` / `SupportReason`. |
| `src/components/stage3d/ObjectStage.tsx` | Shared 3D stage for one object (lazy three.js chunk, studio lighting of the Overview, limited orbit, pause when hidden, reduced motion, fallback). Scenes are modules with a default export receiving `StageSceneProps` + their own props. |

## Game overrides (`control::ProfileDomain`)

Performance, Cooling and Lighting each implement `ProfileDomain`. A center keeps two layers:
the user's own selection and an optional game override. Gaming calls
`apply_game_override(profile_id)` when a game with a linked profile starts (or the user
starts it with its profile) and `clear_game_override()` when it ends; clearing brings back the
user's selection, not the original system state. Gaming receives the three domains as
`Vec<SharedDomain>` when the app starts (`src-tauri/src/lib.rs`).

## What each OS allows (research of 2026-11-13; update when you learn more)

### Performance
- macOS: Low Power Mode and High Power Mode (`pmset`, `IOPMSetPMPreferences`) need root:
  read them (per source: `IOPMCopyPMPreferences` / `NSProcessInfo.isLowPowerModeEnabled`)
  and send the user to System Settings > Battery (or Energy). The app can really control:
  keep-awake assertions (`IOPMAssertionCreateWithName`, system or display), no admin.
- Windows: power schemes (`PowerEnumerate`, `PowerGetActiveScheme`, `PowerSetActiveScheme`),
  the Windows 11 power mode overlay (`PowerGetEffectiveOverlayScheme` /
  `PowerSetActiveOverlayScheme`, powrprof), per-source values of the active scheme
  (`PowerWriteACValueIndex` / `PowerWriteDCValueIndex`: processor min/max state, boost mode,
  cooling policy, display/sleep timeouts), keep-awake (`SetThreadExecutionState`). Standard
  users may change these unless a policy forbids it; a failure is reported, not hidden.

### Cooling
- macOS: fans through the SMC user client (`FNum`, `F<n>Ac/Mn/Mx/Tg/Md`), read only for
  normal users; writes need root. MacBook Air has no fan (`FNum` = 0): passive cooling.
- Windows: no general fan API without vendor drivers. The documented power setting
  "System cooling policy" (active / passive, per source) is a real cooling control.

### Lighting
- Standard devices: HID LampArray (usage page 0x59). Windows: `Windows.Devices.Lights`
  (`LampArray`, Dynamic Lighting). macOS: IOHIDManager on usage page 0x59.
- Mac keyboard backlight: CoreBrightness `KeyboardBrightnessClient` (no entitlement; treat
  as optional, `Unsupported` if the class or the backlight is missing).
- Vendor RGB protocols (proprietary USB reports) are not implemented: such devices are
  listed as recognised but not controllable.

### Gaming
- Steam: `libraryfolders.vdf` + `appmanifest_*.acf`; covers in `appcache/librarycache`.
  Epic: `.item` manifests in `EpicGamesLauncher/Data/Manifests`. macOS apps with
  `LSApplicationCategoryType` `public.app-category.*-games` / `games`. Manual entries.
- Launch through the launcher's URL (`steam://rungameid/<id>`,
  `com.epicgames.launcher://apps/<name>?action=launch&silent=true`) or the app bundle /
  executable, only for entries of the library (the frontend passes an id, never a path).
