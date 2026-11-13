//! State machine tests over a fake backend and `MemoryStore`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use models::control::Support;
use models::control::lighting::{
    LightingColor, LightingDevice, LightingDeviceKind, LightingEffect, LightingSetup,
    LightingSnapshot, LightingSource, LightingTargets,
};
use models::settings::Language;
use models::{AppError, ErrorCode};
use platform::control::lighting::{DeviceLayout, LightingFrame};

use super::*;
use crate::{MemoryStore, ProfileDomain};

#[derive(Default)]
struct Fake {
    devices: Mutex<Vec<LightingDevice>>,
    /// (id, frame, persist) in order.
    writes: Mutex<Vec<(String, LightingFrame, bool)>>,
    restored: Mutex<Vec<String>>,
    released: Mutex<Vec<String>>,
    taken: Mutex<Vec<String>>,
}

impl Fake {
    fn writes_for(&self, id: &str) -> Vec<(LightingFrame, bool)> {
        self.writes
            .lock()
            .expect("lock")
            .iter()
            .filter(|(i, _, _)| i == id)
            .map(|(_, f, p)| (f.clone(), *p))
            .collect()
    }
}

impl LightingBackend for Fake {
    fn devices(&self, _lang: Language) -> Vec<LightingDevice> {
        self.devices.lock().expect("lock").clone()
    }
    fn layout(&self, id: &str) -> DeviceLayout {
        let n = self
            .devices
            .lock()
            .expect("lock")
            .iter()
            .find(|d| d.id == id)
            .map_or(1, |d| d.lamp_count);
        DeviceLayout {
            lamp_count: n,
            positions: Vec::new(),
        }
    }
    fn snapshot(&self, id: &str) -> Result<LightingSnapshot, AppError> {
        Ok(if id.starts_with("kb") {
            LightingSnapshot::Backlight {
                level: 0.4,
                auto: true,
            }
        } else {
            LightingSnapshot::LampArray {
                autonomous: true,
                enabled: None,
                brightness: None,
            }
        })
    }
    fn take_control(&self, id: &str) -> Result<(), AppError> {
        self.taken.lock().expect("lock").push(id.to_owned());
        Ok(())
    }
    fn write(&self, id: &str, frame: &LightingFrame, persist: bool) -> Result<(), AppError> {
        self.writes
            .lock()
            .expect("lock")
            .push((id.to_owned(), frame.clone(), persist));
        Ok(())
    }
    fn restore(&self, id: &str, _s: &LightingSnapshot) -> Result<(), AppError> {
        self.restored.lock().expect("lock").push(id.to_owned());
        Ok(())
    }
    fn release(&self, id: &str) {
        self.released.lock().expect("lock").push(id.to_owned());
    }
    fn platform_note(&self, _lang: Language) -> Option<String> {
        None
    }
}

fn rgb_device(id: &str, lamps: u32) -> LightingDevice {
    LightingDevice {
        id: id.to_owned(),
        name: format!("RGB {id}"),
        kind: LightingDeviceKind::Keyboard,
        vendor: None,
        lamp_count: lamps,
        color_capable: true,
        color: Support::supported(),
        brightness: Support::supported(),
        effects: Support::supported(),
        source: LightingSource::LampArray,
    }
}

fn backlight() -> LightingDevice {
    LightingDevice {
        id: "kb".to_owned(),
        name: "Backlight".to_owned(),
        kind: LightingDeviceKind::KeyboardBacklight,
        vendor: Some("Apple".into()),
        lamp_count: 1,
        color_capable: false,
        color: Support::unsupported("white"),
        brightness: Support::supported(),
        effects: Support::supported(),
        source: LightingSource::MacKeyboardBacklight,
    }
}

fn vendor() -> LightingDevice {
    LightingDevice {
        id: "vendor:1532:0084".to_owned(),
        name: "Razer".to_owned(),
        kind: LightingDeviceKind::Mouse,
        vendor: Some("Razer".into()),
        lamp_count: 0,
        color_capable: false,
        color: Support::unsupported("x"),
        brightness: Support::unsupported("x"),
        effects: Support::unsupported("x"),
        source: LightingSource::VendorProtocol,
    }
}

fn setup(effect: LightingEffect) -> LightingSetup {
    LightingSetup {
        effect,
        brightness: 50,
        targets: LightingTargets::All,
        sync: true,
    }
}

fn controller(devices: Vec<LightingDevice>) -> (Arc<Fake>, Arc<MemoryStore>, LightingController) {
    let fake = Arc::new(Fake::default());
    *fake.devices.lock().expect("lock") = devices;
    let store = Arc::new(MemoryStore::default());
    let c = LightingController::new(fake.clone(), store.clone());
    (fake, store, c)
}

const L: Language = Language::Italian;

#[test]
fn state_lists_devices_controllable_first_and_builtins() {
    let (_, _, c) = controller(vec![vendor(), rgb_device("a", 4), backlight()]);
    let s = c.state(L).expect("state");
    assert!(s.devices[0].controllable() && s.devices[1].controllable());
    assert_eq!(s.devices[2].source, LightingSource::VendorProtocol);
    assert_eq!(s.profiles.len(), 5);
    assert_eq!(s.profiles[0].name, "Statico");
    assert!(!s.can_restore && s.applied.is_none() && !s.engine_running);
}

#[test]
fn nothing_changes_until_asked_and_original_is_saved_first() {
    let (fake, store, c) = controller(vec![rgb_device("a", 3)]);
    c.state(L).expect("state");
    assert!(fake.writes.lock().expect("lock").is_empty());
    let red = LightingColor::rgb(200, 0, 0);
    let s = c
        .apply_setup(setup(LightingEffect::Static { color: red }), L)
        .expect("apply");
    assert!(s.can_restore);
    assert!(store.load(ORIGINAL_KEY).expect("load").is_some());
    let w = fake.writes_for("a");
    assert_eq!(
        w,
        vec![(
            LightingFrame::Colors(vec![LightingColor::rgb(100, 0, 0); 3]),
            true
        )]
    );
    assert!(!s.engine_running, "static effect: no engine");
    assert_eq!(s.applied.expect("applied").device_ids, vec!["a".to_owned()]);
}

#[test]
fn single_color_lights_get_levels_and_refuse_color_effects() {
    let (fake, _, c) = controller(vec![backlight()]);
    c.apply_setup(
        setup(LightingEffect::Static {
            color: LightingColor::rgb(0, 0, 10),
        }),
        L,
    )
    .expect("apply");
    assert_eq!(
        fake.writes_for("kb"),
        vec![(LightingFrame::Level(0.5), true)]
    );
    let e = c
        .apply_setup(setup(LightingEffect::Rainbow { speed: 3 }), L)
        .expect_err("no device can show a rainbow");
    assert_eq!(e.code, ErrorCode::NotSupported);
}

#[test]
fn unsupported_devices_are_never_driven() {
    let (fake, _, c) = controller(vec![vendor()]);
    let e = c
        .apply_setup(
            setup(LightingEffect::Static {
                color: LightingColor::WHITE,
            }),
            L,
        )
        .expect_err("nothing controllable");
    assert_eq!(e.code, ErrorCode::NotSupported);
    assert!(fake.writes.lock().expect("lock").is_empty());
    assert!(fake.taken.lock().expect("lock").is_empty());
}

#[test]
fn dynamic_effects_run_the_engine_until_a_static_one() {
    let (fake, _, c) = controller(vec![rgb_device("a", 2)]);
    let s = c
        .apply_setup(setup(LightingEffect::Rainbow { speed: 10 }), L)
        .expect("apply");
    assert!(s.engine_running);
    std::thread::sleep(Duration::from_millis(200));
    let frames = fake.writes_for("a").len();
    assert!(frames >= 3, "engine wrote {frames} frames");
    assert!(
        fake.writes_for("a").iter().skip(1).all(|(_, p)| !p),
        "animation steps are not persisted"
    );
    let s = c.apply_setup(setup(LightingEffect::Off), L).expect("off");
    assert!(!s.engine_running);
    let after = fake.writes_for("a").len();
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(fake.writes_for("a").len(), after, "idle: no more writes");
}

#[test]
fn targets_and_sync_layout() {
    let (fake, _, c) = controller(vec![rgb_device("a", 2), rgb_device("b", 2)]);
    let mut s = setup(LightingEffect::Gradient {
        colors: vec![
            LightingColor::rgb(0, 0, 0),
            LightingColor::rgb(200, 200, 200),
        ],
    });
    s.brightness = 100;
    c.apply_setup(s.clone(), L).expect("apply");
    // Synced: the gradient spans both devices (positions 0, .5 | .5, 1).
    assert_eq!(
        fake.writes_for("a")[0].0,
        LightingFrame::Colors(vec![
            LightingColor::rgb(0, 0, 0),
            LightingColor::rgb(100, 100, 100)
        ])
    );
    assert_eq!(
        fake.writes_for("b")[0].0,
        LightingFrame::Colors(vec![
            LightingColor::rgb(100, 100, 100),
            LightingColor::rgb(200, 200, 200)
        ])
    );
    // Not synced, only "b": "a" goes back to its original state.
    s.sync = false;
    s.targets = LightingTargets::Devices {
        ids: vec!["b".into()],
    };
    let st = c.apply_setup(s, L).expect("apply b");
    assert_eq!(
        fake.restored.lock().expect("lock").as_slice(),
        &["a".to_owned()]
    );
    assert_eq!(
        fake.writes_for("b")[1].0,
        LightingFrame::Colors(vec![
            LightingColor::rgb(0, 0, 0),
            LightingColor::rgb(200, 200, 200)
        ])
    );
    assert_eq!(
        st.applied.expect("applied").device_ids,
        vec!["b".to_owned()]
    );
}

#[test]
fn restore_puts_devices_back_and_forgets_the_original() {
    let (fake, store, c) = controller(vec![rgb_device("a", 1), backlight()]);
    c.apply_profile("builtin.breathing", L).expect("apply");
    let s = c.restore(L).expect("restore");
    assert!(!s.can_restore && s.applied.is_none() && !s.engine_running);
    let mut restored = fake.restored.lock().expect("lock").clone();
    restored.sort();
    assert_eq!(restored, vec!["a".to_owned(), "kb".to_owned()]);
    assert!(store.load(ORIGINAL_KEY).expect("load").is_none());
    assert_eq!(s.selected, None);
}

#[test]
fn original_survives_a_restart() {
    let (fake, store, c) = controller(vec![rgb_device("a", 1)]);
    c.apply_profile("builtin.static", L).expect("apply");
    c.shutdown();
    assert_eq!(
        fake.released.lock().expect("lock").as_slice(),
        &["a".to_owned()]
    );
    drop(c);
    let c2 = LightingController::new(fake.clone(), store);
    let s = c2.state(L).expect("state");
    assert!(s.can_restore);
    assert_eq!(s.selected.as_deref(), Some("builtin.static"));
    assert!(s.applied.is_none(), "nothing re-applied at startup");
    c2.restore(L).expect("restore");
    assert!(
        fake.restored
            .lock()
            .expect("lock")
            .contains(&"a".to_owned())
    );
}

#[test]
fn shutdown_leaves_single_color_lights_steady() {
    let (fake, _, c) = controller(vec![backlight()]);
    c.apply_profile("builtin.breathing", L).expect("apply");
    c.shutdown();
    let last = fake.writes_for("kb").last().cloned().expect("a write");
    assert_eq!(last, (LightingFrame::Level(0.8), true));
}

#[test]
fn user_profiles_crud() {
    let (_, _, c) = controller(vec![rgb_device("a", 1)]);
    let s = c
        .save_profile(None, "  Notte ", setup(LightingEffect::Off), L)
        .expect("save");
    let p = s
        .profiles
        .iter()
        .find(|p| !p.builtin)
        .expect("user profile")
        .clone();
    assert_eq!(p.name, "Notte");
    assert_eq!(s.selected.as_deref(), Some(p.id.as_str()));
    let s = c
        .save_profile(
            Some(p.id.clone()),
            "Sera",
            setup(LightingEffect::Rainbow { speed: 2 }),
            L,
        )
        .expect("rename");
    assert_eq!(s.profiles.iter().filter(|p| !p.builtin).count(), 1);
    assert!(s.profiles.iter().any(|x| x.id == p.id && x.name == "Sera"));
    assert!(
        c.save_profile(
            Some("builtin.off".into()),
            "x",
            setup(LightingEffect::Off),
            L
        )
        .is_err()
    );
    assert!(
        c.save_profile(None, " ", setup(LightingEffect::Off), L)
            .is_err()
    );
    assert!(c.delete_profile("builtin.static", L).is_err());
    let s = c.delete_profile(&p.id, L).expect("delete");
    assert!(s.profiles.iter().all(|p| p.builtin));
    assert_eq!(s.selected, None);
    assert_eq!(
        c.delete_profile(&p.id, L).expect_err("gone").code,
        ErrorCode::NotFound
    );
}

#[test]
fn invalid_setups_are_rejected() {
    let (_, _, c) = controller(vec![rgb_device("a", 1)]);
    let mut s = setup(LightingEffect::Off);
    s.brightness = 101;
    assert_eq!(
        c.apply_setup(s, L).expect_err("bad").code,
        ErrorCode::InvalidInput
    );
    assert!(c.set_brightness(30, L).is_err(), "nothing applied yet");
}

#[test]
fn brightness_reapplies_what_is_on() {
    let (fake, _, c) = controller(vec![rgb_device("a", 1)]);
    c.apply_setup(
        setup(LightingEffect::Static {
            color: LightingColor::rgb(200, 100, 0),
        }),
        L,
    )
    .expect("apply");
    let s = c.set_brightness(100, L).expect("brightness");
    assert_eq!(s.applied.expect("applied").setup.brightness, 100);
    assert_eq!(
        fake.writes_for("a").last().expect("write").0,
        LightingFrame::Colors(vec![LightingColor::rgb(200, 100, 0)])
    );
}

#[test]
fn game_override_layers_over_the_user_selection() {
    let (fake, _, c) = controller(vec![rgb_device("a", 1)]);
    c.state(L).expect("state");
    assert_eq!(c.domain(), ControlDomainId::Lighting);
    let refs = c.profile_refs(L);
    assert_eq!(refs.len(), 5);
    assert!(refs.iter().all(|r| r.applicable));

    c.apply_profile("builtin.static", L).expect("user");
    c.apply_game_override("builtin.off").expect("game");
    let s = c.state(L).expect("state");
    assert_eq!(s.game_override.as_deref(), Some("builtin.off"));
    assert_eq!(
        s.applied.as_ref().and_then(|a| a.profile_id.as_deref()),
        Some("builtin.off")
    );
    assert_eq!(
        c.apply_game_override("nope").expect_err("unknown").code,
        ErrorCode::NotFound
    );
    c.clear_game_override().expect("clear");
    let s = c.state(L).expect("state");
    assert_eq!(s.game_override, None);
    assert_eq!(
        s.applied.and_then(|a| a.profile_id).as_deref(),
        Some("builtin.static")
    );
    c.clear_game_override().expect("no-op");

    // Without a user selection, clearing gives the devices back.
    c.restore(L).expect("restore");
    fake.restored.lock().expect("lock").clear();
    c.apply_game_override("builtin.rainbow").expect("game");
    c.clear_game_override().expect("clear");
    assert_eq!(
        fake.restored.lock().expect("lock").as_slice(),
        &["a".to_owned()]
    );
    assert!(!c.state(L).expect("state").engine_running);
}

#[test]
fn profile_refs_mark_color_profiles_not_applicable_on_white_lights() {
    let (_, _, c) = controller(vec![backlight()]);
    let refs = c.profile_refs(Language::English);
    let get = |id: &str| refs.iter().find(|r| r.id == id).expect("ref").applicable;
    assert!(get("builtin.static") && get("builtin.breathing") && get("builtin.off"));
    assert!(!get("builtin.rainbow") && !get("builtin.wave"));
    assert_eq!(refs[0].name, "Static");
}
