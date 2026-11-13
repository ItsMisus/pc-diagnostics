//! Gaming controller. Owned by the gaming feature.
//!
//! Keeps the game library (scanned on demand), the user's game profiles (links to profiles
//! of Performance and Cooling), manual entries, hidden ids and game mode (on by default:
//! it switches itself on when a library game starts running).
//!
//! A *session* is the game being played. It starts when the user presses "Gioca con
//! profilo" (trigger `Launch`) or when game mode finds a library game running (`Detected`).
//! Starting one applies the enabled game profile as a game override on each linked domain
//! ([`crate::ProfileDomain::apply_game_override`]); one domain failing never stops the others,
//! and each outcome is reported in the state. Ending it (process gone, Stop, game mode off,
//! app exit) clears the overrides that were applied. Startup applies nothing: a session never
//! survives a restart.
//!
//! Detection is a background thread ([`GamingController::start_watcher`]) that lists running
//! executables every [`POLL_INTERVAL`], but only while game mode is on or a session is active.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use base64::Engine as _;
use models::control::gaming::{
    GameArtKind, GameDomainResult, GameEntry, GameModeState, GameProfile, GameSession,
    GameSessionEndReason, GameSessionTrigger, GameSource, GameSourceStatus, GamingSessionEvent,
    GamingState,
};
use models::control::{ControlDomainId, ProfileRef, Support};
use models::settings::Language;
use models::{AppError, ErrorCode};
use platform::control::gaming::{self as os, LaunchTarget, LibraryScan, ScannedGame};
use serde::{Deserialize, Serialize};

pub use platform::control::gaming::{ProcessLister, SystemProcessLister};

use crate::{DocumentStore, SharedDomain, load_json, save_json};

#[cfg(test)]
mod tests;

pub const KEY_PROFILES: &str = "gaming.profiles";
pub const KEY_MANUAL: &str = "gaming.manual";
pub const KEY_HIDDEN: &str = "gaming.hidden";
/// Game mode on/off. A new key since game mode became automatic and on by default: the old
/// `gaming.mode` (off unless switched on by hand) is not carried over.
pub const KEY_MODE: &str = "gaming.autoMode";

/// How often the watcher looks at the running processes.
pub const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// A launched game that is never seen running ends its session after this long.
pub const LAUNCH_GRACE_MS: u64 = 180_000;
/// Art kept in memory (data URLs).
const ART_CACHE_ENTRIES: usize = 64;

/// The centers a game profile links. Lighting is left out on purpose: a game profile is
/// about how the computer runs (performance, cooling), not about the lights.
const DOMAINS: [ControlDomainId; 2] = [ControlDomainId::Performance, ControlDomainId::Cooling];

/// Library, art and launch access (the OS in the app, fakes in tests).
pub trait GameLibrary: Send + Sync {
    fn scan(&self) -> LibraryScan;
    fn manual(&self, path: &Path) -> Result<ScannedGame, AppError>;
    fn read_art(&self, path: &Path, kind: GameArtKind) -> Option<(Vec<u8>, &'static str)>;
    fn launch(&self, target: &LaunchTarget) -> Result<(), AppError>;
}

/// [`GameLibrary`] of this computer (`platform::control::gaming`).
pub struct SystemLibrary;

impl GameLibrary for SystemLibrary {
    fn scan(&self) -> LibraryScan {
        os::scan_library()
    }
    fn manual(&self, path: &Path) -> Result<ScannedGame, AppError> {
        os::manual_game(path)
    }
    fn read_art(&self, path: &Path, kind: GameArtKind) -> Option<(Vec<u8>, &'static str)> {
        os::read_art(path, kind)
    }
    fn launch(&self, target: &LaunchTarget) -> Result<(), AppError> {
        os::launch(target)
    }
}

/// A manual entry as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualGameDoc {
    pub path: String,
    pub title: String,
    pub added_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModeDoc {
    enabled: bool,
}

/// Game mode is on until the user turns it off: it switches itself on when a game starts.
impl Default for ModeDoc {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone)]
enum Outcome {
    Applied,
    Failed(AppError),
    /// The domain is not connected to Gaming in this build.
    NotConnected,
}

#[derive(Debug, Clone)]
struct DomainOutcome {
    domain: ControlDomainId,
    profile_id: String,
    outcome: Outcome,
}

#[derive(Debug, Clone)]
struct ActiveSession {
    game_id: String,
    title: String,
    since: u64,
    trigger: GameSessionTrigger,
    detected: bool,
    outcomes: Vec<DomainOutcome>,
}

#[derive(Default)]
struct Inner {
    scan: Option<LibraryScan>,
    scanned_at: u64,
    manual_games: Vec<ScannedGame>,
    manual: Vec<ManualGameDoc>,
    profiles: Vec<GameProfile>,
    hidden: BTreeSet<String>,
    mode: bool,
    session: Option<ActiveSession>,
    /// A game the user stopped while it was running: not detected again until it exits.
    dismissed: Option<String>,
    /// Games started with plain "Gioca": detected without applying their profile.
    plain_launches: BTreeMap<String, u64>,
}

impl Inner {
    fn games(&self) -> impl Iterator<Item = &ScannedGame> {
        self.scan
            .iter()
            .flat_map(|s| s.games.iter())
            .chain(self.manual_games.iter())
    }

    fn game(&self, id: &str) -> Option<&ScannedGame> {
        self.games().find(|g| g.id == id)
    }
}

/// Data URLs by (game id, kind); `None` = the source has no such art.
type ArtCache = HashMap<(String, GameArtKind), Option<Arc<str>>>;

type Notify = Arc<dyn Fn(GamingSessionEvent) + Send + Sync>;

struct Watcher {
    stop: Arc<(Mutex<bool>, Condvar)>,
    handle: JoinHandle<()>,
}

pub struct GamingController {
    store: Arc<dyn DocumentStore>,
    domains: Vec<SharedDomain>,
    library: Box<dyn GameLibrary>,
    lister: Mutex<Box<dyn ProcessLister>>,
    inner: Mutex<Inner>,
    art: Mutex<ArtCache>,
    notify: Mutex<Option<Notify>>,
    watcher: Mutex<Option<Watcher>>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn domain_name(domain: ControlDomainId, lang: Language) -> &'static str {
    match domain {
        ControlDomainId::Performance => lang.pick("Prestazioni", "Performance"),
        ControlDomainId::Cooling => lang.pick("Raffreddamento", "Cooling"),
        ControlDomainId::Lighting => lang.pick("Illuminazione", "Lighting"),
    }
}

impl GamingController {
    /// The controller of this computer. `domains`: Performance and Cooling (any subset; a
    /// missing one is reported as not connected; any other domain is ignored). `process_lister`: running
    /// executables for game detection (`SystemProcessLister` in the app).
    pub fn new(
        store: Arc<dyn DocumentStore>,
        domains: Vec<SharedDomain>,
        process_lister: Box<dyn ProcessLister>,
    ) -> Self {
        Self::with_library(store, domains, process_lister, Box::new(SystemLibrary))
    }

    pub fn with_library(
        store: Arc<dyn DocumentStore>,
        domains: Vec<SharedDomain>,
        process_lister: Box<dyn ProcessLister>,
        library: Box<dyn GameLibrary>,
    ) -> Self {
        let inner = Inner {
            manual: load_json(store.as_ref(), KEY_MANUAL),
            profiles: load_json(store.as_ref(), KEY_PROFILES),
            hidden: load_json::<Vec<String>>(store.as_ref(), KEY_HIDDEN)
                .into_iter()
                .collect(),
            mode: load_json::<ModeDoc>(store.as_ref(), KEY_MODE).enabled,
            ..Inner::default()
        };
        Self {
            store,
            domains,
            library,
            lister: Mutex::new(process_lister),
            inner: Mutex::new(inner),
            art: Mutex::new(HashMap::new()),
            notify: Mutex::new(None),
            watcher: Mutex::new(None),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, AppError> {
        self.inner
            .lock()
            .map_err(|_| AppError::internal("gaming state lock poisoned"))
    }

    fn domain(&self, id: ControlDomainId) -> Option<&SharedDomain> {
        self.domains.iter().find(|d| d.domain() == id)
    }

    fn rescan(&self, inner: &mut Inner) {
        inner.scan = Some(self.library.scan());
        inner.scanned_at = now_ms();
        inner.manual_games = inner
            .manual
            .iter()
            .map(|m| {
                let path = PathBuf::from(&m.path);
                self.library
                    .manual(&path)
                    .unwrap_or_else(|_| os::missing_manual_game(&path, &m.title))
            })
            .collect();
        if let Ok(mut art) = self.art.lock() {
            art.clear();
        }
    }

    fn ensure_scanned(&self, inner: &mut Inner) {
        if inner.scan.is_none() {
            self.rescan(inner);
        }
    }

    // ---- state -------------------------------------------------------------------------

    /// The whole state. `refresh` rescans the library (otherwise the first call scans).
    pub fn state(&self, lang: Language, refresh: bool) -> Result<GamingState, AppError> {
        let mut inner = self.lock()?;
        if refresh {
            self.rescan(&mut inner);
        } else {
            self.ensure_scanned(&mut inner);
        }
        Ok(self.build_state(&inner, lang))
    }

    fn build_state(&self, inner: &Inner, lang: Language) -> GamingState {
        let linkable: Vec<ProfileRef> = self
            .domains
            .iter()
            .filter(|d| DOMAINS.contains(&d.domain()))
            .flat_map(|d| d.profile_refs(lang))
            .collect();
        let library: Vec<GameEntry> = inner
            .games()
            .map(|g| entry(g, inner.hidden.contains(&g.id), lang))
            .collect();
        let count = |source: GameSource| {
            u32::try_from(
                library
                    .iter()
                    .filter(|e| e.source == source && !e.hidden)
                    .count(),
            )
            .unwrap_or(u32::MAX)
        };
        let mut sources: Vec<GameSourceStatus> = inner
            .scan
            .iter()
            .flat_map(|s| s.sources.iter())
            .map(|s| GameSourceStatus {
                source: s.source,
                support: if s.supported {
                    Support::supported()
                } else {
                    Support::unsupported(lang.pick(
                        "Questa fonte non esiste su questo sistema operativo.",
                        "This source does not exist on this operating system.",
                    ))
                },
                found: s.found,
                count: count(s.source),
                location: s
                    .location
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
            })
            .collect();
        sources.push(GameSourceStatus {
            source: GameSource::Manual,
            support: Support::supported(),
            found: !inner.manual.is_empty(),
            count: count(GameSource::Manual),
            location: None,
        });
        let session = inner
            .session
            .as_ref()
            .map(|s| session_view(s, &linkable, lang));
        GamingState {
            library,
            sources,
            profiles: inner.profiles.clone(),
            linkable,
            domains: self
                .domains
                .iter()
                .map(|d| d.domain())
                .filter(|d| DOMAINS.contains(d))
                .collect(),
            game_mode: GameModeState {
                enabled: inner.mode,
                session,
            },
            scanned_at: inner.scanned_at,
        }
    }

    // ---- art ---------------------------------------------------------------------------

    /// Art of a library entry as a `data:` URL, `None` when the source has none.
    pub fn art(&self, id: &str, kind: GameArtKind) -> Result<Option<String>, AppError> {
        let key = (id.to_owned(), kind);
        if let Some(hit) = self.art.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return Ok(hit.map(|s| s.to_string()));
        }
        let path = {
            let mut inner = self.lock()?;
            self.ensure_scanned(&mut inner);
            let game = inner
                .game(id)
                .ok_or_else(|| AppError::new(ErrorCode::NotFound, "game not in the library"))?;
            game.art.get(kind).map(Path::to_path_buf)
        };
        let url: Option<Arc<str>> =
            path.and_then(|p| self.library.read_art(&p, kind))
                .map(|(bytes, mime)| {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                    Arc::from(format!("data:{mime};base64,{b64}"))
                });
        if let Ok(mut cache) = self.art.lock() {
            if cache.len() >= ART_CACHE_ENTRIES {
                cache.clear();
            }
            cache.insert(key, url.clone());
        }
        Ok(url.map(|s| s.to_string()))
    }

    // ---- library edits -----------------------------------------------------------------

    /// Adds a user-picked app / executable (validated by the platform layer).
    pub fn add_manual(&self, path: &str, lang: Language) -> Result<GamingState, AppError> {
        let game = self.library.manual(Path::new(path))?;
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        let duplicate = inner.games().any(|g| {
            g.id == game.id || (g.install_path.is_some() && g.install_path == game.install_path)
        });
        if duplicate {
            return Err(AppError::invalid_input(
                "this game is already in the library",
            ));
        }
        let stored = game
            .install_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or_else(|| AppError::internal("manual entry without a path"))?;
        let mut manual = inner.manual.clone();
        manual.push(ManualGameDoc {
            path: stored,
            title: game.title.clone(),
            added_at: now_ms(),
        });
        save_json(self.store.as_ref(), KEY_MANUAL, &manual)?;
        inner.manual = manual;
        inner.manual_games.push(game);
        Ok(self.build_state(&inner, lang))
    }

    /// Removes a manual entry with its profile and hidden flag.
    pub fn remove_manual(&self, id: &str, lang: Language) -> Result<GamingState, AppError> {
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        let game = inner
            .manual_games
            .iter()
            .find(|g| g.id == id)
            .cloned()
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "no manual entry with this id"))?;
        let path = game
            .install_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        let manual: Vec<ManualGameDoc> = inner
            .manual
            .iter()
            .filter(|m| Some(&m.path) != path.as_ref())
            .cloned()
            .collect();
        save_json(self.store.as_ref(), KEY_MANUAL, &manual)?;
        inner.manual = manual;
        inner.manual_games.retain(|g| g.id != id);
        if inner.profiles.iter().any(|p| p.game_id == id) {
            let profiles: Vec<GameProfile> = inner
                .profiles
                .iter()
                .filter(|p| p.game_id != id)
                .cloned()
                .collect();
            save_json(self.store.as_ref(), KEY_PROFILES, &profiles)?;
            inner.profiles = profiles;
        }
        if inner.hidden.remove(id) {
            save_json(self.store.as_ref(), KEY_HIDDEN, &inner.hidden)?;
        }
        let ended = if inner.session.as_ref().is_some_and(|s| s.game_id == id) {
            self.end_session(&mut inner, GameSessionEndReason::Stopped)
        } else {
            None
        };
        let state = self.build_state(&inner, lang);
        drop(inner);
        self.emit(ended);
        Ok(state)
    }

    pub fn set_hidden(
        &self,
        id: &str,
        hidden: bool,
        lang: Language,
    ) -> Result<GamingState, AppError> {
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        if inner.game(id).is_none() && !inner.hidden.contains(id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "game not in the library",
            ));
        }
        let mut set = inner.hidden.clone();
        let changed = if hidden {
            set.insert(id.to_owned())
        } else {
            set.remove(id)
        };
        if changed {
            save_json(self.store.as_ref(), KEY_HIDDEN, &set)?;
            inner.hidden = set;
        }
        Ok(self.build_state(&inner, lang))
    }

    /// Stores a game profile. Linked ids must be profiles of connected domains.
    pub fn set_profile(
        &self,
        mut profile: GameProfile,
        lang: Language,
    ) -> Result<GamingState, AppError> {
        // Game profiles no longer link Lighting (see `DOMAINS`).
        profile.lighting = None;
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        if inner.game(&profile.game_id).is_none() {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "game not in the library",
            ));
        }
        for d in DOMAINS {
            let Some(pid) = profile.profile_for(d) else {
                continue;
            };
            let domain = self
                .domain(d)
                .ok_or_else(|| AppError::invalid_input(format!("{d:?} center is not connected")))?;
            if !domain.profile_refs(lang).iter().any(|r| r.id == pid) {
                return Err(AppError::new(
                    ErrorCode::NotFound,
                    format!("{d:?} has no profile {pid}"),
                ));
            }
        }
        let mut profiles: Vec<GameProfile> = inner
            .profiles
            .iter()
            .filter(|p| p.game_id != profile.game_id)
            .cloned()
            .collect();
        if profile.enabled || profile.has_links() {
            profiles.push(profile);
        }
        profiles.sort_by(|a, b| a.game_id.cmp(&b.game_id));
        save_json(self.store.as_ref(), KEY_PROFILES, &profiles)?;
        inner.profiles = profiles;
        Ok(self.build_state(&inner, lang))
    }

    // ---- game mode and sessions --------------------------------------------------------

    /// Turns detection on or off. Off ends a detected session (and its overrides).
    pub fn set_game_mode(&self, enabled: bool, lang: Language) -> Result<GamingState, AppError> {
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        save_json(self.store.as_ref(), KEY_MODE, &ModeDoc { enabled })?;
        inner.mode = enabled;
        let ended = if !enabled
            && inner
                .session
                .as_ref()
                .is_some_and(|s| s.trigger == GameSessionTrigger::Detected)
        {
            self.end_session(&mut inner, GameSessionEndReason::Stopped)
        } else {
            None
        };
        let state = self.build_state(&inner, lang);
        drop(inner);
        self.emit(ended);
        self.wake_watcher();
        Ok(state)
    }

    /// Starts a library entry; `with_profile` applies its game profile first.
    pub fn launch(
        &self,
        id: &str,
        with_profile: bool,
        lang: Language,
    ) -> Result<GamingState, AppError> {
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        let game = inner
            .game(id)
            .cloned()
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "game not in the library"))?;
        let target = game
            .launch
            .clone()
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "the game's file is missing"))?;
        let mut events = Vec::new();
        if with_profile {
            inner.plain_launches.remove(id);
            if inner.session.is_some() {
                events.extend(self.end_session(&mut inner, GameSessionEndReason::Stopped));
            }
            if inner.dismissed.as_deref() == Some(id) {
                inner.dismissed = None;
            }
            self.begin_session(&mut inner, &game, GameSessionTrigger::Launch, false);
            events.push(GamingSessionEvent {
                game_id: game.id.clone(),
                title: game.title.clone(),
                active: true,
                reason: None,
            });
        } else {
            inner.plain_launches.insert(id.to_owned(), now_ms());
        }
        if let Err(e) = self.library.launch(&target) {
            if with_profile {
                events.extend(self.end_session(&mut inner, GameSessionEndReason::Stopped));
            }
            inner.plain_launches.remove(id);
            drop(inner);
            for ev in events {
                self.emit(Some(ev));
            }
            return Err(e);
        }
        let state = self.build_state(&inner, lang);
        drop(inner);
        for ev in events {
            self.emit(Some(ev));
        }
        self.wake_watcher();
        Ok(state)
    }

    /// "Ripristina": ends the session and clears the overrides. A game still running is not
    /// detected again until it exits.
    pub fn stop_session(&self, lang: Language) -> Result<GamingState, AppError> {
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        let ended = self.end_session(&mut inner, GameSessionEndReason::Stopped);
        if let Some(ev) = &ended {
            inner.dismissed = Some(ev.game_id.clone());
        }
        let state = self.build_state(&inner, lang);
        drop(inner);
        self.emit(ended);
        Ok(state)
    }

    fn begin_session(
        &self,
        inner: &mut Inner,
        game: &ScannedGame,
        trigger: GameSessionTrigger,
        detected: bool,
    ) {
        let apply = !(trigger == GameSessionTrigger::Detected
            && inner.plain_launches.contains_key(&game.id));
        let profile = inner
            .profiles
            .iter()
            .find(|p| p.game_id == game.id && p.enabled)
            .filter(|_| apply)
            .cloned();
        let mut outcomes = Vec::new();
        if let Some(profile) = profile {
            for d in DOMAINS {
                let Some(pid) = profile.profile_for(d) else {
                    continue;
                };
                let outcome = match self.domain(d) {
                    None => Outcome::NotConnected,
                    Some(domain) => match domain.apply_game_override(pid) {
                        Ok(()) => Outcome::Applied,
                        Err(e) => {
                            tracing::warn!(domain = ?d, error = %e.message, "game override failed");
                            Outcome::Failed(e)
                        }
                    },
                };
                outcomes.push(DomainOutcome {
                    domain: d,
                    profile_id: pid.to_owned(),
                    outcome,
                });
            }
        }
        tracing::info!(game = %game.id, ?trigger, applied = outcomes.iter().filter(|o| matches!(o.outcome, Outcome::Applied)).count(), "game session started");
        inner.session = Some(ActiveSession {
            game_id: game.id.clone(),
            title: game.title.clone(),
            since: now_ms(),
            trigger,
            detected,
            outcomes,
        });
    }

    /// Ends the session (if any) and clears the overrides it applied.
    fn end_session(
        &self,
        inner: &mut Inner,
        reason: GameSessionEndReason,
    ) -> Option<GamingSessionEvent> {
        let session = inner.session.take()?;
        for o in &session.outcomes {
            if !matches!(o.outcome, Outcome::Applied) {
                continue;
            }
            if let Some(domain) = self.domain(o.domain)
                && let Err(e) = domain.clear_game_override()
            {
                tracing::warn!(domain = ?o.domain, error = %e.message, "clearing game override failed");
            }
        }
        inner.plain_launches.remove(&session.game_id);
        tracing::info!(game = %session.game_id, ?reason, "game session ended");
        Some(GamingSessionEvent {
            game_id: session.game_id,
            title: session.title,
            active: false,
            reason: Some(reason),
        })
    }

    /// One detection pass at `now` (Unix ms). Lists processes only when game mode is on or
    /// a session is active. Public for tests; the watcher calls it every [`POLL_INTERVAL`].
    pub fn poll(&self, now: u64) {
        let events = match self.poll_inner(now) {
            Ok(events) => events,
            Err(e) => {
                tracing::warn!(error = %e.message, "game detection failed");
                Vec::new()
            }
        };
        for ev in events {
            self.emit(Some(ev));
        }
    }

    fn poll_inner(&self, now: u64) -> Result<Vec<GamingSessionEvent>, AppError> {
        {
            let inner = self.lock()?;
            if !inner.mode && inner.session.is_none() && inner.dismissed.is_none() {
                return Ok(Vec::new());
            }
        }
        let exes = self
            .lister
            .lock()
            .map_err(|_| AppError::internal("process lister lock poisoned"))?
            .running_executables();
        let mut inner = self.lock()?;
        self.ensure_scanned(&mut inner);
        let running = |g: &ScannedGame| {
            g.detect
                .as_ref()
                .is_some_and(|rule| exes.iter().any(|e| os::matches(rule, e)))
        };
        let mut events = Vec::new();

        if let Some(session) = inner.session.clone() {
            let is_running = inner.game(&session.game_id).is_some_and(running);
            if is_running {
                if let Some(s) = inner.session.as_mut() {
                    s.detected = true;
                }
            } else if session.detected || session.trigger == GameSessionTrigger::Detected {
                events.extend(self.end_session(&mut inner, GameSessionEndReason::Exited));
            } else if now.saturating_sub(session.since) > LAUNCH_GRACE_MS {
                events.extend(self.end_session(&mut inner, GameSessionEndReason::NotDetected));
            }
        }

        if let Some(id) = inner.dismissed.clone()
            && !inner.game(&id).is_some_and(running)
        {
            inner.dismissed = None;
        }
        // A plain launch that never showed up within the grace period is forgotten.
        let session_game = inner.session.as_ref().map(|s| s.game_id.clone());
        inner.plain_launches.retain(|id, at| {
            session_game.as_deref() == Some(id.as_str())
                || now.saturating_sub(*at) <= LAUNCH_GRACE_MS
        });

        if inner.mode && inner.session.is_none() {
            let found = inner
                .games()
                .filter(|g| !inner.hidden.contains(&g.id))
                .filter(|g| inner.dismissed.as_deref() != Some(g.id.as_str()))
                .find(|g| running(g))
                .cloned();
            if let Some(game) = found {
                self.begin_session(&mut inner, &game, GameSessionTrigger::Detected, true);
                events.push(GamingSessionEvent {
                    game_id: game.id,
                    title: game.title,
                    active: true,
                    reason: None,
                });
            }
        }
        Ok(events)
    }

    fn emit(&self, event: Option<GamingSessionEvent>) {
        let Some(event) = event else { return };
        let notify = self.notify.lock().ok().and_then(|n| n.clone());
        if let Some(notify) = notify {
            notify(event);
        }
    }

    // ---- watcher -----------------------------------------------------------------------

    /// Starts the detection thread; `notify` receives session start/end events. Calling it
    /// again replaces the callback and keeps the running thread.
    pub fn start_watcher(
        self: &Arc<Self>,
        notify: impl Fn(GamingSessionEvent) + Send + Sync + 'static,
    ) {
        if let Ok(mut n) = self.notify.lock() {
            *n = Some(Arc::new(notify));
        }
        let Ok(mut watcher) = self.watcher.lock() else {
            return;
        };
        if watcher.is_some() {
            return;
        }
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let weak: Weak<Self> = Arc::downgrade(self);
        let flag = Arc::clone(&stop);
        let spawned = std::thread::Builder::new()
            .name("gaming-watcher".into())
            .spawn(move || {
                let (lock, cvar) = &*flag;
                loop {
                    {
                        let Ok(guard) = lock.lock() else { return };
                        if *guard {
                            return;
                        }
                        let Ok((guard, _)) = cvar.wait_timeout(guard, POLL_INTERVAL) else {
                            return;
                        };
                        if *guard {
                            return;
                        }
                    }
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    controller.poll(now_ms());
                }
            });
        match spawned {
            Ok(handle) => *watcher = Some(Watcher { stop, handle }),
            Err(e) => tracing::warn!(error = %e, "could not start the game watcher"),
        }
    }

    /// Makes the watcher poll soon (it re-checks after the wait in any case).
    fn wake_watcher(&self) {
        if let Ok(w) = self.watcher.lock()
            && let Some(w) = w.as_ref()
        {
            w.stop.1.notify_all();
        }
    }

    /// Stops and joins the watcher, then ends the session clearing its overrides.
    pub fn shutdown(&self) {
        let watcher = self.watcher.lock().ok().and_then(|mut w| w.take());
        if let Some(w) = watcher {
            if let Ok(mut stop) = w.stop.0.lock() {
                *stop = true;
            }
            w.stop.1.notify_all();
            if w.handle.join().is_err() {
                tracing::warn!("game watcher panicked");
            }
        }
        if let Ok(mut inner) = self.inner.lock() {
            let _ = self.end_session(&mut inner, GameSessionEndReason::Shutdown);
        }
    }
}

fn entry(g: &ScannedGame, hidden: bool, lang: Language) -> GameEntry {
    GameEntry {
        id: g.id.clone(),
        title: g.title.clone(),
        source: g.source,
        install_path: g
            .install_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        size_bytes: g.size_bytes,
        last_played: g.last_played_ms,
        has_cover: g.art.cover.is_some(),
        has_hero: g.art.hero.is_some(),
        has_logo: g.art.logo.is_some(),
        has_icon: g.art.icon.is_some(),
        hidden,
        launch: if g.launch.is_some() {
            Support::supported()
        } else {
            Support::unsupported(lang.pick(
                "Il file non c'è più: è stato spostato o eliminato.",
                "The file is gone: it was moved or deleted.",
            ))
        },
        source_id: g.source_id.clone(),
    }
}

fn outcome_support(o: &DomainOutcome, lang: Language) -> Support {
    let name = domain_name(o.domain, lang);
    match &o.outcome {
        Outcome::Applied => Support::supported(),
        Outcome::NotConnected => Support::unsupported(match lang {
            Language::Italian => {
                format!("Il centro {name} non è collegato a Gioco in questa versione.")
            }
            Language::English => {
                format!("The {name} center is not connected to Gaming in this version.")
            }
        }),
        Outcome::Failed(e) => Support::unsupported(match (e.code, lang) {
            (ErrorCode::NotFound, Language::Italian) => {
                format!("Il profilo non esiste più in {name}.")
            }
            (ErrorCode::NotFound, Language::English) => {
                format!("The profile no longer exists in {name}.")
            }
            (ErrorCode::NotSupported, Language::Italian) => {
                "Nessuna impostazione del profilo si può applicare su questo computer.".to_owned()
            }
            (ErrorCode::NotSupported, Language::English) => {
                "None of the profile's settings can be applied on this computer.".to_owned()
            }
            (ErrorCode::PermissionDenied, Language::Italian) => {
                "Il sistema non ha permesso la modifica.".to_owned()
            }
            (ErrorCode::PermissionDenied, Language::English) => {
                "The system did not allow the change.".to_owned()
            }
            (_, Language::Italian) => format!("{name} non ha potuto applicare il profilo."),
            (_, Language::English) => format!("{name} could not apply the profile."),
        }),
    }
}

fn session_view(s: &ActiveSession, linkable: &[ProfileRef], lang: Language) -> GameSession {
    GameSession {
        game_id: s.game_id.clone(),
        title: s.title.clone(),
        since: s.since,
        trigger: s.trigger,
        detected: s.detected,
        domains: s
            .outcomes
            .iter()
            .map(|o| GameDomainResult {
                domain: o.domain,
                profile_id: o.profile_id.clone(),
                profile_name: linkable
                    .iter()
                    .find(|r| r.domain == o.domain && r.id == o.profile_id)
                    .map(|r| r.name.clone()),
                applied: matches!(o.outcome, Outcome::Applied),
                support: outcome_support(o, lang),
            })
            .collect(),
    }
}
