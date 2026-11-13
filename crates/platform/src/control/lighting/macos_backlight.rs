//! The Mac keyboard backlight through CoreBrightness's `KeyboardBrightnessClient`, the class
//! Control Center uses. CoreBrightness is a private framework: it is loaded at run time with
//! `dlopen`, needs no entitlement, and every missing piece (framework, class, method, a Mac
//! without a backlight) simply means no device. Methods used (checked on macOS 27):
//! `copyKeyboardBacklightIDs`, `brightnessForKeyboard:` (0–1), `setBrightness:forKeyboard:`,
//! `setBrightness:fadeSpeed:commit:forKeyboard:` (animation steps, not committed),
//! `isAutoBrightnessEnabledForKeyboard:`, `enableAutoBrightness:forKeyboard:`,
//! `isKeyboardBuiltIn:`.

use std::ffi::{CStr, c_void};
use std::sync::{Mutex, OnceLock};

use models::control::Support;
use models::control::lighting::{
    LightingDevice, LightingDeviceKind, LightingSnapshot, LightingSource,
};
use models::settings::Language;
use models::{AppError, ErrorCode};
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
use objc2::{msg_send, sel};

use super::LightingFrame;

const PREFIX: &str = "mac-backlight:";
const FRAMEWORK: &CStr =
    c"/System/Library/PrivateFrameworks/CoreBrightness.framework/CoreBrightness";

pub fn owns(id: &str) -> bool {
    id.starts_with(PREFIX)
}

fn keyboard_id(id: &str) -> Option<u64> {
    id.strip_prefix(PREFIX)?.parse().ok()
}

/// The client object, used from one thread at a time (behind the mutex).
struct Client(Retained<AnyObject>);
// SAFETY: `KeyboardBrightnessClient` talks to corebrightnessd over XPC and keeps no
// thread-affine state; every use is serialized by the `Mutex` in `Backlight`.
unsafe impl Send for Client {}

impl Client {
    fn responds(&self, sel: Sel) -> bool {
        // SAFETY: `respondsToSelector:` is defined on every NSObject.
        let r: Bool = unsafe { msg_send![&*self.0, respondsToSelector: sel] };
        r.as_bool()
    }

    fn ids(&self) -> Vec<u64> {
        if !self.responds(sel!(copyKeyboardBacklightIDs)) {
            return Vec::new();
        }
        // SAFETY: the method returns a retained (+1, "copy" family) NSArray of NSNumber or nil.
        let arr: Option<Retained<AnyObject>> =
            unsafe { msg_send![&*self.0, copyKeyboardBacklightIDs] };
        let Some(arr) = arr else {
            return Vec::new();
        };
        // SAFETY: `arr` is an NSArray; `count` and `objectAtIndex:` are NSArray methods.
        let count: usize = unsafe { msg_send![&*arr, count] };
        (0..count.min(16))
            .filter_map(|i| {
                // SAFETY: index < count; the element is an NSNumber (checked by respondsTo).
                let n: Option<Retained<AnyObject>> = unsafe { msg_send![&*arr, objectAtIndex: i] };
                let n = n?;
                let ok: Bool =
                    unsafe { msg_send![&*n, respondsToSelector: sel!(unsignedLongLongValue)] };
                ok.as_bool()
                    .then(|| unsafe { msg_send![&*n, unsignedLongLongValue] })
            })
            .collect()
    }

    fn level(&self, kb: u64) -> f32 {
        // SAFETY: `brightnessForKeyboard:(unsigned long long) -> float` (encoding f24@0:8Q16).
        unsafe { msg_send![&*self.0, brightnessForKeyboard: kb] }
    }

    fn auto(&self, kb: u64) -> bool {
        if !self.responds(sel!(isAutoBrightnessEnabledForKeyboard:)) {
            return false;
        }
        // SAFETY: `isAutoBrightnessEnabledForKeyboard:(unsigned long long) -> BOOL`.
        let r: Bool = unsafe { msg_send![&*self.0, isAutoBrightnessEnabledForKeyboard: kb] };
        r.as_bool()
    }

    fn built_in(&self, kb: u64) -> bool {
        if !self.responds(sel!(isKeyboardBuiltIn:)) {
            return true;
        }
        // SAFETY: `isKeyboardBuiltIn:(unsigned long long) -> BOOL`.
        let r: Bool = unsafe { msg_send![&*self.0, isKeyboardBuiltIn: kb] };
        r.as_bool()
    }

    fn set(&self, kb: u64, level: f32, commit: bool) -> bool {
        let level = level.clamp(0.0, 1.0);
        let fade = sel!(setBrightness:fadeSpeed:commit:forKeyboard:);
        let r: Bool = if !commit && self.responds(fade) {
            // SAFETY: `setBrightness:(float) fadeSpeed:(int) commit:(BOOL) forKeyboard:(u64)
            // -> BOOL` (encoding B36@0:8f16i20B24Q28). Speed 0 = no fade.
            unsafe {
                msg_send![&*self.0, setBrightness: level, fadeSpeed: 0i32, commit: Bool::NO, forKeyboard: kb]
            }
        } else {
            // SAFETY: `setBrightness:(float) forKeyboard:(u64) -> BOOL` (B28@0:8f16Q20).
            unsafe { msg_send![&*self.0, setBrightness: level, forKeyboard: kb] }
        };
        r.as_bool()
    }

    fn set_auto(&self, kb: u64, on: bool) {
        if self.responds(sel!(enableAutoBrightness:forKeyboard:)) {
            // SAFETY: `enableAutoBrightness:(BOOL) forKeyboard:(u64) -> BOOL`.
            let _: Bool = unsafe {
                msg_send![&*self.0, enableAutoBrightness: Bool::new(on), forKeyboard: kb]
            };
        }
    }
}

/// Loads CoreBrightness once; `None` when it or the class is missing.
fn client_class() -> Option<&'static AnyClass> {
    static LOADED: OnceLock<bool> = OnceLock::new();
    let loaded = *LOADED.get_or_init(|| {
        // SAFETY: a constant, NUL-terminated path; the handle is kept for the process
        // lifetime (never closed), so classes it registers stay valid.
        let handle: *mut c_void = unsafe { libc::dlopen(FRAMEWORK.as_ptr(), libc::RTLD_LAZY) };
        !handle.is_null()
    });
    if !loaded {
        return None;
    }
    AnyClass::get(c"KeyboardBrightnessClient")
}

pub struct Backlight {
    client: Mutex<Option<Client>>,
}

impl Backlight {
    pub fn new() -> Self {
        Self {
            client: Mutex::new(None),
        }
    }

    fn with<T>(&self, f: impl FnOnce(&Client) -> T) -> Option<T> {
        let mut guard = self.client.lock().ok()?;
        if guard.is_none() {
            let class = client_class()?;
            // SAFETY: `new` on an NSObject subclass returns a retained instance or nil.
            let obj: Option<Retained<AnyObject>> = unsafe { msg_send![class, new] };
            *guard = Some(Client(obj?));
        }
        let client = guard.as_ref()?;
        Some(autoreleasepool(|_| f(client)))
    }

    pub fn devices(&self, lang: Language) -> Vec<LightingDevice> {
        let found = self
            .with(|c| {
                c.ids()
                    .into_iter()
                    .map(|kb| (kb, c.built_in(kb)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        found
            .into_iter()
            .map(|(kb, built_in)| LightingDevice {
                id: format!("{PREFIX}{kb}"),
                name: if built_in {
                    lang.pick("Retroilluminazione tastiera", "Keyboard backlight")
                } else {
                    lang.pick("Retroilluminazione tastiera esterna", "External keyboard backlight")
                }
                .to_owned(),
                kind: LightingDeviceKind::KeyboardBacklight,
                vendor: Some("Apple".to_owned()),
                lamp_count: 1,
                color_capable: false,
                color: Support::unsupported(lang.pick(
                    "Luce bianca a un solo colore: il colore scelto viene ignorato, conta solo la luminosità.",
                    "Single white light: the chosen color is ignored, only brightness applies.",
                )),
                brightness: Support::supported(),
                effects: Support::supported(),
                source: LightingSource::MacKeyboardBacklight,
            })
            .collect()
    }

    fn kb(id: &str) -> Result<u64, AppError> {
        keyboard_id(id).ok_or_else(|| AppError::invalid_input("bad keyboard backlight id"))
    }

    pub fn snapshot(&self, id: &str) -> Result<LightingSnapshot, AppError> {
        let kb = Self::kb(id)?;
        self.with(|c| LightingSnapshot::Backlight {
            level: c.level(kb),
            auto: c.auto(kb),
        })
        .ok_or_else(unavailable)
    }

    pub fn write(&self, id: &str, frame: &LightingFrame, persist: bool) -> Result<(), AppError> {
        let kb = Self::kb(id)?;
        let level = match frame {
            LightingFrame::Level(l) => *l,
            LightingFrame::Colors(colors) => colors
                .iter()
                .map(|c| f64::from(c.r.max(c.g).max(c.b)) / 255.0)
                .fold(0.0, f64::max),
        };
        let ok = self
            .with(|c| c.set(kb, level as f32, persist))
            .ok_or_else(unavailable)?;
        if ok {
            Ok(())
        } else {
            Err(AppError::new(
                ErrorCode::Platform,
                "keyboard backlight did not accept the level",
            ))
        }
    }

    pub fn restore(&self, id: &str, snapshot: &LightingSnapshot) -> Result<(), AppError> {
        let kb = Self::kb(id)?;
        let LightingSnapshot::Backlight { level, auto } = snapshot else {
            return Err(AppError::invalid_input("snapshot of another device type"));
        };
        self.with(|c| {
            c.set(kb, *level, true);
            if c.auto(kb) != *auto {
                c.set_auto(kb, *auto);
            }
        })
        .ok_or_else(unavailable)
    }
}

fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::NotSupported,
        "CoreBrightness keyboard client unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_parse() {
        assert_eq!(keyboard_id("mac-backlight:95158272"), Some(95_158_272));
        assert_eq!(keyboard_id("hid:1"), None);
        assert!(owns("mac-backlight:1"));
    }

    /// Reads the backlight of this Mac (no change).
    #[test]
    fn host_backlight_reads() {
        let b = Backlight::new();
        for d in b.devices(Language::English) {
            let snap = b.snapshot(&d.id).expect("snapshot");
            eprintln!("{}: {snap:?}", d.name);
            if let LightingSnapshot::Backlight { level, .. } = snap {
                assert!((0.0..=1.0).contains(&level));
            }
        }
    }

    /// Changes the backlight of this Mac and puts it back (run once by hand):
    /// `cargo test -p platform backlight_round_trip -- --ignored --nocapture`
    #[test]
    #[ignore = "changes the keyboard backlight for a moment"]
    fn backlight_round_trip() {
        let b = Backlight::new();
        let devices = b.devices(Language::English);
        assert!(!devices.is_empty(), "no keyboard backlight found");
        for d in devices {
            let original = b.snapshot(&d.id).expect("snapshot");
            let LightingSnapshot::Backlight { level, .. } = original else {
                panic!("wrong snapshot")
            };
            let target = if level > 0.5 { 0.2 } else { 0.8 };
            b.write(&d.id, &LightingFrame::Level(target), true)
                .expect("set");
            std::thread::sleep(std::time::Duration::from_millis(700));
            let LightingSnapshot::Backlight { level: now, .. } = b.snapshot(&d.id).expect("read")
            else {
                panic!("wrong snapshot")
            };
            eprintln!(
                "{}: original {level}, set {target}, read back {now}",
                d.name
            );
            // Animation steps (not committed).
            for i in 0..10 {
                b.write(&d.id, &LightingFrame::Level(f64::from(i) / 10.0), false)
                    .expect("step");
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            b.restore(&d.id, &original).expect("restore");
            std::thread::sleep(std::time::Duration::from_millis(700));
            let back = b.snapshot(&d.id).expect("read");
            eprintln!("{}: restored to {back:?}", d.name);
            assert!((now - target as f32).abs() < 0.05, "level did not change");
            if let LightingSnapshot::Backlight { level: l, .. } = back {
                assert!((l - level).abs() < 0.02, "not restored");
            }
        }
    }
}
