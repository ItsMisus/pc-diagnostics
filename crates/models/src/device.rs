//! Physical form factor of the computer (tower, laptop, mini PC, ...), detected at
//! inventory time from what the OS reports about the machine itself. Never derived from
//! the window or screen size.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::metric::Metric;

/// Shape of the computer as the firmware / OS describes it. Deliberately coarse: only
/// what the sources can tell apart reliably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FormFactor {
    /// Full-size desktop (tower, mini tower, low-profile desktop). Mac Pro (tower).
    Desktop,
    /// Laptop, notebook, convertible, detachable or tablet with keyboard. MacBook*.
    Laptop,
    /// Compact desktop: mini PC, stick PC. Mac mini and Mac Studio.
    Mini,
    /// Computer built into the display. iMac.
    AllInOne,
    /// A known shape that is none of the above (rack mount, server blade, embedded,
    /// handheld, virtual machine reporting "Other").
    Other,
}

impl FormFactor {
    /// The 3D model that represents this form factor, if there is one. `Other` has none:
    /// the UI asks the user instead of picking a look-alike.
    pub fn shape(self) -> Option<DeviceShape> {
        match self {
            Self::Desktop => Some(DeviceShape::Tower),
            Self::Laptop => Some(DeviceShape::Laptop),
            Self::Mini => Some(DeviceShape::Mini),
            Self::AllInOne => Some(DeviceShape::AllInOne),
            Self::Other => None,
        }
    }
}

/// Device model shown in the Overview. Also the user's manual choice in Settings
/// (`AppSettings::device_shape`), which wins over detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DeviceShape {
    Tower,
    Laptop,
    Mini,
    /// A display with the computer built in.
    AllInOne,
}

/// Which signal decided `FormFactorInfo::form_factor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FormFactorBasis {
    /// macOS device-tree product name (`IODeviceTree:/product` `product-name`).
    ProductName,
    /// macOS model identifier (`hw.model`, e.g. `MacBookPro16,1` on Intel Macs).
    ModelIdentifier,
    /// SMBIOS system enclosure type (`Win32_SystemEnclosure.ChassisTypes`).
    ChassisType,
}

/// Detected form factor plus the evidence behind it, so the UI can say where it comes
/// from ("rilevato da ...") or ask the user when it is unknown.
///
/// `form_factor` is `Available` only when the primary signal is recognized and the
/// battery check does not contradict it; otherwise it is `Unavailable` with
/// `Inconclusive` (signals disagree or none is decisive, detail says which) or the
/// reason the sources could not be read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FormFactorInfo {
    pub form_factor: Metric<FormFactor>,
    /// The signal that decided `form_factor`. `None` when it is unavailable.
    pub basis: Option<FormFactorBasis>,
    /// Whether the machine has an internal battery (corroboration only: a battery alone
    /// never decides the form factor). macOS: `IOPMPowerSource` `BatteryInstalled`;
    /// Windows: `GetSystemPowerStatus` `BatteryFlag`.
    pub internal_battery: Metric<bool>,
    /// macOS product name, e.g. "MacBook Air (13-inch, M5)". `None` elsewhere or when
    /// the device tree has none (Intel Macs).
    pub product_name: Option<String>,
    /// macOS model identifier (`hw.model`), e.g. "Mac17,3". `None` elsewhere.
    pub model_identifier: Option<String>,
    /// Windows SMBIOS chassis type codes as reported (usually one). Empty elsewhere or
    /// when WMI returned none.
    pub chassis_types: Vec<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metric::{Source, Unavailability, Unit};

    #[test]
    fn shapes() {
        assert_eq!(FormFactor::Desktop.shape(), Some(DeviceShape::Tower));
        assert_eq!(FormFactor::Laptop.shape(), Some(DeviceShape::Laptop));
        assert_eq!(FormFactor::Mini.shape(), Some(DeviceShape::Mini));
        assert_eq!(FormFactor::AllInOne.shape(), Some(DeviceShape::AllInOne));
        assert_eq!(FormFactor::Other.shape(), None);
    }

    #[test]
    fn serializes_camel_case() {
        let info = FormFactorInfo {
            form_factor: Metric::available(FormFactor::AllInOne, Unit::None, Source::IoKit),
            basis: Some(FormFactorBasis::ProductName),
            internal_battery: Metric::unavailable(Unavailability::NotSupported),
            product_name: Some("iMac (24-inch, 2024)".into()),
            model_identifier: Some("Mac16,2".into()),
            chassis_types: vec![],
        };
        let v = serde_json::to_value(&info).expect("serialize");
        assert_eq!(v["formFactor"]["value"], "allInOne");
        assert_eq!(v["basis"], "productName");
        assert_eq!(v["internalBattery"]["status"], "unavailable");
        assert_eq!(v["productName"], "iMac (24-inch, 2024)");
        assert_eq!(
            serde_json::to_value(DeviceShape::Tower).expect("s"),
            "tower"
        );
    }
}
