//! Matching Windows Update driver offers to devices. Pure: no I/O.
//!
//! Rules (docs/driver-verification.md):
//! - An offer applies to a device iff its `DriverHardwareID` equals one of the device's
//!   hardware ids (ASCII case-insensitive, exact). No prefix or fuzzy matching.
//! - When several offers match, the one for the device's most specific hardware id wins
//!   (Windows lists hardware ids most specific first), then the first offered.
//! - No offer ⇒ `NotVerifiable`. Absence of an offer never means "up to date", so
//!   `Verified` is never produced from Windows Update alone.
//! - Versions are never compared and dates never imply "outdated".

use models::drivers::{DeviceRecord, UpdateSource, UpdateStatus};
use models::settings::Language;
use platform::devices::DriverUpdateOffer;

pub fn reason_no_offer(lang: Language) -> &'static str {
    lang.pick(
        "Windows Update non offre aggiornamenti per questo dispositivo: non conferma che la \
         versione installata sia la più recente.",
        "Windows Update offers no update for this device: this doesn't confirm that the \
         installed version is the latest.",
    )
}

pub fn reason_no_hardware_id(lang: Language) -> &'static str {
    lang.pick(
        "Il dispositivo non espone hardware ID: Windows Update non può associargli \
         aggiornamenti dei driver.",
        "The device exposes no hardware ID: Windows Update can't match driver updates to it.",
    )
}

/// The inventory is cached across language changes: rewrites the reasons written by this
/// module into `lang`. Other texts are left as they are.
pub fn localize_reasons(devices: &mut [DeviceRecord], lang: Language) {
    for d in devices {
        if let UpdateStatus::NotVerifiable { reason } = &mut d.update {
            for text in [reason_no_offer, reason_no_hardware_id] {
                if *reason == text(Language::Italian) || *reason == text(Language::English) {
                    text(lang).clone_into(reason);
                }
            }
        }
    }
}

fn is_dotted_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    (2..=4).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 10 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Windows Update exposes no version field for driver updates. Two title shapes carry one:
/// `"Intel Corporation - Display - 31.0.101.5186"` (trailing `" - <dotted version>"`) and
/// `"Apple, Inc. USBDevice Driver Update (552.0.0.0)"` (trailing `"(<dotted version>)"`).
/// Only those exact shapes are used; anything else gives `None`: never guessed.
pub fn version_from_title(title: &str) -> Option<String> {
    let title = title.trim();
    if let Some(inner) = title
        .strip_suffix(')')
        .and_then(|t| t.rsplit_once('('))
        .map(|(head, v)| (head, v.trim()))
        .filter(|(head, _)| !head.trim().is_empty())
        .map(|(_, v)| v)
    {
        return is_dotted_version(inner).then(|| inner.to_owned());
    }
    let (_, last) = title.rsplit_once(" - ")?;
    let last = last.trim();
    is_dotted_version(last).then(|| last.to_owned())
}

/// The status Windows Update implies for one device (reasons in `lang`).
pub fn status_for(
    hardware_ids: &[String],
    offers: &[DriverUpdateOffer],
    lang: Language,
) -> UpdateStatus {
    if hardware_ids.is_empty() {
        return UpdateStatus::NotVerifiable {
            reason: reason_no_hardware_id(lang).to_owned(),
        };
    }
    for hwid in hardware_ids {
        if let Some(offer) = offers
            .iter()
            .find(|o| o.hardware_id.trim().eq_ignore_ascii_case(hwid.trim()))
        {
            return UpdateStatus::UpdateAvailable {
                source: UpdateSource::WindowsUpdate,
                title: offer.title.clone(),
                version: version_from_title(&offer.title),
                date: offer.date.clone(),
                hardware_id: hwid.clone(),
            };
        }
    }
    UpdateStatus::NotVerifiable {
        reason: reason_no_offer(lang).to_owned(),
    }
}

/// Sets `update` on every device from the offers. Several devices may share a hardware id
/// (e.g. identical USB controllers): each one gets the update.
pub fn apply_offers(devices: &mut [DeviceRecord], offers: &[DriverUpdateOffer], lang: Language) {
    for d in devices {
        d.update = status_for(&d.hardware_ids, offers, lang);
    }
}

/// Number of devices that received an offer.
pub fn matched_count(devices: &[DeviceRecord]) -> usize {
    devices
        .iter()
        .filter(|d| matches!(d.update, UpdateStatus::UpdateAvailable { .. }))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::drivers::{DeviceCategory, DeviceHealth};

    const IT: Language = Language::Italian;
    const EN: Language = Language::English;

    fn offer(hwid: &str, title: &str) -> DriverUpdateOffer {
        DriverUpdateOffer {
            title: title.into(),
            hardware_id: hwid.into(),
            provider: Some("Intel".into()),
            manufacturer: None,
            model: None,
            class: Some("Display".into()),
            date: Some("2024-05-01".into()),
        }
    }

    fn device(id: &str, hwids: &[&str]) -> DeviceRecord {
        DeviceRecord {
            instance_id: id.into(),
            name: id.into(),
            manufacturer: None,
            class_name: None,
            class_guid: None,
            category: DeviceCategory::Other,
            hardware_ids: hwids.iter().map(|s| (*s).to_owned()).collect(),
            driver: None,
            health: DeviceHealth::Working,
            problem_code: None,
            problem_description: None,
            update: UpdateStatus::NotChecked,
        }
    }

    #[test]
    fn exact_case_insensitive_match() {
        let offers = [offer(
            "pci\\ven_8086&dev_a780",
            "Intel Corporation - Display - 31.0.101.5186",
        )];
        let s = status_for(
            &[
                "PCI\\VEN_8086&DEV_A780&SUBSYS_1&REV_04".into(),
                "PCI\\VEN_8086&DEV_A780".into(),
            ],
            &offers,
            IT,
        );
        assert_eq!(
            s,
            UpdateStatus::UpdateAvailable {
                source: UpdateSource::WindowsUpdate,
                title: "Intel Corporation - Display - 31.0.101.5186".into(),
                version: Some("31.0.101.5186".into()),
                date: Some("2024-05-01".into()),
                hardware_id: "PCI\\VEN_8086&DEV_A780".into(),
            }
        );
    }

    #[test]
    fn prefix_is_not_a_match() {
        let offers = [offer("PCI\\VEN_8086&DEV_A78", "x")];
        assert!(matches!(
            status_for(&["PCI\\VEN_8086&DEV_A780".into()], &offers, IT),
            UpdateStatus::NotVerifiable { .. }
        ));
    }

    #[test]
    fn no_match_is_not_verifiable_never_verified() {
        let s = status_for(&["USB\\VID_1&PID_2".into()], &[], IT);
        assert_eq!(
            s,
            UpdateStatus::NotVerifiable {
                reason: reason_no_offer(IT).into()
            }
        );
        assert!(matches!(
            status_for(&[], &[offer("X", "t")], IT),
            UpdateStatus::NotVerifiable { reason } if reason == reason_no_hardware_id(IT)
        ));
        assert!(matches!(
            status_for(&[], &[], EN),
            UpdateStatus::NotVerifiable { reason } if reason
                == "The device exposes no hardware ID: Windows Update can't match driver \
                    updates to it."
        ));
    }

    #[test]
    fn cached_reasons_follow_the_language() {
        let mut devices = vec![device("A", &["X"]), device("B", &[])];
        apply_offers(&mut devices, &[], IT);
        assert!(
            matches!(&devices[0].update, UpdateStatus::NotVerifiable { reason }
            if reason.starts_with("Windows Update non offre"))
        );
        localize_reasons(&mut devices, EN);
        assert_eq!(
            devices[0].update,
            UpdateStatus::NotVerifiable {
                reason: "Windows Update offers no update for this device: this doesn't confirm \
                         that the installed version is the latest."
                    .into()
            }
        );
        assert_eq!(
            devices[1].update,
            UpdateStatus::NotVerifiable {
                reason: reason_no_hardware_id(EN).into()
            }
        );
        localize_reasons(&mut devices, IT);
        assert_eq!(
            devices[1].update,
            UpdateStatus::NotVerifiable {
                reason: reason_no_hardware_id(IT).into()
            }
        );
    }

    #[test]
    fn most_specific_hardware_id_wins() {
        let offers = [
            offer("PCI\\VEN_1&DEV_2", "generic"),
            offer("PCI\\VEN_1&DEV_2&SUBSYS_3", "specific"),
        ];
        let s = status_for(
            &[
                "PCI\\VEN_1&DEV_2&SUBSYS_3".into(),
                "PCI\\VEN_1&DEV_2".into(),
            ],
            &offers,
            IT,
        );
        assert!(matches!(s, UpdateStatus::UpdateAvailable { title, .. } if title == "specific"));
    }

    #[test]
    fn devices_sharing_an_id_all_get_the_update() {
        let mut devices = vec![
            device("USB\\A\\1", &["USB\\VID_1&PID_2"]),
            device("USB\\A\\2", &["usb\\vid_1&pid_2"]),
            device("PCI\\B\\1", &["PCI\\VEN_9"]),
        ];
        apply_offers(
            &mut devices,
            &[offer("USB\\VID_1&PID_2", "Vendor - USB - 1.2")],
            IT,
        );
        assert_eq!(matched_count(&devices), 2);
        assert!(matches!(
            devices[2].update,
            UpdateStatus::NotVerifiable { .. }
        ));
        assert!(
            devices
                .iter()
                .all(|d| !matches!(d.update, UpdateStatus::Verified { .. }))
        );
    }

    #[test]
    fn version_only_from_trailing_dotted_numbers() {
        assert_eq!(
            version_from_title("Realtek - Net - 10.68.815.2023").as_deref(),
            Some("10.68.815.2023")
        );
        assert_eq!(
            version_from_title("Intel - System - 2.4.0"),
            Some("2.4.0".into())
        );
        assert_eq!(version_from_title("10.1.2.3"), None);
        assert_eq!(
            version_from_title("Intel - Extension - 1/1/2024 12:00:00 AM"),
            None
        );
        assert_eq!(version_from_title("Lenovo - Firmware - N3HET88W"), None);
        assert_eq!(version_from_title("Vendor - x - 1"), None);
        assert_eq!(version_from_title("Vendor - x - 1..2"), None);
        assert_eq!(version_from_title(""), None);
        // Real title seen on the dev machine (2026-11).
        assert_eq!(
            version_from_title("Apple, Inc. USBDevice Driver Update (552.0.0.0)").as_deref(),
            Some("552.0.0.0")
        );
        assert_eq!(version_from_title("Vendor driver (beta)"), None);
        assert_eq!(version_from_title("(1.2.3)"), None);
    }
}
