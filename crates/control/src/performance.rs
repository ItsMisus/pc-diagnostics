//! Performance controller. Owned by the performance feature.
//!
//! Profiles (three built-in, the user's own), the user's selection, the Gaming override on
//! top of it, the original system state and a light watcher that re-applies the active
//! profile when the power supply changes. OS access goes through `PerformanceBackend`
//! (`SystemBackend` in the app, a fake in the tests).
//!
//! Stored documents: `performance.profiles` (user profiles), `performance.selection` (the
//! user's selection, re-applied at start: the user left it active), `performance.original`
//! (the system state before the first change; removed by a successful restore).
//!
//! Rules: only controls the OS reports `Writable` are written; a write refused with access
//! denied turns that control read-only for the rest of the session. A user's own "Applica"
//! replaces a running game override (the user's explicit choice wins). Clearing the override
//! re-applies the user's selection, or, without one, puts the original state back.

use std::collections::HashSet;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use models::control::performance::{
    PerformanceCapabilities, PerformanceControl, PerformanceGameOverride, PerformanceIssue,
    PerformanceKeepAwake, PerformanceMode, PerformanceOsReport, PerformanceOsSupply,
    PerformanceProfile, PerformanceProfileInput, PerformanceState, PerformanceSupplySettings,
};
use models::control::{ControlDomainId, PowerSupply, ProfileRef, Support, validate_profile_name};
use models::settings::Language;
use models::{AppError, ErrorCode};
use platform::control::performance::{Access, KeepAwake, OsPowerState, WriteFailure};
use serde::{Deserialize, Serialize};

use crate::{DocumentStore, ProfileDomain, load_json, save_json};

pub const PROFILES_KEY: &str = "performance.profiles";
pub const SELECTION_KEY: &str = "performance.selection";
pub const ORIGINAL_KEY: &str = "performance.original";

/// Most user profiles kept.
pub const MAX_USER_PROFILES: usize = 24;
/// How often the watcher checks the power supply.
pub const SUPPLY_POLL: Duration = Duration::from_secs(5);

pub const SILENT_ID: &str = "silent";
pub const BALANCED_ID: &str = "balanced";
pub const PERFORMANCE_ID: &str = "performance";
const USER_ID_PREFIX: &str = "custom-";

/// OS access of the performance center.
pub trait PerformanceBackend: Send + Sync {
    fn read(&self) -> OsPowerState;
    /// Supply now and whether there is a battery (cheap: polled).
    fn current_supply(&self) -> (PowerSupply, bool);
    fn write(
        &self,
        scheme_id: Option<&str>,
        current: PowerSupply,
        ac: &PerformanceOsSupply,
        battery: Option<&PerformanceOsSupply>,
    ) -> Vec<WriteFailure>;
    fn set_keep_awake(&self, mode: PerformanceKeepAwake) -> Result<(), AppError>;
    fn keep_awake(&self) -> PerformanceKeepAwake;
    fn open_settings(&self) -> Result<(), AppError>;
}

/// The real OS through `platform::control::performance`.
#[derive(Default)]
pub struct SystemBackend {
    keep_awake: Mutex<KeepAwake>,
}

impl SystemBackend {
    fn keep(&self) -> MutexGuard<'_, KeepAwake> {
        self.keep_awake
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl PerformanceBackend for SystemBackend {
    fn read(&self) -> OsPowerState {
        platform::control::performance::read_state()
    }

    fn current_supply(&self) -> (PowerSupply, bool) {
        platform::control::performance::current_supply()
    }

    fn write(
        &self,
        scheme_id: Option<&str>,
        current: PowerSupply,
        ac: &PerformanceOsSupply,
        battery: Option<&PerformanceOsSupply>,
    ) -> Vec<WriteFailure> {
        platform::control::performance::write_settings(scheme_id, current, ac, battery)
    }

    fn set_keep_awake(&self, mode: PerformanceKeepAwake) -> Result<(), AppError> {
        self.keep().set(mode)
    }

    fn keep_awake(&self) -> PerformanceKeepAwake {
        self.keep().current()
    }

    fn open_settings(&self) -> Result<(), AppError> {
        platform::control::performance::open_power_settings()
    }
}

/// A user profile as stored (built-ins are not stored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfile {
    id: String,
    name: String,
    ac: PerformanceSupplySettings,
    battery: PerformanceSupplySettings,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Selection {
    profile_id: Option<String>,
}

/// The system state before the app's first change: only values of writable controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Original {
    scheme_id: Option<String>,
    ac: PerformanceOsSupply,
    battery: Option<PerformanceOsSupply>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum IssueKind {
    Denied,
    Failed(String),
}

struct Inner {
    profiles: Vec<StoredProfile>,
    selected: Option<String>,
    game_override: Option<String>,
    original: Option<Original>,
    /// Supply the effective profile was last applied for.
    applied_supply: Option<PowerSupply>,
    issues: Vec<(PerformanceControl, IssueKind)>,
    /// Controls whose writes the OS refused this session.
    denied: HashSet<PerformanceControl>,
}

struct Watcher {
    stop: mpsc::Sender<()>,
    handle: JoinHandle<()>,
}

pub struct PerformanceController {
    backend: Arc<dyn PerformanceBackend>,
    store: Arc<dyn DocumentStore>,
    inner: Mutex<Inner>,
    watcher: Mutex<Option<Watcher>>,
}

fn builtin_profiles(lang: Language) -> Vec<PerformanceProfile> {
    let make = |id: &str, name: &str, mode, ac_keep| PerformanceProfile {
        id: id.to_owned(),
        name: name.to_owned(),
        builtin: true,
        ac: PerformanceSupplySettings {
            keep_awake: ac_keep,
            ..PerformanceSupplySettings::mode_only(mode)
        },
        battery: PerformanceSupplySettings::mode_only(mode),
    };
    vec![
        make(
            SILENT_ID,
            lang.pick("Silenzioso", "Silent"),
            PerformanceMode::Silent,
            PerformanceKeepAwake::Off,
        ),
        make(
            BALANCED_ID,
            lang.pick("Bilanciato", "Balanced"),
            PerformanceMode::Balanced,
            PerformanceKeepAwake::Off,
        ),
        // On mains power the performance profile also keeps the system from idle sleep
        // (long renders, builds, downloads); on battery it does not.
        make(
            PERFORMANCE_ID,
            lang.pick("Prestazioni elevate", "High performance"),
            PerformanceMode::HighPerformance,
            PerformanceKeepAwake::System,
        ),
    ]
}

fn is_builtin(id: &str) -> bool {
    matches!(id, SILENT_ID | BALANCED_ID | PERFORMANCE_ID)
}

fn not_found(id: &str) -> AppError {
    AppError::new(
        ErrorCode::NotFound,
        format!("performance profile {id} not found"),
    )
}

fn effective_supply(os_supply: PowerSupply, has_battery: bool) -> PowerSupply {
    if has_battery {
        os_supply
    } else {
        PowerSupply::Ac
    }
}

/// Words the support of one control.
fn support_for(
    control: PerformanceControl,
    access: &Access,
    denied: bool,
    lang: Language,
) -> Support {
    let mac = cfg!(target_os = "macos");
    if denied {
        return Support::read_only(lang.pick(
            "Il sistema ha rifiutato la modifica (criteri o permessi): puoi cambiarla in Impostazioni > Sistema > Alimentazione.",
            "The system refused the change (policy or permissions): change it in Settings > System > Power.",
        ));
    }
    match access {
        Access::Writable => Support::supported(),
        Access::Denied => support_for(control, &Access::Writable, true, lang),
        Access::NeedsAdmin => Support::read_only(match control {
            PerformanceControl::Mode => lang.pick(
                "Su macOS la modalità energetica (Basso consumo, Alta potenza dove presente) si cambia solo in Impostazioni di Sistema > Batteria: da un'app servirebbero i diritti di amministratore.",
                "On macOS the energy mode (Low Power, High Power where available) can only be changed in System Settings > Battery: an app would need administrator rights.",
            ),
            PerformanceControl::DisplayTimeout | PerformanceControl::SleepTimeout => lang.pick(
                "Su macOS i tempi di spegnimento dello schermo e di stop si cambiano in Impostazioni di Sistema > Blocco schermo; senza diritti di amministratore l'app li può solo leggere.",
                "On macOS display-off and sleep times are changed in System Settings > Lock Screen; without administrator rights the app can only read them.",
            ),
            _ => lang.pick(
                "Servono i diritti di amministratore, che l'app non usa.",
                "Needs administrator rights, which the app never uses.",
            ),
        }),
        Access::NotOnThisOs => Support::unsupported(match control {
            PerformanceControl::ProcessorState | PerformanceControl::BoostMode if mac => lang.pick(
                "macOS regola da solo frequenza e boost del processore: non esiste un'impostazione da cambiare.",
                "macOS manages processor frequency and boost itself: there is no setting to change.",
            ),
            _ => lang.pick(
                "Non disponibile su questo sistema operativo.",
                "Not available on this operating system.",
            ),
        }),
        Access::NotAvailable => Support::unsupported(match control {
            PerformanceControl::Mode if mac => lang.pick(
                "Questo Mac non offre la modalità Basso consumo.",
                "This Mac has no Low Power Mode.",
            ),
            PerformanceControl::Mode => lang.pick(
                "Questa versione di Windows non offre la modalità di risparmio energia, oppure lo schema attivo non la supporta (serve lo schema Bilanciato).",
                "This Windows version has no power mode, or the active scheme does not support it (it needs the Balanced scheme).",
            ),
            _ => lang.pick(
                "Lo schema di risparmio energia attivo non espone questa impostazione.",
                "The active power scheme does not expose this setting.",
            ),
        }),
    }
}

impl Inner {
    fn effective_id(&self) -> Option<&str> {
        self.game_override.as_deref().or(self.selected.as_deref())
    }

    fn user_profile(&self, id: &str, _lang: Language) -> Option<PerformanceProfile> {
        self.profiles
            .iter()
            .find(|p| p.id == id)
            .map(|p| PerformanceProfile {
                id: p.id.clone(),
                name: p.name.clone(),
                builtin: false,
                ac: p.ac.clone(),
                battery: p.battery.clone(),
            })
    }

    fn profile(&self, id: &str, lang: Language) -> Option<PerformanceProfile> {
        if is_builtin(id) {
            builtin_profiles(lang).into_iter().find(|p| p.id == id)
        } else {
            self.user_profile(id, lang)
        }
    }

    fn all_profiles(&self, lang: Language) -> Vec<PerformanceProfile> {
        let mut all = builtin_profiles(lang);
        all.extend(
            self.profiles
                .iter()
                .filter_map(|p| self.user_profile(&p.id, lang)),
        );
        all
    }

    fn writable(&self, os: &OsPowerState, control: PerformanceControl) -> bool {
        *os.access.get(control) == Access::Writable && !self.denied.contains(&control)
    }

    /// The values of `settings` the app may write here.
    fn to_write(&self, os: &OsPowerState, s: &PerformanceSupplySettings) -> PerformanceOsSupply {
        let w = |c| self.writable(os, c);
        PerformanceOsSupply {
            mode: w(PerformanceControl::Mode).then_some(s.mode),
            processor_max_percent: s
                .processor_max_percent
                .filter(|_| w(PerformanceControl::ProcessorState)),
            processor_min_percent: s
                .processor_min_percent
                .filter(|_| w(PerformanceControl::ProcessorState)),
            boost_mode: s.boost_mode.filter(|_| w(PerformanceControl::BoostMode)),
            display_off_seconds: s
                .display_off_seconds
                .filter(|_| w(PerformanceControl::DisplayTimeout)),
            sleep_seconds: s
                .sleep_seconds
                .filter(|_| w(PerformanceControl::SleepTimeout)),
        }
    }

    /// The current values of writable controls (the original to restore).
    fn capture(&self, os: &OsPowerState) -> Original {
        let current_mode = match effective_supply(os.supply, os.has_battery) {
            PowerSupply::Ac => os.ac.mode,
            PowerSupply::Battery => os.battery.as_ref().and_then(|b| b.mode),
        };
        let keep = |v: &PerformanceOsSupply| {
            let w = |c| self.writable(os, c);
            PerformanceOsSupply {
                // Windows reports only the effective overlay: restore it on any supply.
                mode: current_mode.filter(|_| w(PerformanceControl::Mode)),
                processor_max_percent: v
                    .processor_max_percent
                    .filter(|_| w(PerformanceControl::ProcessorState)),
                processor_min_percent: v
                    .processor_min_percent
                    .filter(|_| w(PerformanceControl::ProcessorState)),
                boost_mode: v.boost_mode.filter(|_| w(PerformanceControl::BoostMode)),
                display_off_seconds: v
                    .display_off_seconds
                    .filter(|_| w(PerformanceControl::DisplayTimeout)),
                sleep_seconds: v
                    .sleep_seconds
                    .filter(|_| w(PerformanceControl::SleepTimeout)),
            }
        };
        Original {
            scheme_id: os.scheme_id.clone(),
            ac: keep(&os.ac),
            battery: os.battery.as_ref().map(keep),
        }
    }

    fn record(&mut self, failures: Vec<WriteFailure>) {
        for f in failures {
            if f.denied {
                self.denied.insert(f.control);
            }
            tracing::warn!(control = ?f.control, detail = %f.detail, "performance setting not applied");
            self.issues.push((
                f.control,
                if f.denied {
                    IssueKind::Denied
                } else {
                    IssueKind::Failed(f.detail)
                },
            ));
        }
    }
}

fn has_values(v: &PerformanceOsSupply) -> bool {
    *v != PerformanceOsSupply::default()
}

impl PerformanceController {
    pub fn new(backend: Arc<dyn PerformanceBackend>, store: Arc<dyn DocumentStore>) -> Self {
        let profiles: Vec<StoredProfile> = load_json(store.as_ref(), PROFILES_KEY);
        let selection: Selection = load_json(store.as_ref(), SELECTION_KEY);
        let original: Option<Original> = load_json(store.as_ref(), ORIGINAL_KEY);
        let selected = selection
            .profile_id
            .filter(|id| is_builtin(id) || profiles.iter().any(|p| &p.id == id));
        Self {
            backend,
            store,
            inner: Mutex::new(Inner {
                profiles,
                selected,
                game_override: None,
                original,
                applied_supply: None,
                issues: Vec::new(),
                denied: HashSet::new(),
            }),
            watcher: Mutex::new(None),
        }
    }

    /// The controller on the real OS.
    pub fn system(store: Arc<dyn DocumentStore>) -> Self {
        Self::new(Arc::new(SystemBackend::default()), store)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, AppError> {
        self.inner
            .lock()
            .map_err(|_| AppError::internal("performance controller lock poisoned"))
    }

    /// Re-applies the profile the user left active (called once at start). No-op without one.
    pub fn resume(&self) {
        match self.lock() {
            Ok(mut inner) if inner.selected.is_some() => self.apply_effective(&mut inner),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e.message, "performance resume skipped"),
        }
    }

    /// Starts the supply watcher (idempotent). It holds only a weak reference.
    pub fn start_watcher(self: &Arc<Self>, interval: Duration) {
        let Ok(mut slot) = self.watcher.lock() else {
            return;
        };
        if slot.is_some() {
            return;
        }
        let weak: Weak<Self> = Arc::downgrade(self);
        let (stop, rx) = mpsc::channel::<()>();
        let spawned = std::thread::Builder::new()
            .name("performance-supply".into())
            .spawn(move || {
                // Stops on a message, a dropped sender or a dropped controller.
                while let Err(RecvTimeoutError::Timeout) = rx.recv_timeout(interval) {
                    let Some(c) = weak.upgrade() else { break };
                    c.tick();
                }
            });
        match spawned {
            Ok(handle) => *slot = Some(Watcher { stop, handle }),
            Err(e) => tracing::warn!(error = %e, "performance supply watcher not started"),
        }
    }

    /// Stops the watcher and releases the keep-awake request. The OS settings the profile
    /// wrote stay (the selection is re-applied at the next start).
    pub fn shutdown(&self) {
        let watcher = self.watcher.lock().ok().and_then(|mut w| w.take());
        if let Some(w) = watcher {
            let _ = w.stop.send(());
            let _ = w.handle.join();
        }
        if let Err(e) = self.backend.set_keep_awake(PerformanceKeepAwake::Off) {
            tracing::warn!(error = %e.message, "keep-awake not released");
        }
    }

    /// One watcher step: re-applies the effective profile when the supply changed.
    pub fn tick(&self) {
        let (supply, battery) = self.backend.current_supply();
        let supply = effective_supply(supply, battery);
        let Ok(mut inner) = self.lock() else { return };
        if inner.effective_id().is_some() && inner.applied_supply != Some(supply) {
            tracing::info!(
                ?supply,
                "power supply changed, re-applying performance profile"
            );
            self.apply_effective(&mut inner);
        }
    }

    fn ensure_original(&self, inner: &mut Inner, os: &OsPowerState) -> Result<(), AppError> {
        if inner.original.is_some() {
            return Ok(());
        }
        let original = inner.capture(os);
        save_json(self.store.as_ref(), ORIGINAL_KEY, &original)?;
        inner.original = Some(original);
        Ok(())
    }

    /// Applies the effective profile (override, else selection) for the current supply.
    fn apply_effective(&self, inner: &mut Inner) {
        let Some(id) = inner.effective_id().map(str::to_owned) else {
            return;
        };
        let Some(profile) = inner.profile(&id, Language::English) else {
            return;
        };
        let os = self.backend.read();
        let supply = effective_supply(os.supply, os.has_battery);
        inner.issues.clear();
        let ac = inner.to_write(&os, &profile.ac);
        let battery = os
            .has_battery
            .then(|| inner.to_write(&os, &profile.battery));
        if has_values(&ac) || battery.as_ref().is_some_and(has_values) {
            let failures = self.backend.write(None, supply, &ac, battery.as_ref());
            inner.record(failures);
        }
        let keep = if inner.writable(&os, PerformanceControl::KeepAwake) {
            profile.settings_for(supply).keep_awake
        } else {
            PerformanceKeepAwake::Off
        };
        if let Err(e) = self.backend.set_keep_awake(keep) {
            inner
                .issues
                .push((PerformanceControl::KeepAwake, IssueKind::Failed(e.message)));
        }
        inner.applied_supply = Some(supply);
    }

    /// Puts the original state back and forgets it. Partial failures keep it stored, so
    /// "Ripristina" can be tried again.
    fn restore_locked(&self, inner: &mut Inner) {
        inner.issues.clear();
        if let Some(original) = inner.original.clone() {
            let (supply, battery) = self.backend.current_supply();
            let supply = effective_supply(supply, battery);
            let mut failures = Vec::new();
            if has_values(&original.ac) || original.battery.as_ref().is_some_and(has_values) {
                failures = self.backend.write(
                    original.scheme_id.as_deref(),
                    supply,
                    &original.ac,
                    original.battery.as_ref(),
                );
            }
            if failures.is_empty() {
                if let Err(e) = self.store.delete(ORIGINAL_KEY) {
                    tracing::warn!(error = %e.message, "original performance state not removed");
                }
                inner.original = None;
            } else {
                inner.record(failures);
            }
        }
        if let Err(e) = self.backend.set_keep_awake(PerformanceKeepAwake::Off) {
            inner
                .issues
                .push((PerformanceControl::KeepAwake, IssueKind::Failed(e.message)));
        }
        inner.applied_supply = None;
    }

    fn persist_selection(&self, selected: Option<&str>) -> Result<(), AppError> {
        save_json(
            self.store.as_ref(),
            SELECTION_KEY,
            &Selection {
                profile_id: selected.map(str::to_owned),
            },
        )
    }

    fn applicable(&self, inner: &Inner, os: &OsPowerState, p: &PerformanceProfile) -> bool {
        let w = |c| inner.writable(os, c);
        let advanced = |s: &PerformanceSupplySettings| {
            (s.processor_max_percent.is_some() || s.processor_min_percent.is_some())
                && w(PerformanceControl::ProcessorState)
                || s.boost_mode.is_some() && w(PerformanceControl::BoostMode)
                || s.display_off_seconds.is_some() && w(PerformanceControl::DisplayTimeout)
                || s.sleep_seconds.is_some() && w(PerformanceControl::SleepTimeout)
        };
        w(PerformanceControl::Mode)
            || w(PerformanceControl::KeepAwake)
            || advanced(&p.ac)
            || advanced(&p.battery)
    }

    // ------------------------------------------------------------------ Commands

    pub fn state(&self, lang: Language) -> Result<PerformanceState, AppError> {
        let inner = self.lock()?;
        Ok(self.state_locked(&inner, lang))
    }

    fn state_locked(&self, inner: &Inner, lang: Language) -> PerformanceState {
        let os = self.backend.read();
        let support = |c| support_for(c, os.access.get(c), inner.denied.contains(&c), lang);
        let capabilities = PerformanceCapabilities {
            mode: support(PerformanceControl::Mode),
            keep_awake: support(PerformanceControl::KeepAwake),
            processor_state: support(PerformanceControl::ProcessorState),
            boost_mode: support(PerformanceControl::BoostMode),
            display_timeout: support(PerformanceControl::DisplayTimeout),
            sleep_timeout: support(PerformanceControl::SleepTimeout),
        };
        let issues = inner
            .issues
            .iter()
            .map(|(control, kind)| PerformanceIssue {
                control: *control,
                message: match kind {
                    IssueKind::Denied => lang
                        .pick(
                            "Il sistema non ha consentito la modifica (criteri o permessi).",
                            "The system did not allow the change (policy or permissions).",
                        )
                        .to_owned(),
                    IssueKind::Failed(detail) => {
                        format!("{} {detail}", lang.pick("Non applicata:", "Not applied:"))
                    }
                },
            })
            .collect();
        PerformanceState {
            capabilities,
            os: PerformanceOsReport {
                ac: os.ac.clone(),
                battery: os.battery.clone(),
                scheme_name: os.scheme_name.clone(),
                keep_awake: self.backend.keep_awake(),
            },
            supply: effective_supply(os.supply, os.has_battery),
            has_battery: os.has_battery,
            profiles: inner.all_profiles(lang),
            selected_profile_id: inner.selected.clone(),
            game_override: inner.game_override.as_ref().and_then(|id| {
                inner.profile(id, lang).map(|p| PerformanceGameOverride {
                    profile_id: p.id,
                    profile_name: p.name,
                })
            }),
            can_restore: inner.original.is_some(),
            issues,
        }
    }

    /// The user applies a profile: it becomes the selection (stored) and replaces a running
    /// game override.
    pub fn apply_profile(&self, id: &str, lang: Language) -> Result<PerformanceState, AppError> {
        let mut inner = self.lock()?;
        let profile = inner.profile(id, lang).ok_or_else(|| not_found(id))?;
        let os = self.backend.read();
        if !self.applicable(&inner, &os, &profile) {
            return Err(AppError::new(
                ErrorCode::NotSupported,
                "nothing in this profile can be applied on this computer",
            ));
        }
        self.ensure_original(&mut inner, &os)?;
        self.persist_selection(Some(id))?;
        inner.selected = Some(id.to_owned());
        inner.game_override = None;
        self.apply_effective(&mut inner);
        Ok(self.state_locked(&inner, lang))
    }

    /// Creates (`id` null) or replaces a user profile. Re-applies it when it is in effect.
    pub fn save_profile(
        &self,
        input: PerformanceProfileInput,
        lang: Language,
    ) -> Result<PerformanceState, AppError> {
        let name = validate_profile_name(&input.name)?;
        input.ac.validate()?;
        input.battery.validate()?;
        let mut inner = self.lock()?;
        let id = match input.id {
            Some(id) => {
                if is_builtin(&id) {
                    return Err(AppError::invalid_input(
                        "built-in profiles cannot be edited",
                    ));
                }
                if !inner.profiles.iter().any(|p| p.id == id) {
                    return Err(not_found(&id));
                }
                id
            }
            None => {
                if inner.profiles.len() >= MAX_USER_PROFILES {
                    return Err(AppError::invalid_input(format!(
                        "at most {MAX_USER_PROFILES} performance profiles"
                    )));
                }
                let next = inner
                    .profiles
                    .iter()
                    .filter_map(|p| p.id.strip_prefix(USER_ID_PREFIX)?.parse::<u32>().ok())
                    .max()
                    .unwrap_or(0)
                    + 1;
                format!("{USER_ID_PREFIX}{next}")
            }
        };
        let stored = StoredProfile {
            id: id.clone(),
            name,
            ac: input.ac,
            battery: input.battery,
        };
        let mut profiles = inner.profiles.clone();
        match profiles.iter_mut().find(|p| p.id == id) {
            Some(p) => *p = stored,
            None => profiles.push(stored),
        }
        save_json(self.store.as_ref(), PROFILES_KEY, &profiles)?;
        inner.profiles = profiles;
        if inner.effective_id() == Some(id.as_str()) {
            self.apply_effective(&mut inner);
        }
        Ok(self.state_locked(&inner, lang))
    }

    /// Deletes a user profile. If it was selected the selection is cleared (the system
    /// keeps its current settings; "Ripristina" still brings back the original).
    pub fn delete_profile(&self, id: &str, lang: Language) -> Result<PerformanceState, AppError> {
        if is_builtin(id) {
            return Err(AppError::invalid_input(
                "built-in profiles cannot be deleted",
            ));
        }
        let mut inner = self.lock()?;
        if !inner.profiles.iter().any(|p| p.id == id) {
            return Err(not_found(id));
        }
        let profiles: Vec<StoredProfile> = inner
            .profiles
            .iter()
            .filter(|p| p.id != id)
            .cloned()
            .collect();
        save_json(self.store.as_ref(), PROFILES_KEY, &profiles)?;
        inner.profiles = profiles;
        if inner.selected.as_deref() == Some(id) {
            self.persist_selection(None)?;
            inner.selected = None;
            if inner.game_override.is_none() {
                // Nothing is in effect any more: release what only the profile held.
                if let Err(e) = self.backend.set_keep_awake(PerformanceKeepAwake::Off) {
                    tracing::warn!(error = %e.message, "keep-awake not released");
                }
                inner.applied_supply = None;
            }
        }
        if inner.game_override.as_deref() == Some(id) {
            inner.game_override = None;
            if inner.selected.is_some() {
                self.apply_effective(&mut inner);
            } else {
                self.restore_locked(&mut inner);
            }
        }
        Ok(self.state_locked(&inner, lang))
    }

    /// "Ripristina impostazioni originali": the original state back, no selection, no
    /// override.
    pub fn restore_original(&self, lang: Language) -> Result<PerformanceState, AppError> {
        let mut inner = self.lock()?;
        self.persist_selection(None)?;
        inner.selected = None;
        inner.game_override = None;
        self.restore_locked(&mut inner);
        Ok(self.state_locked(&inner, lang))
    }

    pub fn open_power_settings(&self) -> Result<(), AppError> {
        self.backend.open_settings()
    }
}

impl ProfileDomain for PerformanceController {
    fn domain(&self) -> ControlDomainId {
        ControlDomainId::Performance
    }

    fn profile_refs(&self, lang: Language) -> Vec<ProfileRef> {
        let Ok(inner) = self.lock() else {
            return Vec::new();
        };
        let os = self.backend.read();
        inner
            .all_profiles(lang)
            .into_iter()
            .map(|p| ProfileRef {
                domain: ControlDomainId::Performance,
                applicable: self.applicable(&inner, &os, &p),
                id: p.id,
                name: p.name,
                builtin: p.builtin,
            })
            .collect()
    }

    fn apply_game_override(&self, profile_id: &str) -> Result<(), AppError> {
        let mut inner = self.lock()?;
        let profile = inner
            .profile(profile_id, Language::English)
            .ok_or_else(|| not_found(profile_id))?;
        let os = self.backend.read();
        if !self.applicable(&inner, &os, &profile) {
            return Err(AppError::new(
                ErrorCode::NotSupported,
                "nothing in this profile can be applied on this computer",
            ));
        }
        self.ensure_original(&mut inner, &os)?;
        inner.game_override = Some(profile_id.to_owned());
        self.apply_effective(&mut inner);
        Ok(())
    }

    fn clear_game_override(&self) -> Result<(), AppError> {
        let mut inner = self.lock()?;
        if inner.game_override.take().is_none() {
            return Ok(());
        }
        if inner.selected.is_some() {
            self.apply_effective(&mut inner);
        } else {
            // Without a selection of their own the user was on the system's settings.
            self.restore_locked(&mut inner);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "performance/tests.rs"]
mod tests;
