//! Known RGB peripheral vendors (USB vendor ids) and name-based device kinds. Devices of
//! these vendors without a LampArray interface use proprietary protocols the app does not
//! implement: they are listed as recognised, never as controllable.

use models::control::Support;
use models::control::lighting::{LightingDevice, LightingDeviceKind, LightingSource};
use models::settings::Language;

/// Vendor name for a USB vendor id of a known RGB peripheral maker.
pub fn known_rgb_vendor(vendor_id: u32) -> Option<&'static str> {
    Some(match vendor_id {
        0x1b1c => "Corsair",
        0x1532 => "Razer",
        0x1038 => "SteelSeries",
        0x0b05 => "ASUS",
        0x0951 | 0x03f0 => "HyperX / HP",
        0x2516 => "Cooler Master",
        0x1e71 => "NZXT",
        0x1462 => "MSI",
        0x046d => "Logitech",
        0x3633 => "Glorious",
        0x1044 => "Gigabyte",
        0x0cf2 => "Lian Li",
        _ => return None,
    })
}

/// Device kind guessed from the product name (English and Italian words).
pub fn kind_from_name(name: &str) -> LightingDeviceKind {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["keyboard", "tastiera", "keypad"]) {
        LightingDeviceKind::Keyboard
    } else if has(&["mouse"]) {
        LightingDeviceKind::Mouse
    } else if has(&["headset", "headphone", "cuffie"]) {
        LightingDeviceKind::Headset
    } else if has(&["strip", "striscia", "led bar"]) {
        LightingDeviceKind::Strip
    } else if has(&["fan", "ventola"]) {
        LightingDeviceKind::Fan
    } else if has(&["case", "chassis"]) {
        LightingDeviceKind::Case
    } else {
        LightingDeviceKind::Other
    }
}

/// Kind from a HID LampArrayKind value (HID Usage Tables 0x59, `LampArrayKind`).
pub fn kind_from_lamp_array_kind(kind: u32, name: &str) -> LightingDeviceKind {
    match kind {
        1 => LightingDeviceKind::Keyboard,
        2 => LightingDeviceKind::Mouse,
        7 => LightingDeviceKind::Case,
        _ => kind_from_name(name),
    }
}

/// The entry of a recognised device the app cannot control.
pub fn vendor_device(
    id: String,
    name: String,
    vendor: &str,
    kind: LightingDeviceKind,
    lang: Language,
) -> LightingDevice {
    let reason = lang.pick(
        "Riconosciuto, protocollo proprietario non supportato: se ha luci RGB, si comandano solo con l'app del produttore.",
        "Recognised, proprietary protocol not supported: if it has RGB lights, only the maker's app can control them.",
    );
    LightingDevice {
        id,
        name,
        kind,
        vendor: Some(vendor.to_owned()),
        lamp_count: 0,
        color_capable: false,
        color: Support::unsupported(reason),
        brightness: Support::unsupported(reason),
        effects: Support::unsupported(reason),
        source: LightingSource::VendorProtocol,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendors_and_kinds() {
        assert_eq!(known_rgb_vendor(0x1532), Some("Razer"));
        assert_eq!(known_rgb_vendor(0x05ac), None);
        assert_eq!(
            kind_from_name("Razer BlackWidow Keyboard"),
            LightingDeviceKind::Keyboard
        );
        assert_eq!(kind_from_name("G502 Mouse"), LightingDeviceKind::Mouse);
        assert_eq!(
            kind_from_name("Arctis Headset"),
            LightingDeviceKind::Headset
        );
        assert_eq!(kind_from_name("Widget"), LightingDeviceKind::Other);
        assert_eq!(kind_from_lamp_array_kind(2, "x"), LightingDeviceKind::Mouse);
        let d = vendor_device(
            "vendor:1532:0084".into(),
            "Razer DeathAdder".into(),
            "Razer",
            LightingDeviceKind::Mouse,
            Language::Italian,
        );
        assert!(!d.controllable());
        assert!(
            d.color
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("proprietario"))
        );
    }
}
