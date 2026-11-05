//! Device category from the setup class (name, then GUID) plus hardware-id hints.
//!
//! Setup class names come from INF files and are not localized, so they are the primary
//! key; the class GUID is the fallback when the name is absent. Network adapters are split
//! by bus and wireless hints; see docs/driver-verification.md for the heuristic.

use models::drivers::DeviceCategory;

/// Well-known setup class GUIDs (lower case, braces), from `devguid.h`.
const CLASS_GUIDS: &[(&str, &str)] = &[
    ("{4d36e968-e325-11ce-bfc1-08002be10318}", "display"),
    ("{4d36e97d-e325-11ce-bfc1-08002be10318}", "system"),
    ("{4d36e96c-e325-11ce-bfc1-08002be10318}", "media"),
    ("{c166523c-fe0c-4a94-a586-f1a80cfbbf3e}", "audioendpoint"),
    ("{4d36e972-e325-11ce-bfc1-08002be10318}", "net"),
    ("{e0cbf06c-cd8b-4647-bb8a-263b43f0f974}", "bluetooth"),
    ("{4d36e97b-e325-11ce-bfc1-08002be10318}", "scsiadapter"),
    ("{4d36e96a-e325-11ce-bfc1-08002be10318}", "hdc"),
    ("{4d36e967-e325-11ce-bfc1-08002be10318}", "diskdrive"),
    ("{36fc9e60-c465-11cf-8056-444553540000}", "usb"),
    ("{88bae032-5a81-49f0-bc3d-a4ff138216d6}", "usbdevice"),
    ("{745a17a0-74d3-11d0-b6fe-00a0c90f57da}", "hidclass"),
    ("{4d36e96b-e325-11ce-bfc1-08002be10318}", "keyboard"),
    ("{4d36e96f-e325-11ce-bfc1-08002be10318}", "mouse"),
    ("{4d36e96e-e325-11ce-bfc1-08002be10318}", "monitor"),
    ("{f2e7dd72-6468-4e36-b6f1-6488f42c1b52}", "firmware"),
];

/// Enumerators of physical buses. Network adapters on any other enumerator (`ROOT`, `SWD`,
/// `BTH`, `VMBUS`, `{…}` software enumerators, WAN miniports, …) are virtual.
const PHYSICAL_NET_ENUMERATORS: &[&str] = &["PCI", "USB", "SD", "SDIO", "ACPI"];

const WIRELESS_HINTS: &[&str] = &[
    "wi-fi", "wifi", "wireless", "802.11", "wlan", "wi fi", "airport",
];

/// Inputs needed to categorize a device.
pub struct CategoryInput<'a> {
    pub class_name: Option<&'a str>,
    pub class_guid: Option<&'a str>,
    pub instance_id: &'a str,
    pub name: &'a str,
    pub hardware_ids: &'a [String],
}

fn class_key(input: &CategoryInput<'_>) -> Option<String> {
    if let Some(n) = input.class_name.filter(|n| !n.trim().is_empty()) {
        return Some(n.trim().to_ascii_lowercase());
    }
    let guid = input.class_guid?.to_ascii_lowercase();
    CLASS_GUIDS
        .iter()
        .find(|(g, _)| *g == guid)
        .map(|(_, n)| (*n).to_owned())
}

/// First component of a device instance id (`PCI\VEN_…` → `PCI`), upper case.
pub fn enumerator(instance_id: &str) -> String {
    instance_id
        .split('\\')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
}

fn is_wireless(input: &CategoryInput<'_>) -> bool {
    let name = input.name.to_ascii_lowercase();
    WIRELESS_HINTS.iter().any(|h| name.contains(h))
        || input.hardware_ids.iter().any(|id| {
            let id = id.to_ascii_lowercase();
            WIRELESS_HINTS.iter().any(|h| id.contains(h))
        })
}

fn net_category(input: &CategoryInput<'_>) -> DeviceCategory {
    let en = enumerator(input.instance_id);
    if !PHYSICAL_NET_ENUMERATORS.contains(&en.as_str()) {
        return DeviceCategory::Other;
    }
    if is_wireless(input) {
        DeviceCategory::Wifi
    } else {
        DeviceCategory::Ethernet
    }
}

pub fn categorize(input: &CategoryInput<'_>) -> DeviceCategory {
    let Some(key) = class_key(input) else {
        return DeviceCategory::Other;
    };
    match key.as_str() {
        "display" => DeviceCategory::Gpu,
        "system" | "firmware" => DeviceCategory::Chipset,
        "media" | "audioendpoint" => DeviceCategory::Audio,
        "net" => net_category(input),
        "bluetooth" => DeviceCategory::Bluetooth,
        "scsiadapter" | "hdc" => DeviceCategory::StorageController,
        "diskdrive" => DeviceCategory::Disk,
        "usb" | "usbdevice" => DeviceCategory::Usb,
        "hidclass" | "keyboard" | "mouse" => DeviceCategory::Input,
        "monitor" => DeviceCategory::Display,
        _ => DeviceCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(
        class: Option<&str>,
        guid: Option<&str>,
        id: &str,
        name: &str,
        hw: &[&str],
    ) -> DeviceCategory {
        let hw: Vec<String> = hw.iter().map(|s| (*s).to_owned()).collect();
        categorize(&CategoryInput {
            class_name: class,
            class_guid: guid,
            instance_id: id,
            name,
            hardware_ids: &hw,
        })
    }

    #[test]
    fn class_names_map_case_insensitively() {
        let id = "PCI\\VEN_1\\0";
        assert_eq!(
            cat(Some("Display"), None, id, "GPU", &[]),
            DeviceCategory::Gpu
        );
        assert_eq!(
            cat(Some("System"), None, id, "PCI bridge", &[]),
            DeviceCategory::Chipset
        );
        assert_eq!(
            cat(Some("Firmware"), None, id, "UEFI", &[]),
            DeviceCategory::Chipset
        );
        assert_eq!(
            cat(Some("MEDIA"), None, id, "Realtek", &[]),
            DeviceCategory::Audio
        );
        assert_eq!(
            cat(Some("AudioEndpoint"), None, id, "Speakers", &[]),
            DeviceCategory::Audio
        );
        assert_eq!(
            cat(Some("Bluetooth"), None, id, "BT", &[]),
            DeviceCategory::Bluetooth
        );
        assert_eq!(
            cat(Some("SCSIAdapter"), None, id, "NVMe", &[]),
            DeviceCategory::StorageController
        );
        assert_eq!(
            cat(Some("hdc"), None, id, "SATA AHCI", &[]),
            DeviceCategory::StorageController
        );
        assert_eq!(
            cat(Some("DiskDrive"), None, id, "SSD", &[]),
            DeviceCategory::Disk
        );
        assert_eq!(cat(Some("USB"), None, id, "xHCI", &[]), DeviceCategory::Usb);
        assert_eq!(
            cat(Some("USBDevice"), None, id, "dev", &[]),
            DeviceCategory::Usb
        );
        assert_eq!(
            cat(Some("HIDClass"), None, id, "HID", &[]),
            DeviceCategory::Input
        );
        assert_eq!(
            cat(Some("Keyboard"), None, id, "kbd", &[]),
            DeviceCategory::Input
        );
        assert_eq!(
            cat(Some("Mouse"), None, id, "mouse", &[]),
            DeviceCategory::Input
        );
        assert_eq!(
            cat(Some("Monitor"), None, id, "Generic PnP Monitor", &[]),
            DeviceCategory::Display
        );
        assert_eq!(
            cat(Some("Processor"), None, id, "CPU", &[]),
            DeviceCategory::Other
        );
        assert_eq!(
            cat(Some("SoftwareComponent"), None, id, "x", &[]),
            DeviceCategory::Other
        );
        assert_eq!(cat(None, None, id, "x", &[]), DeviceCategory::Other);
    }

    #[test]
    fn guid_is_the_fallback() {
        assert_eq!(
            cat(
                None,
                Some("{4D36E968-E325-11CE-BFC1-08002BE10318}"),
                "PCI\\X",
                "GPU",
                &[]
            ),
            DeviceCategory::Gpu
        );
        assert_eq!(
            cat(
                Some(" "),
                Some("{4d36e967-e325-11ce-bfc1-08002be10318}"),
                "SCSI\\X",
                "d",
                &[]
            ),
            DeviceCategory::Disk
        );
        assert_eq!(
            cat(
                None,
                Some("{00000000-1111-2222-3333-444444444444}"),
                "X",
                "d",
                &[]
            ),
            DeviceCategory::Other
        );
    }

    #[test]
    fn physical_net_adapters_split_by_wireless_hints() {
        assert_eq!(
            cat(
                Some("Net"),
                None,
                "PCI\\VEN_8086&DEV_2725\\1",
                "Intel(R) Wi-Fi 6E AX210 160MHz",
                &[]
            ),
            DeviceCategory::Wifi
        );
        assert_eq!(
            cat(
                Some("Net"),
                None,
                "USB\\VID_0BDA\\1",
                "Realtek 8811CU Wireless LAN 802.11ac USB NIC",
                &[]
            ),
            DeviceCategory::Wifi
        );
        assert_eq!(
            cat(
                Some("Net"),
                None,
                "PCI\\VEN_10EC&DEV_8125\\1",
                "Realtek Gaming 2.5GbE Family Controller",
                &[]
            ),
            DeviceCategory::Ethernet
        );
        assert_eq!(
            cat(
                Some("Net"),
                None,
                "pci\\ven_1\\1",
                "Adapter",
                &["PCI\\VEN_1&DEV_2&WLAN"]
            ),
            DeviceCategory::Wifi
        );
    }

    #[test]
    fn virtual_net_adapters_are_other() {
        for (id, name) in [
            ("ROOT\\VMS_MP\\0000", "Hyper-V Virtual Ethernet Adapter"),
            ("SWD\\MSRRAS\\MS_NDISWANIP", "WAN Miniport (IP)"),
            (
                "BTH\\MS_BTHPAN\\7&1",
                "Bluetooth Device (Personal Area Network)",
            ),
            ("SWD\\NETCFG\\{X}", "Microsoft Wi-Fi Direct Virtual Adapter"),
            ("ROOT\\NET\\0000", "TAP-Windows Adapter V9"),
        ] {
            assert_eq!(
                cat(Some("Net"), None, id, name, &[]),
                DeviceCategory::Other,
                "{name}"
            );
        }
    }

    #[test]
    fn enumerator_is_first_component() {
        assert_eq!(enumerator("pci\\VEN_1\\0"), "PCI");
        assert_eq!(enumerator("HTREE\\ROOT\\0"), "HTREE");
        assert_eq!(enumerator(""), "");
    }
}
