//! Lighting controller. Owned by the lighting feature.
//!
//! Keeps the devices (from `platform::control::lighting`, behind `LightingBackend` so the
//! state machine is tested with a fake), the built-in and user profiles, the user's
//! selection and Gaming's override, the original state of every device it touched, and the
//! render engine of dynamic effects.
//!
//! Documents (`DocumentStore`):
//! - `lighting.profiles`: the user's profiles.
//! - `lighting.selection`: the selected profile id (shown on the page; never applied at
//!   startup: lights change only on a user action or a game override).
//! - `lighting.original`: what each device showed before the app first changed it, saved
//!   before that change; "Ripristina" puts it back (also after a restart) and clears it.
//!
//! Game override: Gaming applies a profile while a game runs; clearing it re-applies the
//! user's own selection, or, when the user never applied anything, gives the devices back as
//! they were (their original state is the user's own lighting).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use models::control::lighting::{
    LightingApplied, LightingColor, LightingDevice, LightingDirection, LightingEffect,
    LightingProfile, LightingSetup, LightingSnapshot, LightingSource, LightingState,
    LightingTargets,
};
use models::control::{ControlDomainId, ProfileRef, validate_profile_name};
use models::settings::Language;
use models::{AppError, ErrorCode};
use platform::control::lighting::{DeviceLayout, LightingFrame, SystemLighting};
use serde::{Deserialize, Serialize};

use crate::{DocumentStore, ProfileDomain, load_json, save_json};

pub mod effect;
mod engine;

use engine::{Engine, Program, Target};

pub const PROFILES_KEY: &str = "lighting.profiles";
pub const SELECTION_KEY: &str = "lighting.selection";
pub const ORIGINAL_KEY: &str = "lighting.original";
/// Most user profiles kept.
pub const MAX_USER_PROFILES: usize = 32;

/// OS access the controller needs (implemented by `platform::control::lighting`).
pub trait LightingBackend: Send + Sync {
    fn devices(&self, lang: Language) -> Vec<LightingDevice>;
    fn layout(&self, id: &str) -> DeviceLayout;
    fn snapshot(&self, id: &str) -> Result<LightingSnapshot, AppError>;
    fn take_control(&self, id: &str) -> Result<(), AppError>;
    fn write(&self, id: &str, frame: &LightingFrame, persist: bool) -> Result<(), AppError>;
    fn restore(&self, id: &str, snapshot: &LightingSnapshot) -> Result<(), AppError>;
    fn release(&self, id: &str);
    fn platform_note(&self, lang: Language) -> Option<String>;
}

impl LightingBackend for SystemLighting {
    fn devices(&self, lang: Language) -> Vec<LightingDevice> {
        SystemLighting::devices(self, lang)
    }
    fn layout(&self, id: &str) -> DeviceLayout {
        SystemLighting::layout(self, id)
    }
    fn snapshot(&self, id: &str) -> Result<LightingSnapshot, AppError> {
        SystemLighting::snapshot(self, id)
    }
    fn take_control(&self, id: &str) -> Result<(), AppError> {
        SystemLighting::take_control(self, id)
    }
    fn write(&self, id: &str, frame: &LightingFrame, persist: bool) -> Result<(), AppError> {
        SystemLighting::write(self, id, frame, persist)
    }
    fn restore(&self, id: &str, snapshot: &LightingSnapshot) -> Result<(), AppError> {
        SystemLighting::restore(self, id, snapshot)
    }
    fn release(&self, id: &str) {
        SystemLighting::release(self, id);
    }
    fn platform_note(&self, lang: Language) -> Option<String> {
        SystemLighting::platform_note(self, lang)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSelection {
    selected: Option<String>,
}

type Original = BTreeMap<String, LightingSnapshot>;

const AZURE: LightingColor = LightingColor::rgb(78, 165, 255);

/// The built-in profiles, named in `lang`.
pub fn builtin_profiles(lang: Language) -> Vec<LightingProfile> {
    let p =
        |id: &str, it: &str, en: &str, effect: LightingEffect, brightness: u8| LightingProfile {
            id: id.to_owned(),
            name: lang.pick(it, en).to_owned(),
            builtin: true,
            effect,
            brightness,
            targets: LightingTargets::All,
            sync: true,
        };
    vec![
        p(
            "builtin.static",
            "Statico",
            "Static",
            LightingEffect::Static { color: AZURE },
            80,
        ),
        p(
            "builtin.breathing",
            "Respiro",
            "Breathing",
            LightingEffect::Breathing {
                color: AZURE,
                period: 4_000,
            },
            80,
        ),
        p(
            "builtin.rainbow",
            "Arcobaleno",
            "Rainbow",
            LightingEffect::Rainbow { speed: 3 },
            80,
        ),
        p(
            "builtin.wave",
            "Onda",
            "Wave",
            LightingEffect::Wave {
                colors: vec![
                    LightingColor::rgb(255, 59, 107),
                    LightingColor::rgb(255, 179, 64),
                    AZURE,
                ],
                speed: 4,
                direction: LightingDirection::LeftToRight,
            },
            80,
        ),
        p("builtin.off", "Spento", "Off", LightingEffect::Off, 0),
    ]
}

fn setup_of(p: &LightingProfile) -> LightingSetup {
    LightingSetup {
        effect: p.effect.clone(),
        brightness: p.brightness,
        targets: p.targets.clone(),
        sync: p.sync,
    }
}

fn validate_setup(s: &LightingSetup) -> Result<(), AppError> {
    s.effect.validate()?;
    if s.brightness > 100 {
        return Err(AppError::invalid_input("brightness above 100"));
    }
    if let LightingTargets::Devices { ids } = &s.targets
        && (ids.len() > 64 || ids.iter().any(|i| i.len() > 512))
    {
        return Err(AppError::invalid_input("too many or too long device ids"));
    }
    Ok(())
}

/// The device can show the effect (RGB devices any; single-color lights only an envelope).
fn shows(d: &LightingDevice, effect: &LightingEffect) -> bool {
    d.controllable() && (d.color_capable || effect.monochrome_compatible())
}

struct Inner {
    devices: Option<(Language, Vec<LightingDevice>)>,
    /// The user's own last applied setup and its profile.
    user: Option<(LightingSetup, Option<String>)>,
    game: Option<String>,
    applied: Option<LightingApplied>,
    /// Devices the app has taken over in this session.
    controlled: BTreeSet<String>,
    /// When each device joined (timeline origin without sync).
    joined: HashMap<String, Instant>,
    engine: Option<Engine>,
}

pub struct LightingController {
    backend: Arc<dyn LightingBackend>,
    store: Arc<dyn DocumentStore>,
    inner: Mutex<Inner>,
    program: Arc<Mutex<Option<Program>>>,
}

impl LightingController {
    pub fn new(backend: Arc<dyn LightingBackend>, store: Arc<dyn DocumentStore>) -> Self {
        Self {
            backend,
            store,
            inner: Mutex::new(Inner {
                devices: None,
                user: None,
                game: None,
                applied: None,
                controlled: BTreeSet::new(),
                joined: HashMap::new(),
                engine: None,
            }),
            program: Arc::new(Mutex::new(None)),
        }
    }

    /// The controller over this computer's devices.
    pub fn system(store: Arc<dyn DocumentStore>) -> Self {
        Self::new(Arc::new(SystemLighting::new()), store)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, AppError> {
        self.inner
            .lock()
            .map_err(|_| AppError::internal("lighting controller lock poisoned"))
    }

    fn enumerate(&self, lang: Language) -> Vec<LightingDevice> {
        let mut list = self.backend.devices(lang);
        let rank = |d: &LightingDevice| match (d.controllable(), d.source) {
            (true, _) => 0,
            (false, LightingSource::VendorProtocol) => 2,
            (false, _) => 1,
        };
        list.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.cmp(&b.name)));
        list
    }

    fn devices<'a>(&self, inner: &'a mut Inner, lang: Language) -> &'a [LightingDevice] {
        let fresh = !matches!(&inner.devices, Some((l, _)) if *l == lang);
        if fresh {
            inner.devices = Some((lang, self.enumerate(lang)));
        }
        inner.devices.as_ref().map_or(&[], |(_, d)| d.as_slice())
    }

    fn user_profiles(&self) -> Vec<LightingProfile> {
        load_json(&*self.store, PROFILES_KEY)
    }

    /// Built-in then user profiles.
    pub fn profiles(&self, lang: Language) -> Vec<LightingProfile> {
        let mut all = builtin_profiles(lang);
        all.extend(self.user_profiles());
        all
    }

    fn find_profile(&self, id: &str, lang: Language) -> Result<LightingProfile, AppError> {
        self.profiles(lang)
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::NotFound,
                    format!("lighting profile {id} not found"),
                )
            })
    }

    fn compose(&self, inner: &mut Inner, lang: Language) -> LightingState {
        let devices = self.devices(inner, lang).to_vec();
        let selected = load_json::<StoredSelection>(&*self.store, SELECTION_KEY).selected;
        let original: Original = load_json(&*self.store, ORIGINAL_KEY);
        LightingState {
            devices,
            profiles: self.profiles(lang),
            selected,
            game_override: inner.game.clone(),
            applied: inner.applied.clone(),
            can_restore: !original.is_empty(),
            engine_running: inner.engine.as_ref().is_some_and(Engine::running),
            platform_note: self.backend.platform_note(lang),
        }
    }

    pub fn state(&self, lang: Language) -> Result<LightingState, AppError> {
        let mut inner = self.lock()?;
        Ok(self.compose(&mut inner, lang))
    }

    /// Enumerates the devices again (plugged in or out).
    pub fn refresh(&self, lang: Language) -> Result<LightingState, AppError> {
        let mut inner = self.lock()?;
        inner.devices = Some((lang, self.enumerate(lang)));
        Ok(self.compose(&mut inner, lang))
    }

    fn stop_engine(&self, inner: &mut Inner) {
        if let Some(e) = inner.engine.take() {
            e.stop();
        }
    }

    /// Puts `setup` on the devices it targets. Returns the ids that took it.
    fn put(
        &self,
        inner: &mut Inner,
        setup: &LightingSetup,
        profile_id: Option<String>,
        lang: Language,
    ) -> Result<(), AppError> {
        validate_setup(setup)?;
        let devices: Vec<LightingDevice> = self
            .devices(inner, lang)
            .iter()
            .filter(|d| setup.targets.includes(&d.id) && shows(d, &setup.effect))
            .cloned()
            .collect();
        if devices.is_empty() {
            return Err(AppError::new(
                ErrorCode::NotSupported,
                "no connected lighting device can show this effect",
            ));
        }

        // Record the original state before the first change of each device.
        let mut original: Original = load_json(&*self.store, ORIGINAL_KEY);
        let mut usable = Vec::new();
        let mut changed = false;
        for d in devices {
            if !original.contains_key(&d.id) {
                match self.backend.snapshot(&d.id) {
                    Ok(s) => {
                        original.insert(d.id.clone(), s);
                        changed = true;
                    }
                    Err(e) => {
                        tracing::warn!(id = d.id, error = %e.message, "no snapshot, device left alone");
                        continue;
                    }
                }
            }
            usable.push(d);
        }
        if changed {
            save_json(&*self.store, ORIGINAL_KEY, &original)?;
        }
        if usable.is_empty() {
            return Err(AppError::new(
                ErrorCode::Platform,
                "the lighting devices could not be read before the change",
            ));
        }

        self.stop_engine(inner);

        // Devices driven before but not targeted now go back to how they were.
        let targeted: BTreeSet<String> = usable.iter().map(|d| d.id.clone()).collect();
        let dropped: Vec<String> = inner.controlled.difference(&targeted).cloned().collect();
        for id in dropped {
            if let Some(s) = original.get(&id)
                && let Err(e) = self.backend.restore(&id, s)
            {
                tracing::warn!(id, error = %e.message, "device not restored");
            }
            self.backend.release(&id);
            inner.controlled.remove(&id);
        }

        let now = Instant::now();
        let mut targets = Vec::new();
        for d in &usable {
            if !inner.controlled.contains(&d.id) {
                if let Err(e) = self.backend.take_control(&d.id) {
                    tracing::warn!(id = d.id, error = %e.message, "device not taken over");
                    continue;
                }
                inner.controlled.insert(d.id.clone());
            }
            let joined = *inner.joined.entry(d.id.clone()).or_insert(now);
            let layout = self.backend.layout(&d.id);
            let count = if d.color_capable {
                layout.lamp_count.max(d.lamp_count).max(1)
            } else {
                1
            };
            let positions = if layout.positions.len() == count as usize {
                layout.positions
            } else {
                (0..count)
                    .map(|i| effect::default_position(i, count))
                    .collect()
            };
            targets.push(Target {
                id: d.id.clone(),
                color_capable: d.color_capable,
                lamp_count: count,
                positions,
                t0: if setup.sync { now } else { joined },
            });
        }
        if setup.sync && targets.len() > 1 {
            let n = targets.len() as f64;
            for (i, t) in targets.iter_mut().enumerate() {
                for p in &mut t.positions {
                    *p = (i as f64 + *p) / n;
                }
            }
        }

        let program = Program {
            effect: setup.effect.clone(),
            brightness: setup.brightness,
            targets,
        };
        let steady = !program.effect.is_dynamic();
        let mut took = Vec::new();
        for t in &program.targets {
            let frame = engine::render(&program, t, now);
            match self.backend.write(&t.id, &frame, steady) {
                Ok(()) => took.push(t.id.clone()),
                Err(e) => tracing::warn!(id = t.id, error = %e.message, "lighting write failed"),
            }
        }
        if took.is_empty() {
            return Err(AppError::new(
                ErrorCode::Platform,
                "no lighting device accepted the change",
            ));
        }
        let dynamic = !steady;
        if let Ok(mut g) = self.program.lock() {
            *g = Some(program);
        }
        if dynamic {
            inner.engine = Some(Engine::start(self.backend.clone(), self.program.clone()));
        }
        inner.applied = Some(LightingApplied {
            setup: setup.clone(),
            profile_id,
            device_ids: took,
        });
        Ok(())
    }

    fn select(&self, id: Option<String>) -> Result<(), AppError> {
        save_json(
            &*self.store,
            SELECTION_KEY,
            &StoredSelection { selected: id },
        )
    }

    /// Applies a profile as the user's selection.
    pub fn apply_profile(&self, id: &str, lang: Language) -> Result<LightingState, AppError> {
        let profile = self.find_profile(id, lang)?;
        let setup = setup_of(&profile);
        let mut inner = self.lock()?;
        self.put(&mut inner, &setup, Some(profile.id.clone()), lang)?;
        inner.user = Some((setup, Some(profile.id.clone())));
        self.select(Some(profile.id))?;
        Ok(self.compose(&mut inner, lang))
    }

    /// Applies an edited setup as the user's selection (no profile).
    pub fn apply_setup(
        &self,
        setup: LightingSetup,
        lang: Language,
    ) -> Result<LightingState, AppError> {
        let mut inner = self.lock()?;
        self.put(&mut inner, &setup, None, lang)?;
        inner.user = Some((setup, None));
        self.select(None)?;
        Ok(self.compose(&mut inner, lang))
    }

    /// Shows `setup` on the devices without making it the user's selection (live preview
    /// while editing). "Ripristina" or the next apply replaces it.
    pub fn preview_setup(
        &self,
        setup: LightingSetup,
        lang: Language,
    ) -> Result<LightingState, AppError> {
        let mut inner = self.lock()?;
        self.put(&mut inner, &setup, None, lang)?;
        Ok(self.compose(&mut inner, lang))
    }

    /// Changes the brightness of what is on the devices.
    pub fn set_brightness(
        &self,
        brightness: u8,
        lang: Language,
    ) -> Result<LightingState, AppError> {
        let mut inner = self.lock()?;
        let Some(applied) = inner.applied.clone() else {
            return Err(AppError::invalid_input("no lighting applied yet"));
        };
        let setup = LightingSetup {
            brightness,
            ..applied.setup
        };
        self.put(&mut inner, &setup, applied.profile_id.clone(), lang)?;
        if inner.game.is_none() {
            inner.user = Some((setup, applied.profile_id));
        }
        Ok(self.compose(&mut inner, lang))
    }

    /// Creates (`id` None) or updates a user profile. Built-in profiles cannot be changed.
    pub fn save_profile(
        &self,
        id: Option<String>,
        name: &str,
        setup: LightingSetup,
        lang: Language,
    ) -> Result<LightingState, AppError> {
        let name = validate_profile_name(name)?;
        validate_setup(&setup)?;
        let mut list = self.user_profiles();
        let id = match id {
            Some(id) if id.starts_with("builtin.") => {
                return Err(AppError::invalid_input(
                    "built-in profiles cannot be changed",
                ));
            }
            Some(id) => {
                if !list.iter().any(|p| p.id == id) {
                    return Err(AppError::new(
                        ErrorCode::NotFound,
                        format!("lighting profile {id} not found"),
                    ));
                }
                id
            }
            None => {
                if list.len() >= MAX_USER_PROFILES {
                    return Err(AppError::invalid_input("too many lighting profiles"));
                }
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis());
                let mut n = 0u32;
                loop {
                    let candidate = format!("user.{stamp:x}{n}");
                    if !list.iter().any(|p| p.id == candidate) {
                        break candidate;
                    }
                    n += 1;
                }
            }
        };
        let profile = LightingProfile {
            id: id.clone(),
            name,
            builtin: false,
            effect: setup.effect,
            brightness: setup.brightness,
            targets: setup.targets,
            sync: setup.sync,
        };
        match list.iter_mut().find(|p| p.id == id) {
            Some(p) => *p = profile,
            None => list.push(profile),
        }
        save_json(&*self.store, PROFILES_KEY, &list)?;
        self.select(Some(id))?;
        self.state(lang)
    }

    pub fn delete_profile(&self, id: &str, lang: Language) -> Result<LightingState, AppError> {
        if id.starts_with("builtin.") {
            return Err(AppError::invalid_input(
                "built-in profiles cannot be deleted",
            ));
        }
        let mut list = self.user_profiles();
        let before = list.len();
        list.retain(|p| p.id != id);
        if list.len() == before {
            return Err(AppError::new(
                ErrorCode::NotFound,
                format!("lighting profile {id} not found"),
            ));
        }
        save_json(&*self.store, PROFILES_KEY, &list)?;
        let selected = load_json::<StoredSelection>(&*self.store, SELECTION_KEY).selected;
        if selected.as_deref() == Some(id) {
            self.select(None)?;
        }
        self.state(lang)
    }

    /// Gives every device its original state back and forgets it.
    fn restore_devices(&self, inner: &mut Inner) -> Result<(), AppError> {
        self.stop_engine(inner);
        if let Ok(mut g) = self.program.lock() {
            *g = None;
        }
        let mut original: Original = load_json(&*self.store, ORIGINAL_KEY);
        let present: BTreeSet<String> = inner
            .devices
            .as_ref()
            .map(|(_, d)| d.iter().map(|d| d.id.clone()).collect())
            .unwrap_or_default();
        let mut failed = Vec::new();
        original.retain(|id, snap| {
            if !present.contains(id) {
                return true; // not connected now: keep it for later
            }
            match self.backend.restore(id, snap) {
                Ok(()) => {
                    self.backend.release(id);
                    false
                }
                Err(e) => {
                    failed.push(e);
                    true
                }
            }
        });
        inner.controlled.clear();
        inner.joined.clear();
        inner.applied = None;
        if original.is_empty() {
            self.store.delete(ORIGINAL_KEY)?;
        } else {
            save_json(&*self.store, ORIGINAL_KEY, &original)?;
        }
        match failed.into_iter().next() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// "Ripristina": the devices as they were before the app changed them.
    pub fn restore(&self, lang: Language) -> Result<LightingState, AppError> {
        let mut inner = self.lock()?;
        self.devices(&mut inner, lang);
        self.restore_devices(&mut inner)?;
        inner.user = None;
        inner.game = None;
        self.select(None)?;
        Ok(self.compose(&mut inner, lang))
    }

    /// App exit: stops the engine, leaves single-color lights steady and hands LampArray
    /// devices back to their own effect. The original state stays stored.
    pub fn shutdown(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        self.stop_engine(&mut inner);
        let program = self.program.lock().ok().and_then(|mut g| g.take());
        if let Some(p) = program.filter(|p| p.effect.is_dynamic()) {
            for t in p.targets.iter().filter(|t| !t.color_capable) {
                let level = f64::from(p.brightness) / 100.0;
                let _ = self
                    .backend
                    .write(&t.id, &LightingFrame::Level(level), true);
            }
        }
        for id in std::mem::take(&mut inner.controlled) {
            self.backend.release(&id);
        }
    }
}

impl Drop for LightingController {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            self.stop_engine(&mut inner);
        }
    }
}

impl ProfileDomain for LightingController {
    fn domain(&self) -> ControlDomainId {
        ControlDomainId::Lighting
    }

    fn profile_refs(&self, lang: Language) -> Vec<ProfileRef> {
        let devices = self
            .lock()
            .map(|mut i| self.devices(&mut i, lang).to_vec())
            .unwrap_or_default();
        self.profiles(lang)
            .into_iter()
            .map(|p| ProfileRef {
                domain: ControlDomainId::Lighting,
                applicable: devices
                    .iter()
                    .any(|d| p.targets.includes(&d.id) && shows(d, &p.effect)),
                id: p.id,
                name: p.name,
                builtin: p.builtin,
            })
            .collect()
    }

    fn apply_game_override(&self, profile_id: &str) -> Result<(), AppError> {
        let lang = self
            .lock()?
            .devices
            .as_ref()
            .map_or(Language::English, |(l, _)| *l);
        let profile = self.find_profile(profile_id, lang)?;
        let mut inner = self.lock()?;
        self.put(
            &mut inner,
            &setup_of(&profile),
            Some(profile.id.clone()),
            lang,
        )?;
        inner.game = Some(profile.id);
        Ok(())
    }

    fn clear_game_override(&self) -> Result<(), AppError> {
        let mut inner = self.lock()?;
        if inner.game.take().is_none() {
            return Ok(());
        }
        let lang = inner
            .devices
            .as_ref()
            .map_or(Language::English, |(l, _)| *l);
        match inner.user.clone() {
            Some((setup, profile_id)) => self.put(&mut inner, &setup, profile_id, lang),
            None => self.restore_devices(&mut inner),
        }
    }
}

#[cfg(test)]
mod tests;
