//! Form factor of the computer (tower / laptop / mini PC / all-in-one), collected with
//! the hardware inventory. The mappings are pure and unit-tested on every platform; the
//! OS reads live in `macos::device` and `windows::wmi` / `windows::bench_env`.
//!
//! Sources (no privileges needed):
//! - macOS: the device-tree product name (`IODeviceTree:/product` `product-name`, e.g.
//!   "MacBook Air (13-inch, M5)") decides; the model identifier (`hw.model`) decides on
//!   Intel Macs, whose identifiers carry the family ("MacBookPro16,1", "Macmini8,1"). Apple
//!   silicon identifiers are generic ("Mac17,3") and decide nothing. `IOPMPowerSource`
//!   `BatteryInstalled` corroborates.
//! - Windows: SMBIOS `Win32_SystemEnclosure.ChassisTypes` decides; `GetSystemPowerStatus`
//!   (`BatteryFlag`) corroborates.
//!
//! The battery never decides alone (a desktop on a UPS reports a battery on Windows). When
//! the signals disagree, or none is recognized, the form factor is `Inconclusive` with the
//! evidence in the detail: the UI then asks the user instead of guessing.
//!
//! Mac Studio maps to `Mini`: it is a compact desktop, and the 3D models are tower /
//! laptop / mini; so does the 2013 cylinder Mac Pro ("MacPro6,1"). Rack-mounted Mac Pro
//! maps to `Other` (it is not a tower). "MacPro7,1" is shared by the 2019 tower and rack
//! models, so without a product name it is `Inconclusive`.

use models::device::{FormFactor, FormFactorBasis, FormFactorInfo};
use models::metric::{Metric, Source, Unavailability, Unit};

/// Form factor from a macOS device-tree product name ("MacBook Pro (14-inch, M4, 2024)",
/// "Mac mini (2024)", "Mac Studio (2025)", "iMac (24-inch, 2024)", "Mac Pro (2023)").
pub fn from_mac_product_name(name: &str) -> Option<FormFactor> {
    let n = name.trim();
    // "MacBook" first: "MacBook Pro" must not be read as "Mac Pro".
    if n.starts_with("MacBook") {
        Some(FormFactor::Laptop)
    } else if n.starts_with("Mac mini") || n.starts_with("Mac Studio") {
        Some(FormFactor::Mini)
    } else if n.starts_with("iMac") {
        Some(FormFactor::AllInOne)
    } else if n.starts_with("Mac Pro") {
        Some(if n.contains("Rack") {
            FormFactor::Other
        } else {
            FormFactor::Desktop
        })
    } else {
        None
    }
}

/// Model identifiers shared by machines of different form factors: the identifier alone
/// cannot decide, the product name must. Returns why.
pub fn mac_model_identifier_ambiguity(id: &str) -> Option<&'static str> {
    match id.trim() {
        "MacPro7,1" => Some("MacPro7,1 is used by both the tower and the rack-mount Mac Pro"),
        _ => None,
    }
}

/// Form factor from a macOS model identifier. Only the family-named identifiers of Intel
/// (and older) Macs are recognized; Apple silicon's generic "MacNN,N", virtual machines
/// ("VirtualMac2,1") and identifiers shared across form factors (see
/// `mac_model_identifier_ambiguity`) return `None`. The 2013 cylinder Mac Pro
/// ("MacPro6,1") is a compact desktop and maps to `Mini`, like Mac Studio.
pub fn from_mac_model_identifier(id: &str) -> Option<FormFactor> {
    if mac_model_identifier_ambiguity(id).is_some() {
        return None;
    }
    if id.trim() == "MacPro6,1" {
        return Some(FormFactor::Mini);
    }
    let family: String = id
        .trim()
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect();
    match family.as_str() {
        "MacBook" | "MacBookAir" | "MacBookPro" | "PowerBook" | "iBook" => Some(FormFactor::Laptop),
        "Macmini" => Some(FormFactor::Mini),
        "iMac" | "iMacPro" => Some(FormFactor::AllInOne),
        "MacPro" => Some(FormFactor::Desktop),
        _ => None,
    }
}

/// One SMBIOS system enclosure type (DSP0134, 7.4.1). `None` for "Unknown" (2) and for
/// codes the spec does not define.
pub fn from_chassis_type(code: u16) -> Option<FormFactor> {
    match code {
        // Desktop, Low Profile Desktop, Pizza Box, Mini Tower, Tower, Space-saving,
        // Lunch Box, Main Server Chassis.
        3 | 4 | 5 | 6 | 7 | 15 | 16 | 17 => Some(FormFactor::Desktop),
        // Portable, Laptop, Notebook, Sub Notebook, Tablet, Convertible, Detachable.
        8 | 9 | 10 | 14 | 30 | 31 | 32 => Some(FormFactor::Laptop),
        // Mini PC, Stick PC.
        35 | 36 => Some(FormFactor::Mini),
        13 => Some(FormFactor::AllInOne),
        // Other, Hand Held, Docking Station, Expansion/Sub chassis, Bus Expansion,
        // Peripheral, RAID, Rack Mount, Sealed-case PC, Multi-system, CompactPCI,
        // AdvancedTCA, Blade, Blade Enclosure, IoT Gateway, Embedded PC.
        1 | 11 | 12 | 18..=29 | 33 | 34 => Some(FormFactor::Other),
        _ => None,
    }
}

/// The form factor all recognized chassis types agree on, or why there is none.
pub fn from_chassis_types(codes: &[u16]) -> Result<FormFactor, String> {
    let mut found: Option<FormFactor> = None;
    for &c in codes {
        let Some(f) = from_chassis_type(c) else {
            continue;
        };
        match found {
            Some(prev) if prev != f => {
                return Err(format!(
                    "chassis types {codes:?} describe different form factors"
                ));
            }
            _ => found = Some(f),
        }
    }
    found.ok_or_else(|| {
        if codes.is_empty() {
            "no chassis type reported".to_owned()
        } else {
            format!("chassis types {codes:?} do not identify a form factor")
        }
    })
}

/// Whether a form factor implies an internal battery. `None`: no expectation.
fn expects_battery(f: FormFactor) -> Option<bool> {
    match f {
        FormFactor::Laptop => Some(true),
        FormFactor::Desktop | FormFactor::Mini => Some(false),
        FormFactor::AllInOne | FormFactor::Other => None,
    }
}

fn name(f: FormFactor) -> &'static str {
    match f {
        FormFactor::Desktop => "desktop",
        FormFactor::Laptop => "laptop",
        FormFactor::Mini => "mini",
        FormFactor::AllInOne => "all-in-one",
        FormFactor::Other => "other",
    }
}

/// Final verdict: the primary signal, unless the battery contradicts it.
pub fn corroborate(
    primary: Result<(FormFactor, FormFactorBasis), Unavailability>,
    battery: Option<bool>,
    source: Source,
) -> (Metric<FormFactor>, Option<FormFactorBasis>) {
    let (f, basis) = match primary {
        Ok(p) => p,
        Err(reason) => return (Metric::unavailable(reason), None),
    };
    if let (Some(expected), Some(has)) = (expects_battery(f), battery)
        && expected != has
    {
        let detail = format!(
            "{} indicates {} but an internal battery is {}",
            basis_label(basis),
            name(f),
            if has { "present" } else { "absent" }
        );
        return (
            Metric::unavailable(Unavailability::Inconclusive(detail)),
            None,
        );
    }
    (Metric::available(f, Unit::None, source), Some(basis))
}

fn basis_label(b: FormFactorBasis) -> &'static str {
    match b {
        FormFactorBasis::ProductName => "the product name",
        FormFactorBasis::ModelIdentifier => "the model identifier",
        FormFactorBasis::ChassisType => "the chassis type",
    }
}

/// Primary signal on macOS from the product name and the model identifier.
pub fn mac_primary(
    product_name: Option<&str>,
    model_identifier: Option<&str>,
) -> Result<(FormFactor, FormFactorBasis), Unavailability> {
    let by_name = product_name.and_then(from_mac_product_name);
    let by_id = model_identifier.and_then(from_mac_model_identifier);
    match (by_name, by_id) {
        (Some(a), Some(b)) if a != b => Err(Unavailability::Inconclusive(format!(
            "product name {:?} indicates {} but model identifier {:?} indicates {}",
            product_name.unwrap_or_default(),
            name(a),
            model_identifier.unwrap_or_default(),
            name(b)
        ))),
        (Some(a), _) => Ok((a, FormFactorBasis::ProductName)),
        (None, Some(b)) => Ok((b, FormFactorBasis::ModelIdentifier)),
        (None, None) if product_name.is_none() && model_identifier.is_none() => {
            Err(Unavailability::NotSupported)
        }
        (None, None)
            if model_identifier
                .and_then(mac_model_identifier_ambiguity)
                .is_some() =>
        {
            Err(Unavailability::Inconclusive(format!(
                "{}{}",
                model_identifier
                    .and_then(mac_model_identifier_ambiguity)
                    .unwrap_or_default(),
                product_name.map_or_else(
                    || " and no product name is available".to_owned(),
                    |n| format!(" and product name {n:?} is not recognized"),
                )
            )))
        }
        (None, None) => Err(Unavailability::Inconclusive(format!(
            "product name {:?} and model identifier {:?} not recognized",
            product_name.unwrap_or("none"),
            model_identifier.unwrap_or("none")
        ))),
    }
}

/// Assembles the macOS result from the raw reads.
pub fn mac_info(
    product_name: Option<String>,
    model_identifier: Option<String>,
    internal_battery: Metric<bool>,
) -> FormFactorInfo {
    let primary = mac_primary(product_name.as_deref(), model_identifier.as_deref());
    let (form_factor, basis) =
        corroborate(primary, internal_battery.value().copied(), Source::IoKit);
    FormFactorInfo {
        form_factor,
        basis,
        internal_battery,
        product_name,
        model_identifier,
        chassis_types: Vec::new(),
    }
}

/// Assembles the Windows result. `chassis`: the WMI read (codes or why it failed).
pub fn windows_info(
    chassis: Result<Vec<u16>, Unavailability>,
    internal_battery: Metric<bool>,
) -> FormFactorInfo {
    let (primary, chassis_types) = match chassis {
        Ok(codes) => (
            from_chassis_types(&codes)
                .map(|f| (f, FormFactorBasis::ChassisType))
                .map_err(Unavailability::Inconclusive),
            codes,
        ),
        Err(reason) => (Err(reason), Vec::new()),
    };
    let (form_factor, basis) = corroborate(primary, internal_battery.value().copied(), Source::Wmi);
    FormFactorInfo {
        form_factor,
        basis,
        internal_battery,
        product_name: None,
        model_identifier: None,
        chassis_types,
    }
}

/// `SYSTEM_POWER_STATUS.BatteryFlag` → internal battery present. 128 = "no system
/// battery", 255 = "unknown status"; any other value describes a present battery.
pub fn battery_from_flag(flag: u8) -> Option<bool> {
    match flag {
        128 => Some(false),
        255 => None,
        _ => Some(true),
    }
}

/// Detects the form factor of this machine. Cheap (one registry walk / one WMI query);
/// called at inventory time.
pub fn detect() -> FormFactorInfo {
    #[cfg(target_os = "macos")]
    {
        crate::macos::device::form_factor()
    }
    #[cfg(windows)]
    {
        windows_info(
            crate::windows::wmi::chassis_types(),
            crate::windows::bench_env::internal_battery(),
        )
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        FormFactorInfo {
            form_factor: Metric::unavailable(Unavailability::NotSupported),
            basis: None,
            internal_battery: Metric::unavailable(Unavailability::NotSupported),
            product_name: None,
            model_identifier: None,
            chassis_types: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(b: bool) -> Metric<bool> {
        Metric::available(b, Unit::None, Source::IoKit)
    }

    fn inconclusive(m: &Metric<FormFactor>) -> bool {
        matches!(
            m,
            Metric::Unavailable {
                reason: Unavailability::Inconclusive(_)
            }
        )
    }

    #[test]
    fn mac_product_names() {
        use FormFactor::*;
        for (n, f) in [
            ("MacBook Air (13-inch, M5)", Some(Laptop)),
            ("MacBook Pro (14-inch, M4, 2024)", Some(Laptop)),
            ("MacBook (Retina, 12-inch, 2017)", Some(Laptop)),
            ("Mac mini (2024)", Some(Mini)),
            ("Mac Studio (2025)", Some(Mini)),
            ("iMac (24-inch, 2024)", Some(AllInOne)),
            ("iMac Pro (2017)", Some(AllInOne)),
            ("Mac Pro (2023)", Some(Desktop)),
            ("Mac Pro (Rack, 2023)", Some(Other)),
            ("Apple Virtual Machine 1", None),
            ("", None),
        ] {
            assert_eq!(from_mac_product_name(n), f, "{n}");
        }
    }

    #[test]
    fn mac_model_identifiers() {
        use FormFactor::*;
        for (id, f) in [
            ("MacBookPro16,1", Some(Laptop)),
            ("MacBookAir10,1", Some(Laptop)),
            ("MacBook10,1", Some(Laptop)),
            ("Macmini8,1", Some(Mini)),
            ("iMac20,1", Some(AllInOne)),
            ("iMacPro1,1", Some(AllInOne)),
            ("MacPro5,1", Some(Desktop)),
            ("MacPro6,1", Some(Mini)),
            // Tower and rack-mount Mac Pro (2019) share it.
            ("MacPro7,1", None),
            ("Mac17,3", None),
            ("Mac14,13", None),
            ("VirtualMac2,1", None),
        ] {
            assert_eq!(from_mac_model_identifier(id), f, "{id}");
        }
    }

    #[test]
    fn chassis_types_per_smbios() {
        use FormFactor::*;
        for c in [3, 4, 5, 6, 7, 15, 16, 17] {
            assert_eq!(from_chassis_type(c), Some(Desktop), "{c}");
        }
        for c in [8, 9, 10, 14, 30, 31, 32] {
            assert_eq!(from_chassis_type(c), Some(Laptop), "{c}");
        }
        for c in [35, 36] {
            assert_eq!(from_chassis_type(c), Some(Mini), "{c}");
        }
        assert_eq!(from_chassis_type(13), Some(AllInOne));
        for c in [1, 11, 23, 28, 34] {
            assert_eq!(from_chassis_type(c), Some(Other), "{c}");
        }
        for c in [0, 2, 37, 999] {
            assert_eq!(from_chassis_type(c), None, "{c}");
        }
    }

    #[test]
    fn chassis_lists() {
        assert_eq!(from_chassis_types(&[10]), Ok(FormFactor::Laptop));
        assert_eq!(from_chassis_types(&[2, 3]), Ok(FormFactor::Desktop));
        assert_eq!(from_chassis_types(&[9, 10]), Ok(FormFactor::Laptop));
        assert!(from_chassis_types(&[3, 10]).is_err());
        assert!(from_chassis_types(&[2]).is_err());
        assert!(from_chassis_types(&[]).is_err());
    }

    #[test]
    fn battery_flag() {
        assert_eq!(battery_from_flag(128), Some(false));
        assert_eq!(battery_from_flag(255), None);
        assert_eq!(battery_from_flag(0), Some(true));
        assert_eq!(battery_from_flag(1 | 8), Some(true));
    }

    #[test]
    fn this_macbook_air() {
        let i = mac_info(
            Some("MacBook Air (13-inch, M5)".into()),
            Some("Mac17,3".into()),
            battery(true),
        );
        assert_eq!(i.form_factor.value(), Some(&FormFactor::Laptop));
        assert_eq!(i.basis, Some(FormFactorBasis::ProductName));
    }

    #[test]
    fn battery_contradiction_is_inconclusive() {
        let i = mac_info(Some("Mac mini (2024)".into()), None, battery(true));
        assert!(inconclusive(&i.form_factor));
        assert_eq!(i.basis, None);
        let i = mac_info(Some("MacBook Pro".into()), None, battery(false));
        assert!(inconclusive(&i.form_factor));
        // No battery expectation for an all-in-one.
        let i = mac_info(Some("iMac (24-inch, 2024)".into()), None, battery(false));
        assert_eq!(i.form_factor.value(), Some(&FormFactor::AllInOne));
    }

    #[test]
    fn unknown_battery_keeps_primary() {
        let i = mac_info(
            Some("Mac Studio (2025)".into()),
            Some("Mac16,9".into()),
            Metric::unavailable(Unavailability::NotSupported),
        );
        assert_eq!(i.form_factor.value(), Some(&FormFactor::Mini));
    }

    #[test]
    fn mac_sources_disagree_or_missing() {
        assert!(matches!(
            mac_primary(Some("Mac mini (2018)"), Some("MacBookPro15,1")),
            Err(Unavailability::Inconclusive(_))
        ));
        assert_eq!(
            mac_primary(None, Some("MacPro5,1")),
            Ok((FormFactor::Desktop, FormFactorBasis::ModelIdentifier))
        );
        // Shared by the tower and the rack-mount Mac Pro: the identifier alone never
        // decides, the product name does.
        match mac_primary(None, Some("MacPro7,1")) {
            Err(Unavailability::Inconclusive(d)) => {
                assert!(d.contains("rack-mount"), "{d}");
                assert!(d.contains("no product name"), "{d}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            mac_primary(Some("Mac Pro (Rack, 2019)"), Some("MacPro7,1")),
            Ok((FormFactor::Other, FormFactorBasis::ProductName))
        );
        assert_eq!(
            mac_primary(Some("Mac Pro (2019)"), Some("MacPro7,1")),
            Ok((FormFactor::Desktop, FormFactorBasis::ProductName))
        );
        assert!(matches!(
            mac_primary(None, Some("Mac17,3")),
            Err(Unavailability::Inconclusive(_))
        ));
        assert_eq!(mac_primary(None, None), Err(Unavailability::NotSupported));
        // A battery alone never decides.
        let i = mac_info(None, Some("Mac17,3".into()), battery(true));
        assert!(inconclusive(&i.form_factor));
    }

    #[test]
    fn windows_verdicts() {
        let b = |v| Metric::available(v, Unit::None, Source::PowerApi);
        let i = windows_info(Ok(vec![10]), b(true));
        assert_eq!(i.form_factor.value(), Some(&FormFactor::Laptop));
        assert_eq!(i.basis, Some(FormFactorBasis::ChassisType));
        assert_eq!(i.chassis_types, vec![10]);
        let i = windows_info(Ok(vec![3]), b(false));
        assert_eq!(i.form_factor.value(), Some(&FormFactor::Desktop));
        // Desktop on a UPS (Windows lists it as a battery): not guessed.
        assert!(inconclusive(
            &windows_info(Ok(vec![3]), b(true)).form_factor
        ));
        assert!(inconclusive(
            &windows_info(Ok(vec![2]), b(false)).form_factor
        ));
        let i = windows_info(Err(Unavailability::PermissionDenied), b(false));
        assert_eq!(
            i.form_factor,
            Metric::unavailable(Unavailability::PermissionDenied)
        );
        assert!(i.chassis_types.is_empty());
    }

    // Runs against the real OS. On a Mac the product name must be readable and decide.
    #[cfg(target_os = "macos")]
    #[test]
    fn host_detects_mac_form_factor() {
        let t = std::time::Instant::now();
        let i = detect();
        eprintln!("form factor on host ({:?}): {i:?}", t.elapsed());
        assert!(i.product_name.as_deref().is_some_and(|n| !n.is_empty()));
        assert!(
            i.model_identifier
                .as_deref()
                .is_some_and(|m| m.contains(','))
        );
        assert!(i.internal_battery.is_available());
        if i.product_name
            .as_deref()
            .is_some_and(|n| n.starts_with("MacBook"))
        {
            assert_eq!(i.form_factor.value(), Some(&FormFactor::Laptop));
            assert_eq!(i.internal_battery.value(), Some(&true));
        }
    }
}
