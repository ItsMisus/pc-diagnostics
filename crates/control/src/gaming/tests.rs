use super::*;
use crate::{MemoryStore, ProfileDomain};
use platform::control::gaming::{ArtPaths, MatchRule, SourceScan};

#[derive(Default)]
struct FakeDomain {
    id: Option<ControlDomainId>,
    fail: Option<ErrorCode>,
    calls: Mutex<Vec<String>>,
}

impl FakeDomain {
    fn new(id: ControlDomainId, fail: Option<ErrorCode>) -> Arc<Self> {
        Arc::new(Self {
            id: Some(id),
            fail,
            calls: Mutex::new(Vec::new()),
        })
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("lock").clone()
    }
}

impl ProfileDomain for FakeDomain {
    fn domain(&self) -> ControlDomainId {
        self.id.unwrap_or(ControlDomainId::Performance)
    }
    fn profile_refs(&self, _lang: Language) -> Vec<ProfileRef> {
        vec![ProfileRef {
            domain: self.domain(),
            id: "p1".into(),
            name: "Profile one".into(),
            builtin: true,
            applicable: true,
        }]
    }
    fn apply_game_override(&self, profile_id: &str) -> Result<(), AppError> {
        self.calls
            .lock()
            .expect("lock")
            .push(format!("apply:{profile_id}"));
        match self.fail {
            Some(code) => Err(AppError::new(code, "fake failure")),
            None => Ok(()),
        }
    }
    fn clear_game_override(&self) -> Result<(), AppError> {
        self.calls.lock().expect("lock").push("clear".into());
        Ok(())
    }
}

struct FakeLibrary {
    games: Vec<ScannedGame>,
    launches: Arc<Mutex<Vec<LaunchTarget>>>,
    fail_launch: bool,
}

impl GameLibrary for FakeLibrary {
    fn scan(&self) -> LibraryScan {
        LibraryScan {
            games: self.games.clone(),
            sources: vec![SourceScan {
                source: GameSource::Steam,
                supported: true,
                found: true,
                location: Some("/steam".into()),
            }],
        }
    }
    fn manual(&self, path: &Path) -> Result<ScannedGame, AppError> {
        if !path.to_string_lossy().ends_with(".app") {
            return Err(AppError::invalid_input("choose an application (.app)"));
        }
        Ok(ScannedGame {
            id: os::manual_id(path),
            title: "Manual".into(),
            source: GameSource::Manual,
            source_id: path.to_string_lossy().into_owned(),
            install_path: Some(path.to_path_buf()),
            size_bytes: None,
            last_played_ms: None,
            art: ArtPaths::default(),
            launch: Some(LaunchTarget::AppBundle(path.to_path_buf())),
            detect: Some(MatchRule::UnderDir(path.to_path_buf())),
        })
    }
    fn read_art(&self, _path: &Path, _kind: GameArtKind) -> Option<(Vec<u8>, &'static str)> {
        Some((vec![1, 2, 3], "image/jpeg"))
    }
    fn launch(&self, target: &LaunchTarget) -> Result<(), AppError> {
        if self.fail_launch {
            return Err(AppError::new(ErrorCode::Platform, "launch failed"));
        }
        self.launches.lock().expect("lock").push(target.clone());
        Ok(())
    }
}

struct FakeLister(Arc<Mutex<Vec<PathBuf>>>);

impl ProcessLister for FakeLister {
    fn running_executables(&mut self) -> Vec<PathBuf> {
        self.0.lock().expect("lock").clone()
    }
}

fn game(id: u32, title: &str) -> ScannedGame {
    let dir = PathBuf::from(format!("/steam/steamapps/common/{title}"));
    ScannedGame {
        id: format!("steam:{id}"),
        title: title.into(),
        source: GameSource::Steam,
        source_id: id.to_string(),
        install_path: Some(dir.clone()),
        size_bytes: Some(10),
        last_played_ms: None,
        art: ArtPaths {
            cover: Some("/steam/cover.jpg".into()),
            ..ArtPaths::default()
        },
        launch: Some(LaunchTarget::Url(format!("steam://rungameid/{id}"))),
        detect: Some(MatchRule::UnderDir(dir)),
    }
}

struct Rig {
    ctl: Arc<GamingController>,
    store: Arc<MemoryStore>,
    running: Arc<Mutex<Vec<PathBuf>>>,
    launches: Arc<Mutex<Vec<LaunchTarget>>>,
    perf: Arc<FakeDomain>,
    cool: Arc<FakeDomain>,
    events: Arc<Mutex<Vec<GamingSessionEvent>>>,
}

fn rig_with(store: Arc<MemoryStore>, fail_launch: bool) -> Rig {
    let running = Arc::new(Mutex::new(Vec::new()));
    let launches = Arc::new(Mutex::new(Vec::new()));
    let perf = FakeDomain::new(ControlDomainId::Performance, None);
    // Cooling fails: it must not stop Performance.
    let cool = FakeDomain::new(ControlDomainId::Cooling, Some(ErrorCode::NotSupported));
    let domains: Vec<SharedDomain> = vec![perf.clone(), cool.clone()];
    let ctl = Arc::new(GamingController::with_library(
        store.clone(),
        domains,
        Box::new(FakeLister(running.clone())),
        Box::new(FakeLibrary {
            games: vec![game(1, "Alpha"), game(2, "Beta")],
            launches: launches.clone(),
            fail_launch,
        }),
    ));
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    if let Ok(mut n) = ctl.notify.lock() {
        *n = Some(Arc::new(move |e| sink.lock().expect("lock").push(e)));
    }
    Rig {
        ctl,
        store,
        running,
        launches,
        perf,
        cool,
        events,
    }
}

fn rig() -> Rig {
    rig_with(Arc::new(MemoryStore::default()), false)
}

fn link_all(r: &Rig, game: &str) {
    r.ctl
        .set_profile(
            GameProfile {
                game_id: game.into(),
                enabled: true,
                performance: Some("p1".into()),
                cooling: Some("p1".into()),
                lighting: None,
            },
            Language::English,
        )
        .expect("set profile");
}

fn run(r: &Rig, title: &str) {
    *r.running.lock().expect("lock") = vec![PathBuf::from(format!(
        "/steam/steamapps/common/{title}/bin/game"
    ))];
}

fn stop_running(r: &Rig) {
    r.running.lock().expect("lock").clear();
}

#[test]
fn state_lists_library_sources_and_linkable_profiles() {
    let r = rig();
    let s = r.ctl.state(Language::Italian, false).expect("state");
    assert_eq!(s.library.len(), 2);
    assert!(s.library[0].has_cover);
    assert!(s.library[0].launch.is_supported());
    assert_eq!(s.linkable.len(), 2);
    assert_eq!(
        s.domains,
        vec![ControlDomainId::Performance, ControlDomainId::Cooling]
    );
    let steam = s
        .sources
        .iter()
        .find(|x| x.source == GameSource::Steam)
        .expect("steam");
    assert_eq!(steam.count, 2);
    assert!(
        s.sources
            .iter()
            .any(|x| x.source == GameSource::Manual && !x.found)
    );
    assert!(s.game_mode.session.is_none());
}

#[test]
fn launch_with_profile_applies_each_domain_and_isolates_failures() {
    // Stored while Lighting was connected; this build has no Lighting domain.
    let store = Arc::new(MemoryStore::default());
    let stored = vec![GameProfile {
        game_id: "steam:1".into(),
        enabled: true,
        performance: Some("p1".into()),
        cooling: Some("p1".into()),
        lighting: Some("p1".into()),
    }];
    save_json(store.as_ref(), KEY_PROFILES, &stored).expect("seed");
    let r = rig_with(store, false);
    let s = r
        .ctl
        .launch("steam:1", true, Language::Italian)
        .expect("launch");
    assert_eq!(
        r.launches.lock().expect("lock").as_slice(),
        &[LaunchTarget::Url("steam://rungameid/1".into())]
    );
    let session = s.game_mode.session.expect("session");
    assert_eq!(session.trigger, GameSessionTrigger::Launch);
    assert_eq!(session.domains.len(), 3);
    let by = |d| {
        session
            .domains
            .iter()
            .find(|x| x.domain == d)
            .expect("domain")
    };
    assert!(by(ControlDomainId::Performance).applied);
    assert_eq!(
        by(ControlDomainId::Performance).profile_name.as_deref(),
        Some("Profile one")
    );
    assert!(!by(ControlDomainId::Cooling).applied);
    assert!(by(ControlDomainId::Cooling).support.reason.is_some());
    let lighting = by(ControlDomainId::Lighting);
    assert!(!lighting.applied, "lighting is not connected");
    assert!(
        lighting
            .support
            .reason
            .as_deref()
            .is_some_and(|t| t.contains("Illuminazione"))
    );
    assert_eq!(r.perf.calls(), vec!["apply:p1"]);
    assert_eq!(r.cool.calls(), vec!["apply:p1"]);

    // The game shows up, then exits: overrides cleared only where applied.
    run(&r, "Alpha");
    r.ctl.poll(now_ms());
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_some_and(|s| s.detected)
    );
    stop_running(&r);
    r.ctl.poll(now_ms());
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_none()
    );
    assert_eq!(r.perf.calls(), vec!["apply:p1", "clear"]);
    assert_eq!(r.cool.calls(), vec!["apply:p1"]);
    let events = r.events.lock().expect("lock").clone();
    assert_eq!(events.len(), 2);
    assert!(events[0].active);
    assert_eq!(events[1].reason, Some(GameSessionEndReason::Exited));
}

#[test]
fn failed_launch_rolls_back_the_overrides() {
    let r = rig_with(Arc::new(MemoryStore::default()), true);
    link_all(&r, "steam:1");
    assert!(r.ctl.launch("steam:1", true, Language::English).is_err());
    assert_eq!(r.perf.calls(), vec!["apply:p1", "clear"]);
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_none()
    );
}

#[test]
fn launched_game_never_seen_ends_after_the_grace_period() {
    let r = rig();
    link_all(&r, "steam:1");
    r.ctl
        .launch("steam:1", true, Language::English)
        .expect("launch");
    r.ctl.poll(now_ms());
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_some()
    );
    r.ctl.poll(now_ms() + LAUNCH_GRACE_MS + 1);
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_none()
    );
    assert_eq!(
        r.events.lock().expect("lock").last().and_then(|e| e.reason),
        Some(GameSessionEndReason::NotDetected)
    );
}

#[test]
fn game_mode_detects_running_games_and_respects_stop() {
    let r = rig();
    link_all(&r, "steam:2");
    run(&r, "Beta");
    r.ctl.poll(now_ms());
    assert!(
        r.perf.calls().is_empty(),
        "nothing happens with game mode off"
    );

    r.ctl.set_game_mode(true, Language::English).expect("mode");
    r.ctl.poll(now_ms());
    let s = r.ctl.state(Language::English, false).expect("state");
    let session = s.game_mode.session.expect("detected");
    assert_eq!(session.game_id, "steam:2");
    assert_eq!(session.trigger, GameSessionTrigger::Detected);
    assert_eq!(r.perf.calls(), vec!["apply:p1"]);

    // Stop while running: cleared and not re-detected until the game exits.
    r.ctl.stop_session(Language::English).expect("stop");
    r.ctl.poll(now_ms());
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_none()
    );
    assert_eq!(r.perf.calls(), vec!["apply:p1", "clear"]);
    stop_running(&r);
    r.ctl.poll(now_ms());
    run(&r, "Beta");
    r.ctl.poll(now_ms());
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_some()
    );

    // Turning game mode off ends a detected session.
    r.ctl
        .set_game_mode(false, Language::English)
        .expect("mode off");
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_none()
    );
    assert_eq!(
        r.perf.calls(),
        vec!["apply:p1", "clear", "apply:p1", "clear"]
    );
}

#[test]
fn plain_launch_is_detected_without_applying_and_hidden_games_are_ignored() {
    let r = rig();
    link_all(&r, "steam:1");
    r.ctl.set_game_mode(true, Language::English).expect("mode");
    r.ctl
        .launch("steam:1", false, Language::English)
        .expect("launch");
    run(&r, "Alpha");
    r.ctl.poll(now_ms());
    let session = r
        .ctl
        .state(Language::English, false)
        .expect("state")
        .game_mode
        .session
        .expect("session");
    assert!(session.domains.is_empty());
    assert!(r.perf.calls().is_empty());
    stop_running(&r);
    r.ctl.poll(now_ms());

    r.ctl
        .set_hidden("steam:2", true, Language::English)
        .expect("hide");
    run(&r, "Beta");
    r.ctl.poll(now_ms());
    assert!(
        r.ctl
            .state(Language::English, false)
            .expect("state")
            .game_mode
            .session
            .is_none()
    );
}

#[test]
fn disabled_profile_applies_nothing() {
    let r = rig();
    r.ctl
        .set_profile(
            GameProfile {
                game_id: "steam:1".into(),
                enabled: false,
                performance: Some("p1".into()),
                ..GameProfile::default()
            },
            Language::English,
        )
        .expect("profile");
    let s = r
        .ctl
        .launch("steam:1", true, Language::English)
        .expect("launch");
    assert!(s.game_mode.session.expect("session").domains.is_empty());
    assert!(r.perf.calls().is_empty());
}

#[test]
fn profile_validation() {
    let r = rig();
    let bad_game = GameProfile {
        game_id: "steam:999".into(),
        enabled: true,
        ..GameProfile::default()
    };
    assert!(r.ctl.set_profile(bad_game, Language::English).is_err());
    let unknown_profile = GameProfile {
        game_id: "steam:1".into(),
        enabled: true,
        performance: Some("nope".into()),
        ..GameProfile::default()
    };
    assert!(
        r.ctl
            .set_profile(unknown_profile, Language::English)
            .is_err()
    );
    let not_connected = GameProfile {
        game_id: "steam:1".into(),
        enabled: true,
        lighting: Some("p1".into()),
        ..GameProfile::default()
    };
    assert_eq!(
        r.ctl
            .set_profile(not_connected, Language::English)
            .map(|_| ())
            .map_err(|e| e.code),
        Err(ErrorCode::InvalidInput)
    );
    // Clearing every link and disabling removes the profile.
    link_all_connected(&r);
    let cleared = GameProfile {
        game_id: "steam:1".into(),
        ..GameProfile::default()
    };
    assert!(
        r.ctl
            .set_profile(cleared, Language::English)
            .expect("clear")
            .profiles
            .is_empty()
    );
}

fn link_all_connected(r: &Rig) {
    r.ctl
        .set_profile(
            GameProfile {
                game_id: "steam:1".into(),
                enabled: true,
                performance: Some("p1".into()),
                ..GameProfile::default()
            },
            Language::English,
        )
        .expect("profile");
}

#[test]
fn manual_entries_persist_and_are_validated() {
    let store = Arc::new(MemoryStore::default());
    let r = rig_with(store.clone(), false);
    assert!(
        r.ctl
            .add_manual("/Applications/Tool.txt", Language::English)
            .is_err()
    );
    let s = r
        .ctl
        .add_manual("/Applications/Game.app", Language::English)
        .expect("add");
    assert_eq!(s.library.len(), 3);
    assert!(
        r.ctl
            .add_manual("/Applications/Game.app", Language::English)
            .is_err(),
        "duplicate"
    );
    let id = os::manual_id(Path::new("/Applications/Game.app"));
    r.ctl
        .set_hidden(&id, true, Language::English)
        .expect("hide");

    // A new controller on the same store sees the entry, the hidden flag and game mode.
    r.ctl.set_game_mode(true, Language::English).expect("mode");
    let again = rig_with(r.store.clone(), false);
    let s = again.ctl.state(Language::English, false).expect("state");
    let manual = s.library.iter().find(|e| e.id == id).expect("manual");
    assert!(manual.hidden);
    assert!(s.game_mode.enabled);
    assert!(s.game_mode.session.is_none(), "startup applies nothing");

    assert!(
        again
            .ctl
            .remove_manual("steam:1", Language::English)
            .is_err(),
        "only manual entries"
    );
    let s = again
        .ctl
        .remove_manual(&id, Language::English)
        .expect("remove");
    assert_eq!(s.library.len(), 2);
    assert!(load_json::<Vec<String>>(store.as_ref(), KEY_HIDDEN).is_empty());
}

#[test]
fn art_is_a_cached_data_url() {
    let r = rig();
    let url = r.ctl.art("steam:1", GameArtKind::Cover).expect("art");
    assert_eq!(url.as_deref(), Some("data:image/jpeg;base64,AQID"));
    assert_eq!(r.ctl.art("steam:1", GameArtKind::Hero).expect("art"), None);
    assert!(r.ctl.art("steam:404", GameArtKind::Cover).is_err());
}

#[test]
fn shutdown_joins_the_watcher_and_clears_overrides() {
    let r = rig();
    link_all(&r, "steam:1");
    r.ctl.start_watcher(|_| {});
    r.ctl
        .launch("steam:1", true, Language::English)
        .expect("launch");
    r.ctl.shutdown();
    assert_eq!(r.perf.calls(), vec!["apply:p1", "clear"]);
    assert!(r.ctl.watcher.lock().expect("lock").is_none());
}
