use std::sync::{Arc, Mutex};

use super::*;
use crate::MemoryStore;

/// Fake OS: a configurable fan probe and a cooling policy in memory.
struct Fake {
    probe: FanProbe,
    laptop: Option<bool>,
    /// `None`: no policy on this OS.
    policy: Mutex<Option<PolicyValues>>,
    deny_writes: bool,
    writes: Mutex<Vec<PolicyValues>>,
}

impl Fake {
    fn mac_air() -> Self {
        Self {
            probe: FanProbe::Read {
                source: FanSource::AppleSmc,
                declared: None,
                fans: Vec::new(),
            },
            laptop: Some(true),
            policy: Mutex::new(None),
            deny_writes: false,
            writes: Mutex::new(Vec::new()),
        }
    }

    fn windows(ac: u32, dc: u32) -> Self {
        Self {
            probe: FanProbe::Read {
                source: FanSource::Wmi,
                declared: Some(1),
                fans: vec![FanReading {
                    id: "fan0".into(),
                    label: Some("Cooling Device".into()),
                    target_rpm: Some(1800.0),
                    ..FanReading::default()
                }],
            },
            laptop: Some(false),
            policy: Mutex::new(Some(PolicyValues {
                ac: Some(ac),
                dc: Some(dc),
            })),
            deny_writes: false,
            writes: Mutex::new(Vec::new()),
        }
    }

    fn policy(&self) -> PolicyValues {
        self.policy.lock().expect("lock").expect("policy")
    }
}

impl CoolingBackend for Fake {
    fn probe_fans(&self) -> FanProbe {
        self.probe.clone()
    }
    fn is_laptop(&self) -> Option<bool> {
        self.laptop
    }
    fn read_policy(&self) -> Result<PolicyValues, PolicyError> {
        self.policy
            .lock()
            .expect("lock")
            .ok_or(PolicyError::NotSupported)
    }
    fn write_policy(&self, values: PolicyValues) -> Result<(), PolicyError> {
        let mut p = self.policy.lock().expect("lock");
        let Some(cur) = p.as_mut() else {
            return Err(PolicyError::NotSupported);
        };
        if self.deny_writes {
            return Err(PolicyError::AccessDenied);
        }
        self.writes.lock().expect("lock").push(values);
        if values.ac.is_some() {
            cur.ac = values.ac;
        }
        if values.dc.is_some() {
            cur.dc = values.dc;
        }
        Ok(())
    }
    fn cpu_temperature(&self) -> Option<f64> {
        Some(48.0)
    }
}

fn controller(fake: Fake) -> (CoolingController, Arc<Fake>, Arc<MemoryStore>) {
    let fake = Arc::new(fake);
    let store = Arc::new(MemoryStore::default());
    (
        CoolingController::new(fake.clone(), store.clone()),
        fake,
        store,
    )
}

const IT: Language = Language::Italian;

fn draft(name: &str) -> CoolingProfileDraft {
    CoolingProfileDraft {
        id: None,
        name: name.into(),
        curves: vec![CoolingFanCurve {
            fan_id: None,
            curve: CoolingCurve::aggressive(),
        }],
        cooling_policy: Vec::new(),
    }
}

#[test]
fn fanless_mac_is_passive_and_nothing_is_controllable() {
    let (c, _, _) = controller(Fake::mac_air());
    let s = c.state(IT).expect("state");
    assert_eq!(s.system.kind, CoolingSystemKind::Passive);
    assert!(s.fans.is_empty());
    assert_eq!(s.capabilities.fan_speed.state, SupportState::Unsupported);
    assert_eq!(s.capabilities.curves.state, SupportState::Unsupported);
    assert_eq!(
        s.capabilities.cooling_policy.state,
        SupportState::Unsupported
    );
    assert!(s.capabilities.cooling_policy.reason.is_some());
    assert_eq!(s.active.as_deref(), Some(ID_AUTO));
    assert_eq!(s.cpu_temperature_c, Some(48.0));
    let applicable: Vec<_> = s
        .profiles
        .iter()
        .map(|e| (e.profile.id.as_str(), e.applicable))
        .collect();
    assert_eq!(
        applicable,
        vec![(ID_AUTO, true), (ID_SILENT, false), (ID_BALANCED, false)]
    );
    assert_eq!(s.profiles[0].profile.name, "Automatico");

    let err = c
        .apply_profile(ID_SILENT, IT)
        .expect_err("silent not applicable");
    assert_eq!(err.code, ErrorCode::NotSupported);
    // Automatic is the Mac's own behaviour: selecting it changes nothing and stores nothing.
    let s = c.apply_profile(ID_AUTO, IT).expect("auto");
    assert_eq!(s.selected.as_deref(), Some(ID_AUTO));
    assert!(!s.can_restore);
}

#[test]
fn fan_count_zero_without_laptop_flag_is_still_passive_and_missing_is_unknown() {
    let mut f = Fake::mac_air();
    f.laptop = None;
    f.probe = FanProbe::Read {
        source: FanSource::AppleSmc,
        declared: Some(0),
        fans: Vec::new(),
    };
    let (c, _, _) = controller(f);
    assert_eq!(
        c.state(IT).expect("s").system.kind,
        CoolingSystemKind::Passive
    );

    let mut f = Fake::mac_air();
    f.laptop = Some(false);
    let (c, _, _) = controller(f);
    assert_eq!(
        c.state(IT).expect("s").system.kind,
        CoolingSystemKind::Unknown
    );
}

#[test]
fn mac_fans_are_read_only_with_readings() {
    let mut f = Fake::mac_air();
    f.laptop = Some(false);
    f.probe = FanProbe::Read {
        source: FanSource::AppleSmc,
        declared: Some(1),
        fans: vec![FanReading {
            id: "smc-0".into(),
            label: None,
            current_rpm: Some(1200.0),
            min_rpm: Some(1000.0),
            max_rpm: Some(4900.0),
            target_rpm: Some(1200.0),
            forced: Some(false),
        }],
    };
    let (c, _, _) = controller(f);
    let s = c.state(Language::English).expect("s");
    assert_eq!(s.system.kind, CoolingSystemKind::Fans);
    assert_eq!(s.fans[0].label, "Fan 1");
    assert_eq!(s.fans[0].mode, Some(CoolingFanMode::Auto));
    assert_eq!(s.fans[0].rpm_reason, None);
    assert_eq!(s.fans[0].speed_control.state, SupportState::ReadOnly);
    assert_eq!(s.capabilities.curves.state, SupportState::ReadOnly);
}

#[test]
fn windows_policy_applies_and_restores_the_original() {
    let (c, fake, store) = controller(Fake::windows(1, 0));
    let s = c.state(IT).expect("s");
    assert!(s.capabilities.cooling_policy.is_supported());
    assert!(s.fans[0].rpm_reason.is_some());
    assert!(s.profiles.iter().all(|e| e.applicable));

    let s = c.apply_profile(ID_SILENT, IT).expect("silent");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(0),
            dc: Some(0)
        }
    );
    assert!(s.can_restore);
    assert_eq!(s.active.as_deref(), Some(ID_SILENT));
    assert!(store.load(KEY_ORIGINAL).expect("load").is_some());

    c.apply_profile(ID_BALANCED, IT).expect("balanced");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(1),
            dc: Some(1)
        }
    );

    // Automatic puts back the values found at first, not a fixed value.
    c.apply_profile(ID_AUTO, IT).expect("auto");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(1),
            dc: Some(0)
        }
    );

    c.apply_profile(ID_SILENT, IT).expect("silent");
    let s = c.restore(IT).expect("restore");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(1),
            dc: Some(0)
        }
    );
    assert!(!s.can_restore);
    assert_eq!(s.selected, None);
    assert_eq!(s.active.as_deref(), Some(ID_AUTO));
}

#[test]
fn original_survives_a_restart() {
    let fake = Arc::new(Fake::windows(1, 1));
    let store = Arc::new(MemoryStore::default());
    {
        let c = CoolingController::new(fake.clone(), store.clone());
        c.apply_profile(ID_SILENT, IT).expect("silent");
    }
    let c = CoolingController::new(fake.clone(), store.clone());
    let s = c.state(IT).expect("s");
    assert!(s.can_restore);
    assert_eq!(s.selected.as_deref(), Some(ID_SILENT));
    c.restore(IT).expect("restore");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(1),
            dc: Some(1)
        }
    );
}

#[test]
fn custom_policy_keeps_unlisted_supply_at_original() {
    let (c, fake, _) = controller(Fake::windows(1, 0));
    let mut d = draft("Notte");
    d.cooling_policy = vec![CoolingPolicySetting {
        supply: PowerSupply::Ac,
        policy: CoolingPolicy::Passive,
    }];
    let s = c.save_profile(d, IT).expect("save");
    let id = s
        .profiles
        .iter()
        .find(|e| !e.profile.builtin)
        .map(|e| e.profile.id.clone())
        .expect("user profile");
    let entry = s.profiles.iter().find(|e| e.profile.id == id).expect("e");
    assert!(entry.applicable);
    assert!(entry.note.is_some(), "curve saved but not applied");
    c.apply_profile(&id, IT).expect("apply");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(0),
            dc: Some(0)
        }
    );
}

#[test]
fn access_denied_turns_the_policy_read_only() {
    let mut f = Fake::windows(1, 1);
    f.deny_writes = true;
    let (c, _, _) = controller(f);
    let err = c.apply_profile(ID_SILENT, IT).expect_err("denied");
    assert_eq!(err.code, ErrorCode::PermissionDenied);
    let s = c.state(IT).expect("s");
    assert_eq!(s.capabilities.cooling_policy.state, SupportState::ReadOnly);
    assert_eq!(s.selected, None);
}

#[test]
fn user_profiles_crud_and_validation() {
    let (c, _, store) = controller(Fake::mac_air());
    assert!(c.save_profile(draft("   "), IT).is_err());
    let mut bad = draft("Bad");
    bad.curves[0].curve = CoolingCurve::new(&[(50.0, 60.0), (40.0, 20.0)]);
    assert_eq!(
        c.save_profile(bad, IT).expect_err("bad").code,
        ErrorCode::InvalidInput
    );

    let s = c.save_profile(draft(" Mio "), IT).expect("save");
    let e = s.profiles.last().expect("entry");
    assert_eq!(e.profile.name, "Mio");
    assert_eq!(e.profile.id, "user-1");
    assert_eq!(e.profile.mode, CoolingMode::Custom);
    assert!(!e.applicable, "no fan control on a fanless Mac");
    assert!(
        e.note
            .as_deref()
            .is_some_and(|n| n.contains("Curva salvata"))
    );
    assert!(store.load(KEY_PROFILES).expect("load").is_some());

    let s = c.save_profile(draft("Due"), IT).expect("save 2");
    assert_eq!(s.profiles.last().expect("e").profile.id, "user-2");

    let mut rename = draft("Rinominato");
    rename.id = Some("user-1".into());
    let s = c.save_profile(rename, IT).expect("rename");
    assert!(s.profiles.iter().any(|e| e.profile.name == "Rinominato"));
    let mut builtin = draft("X");
    builtin.id = Some(ID_SILENT.into());
    assert_eq!(
        c.save_profile(builtin, IT).expect_err("builtin").code,
        ErrorCode::NotFound
    );

    let s = c.delete_profile("user-1", IT).expect("delete");
    assert_eq!(s.profiles.len(), 4);
    assert!(c.delete_profile("user-1", IT).is_err());
    assert!(c.delete_profile(ID_AUTO, IT).is_err());
}

#[test]
fn game_override_layers_over_the_selection() {
    let (c, fake, _) = controller(Fake::windows(1, 1));
    c.apply_profile(ID_BALANCED, IT).expect("balanced");
    c.apply_game_override(ID_SILENT).expect("override");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(0),
            dc: Some(0)
        }
    );
    let s = c.state(IT).expect("s");
    assert_eq!(s.active.as_deref(), Some(ID_SILENT));
    assert_eq!(s.selected.as_deref(), Some(ID_BALANCED));

    // A selection made during the override waits for its end.
    c.apply_profile(ID_AUTO, IT).expect("auto during override");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(0),
            dc: Some(0)
        }
    );

    c.clear_game_override().expect("clear");
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(1),
            dc: Some(1)
        }
    );
    assert_eq!(c.state(IT).expect("s").game_override, None);
    c.clear_game_override().expect("no-op");

    assert_eq!(
        c.apply_game_override("nope").expect_err("unknown").code,
        ErrorCode::NotFound
    );
}

#[test]
fn game_override_on_a_fanless_mac() {
    let (c, _, _) = controller(Fake::mac_air());
    assert_eq!(
        c.apply_game_override(ID_BALANCED).expect_err("no").code,
        ErrorCode::NotSupported
    );
    c.apply_game_override(ID_AUTO)
        .expect("auto is the system's own");
    c.shutdown();
    assert_eq!(c.state(IT).expect("s").game_override, None);
    let refs = c.profile_refs(Language::English);
    assert_eq!(refs.len(), 3);
    assert!(refs.iter().all(|r| r.domain == ControlDomainId::Cooling));
    assert_eq!(refs[1].name, "Silent");
    assert!(!refs[1].applicable);
}

#[test]
fn shutdown_clears_an_override_back_to_the_selection() {
    let (c, fake, _) = controller(Fake::windows(1, 1));
    c.apply_game_override(ID_SILENT).expect("override");
    c.shutdown();
    // No selection: back to the values found at first.
    assert_eq!(
        fake.policy(),
        PolicyValues {
            ac: Some(1),
            dc: Some(1)
        }
    );
    assert!(fake.writes.lock().expect("lock").len() >= 2);
}
