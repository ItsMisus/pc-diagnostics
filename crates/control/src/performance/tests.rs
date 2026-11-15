//! State machine tests of the performance controller with a fake OS and `MemoryStore`.

use std::sync::Mutex;

use models::control::SupportState;
use models::control::performance::PerformanceBoostMode;

use super::*;
use crate::MemoryStore;

/// A fake OS: Windows-like (everything writable) unless configured otherwise.
struct Fake {
    os: Mutex<OsPowerState>,
    writes: Mutex<
        Vec<(
            PowerSupply,
            PerformanceOsSupply,
            Option<PerformanceOsSupply>,
        )>,
    >,
    keep: Mutex<PerformanceKeepAwake>,
    deny: Mutex<Vec<PerformanceControl>>,
}

fn supply_values(mode: PerformanceMode, max: u8) -> PerformanceOsSupply {
    PerformanceOsSupply {
        mode: Some(mode),
        processor_max_percent: Some(max),
        processor_min_percent: Some(5),
        boost_mode: Some(PerformanceBoostMode::Aggressive),
        display_off_seconds: Some(600),
        sleep_seconds: Some(1800),
    }
}

impl Fake {
    fn windows() -> Arc<Self> {
        Arc::new(Self {
            os: Mutex::new(OsPowerState {
                supply: PowerSupply::Ac,
                has_battery: true,
                ac: supply_values(PerformanceMode::Balanced, 100),
                battery: Some(PerformanceOsSupply {
                    mode: None,
                    ..supply_values(PerformanceMode::Balanced, 90)
                }),
                scheme_id: Some("381B4222-F694-41F0-9685-FF5BB260DF2E".into()),
                scheme_name: Some("Balanced".into()),
                access: platform::control::performance::PerformanceAccess {
                    mode: Access::Writable,
                    keep_awake: Access::Writable,
                    processor_state: Access::Writable,
                    boost_mode: Access::Writable,
                    display_timeout: Access::Writable,
                    sleep_timeout: Access::Writable,
                },
            }),
            writes: Mutex::new(Vec::new()),
            keep: Mutex::new(PerformanceKeepAwake::Off),
            deny: Mutex::new(Vec::new()),
        })
    }

    fn mac() -> Arc<Self> {
        let f = Self::windows();
        {
            let mut os = f.os.lock().expect("lock");
            os.scheme_id = None;
            os.scheme_name = None;
            os.access.mode = Access::NeedsAdmin;
            os.access.processor_state = Access::NotOnThisOs;
            os.access.boost_mode = Access::NotOnThisOs;
            os.access.display_timeout = Access::NeedsAdmin;
            os.access.sleep_timeout = Access::NeedsAdmin;
        }
        f
    }

    fn set_supply(&self, supply: PowerSupply) {
        self.os.lock().expect("lock").supply = supply;
    }

    fn writes(
        &self,
    ) -> Vec<(
        PowerSupply,
        PerformanceOsSupply,
        Option<PerformanceOsSupply>,
    )> {
        self.writes.lock().expect("lock").clone()
    }
}

impl PerformanceBackend for Fake {
    fn read(&self) -> OsPowerState {
        self.os.lock().expect("lock").clone()
    }

    fn current_supply(&self) -> (PowerSupply, bool) {
        let os = self.os.lock().expect("lock");
        (os.supply, os.has_battery)
    }

    fn write(
        &self,
        _scheme_id: Option<&str>,
        current: PowerSupply,
        ac: &PerformanceOsSupply,
        battery: Option<&PerformanceOsSupply>,
    ) -> Vec<WriteFailure> {
        self.writes
            .lock()
            .expect("lock")
            .push((current, ac.clone(), battery.cloned()));
        let deny = self.deny.lock().expect("lock").clone();
        deny.into_iter()
            .map(|control| WriteFailure {
                control,
                denied: true,
                detail: "ERROR_ACCESS_DENIED".into(),
            })
            .collect()
    }

    fn set_keep_awake(&self, mode: PerformanceKeepAwake) -> Result<(), AppError> {
        *self.keep.lock().expect("lock") = mode;
        Ok(())
    }

    fn keep_awake(&self) -> PerformanceKeepAwake {
        *self.keep.lock().expect("lock")
    }

    fn open_settings(&self) -> Result<(), AppError> {
        Ok(())
    }
}

fn controller(fake: &Arc<Fake>, store: &Arc<MemoryStore>) -> PerformanceController {
    PerformanceController::new(fake.clone(), store.clone())
}

fn input(name: &str, ac_keep: PerformanceKeepAwake) -> PerformanceProfileInput {
    PerformanceProfileInput {
        id: None,
        name: name.into(),
        ac: PerformanceSupplySettings {
            keep_awake: ac_keep,
            processor_max_percent: Some(80),
            ..PerformanceSupplySettings::mode_only(PerformanceMode::HighPerformance)
        },
        battery: PerformanceSupplySettings {
            sleep_seconds: Some(300),
            ..PerformanceSupplySettings::mode_only(PerformanceMode::Silent)
        },
    }
}

const IT: Language = Language::Italian;

#[test]
fn builtins_are_listed_and_named_in_the_language() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    let it = c.state(IT).expect("state");
    assert_eq!(
        it.profiles
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Silenzioso", "Bilanciato", "Prestazioni elevate"]
    );
    assert!(it.profiles.iter().all(|p| p.builtin));
    let en = c.state(Language::English).expect("state");
    assert_eq!(en.profiles[2].name, "High performance");
    assert_eq!(it.selected_profile_id, None);
    assert!(!it.can_restore);
    assert!(fake.writes().is_empty(), "reading never writes");
}

#[test]
fn applying_stores_the_original_first_and_writes_the_current_supply_mode() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    let s = c.apply_profile(PERFORMANCE_ID, IT).expect("apply");
    assert_eq!(s.selected_profile_id.as_deref(), Some(PERFORMANCE_ID));
    assert!(s.can_restore);
    let original: Option<Original> = load_json(store.as_ref(), ORIGINAL_KEY);
    let original = original.expect("original stored");
    assert_eq!(original.ac.processor_max_percent, Some(100));
    assert_eq!(
        original.battery.as_ref().and_then(|b| b.mode),
        Some(PerformanceMode::Balanced)
    );
    let writes = fake.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].1.mode, Some(PerformanceMode::HighPerformance));
    // Built-ins leave the advanced values alone.
    assert_eq!(writes[0].1.processor_max_percent, None);
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::System);
    let sel: Selection = load_json(store.as_ref(), SELECTION_KEY);
    assert_eq!(sel.profile_id.as_deref(), Some(PERFORMANCE_ID));
}

#[test]
fn a_second_apply_keeps_the_first_original() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    c.apply_profile(SILENT_ID, IT).expect("apply");
    fake.os.lock().expect("lock").ac.processor_max_percent = Some(42);
    c.apply_profile(BALANCED_ID, IT).expect("apply");
    let original: Option<Original> = load_json(store.as_ref(), ORIGINAL_KEY);
    assert_eq!(
        original.expect("original").ac.processor_max_percent,
        Some(100)
    );
}

#[test]
fn supply_change_reapplies_the_other_settings() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    let s = c
        .save_profile(input("Notte", PerformanceKeepAwake::Display), IT)
        .expect("save");
    let id = s.profiles.last().expect("profile").id.clone();
    c.apply_profile(&id, IT).expect("apply");
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::Display);
    let first = fake.writes();
    assert_eq!(first[0].0, PowerSupply::Ac);
    assert_eq!(first[0].1.processor_max_percent, Some(80));
    assert_eq!(first[0].2.as_ref().and_then(|b| b.sleep_seconds), Some(300));

    c.tick();
    assert_eq!(fake.writes().len(), 1, "no change, no write");
    fake.set_supply(PowerSupply::Battery);
    c.tick();
    let writes = fake.writes();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[1].0, PowerSupply::Battery);
    assert_eq!(
        writes[1].2.as_ref().and_then(|b| b.mode),
        Some(PerformanceMode::Silent)
    );
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::Off);
}

#[test]
fn restore_puts_the_original_back_and_clears_everything() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    c.apply_profile(PERFORMANCE_ID, IT).expect("apply");
    let s = c.restore_original(IT).expect("restore");
    assert_eq!(s.selected_profile_id, None);
    assert!(!s.can_restore);
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::Off);
    let last = fake.writes().pop().expect("restore write");
    assert_eq!(last.1.processor_max_percent, Some(100));
    assert_eq!(last.1.mode, Some(PerformanceMode::Balanced));
    assert_eq!(store.load(ORIGINAL_KEY).expect("load"), None);
    c.tick();
    assert_eq!(fake.writes().len(), 2, "nothing active after restore");
}

#[test]
fn state_survives_a_restart() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let id = {
        let c = controller(&fake, &store);
        let s = c
            .save_profile(input("Studio", PerformanceKeepAwake::System), IT)
            .expect("save");
        let id = s.profiles.last().expect("p").id.clone();
        c.apply_profile(&id, IT).expect("apply");
        c.shutdown();
        id
    };
    assert_eq!(
        fake.keep_awake(),
        PerformanceKeepAwake::Off,
        "released on shutdown"
    );
    let c = controller(&fake, &store);
    let s = c.state(IT).expect("state");
    assert_eq!(s.selected_profile_id.as_deref(), Some(id.as_str()));
    assert!(s.can_restore);
    assert_eq!(s.profiles.len(), 4);
    c.resume();
    assert_eq!(
        fake.keep_awake(),
        PerformanceKeepAwake::System,
        "left active, re-applied"
    );
}

#[test]
fn user_profiles_are_validated_edited_and_deleted() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    assert!(
        c.save_profile(input("  ", PerformanceKeepAwake::Off), IT)
            .is_err()
    );
    let mut bad = input("X", PerformanceKeepAwake::Off);
    bad.ac.processor_min_percent = Some(90);
    assert!(c.save_profile(bad, IT).is_err());
    let mut builtin = input("X", PerformanceKeepAwake::Off);
    builtin.id = Some(SILENT_ID.into());
    assert!(c.save_profile(builtin, IT).is_err());

    let s = c
        .save_profile(input(" Uno ", PerformanceKeepAwake::Off), IT)
        .expect("save");
    let p = s.profiles.last().expect("p").clone();
    assert_eq!(
        (p.id.as_str(), p.name.as_str(), p.builtin),
        ("custom-1", "Uno", false)
    );
    let s = c
        .save_profile(input("Due", PerformanceKeepAwake::Off), IT)
        .expect("save");
    assert_eq!(s.profiles.last().expect("p").id, "custom-2");

    let mut edit = input("Uno bis", PerformanceKeepAwake::Off);
    edit.id = Some("custom-1".into());
    let s = c.save_profile(edit, IT).expect("edit");
    assert_eq!(s.profiles[3].name, "Uno bis");
    assert!(
        fake.writes().is_empty(),
        "editing an inactive profile applies nothing"
    );

    c.apply_profile("custom-2", IT).expect("apply");
    let s = c.delete_profile("custom-2", IT).expect("delete");
    assert_eq!(s.selected_profile_id, None);
    assert!(s.can_restore, "the original stays restorable");
    assert!(c.delete_profile(SILENT_ID, IT).is_err());
    assert_eq!(
        c.delete_profile("custom-9", IT)
            .map(|_| ())
            .map_err(|e| e.code),
        Err(ErrorCode::NotFound)
    );
    let stored: Vec<StoredProfile> = load_json(store.as_ref(), PROFILES_KEY);
    assert_eq!(stored.len(), 1);
}

#[test]
fn editing_the_active_profile_reapplies_it() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    c.save_profile(input("Uno", PerformanceKeepAwake::Off), IT)
        .expect("save");
    c.apply_profile("custom-1", IT).expect("apply");
    let mut edit = input("Uno", PerformanceKeepAwake::Display);
    edit.id = Some("custom-1".into());
    c.save_profile(edit, IT).expect("edit");
    assert_eq!(fake.writes().len(), 2);
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::Display);
}

#[test]
fn game_override_sits_over_the_selection() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    c.apply_profile(SILENT_ID, IT).expect("apply");
    c.apply_game_override(PERFORMANCE_ID).expect("override");
    let s = c.state(IT).expect("state");
    assert_eq!(s.selected_profile_id.as_deref(), Some(SILENT_ID));
    assert_eq!(
        s.game_override.as_ref().map(|o| o.profile_name.as_str()),
        Some("Prestazioni elevate")
    );
    assert_eq!(
        fake.writes().last().expect("w").1.mode,
        Some(PerformanceMode::HighPerformance)
    );
    c.clear_game_override().expect("clear");
    assert_eq!(
        fake.writes().last().expect("w").1.mode,
        Some(PerformanceMode::Silent)
    );
    assert!(c.state(IT).expect("state").game_override.is_none());
    let n = fake.writes().len();
    c.clear_game_override().expect("no-op");
    assert_eq!(fake.writes().len(), n);
    assert_eq!(
        c.apply_game_override("missing").map_err(|e| e.code),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn clearing_an_override_without_a_selection_restores_the_original() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    c.apply_game_override(PERFORMANCE_ID).expect("override");
    assert!(c.state(IT).expect("state").can_restore);
    c.clear_game_override().expect("clear");
    let s = c.state(IT).expect("state");
    assert!(!s.can_restore);
    assert_eq!(
        fake.writes().last().expect("w").1.processor_max_percent,
        Some(100)
    );
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::Off);
}

#[test]
fn a_user_apply_replaces_the_override() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    c.apply_game_override(PERFORMANCE_ID).expect("override");
    let s = c.apply_profile(BALANCED_ID, IT).expect("apply");
    assert!(s.game_override.is_none());
    assert_eq!(s.selected_profile_id.as_deref(), Some(BALANCED_ID));
}

#[test]
fn profile_refs_name_every_profile() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    c.save_profile(input("Uno", PerformanceKeepAwake::Off), IT)
        .expect("save");
    let refs = c.profile_refs(Language::English);
    assert_eq!(refs.len(), 4);
    assert!(
        refs.iter()
            .all(|r| r.applicable && r.domain == ControlDomainId::Performance)
    );
    assert_eq!(c.domain(), ControlDomainId::Performance);
}

#[test]
fn mac_writes_only_keep_awake_and_reports_read_only_controls() {
    let (fake, store) = (Fake::mac(), Arc::new(MemoryStore::default()));
    let c = controller(&fake, &store);
    let s = c.state(IT).expect("state");
    assert_eq!(s.capabilities.mode.state, SupportState::ReadOnly);
    assert!(s.capabilities.mode.reason.is_some());
    assert_eq!(s.capabilities.keep_awake.state, SupportState::Supported);
    assert_eq!(
        s.capabilities.processor_state.state,
        SupportState::Unsupported
    );
    assert_eq!(s.capabilities.sleep_timeout.state, SupportState::ReadOnly);
    c.save_profile(input("Uno", PerformanceKeepAwake::Display), IT)
        .expect("save");
    c.apply_profile("custom-1", IT).expect("apply");
    assert!(fake.writes().is_empty(), "nothing writable but keep-awake");
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::Display);
    let original: Option<Original> = load_json(store.as_ref(), ORIGINAL_KEY);
    let original = original.expect("original");
    assert_eq!(
        original.ac,
        PerformanceOsSupply::default(),
        "read-only values are not captured"
    );
    c.restore_original(IT).expect("restore");
    assert!(fake.writes().is_empty());
    assert_eq!(fake.keep_awake(), PerformanceKeepAwake::Off);
}

#[test]
fn access_denied_turns_a_control_read_only() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    *fake.deny.lock().expect("lock") = vec![PerformanceControl::ProcessorState];
    let c = controller(&fake, &store);
    c.save_profile(input("Uno", PerformanceKeepAwake::Off), IT)
        .expect("save");
    let s = c.apply_profile("custom-1", IT).expect("apply");
    assert_eq!(s.capabilities.processor_state.state, SupportState::ReadOnly);
    assert_eq!(s.issues.len(), 1);
    assert_eq!(s.issues[0].control, PerformanceControl::ProcessorState);
    fake.deny.lock().expect("lock").clear();
    fake.set_supply(PowerSupply::Battery);
    c.tick();
    let last = fake.writes().pop().expect("w");
    assert_eq!(
        last.1.processor_max_percent, None,
        "a denied control is not retried"
    );
}

#[test]
fn desktop_without_battery_uses_the_ac_settings_only() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    {
        let mut os = fake.os.lock().expect("lock");
        os.has_battery = false;
        os.battery = None;
        os.supply = PowerSupply::Battery; // a stray report must not matter
    }
    let c = controller(&fake, &store);
    c.save_profile(input("Uno", PerformanceKeepAwake::Off), IT)
        .expect("save");
    let s = c.apply_profile("custom-1", IT).expect("apply");
    assert_eq!(s.supply, PowerSupply::Ac);
    let w = fake.writes().pop().expect("w");
    assert_eq!(w.0, PowerSupply::Ac);
    assert!(w.2.is_none());
}

#[test]
fn watcher_starts_and_stops() {
    let (fake, store) = (Fake::windows(), Arc::new(MemoryStore::default()));
    let c = Arc::new(controller(&fake, &store));
    c.apply_profile(SILENT_ID, IT).expect("apply");
    c.start_watcher(Duration::from_millis(10));
    c.start_watcher(Duration::from_millis(10));
    fake.set_supply(PowerSupply::Battery);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while fake.writes().len() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fake.writes().len(), 2, "the watcher re-applied on battery");
    c.shutdown();
    assert!(c.watcher.lock().expect("lock").is_none());
}
