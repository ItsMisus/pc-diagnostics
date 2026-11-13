//! Cooling controller. Owned by the cooling feature.
//!
//! Keeps the cooling profiles (three built-in, user-made ones in `cooling.profiles`), the
//! user's selection (`cooling.selection`), the system state found before the first change
//! (`cooling.original`) and the game override (memory only). What it can change:
//! - the Windows "System cooling policy" of the active power scheme, per supply;
//! - fan speeds and curves: nowhere today (macOS needs root to write the SMC, Windows has
//!   no general fan API), so curves are saved in profiles but reported as not applicable.
//!
//! OS access goes through [`CoolingBackend`] so the state machine is tested with a fake.

use std::sync::{Arc, Mutex, MutexGuard};

use models::control::cooling::{
    CoolingCapabilities, CoolingCurve, CoolingFan, CoolingFanCurve, CoolingFanMode, CoolingMode,
    CoolingPolicy, CoolingPolicySetting, CoolingProfile, CoolingProfileDraft, CoolingProfileEntry,
    CoolingState, CoolingSystem, CoolingSystemKind,
};
use models::control::{
    ControlDomainId, PowerSupply, ProfileRef, Support, SupportState, validate_profile_name,
};
use models::settings::Language;
use models::{AppError, ErrorCode};
use platform::control::cooling::{FanProbe, FanReading, FanSource, PolicyError, PolicyValues};
use serde::{Deserialize, Serialize};

use crate::{DocumentStore, ProfileDomain, load_json, save_json};

pub const KEY_PROFILES: &str = "cooling.profiles";
pub const KEY_SELECTION: &str = "cooling.selection";
pub const KEY_ORIGINAL: &str = "cooling.original";

pub const ID_AUTO: &str = "auto";
pub const ID_SILENT: &str = "silent";
pub const ID_BALANCED: &str = "balanced";

/// Most user profiles kept.
pub const MAX_USER_PROFILES: usize = 32;

/// OS access used by the controller.
pub trait CoolingBackend: Send + Sync {
    fn probe_fans(&self) -> FanProbe;
    fn is_laptop(&self) -> Option<bool>;
    fn read_policy(&self) -> Result<PolicyValues, PolicyError>;
    fn write_policy(&self, values: PolicyValues) -> Result<(), PolicyError>;
    fn cpu_temperature(&self) -> Option<f64>;
}

pub type TemperatureSource = Arc<dyn Fn() -> Option<f64> + Send + Sync>;

/// The real backend (`platform::control::cooling`). The CPU temperature comes from the
/// app's live monitor, so no second sensor reader runs.
pub struct OsCoolingBackend {
    temperature: TemperatureSource,
}

impl OsCoolingBackend {
    pub fn new(temperature: TemperatureSource) -> Self {
        Self { temperature }
    }
}

impl CoolingBackend for OsCoolingBackend {
    fn probe_fans(&self) -> FanProbe {
        platform::control::cooling::probe_fans()
    }
    fn is_laptop(&self) -> Option<bool> {
        platform::control::cooling::is_laptop()
    }
    fn read_policy(&self) -> Result<PolicyValues, PolicyError> {
        platform::control::cooling::read_cooling_policy()
    }
    fn write_policy(&self, values: PolicyValues) -> Result<(), PolicyError> {
        platform::control::cooling::write_cooling_policy(values)
    }
    fn cpu_temperature(&self) -> Option<f64> {
        (self.temperature)()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Selection {
    selected: Option<String>,
}

/// The cooling policy found before the app's first change.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Original {
    ac: Option<u32>,
    dc: Option<u32>,
}

#[derive(Default)]
struct Inner {
    game_override: Option<String>,
    /// Windows refused a write (ERROR_ACCESS_DENIED): the policy is read only from now on.
    policy_denied: bool,
    laptop: Option<Option<bool>>,
}

pub struct CoolingController {
    backend: Arc<dyn CoolingBackend>,
    store: Arc<dyn DocumentStore>,
    inner: Mutex<Inner>,
}

/// What the probe found, worded.
struct Probe {
    system: CoolingSystem,
    fans: Vec<CoolingFan>,
    capabilities: CoolingCapabilities,
    current_policy: Vec<CoolingPolicySetting>,
}

pub fn builtin_profiles(lang: Language) -> Vec<CoolingProfile> {
    let both = |policy| {
        vec![
            CoolingPolicySetting {
                supply: PowerSupply::Ac,
                policy,
            },
            CoolingPolicySetting {
                supply: PowerSupply::Battery,
                policy,
            },
        ]
    };
    let all = |curve| {
        vec![CoolingFanCurve {
            fan_id: None,
            curve,
        }]
    };
    vec![
        CoolingProfile {
            id: ID_AUTO.into(),
            name: lang.pick("Automatico", "Automatic").into(),
            builtin: true,
            mode: CoolingMode::Automatic,
            curves: Vec::new(),
            cooling_policy: Vec::new(),
        },
        CoolingProfile {
            id: ID_SILENT.into(),
            name: lang.pick("Silenzioso", "Silent").into(),
            builtin: true,
            mode: CoolingMode::Silent,
            curves: all(CoolingCurve::silent()),
            cooling_policy: both(CoolingPolicy::Passive),
        },
        CoolingProfile {
            id: ID_BALANCED.into(),
            name: lang.pick("Bilanciato", "Balanced").into(),
            builtin: true,
            mode: CoolingMode::Balanced,
            curves: all(CoolingCurve::balanced()),
            cooling_policy: both(CoolingPolicy::Active),
        },
    ]
}

fn not_supported(message: &str) -> AppError {
    AppError::new(ErrorCode::NotSupported, message)
}

fn policy_error(e: PolicyError) -> AppError {
    match e {
        PolicyError::NotSupported => not_supported("cooling policy is a Windows setting"),
        PolicyError::NotFound => not_supported("the active power scheme has no cooling policy"),
        PolicyError::AccessDenied => AppError::new(
            ErrorCode::PermissionDenied,
            "Windows denied changing the cooling policy",
        ),
        PolicyError::Failed(d) => {
            AppError::new(ErrorCode::Platform, "cooling policy call failed").with_detail(d)
        }
    }
}

impl Original {
    fn values(self) -> PolicyValues {
        PolicyValues {
            ac: self.ac,
            dc: self.dc,
        }
    }
}

fn policy_list(v: PolicyValues) -> Vec<CoolingPolicySetting> {
    [(PowerSupply::Ac, v.ac), (PowerSupply::Battery, v.dc)]
        .into_iter()
        .filter_map(|(supply, raw)| {
            Some(CoolingPolicySetting {
                supply,
                policy: CoolingPolicy::from_index(raw?)?,
            })
        })
        .collect()
}

/// Whether applying `p` touches the cooling policy.
fn touches_policy(p: &CoolingProfile) -> bool {
    p.mode == CoolingMode::Automatic || !p.cooling_policy.is_empty()
}

impl CoolingController {
    pub fn new(backend: Arc<dyn CoolingBackend>, store: Arc<dyn DocumentStore>) -> Self {
        Self {
            backend,
            store,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, AppError> {
        self.inner
            .lock()
            .map_err(|_| AppError::internal("cooling controller lock poisoned"))
    }

    fn user_profiles(&self) -> Vec<CoolingProfile> {
        load_json(self.store.as_ref(), KEY_PROFILES)
    }

    fn all_profiles(&self, lang: Language) -> Vec<CoolingProfile> {
        let mut all = builtin_profiles(lang);
        all.extend(self.user_profiles());
        all
    }

    fn find(&self, id: &str) -> Result<CoolingProfile, AppError> {
        self.all_profiles(Language::English)
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "unknown cooling profile"))
    }

    fn original(&self) -> Result<Option<Original>, AppError> {
        Ok(self
            .store
            .load(KEY_ORIGINAL)?
            .map(|_| load_json(self.store.as_ref(), KEY_ORIGINAL)))
    }

    fn selection(&self) -> Option<String> {
        load_json::<Selection>(self.store.as_ref(), KEY_SELECTION).selected
    }

    fn set_selection(&self, selected: Option<String>) -> Result<(), AppError> {
        save_json(self.store.as_ref(), KEY_SELECTION, &Selection { selected })
    }

    fn policy_support(&self, inner: &Inner, lang: Language) -> (Support, PolicyValues) {
        match self.backend.read_policy() {
            Ok(v) if inner.policy_denied => (
                Support::read_only(lang.pick(
                    "Windows ha rifiutato la modifica: un criterio di sistema blocca le impostazioni di alimentazione. Chiedi all'amministratore del computer.",
                    "Windows refused the change: a system policy locks the power settings. Ask the computer's administrator.",
                )),
                v,
            ),
            Ok(v) => (Support::supported(), v),
            Err(PolicyError::NotSupported) => (
                Support::unsupported(lang.pick(
                    "La politica di raffreddamento è un'impostazione di Windows: macOS gestisce il raffreddamento da solo.",
                    "The cooling policy is a Windows setting: macOS manages cooling on its own.",
                )),
                PolicyValues::default(),
            ),
            Err(PolicyError::NotFound) => (
                Support::unsupported(lang.pick(
                    "Lo schema di alimentazione attivo non ha l'impostazione \"Criterio di raffreddamento del sistema\".",
                    "The active power plan has no \"System cooling policy\" setting.",
                )),
                PolicyValues::default(),
            ),
            Err(PolicyError::AccessDenied) => (
                Support::read_only(lang.pick(
                    "Windows non permette di leggere o cambiare questa impostazione con il tuo account.",
                    "Windows does not let your account read or change this setting.",
                )),
                PolicyValues::default(),
            ),
            Err(PolicyError::Failed(d)) => (
                Support::unsupported(format!(
                    "{} ({d})",
                    lang.pick(
                        "Impossibile leggere la politica di raffreddamento",
                        "Could not read the cooling policy"
                    )
                )),
                PolicyValues::default(),
            ),
        }
    }

    fn probe(&self, inner: &mut Inner, lang: Language) -> Probe {
        let laptop = *inner.laptop.get_or_insert_with(|| self.backend.is_laptop());
        let (policy, values) = self.policy_support(inner, lang);
        let probe = self.backend.probe_fans();
        let (system, fan_speed, readings, source) = describe_fans(&probe, laptop, lang);
        let curves = if fan_speed.is_supported() {
            // No backend writes fan speeds today; keep the curve honest if one ever does.
            Support::unsupported(lang.pick(
                "Le curve personalizzate non sono ancora applicabili.",
                "Custom curves cannot be applied yet.",
            ))
        } else {
            Support {
                state: if fan_speed.state == SupportState::ReadOnly {
                    SupportState::ReadOnly
                } else {
                    SupportState::Unsupported
                },
                reason: Some(lang.pick(
                    "Le curve si possono progettare e salvare, ma applicarle richiede il controllo delle ventole, che questo computer non concede all'app.",
                    "Curves can be designed and saved, but applying them needs fan control, which this computer does not give the app.",
                ).into()),
            }
        };
        let fans = readings
            .iter()
            .enumerate()
            .map(|(i, r)| fan_view(i, r, source, &fan_speed, lang))
            .collect();
        Probe {
            system,
            fans,
            capabilities: CoolingCapabilities {
                fan_speed,
                curves,
                cooling_policy: policy,
            },
            current_policy: policy_list(values),
        }
    }

    fn entry(p: CoolingProfile, caps: &CoolingCapabilities, lang: Language) -> CoolingProfileEntry {
        let policy_ok = caps.cooling_policy.is_supported();
        let curves_ok = caps.curves.is_supported();
        let has_curves = !p.curves.is_empty();
        let policy_effect = policy_ok && touches_policy(&p);
        let applicable =
            p.mode == CoolingMode::Automatic || policy_effect || (curves_ok && has_curves);
        let note = if p.mode == CoolingMode::Automatic {
            Some(if policy_ok {
                lang.pick(
                    "Rimette la politica di raffreddamento trovata all'inizio; le ventole restano al sistema.",
                    "Puts back the cooling policy found at first; the fans stay with the system.",
                )
            } else {
                lang.pick(
                    "Il sistema regola già il raffreddamento da solo.",
                    "The system already manages cooling on its own.",
                )
            })
        } else if !applicable {
            Some(if has_curves {
                lang.pick(
                    "Curva salvata, ma non applicabile su questo computer.",
                    "Curve saved, but it cannot be applied on this computer.",
                )
            } else {
                lang.pick(
                    "Niente in questo profilo si può applicare su questo computer.",
                    "Nothing in this profile can be applied on this computer.",
                )
            })
        } else if has_curves && !curves_ok {
            Some(lang.pick(
                "Applica la politica di raffreddamento di Windows; la curva resta salvata ma non si applica qui.",
                "Applies the Windows cooling policy; the curve stays saved but is not applied here.",
            ))
        } else {
            None
        };
        CoolingProfileEntry {
            profile: p,
            applicable,
            note: note.map(str::to_owned),
        }
    }

    fn state_locked(&self, inner: &mut Inner, lang: Language) -> Result<CoolingState, AppError> {
        let probe = self.probe(inner, lang);
        let selected = self.selection();
        let can_restore = self.original()?.is_some();
        let profiles = self
            .all_profiles(lang)
            .into_iter()
            .map(|p| Self::entry(p, &probe.capabilities, lang))
            .collect();
        let active = inner
            .game_override
            .clone()
            .or_else(|| selected.clone())
            .or_else(|| (!can_restore).then(|| ID_AUTO.to_owned()));
        Ok(CoolingState {
            system: probe.system,
            fans: probe.fans,
            capabilities: probe.capabilities,
            current_policy: probe.current_policy,
            profiles,
            selected,
            game_override: inner.game_override.clone(),
            active,
            can_restore,
            cpu_temperature_c: self
                .backend
                .cpu_temperature()
                .filter(|t| t.is_finite() && (1.0..=150.0).contains(t)),
        })
    }

    pub fn state(&self, lang: Language) -> Result<CoolingState, AppError> {
        let mut inner = self.lock()?;
        self.state_locked(&mut inner, lang)
    }

    /// Stores the system's cooling policy before the first change.
    fn ensure_original(&self) -> Result<Original, AppError> {
        if let Some(o) = self.original()? {
            return Ok(o);
        }
        let v = self.backend.read_policy().map_err(policy_error)?;
        let o = Original { ac: v.ac, dc: v.dc };
        save_json(self.store.as_ref(), KEY_ORIGINAL, &o)?;
        Ok(o)
    }

    /// Applies what of `p` works here. Fails without changing anything when nothing does.
    fn apply_effect(&self, inner: &mut Inner, p: &CoolingProfile) -> Result<(), AppError> {
        let lang = Language::English;
        let (policy, _) = self.policy_support(inner, lang);
        let entry = Self::entry(
            p.clone(),
            &CoolingCapabilities {
                fan_speed: Support::unsupported(""),
                curves: Support::unsupported(""),
                cooling_policy: policy.clone(),
            },
            lang,
        );
        if !entry.applicable {
            return Err(not_supported(
                "nothing in this cooling profile can be applied on this computer",
            ));
        }
        if !policy.is_supported() || !touches_policy(p) {
            return Ok(()); // Automatic where the system already decides: nothing to write.
        }
        let target = if p.mode == CoolingMode::Automatic {
            match self.original()? {
                Some(o) => o.values(),
                None => return Ok(()), // never changed: already the system's own values
            }
        } else {
            let original = self.ensure_original()?;
            let pick = |supply, fallback: Option<u32>| {
                p.cooling_policy
                    .iter()
                    .find(|s| s.supply == supply)
                    .map(|s| s.policy.index())
                    .or(fallback)
            };
            PolicyValues {
                ac: pick(PowerSupply::Ac, original.ac),
                dc: pick(PowerSupply::Battery, original.dc),
            }
        };
        match self.backend.write_policy(target) {
            Ok(()) => Ok(()),
            Err(PolicyError::AccessDenied) => {
                inner.policy_denied = true;
                Err(policy_error(PolicyError::AccessDenied))
            }
            Err(e) => Err(policy_error(e)),
        }
    }

    /// Applies a profile as the user's selection. With a game override active the
    /// selection is stored and comes into effect when the override ends.
    pub fn apply_profile(&self, id: &str, lang: Language) -> Result<CoolingState, AppError> {
        let mut inner = self.lock()?;
        let p = self.find(id)?;
        if inner.game_override.is_none() {
            self.apply_effect(&mut inner, &p)?;
        }
        self.set_selection(Some(p.id))?;
        self.state_locked(&mut inner, lang)
    }

    pub fn save_profile(
        &self,
        draft: CoolingProfileDraft,
        lang: Language,
    ) -> Result<CoolingState, AppError> {
        let mut inner = self.lock()?;
        let name = validate_profile_name(&draft.name)?;
        let mut users = self.user_profiles();
        let id = match &draft.id {
            Some(id) => {
                if !users.iter().any(|p| &p.id == id) {
                    return Err(AppError::new(
                        ErrorCode::NotFound,
                        "unknown user cooling profile",
                    ));
                }
                id.clone()
            }
            None => {
                if users.len() >= MAX_USER_PROFILES {
                    return Err(AppError::invalid_input(format!(
                        "at most {MAX_USER_PROFILES} cooling profiles"
                    )));
                }
                let next = users
                    .iter()
                    .filter_map(|p| p.id.strip_prefix("user-")?.parse::<u32>().ok())
                    .max()
                    .map_or(1, |n| n + 1);
                format!("user-{next}")
            }
        };
        let profile = CoolingProfile {
            id: id.clone(),
            name,
            builtin: false,
            mode: CoolingMode::Custom,
            curves: draft.curves,
            cooling_policy: draft.cooling_policy,
        };
        profile.validate()?;
        match users.iter_mut().find(|p| p.id == id) {
            Some(slot) => *slot = profile,
            None => users.push(profile),
        }
        save_json(self.store.as_ref(), KEY_PROFILES, &users)?;
        self.state_locked(&mut inner, lang)
    }

    /// Deletes a user profile. The system is not changed; a selection or override that
    /// pointed at it is cleared.
    pub fn delete_profile(&self, id: &str, lang: Language) -> Result<CoolingState, AppError> {
        let mut inner = self.lock()?;
        let mut users = self.user_profiles();
        let before = users.len();
        users.retain(|p| p.id != id);
        if users.len() == before {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "unknown user cooling profile",
            ));
        }
        save_json(self.store.as_ref(), KEY_PROFILES, &users)?;
        if self.selection().as_deref() == Some(id) {
            self.set_selection(None)?;
        }
        if inner.game_override.as_deref() == Some(id) {
            inner.game_override = None;
        }
        self.state_locked(&mut inner, lang)
    }

    /// Puts back the state found before the first change and forgets the selection.
    pub fn restore(&self, lang: Language) -> Result<CoolingState, AppError> {
        let mut inner = self.lock()?;
        if let Some(o) = self.original()? {
            match self.backend.write_policy(o.values()) {
                Ok(()) | Err(PolicyError::NotSupported) => {}
                Err(PolicyError::AccessDenied) => {
                    inner.policy_denied = true;
                    return Err(policy_error(PolicyError::AccessDenied));
                }
                Err(e) => return Err(policy_error(e)),
            }
            self.store.delete(KEY_ORIGINAL)?;
        }
        self.set_selection(None)?;
        inner.game_override = None;
        self.state_locked(&mut inner, lang)
    }

    /// On exit: a game override never outlives the app.
    pub fn shutdown(&self) {
        if let Err(e) = self.clear_game_override() {
            tracing::warn!(error = %e.message, "cooling: could not clear the game override on exit");
        }
    }
}

fn describe_fans(
    probe: &FanProbe,
    laptop: Option<bool>,
    lang: Language,
) -> (CoolingSystem, Support, Vec<FanReading>, Option<FanSource>) {
    let system = |kind, description: &str| CoolingSystem {
        kind,
        description: description.to_owned(),
    };
    match probe {
        FanProbe::Read { source, fans, .. } if !fans.is_empty() => {
            let (description, reason) = match source {
                FanSource::AppleSmc => (
                    lang.pick(
                        "Ventole rilevate dal controller di sistema (SMC).",
                        "Fans found by the system controller (SMC).",
                    ),
                    lang.pick(
                        "macOS permette di cambiare la velocità delle ventole solo con i permessi di amministratore, che l'app non usa: il sistema le regola da solo.",
                        "macOS lets fan speeds change only with administrator rights, which the app does not use: the system manages them.",
                    ),
                ),
                FanSource::Wmi => (
                    lang.pick(
                        "Ventole riportate da Windows (WMI, Win32_Fan).",
                        "Fans reported by Windows (WMI, Win32_Fan).",
                    ),
                    lang.pick(
                        "Windows non offre un controllo generale delle ventole senza i driver del produttore.",
                        "Windows offers no general fan control without the manufacturer's drivers.",
                    ),
                ),
            };
            (
                system(CoolingSystemKind::Fans, description),
                Support::read_only(reason),
                fans.clone(),
                Some(*source),
            )
        }
        FanProbe::Read {
            source: FanSource::AppleSmc,
            declared,
            ..
        } if *declared == Some(0) || laptop == Some(true) => (
            system(
                CoolingSystemKind::Passive,
                lang.pick(
                    "Raffreddamento passivo: nessuna ventola, il calore si disperde attraverso la scocca in alluminio. Il sistema riduce le prestazioni quando serve.",
                    "Passive cooling: no fan, heat spreads out through the aluminium chassis. The system lowers performance when needed.",
                ),
            ),
            Support::unsupported(lang.pick(
                "Questo computer non ha ventole.",
                "This computer has no fans.",
            )),
            Vec::new(),
            Some(FanSource::AppleSmc),
        ),
        FanProbe::Read { source, .. } => (
            system(
                CoolingSystemKind::Unknown,
                match source {
                    FanSource::AppleSmc => lang.pick(
                        "Il controller di sistema (SMC) non dichiara ventole.",
                        "The system controller (SMC) declares no fans.",
                    ),
                    FanSource::Wmi => lang.pick(
                        "Windows non riporta ventole (Win32_Fan): di solito il firmware le gestisce senza esporle.",
                        "Windows reports no fans (Win32_Fan): usually the firmware runs them without exposing them.",
                    ),
                },
            ),
            Support::unsupported(lang.pick(
                "Nessuna ventola riportata dal sistema.",
                "No fan reported by the system.",
            )),
            Vec::new(),
            Some(*source),
        ),
        FanProbe::Unavailable { detail, .. } => (
            system(
                CoolingSystemKind::Unknown,
                lang.pick(
                    "Impossibile leggere le ventole.",
                    "The fans could not be read.",
                ),
            ),
            Support::unsupported(format!(
                "{} ({detail})",
                lang.pick("Lettura non riuscita", "Reading failed")
            )),
            Vec::new(),
            None,
        ),
        FanProbe::NotSupported => (
            system(
                CoolingSystemKind::Unknown,
                lang.pick(
                    "Questo sistema operativo non riporta le ventole.",
                    "This operating system does not report fans.",
                ),
            ),
            Support::unsupported(lang.pick(
                "Non supportato su questo sistema operativo.",
                "Not supported on this operating system.",
            )),
            Vec::new(),
            None,
        ),
    }
}

fn fan_view(
    index: usize,
    r: &FanReading,
    source: Option<FanSource>,
    speed: &Support,
    lang: Language,
) -> CoolingFan {
    let missing = r.current_rpm.is_none()
        || r.min_rpm.is_none()
        || r.max_rpm.is_none()
        || r.target_rpm.is_none();
    let rpm_reason = missing.then(|| {
        match source {
            Some(FanSource::Wmi) => lang.pick(
                "Windows riporta al massimo la velocità richiesta (Win32_Fan), non quella attuale.",
                "Windows reports at most the requested speed (Win32_Fan), not the actual one.",
            ),
            _ => lang.pick(
                "Il controller di sistema non riporta questo valore.",
                "The system controller does not report this value.",
            ),
        }
        .to_owned()
    });
    CoolingFan {
        id: r.id.clone(),
        label: r
            .label
            .clone()
            .unwrap_or_else(|| format!("{} {}", lang.pick("Ventola", "Fan"), index + 1)),
        current_rpm: r.current_rpm,
        min_rpm: r.min_rpm,
        max_rpm: r.max_rpm,
        target_rpm: r.target_rpm,
        rpm_reason,
        mode: r.forced.map(|f| {
            if f {
                CoolingFanMode::Forced
            } else {
                CoolingFanMode::Auto
            }
        }),
        speed_control: speed.clone(),
    }
}

impl ProfileDomain for CoolingController {
    fn domain(&self) -> ControlDomainId {
        ControlDomainId::Cooling
    }

    fn profile_refs(&self, lang: Language) -> Vec<ProfileRef> {
        let Ok(mut inner) = self.lock() else {
            return Vec::new();
        };
        let probe = self.probe(&mut inner, lang);
        self.all_profiles(lang)
            .into_iter()
            .map(|p| {
                let e = Self::entry(p, &probe.capabilities, lang);
                ProfileRef {
                    domain: ControlDomainId::Cooling,
                    id: e.profile.id,
                    name: e.profile.name,
                    builtin: e.profile.builtin,
                    applicable: e.applicable,
                }
            })
            .collect()
    }

    fn apply_game_override(&self, profile_id: &str) -> Result<(), AppError> {
        let mut inner = self.lock()?;
        let p = self.find(profile_id)?;
        self.apply_effect(&mut inner, &p)?;
        inner.game_override = Some(p.id);
        Ok(())
    }

    fn clear_game_override(&self) -> Result<(), AppError> {
        let mut inner = self.lock()?;
        if inner.game_override.take().is_none() {
            return Ok(());
        }
        let back = self
            .selection()
            .and_then(|id| self.find(&id).ok())
            .unwrap_or_else(|| builtin_profiles(Language::English).remove(0));
        self.apply_effect(&mut inner, &back)
    }
}

#[cfg(test)]
mod tests;
