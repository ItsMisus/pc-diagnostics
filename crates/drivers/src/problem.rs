//! Configuration Manager problem codes (`CM_PROB_*`) → device health and description.
//!
//! Descriptions follow Device Manager's "Code N: …" wording. `DriverMissing` is used only
//! for codes that state that no driver is installed or that the driver association is
//! broken; every other code is `ProblemDetected`. See docs/driver-verification.md.

use models::drivers::DeviceHealth;
use platform::devices::DevNodeStatus;

pub const CM_PROB_NOT_CONFIGURED: u32 = 1;
pub const CM_PROB_REINSTALL: u32 = 18;
pub const CM_PROB_DISABLED: u32 = 22;
pub const CM_PROB_FAILED_INSTALL: u32 = 28;

/// `(code, Device Manager text)`. Codes 2–8, 11, 13, 15, 17, 20, 23, 25–27, 30 are legacy
/// (Windows 9x era) and practically never reported; they get short neutral texts.
const DESCRIPTIONS: &[(u32, &str)] = &[
    (1, "This device is not configured correctly."),
    (
        2,
        "Windows cannot load the driver for this device (device loader failed).",
    ),
    (
        3,
        "The driver for this device might be corrupted, or the system may be running low on memory or other resources.",
    ),
    (
        4,
        "This device is not working properly: a driver or registry entry has the wrong type.",
    ),
    (
        5,
        "The driver for this device requested a resource that Windows does not know how to handle.",
    ),
    (
        6,
        "The boot configuration for this device conflicts with other devices.",
    ),
    (7, "Cannot filter."),
    (8, "The driver loader for this device is missing."),
    (
        9,
        "Windows cannot identify this hardware because it does not have a valid hardware identification number.",
    ),
    (10, "This device cannot start."),
    (11, "This device failed."),
    (
        12,
        "This device cannot find enough free resources that it can use.",
    ),
    (13, "Windows cannot verify this device's resources."),
    (
        14,
        "This device cannot work properly until you restart your computer.",
    ),
    (
        15,
        "This device is not working properly because there is probably a re-enumeration problem.",
    ),
    (
        16,
        "Windows cannot identify all the resources this device uses.",
    ),
    (17, "This device is asking for an unknown resource type."),
    (18, "Reinstall the drivers for this device."),
    (
        19,
        "Windows cannot start this hardware device because its configuration information (in the registry) is incomplete or damaged.",
    ),
    (20, "Failure using the VxD loader."),
    (21, "Windows is removing this device."),
    (22, "This device is disabled."),
    (23, "The device loader for this device is not ready."),
    (
        24,
        "This device is not present, is not working properly, or does not have all its drivers installed.",
    ),
    (25, "Windows is still setting up this device."),
    (26, "Windows is still setting up this device."),
    (27, "Windows cannot specify the resources for this device."),
    (28, "The drivers for this device are not installed."),
    (
        29,
        "This device is disabled because the firmware of the device did not give it the required resources.",
    ),
    (
        30,
        "This device is using an Interrupt Request (IRQ) resource that another device is using.",
    ),
    (
        31,
        "This device is not working properly because Windows cannot load the drivers required for this device.",
    ),
    (32, "A driver (service) for this device has been disabled."),
    (
        33,
        "Windows cannot determine which resources are required for this device.",
    ),
    (34, "Windows cannot determine the settings for this device."),
    (
        35,
        "The system firmware does not include enough information to properly configure and use this device.",
    ),
    (
        36,
        "This device is requesting a PCI interrupt but is configured for an ISA interrupt (or vice versa).",
    ),
    (
        37,
        "Windows cannot initialize the device driver for this hardware.",
    ),
    (
        38,
        "Windows cannot load the device driver for this hardware because a previous instance of the device driver is still in memory.",
    ),
    (
        39,
        "Windows cannot load the device driver for this hardware. The driver may be corrupted or missing.",
    ),
    (
        40,
        "Windows cannot access this hardware because its service key information in the registry is missing or recorded incorrectly.",
    ),
    (
        41,
        "Windows successfully loaded the device driver for this hardware but cannot find the hardware device.",
    ),
    (
        42,
        "Windows cannot load the device driver for this hardware because there is a duplicate device already running in the system.",
    ),
    (
        43,
        "Windows has stopped this device because it has reported problems.",
    ),
    (
        44,
        "An application or service has shut down this hardware device.",
    ),
    (
        45,
        "Currently, this hardware device is not connected to the computer.",
    ),
    (
        46,
        "Windows cannot gain access to this hardware device because the operating system is in the process of shutting down.",
    ),
    (
        47,
        "Windows cannot use this hardware device because it has been prepared for safe removal, but it has not been removed from the computer.",
    ),
    (
        48,
        "The software for this device has been blocked from starting because it is known to have problems with Windows.",
    ),
    (
        49,
        "Windows cannot start new hardware devices because the system hive is too large (exceeds the Registry Size Limit).",
    ),
    (
        50,
        "Windows cannot apply all of the properties for this device.",
    ),
    (
        51,
        "This device is currently waiting on another device or set of devices to start.",
    ),
    (
        52,
        "Windows cannot verify the digital signature for the drivers required for this device.",
    ),
    (
        53,
        "This device has been reserved for use by the Windows kernel debugger.",
    ),
    (54, "This device has failed and is undergoing a reset."),
    (
        55,
        "This device is not available while the console is locked.",
    ),
    (
        56,
        "Windows is still setting up the class configuration for this device.",
    ),
    (
        57,
        "This device could not be assigned to a virtual machine.",
    ),
];

/// Device Manager style text: `"Code 43: Windows has stopped this device …"`.
pub fn problem_description(code: u32) -> String {
    let text = DESCRIPTIONS
        .iter()
        .find(|(c, _)| *c == code)
        .map_or("Windows reports a problem with this device.", |(_, t)| *t);
    format!("Code {code}: {text}")
}

/// Health for a problem code. Only the CfgMgr32 code decides; driver age never does.
pub fn health_for_problem(code: u32) -> DeviceHealth {
    match code {
        CM_PROB_DISABLED => DeviceHealth::Disabled,
        // "Not configured" (no driver selected), "reinstall the drivers" (install
        // incomplete) and "drivers not installed": no usable driver association.
        CM_PROB_NOT_CONFIGURED | CM_PROB_REINSTALL | CM_PROB_FAILED_INSTALL => {
            DeviceHealth::DriverMissing
        }
        _ => DeviceHealth::ProblemDetected,
    }
}

/// `(health, problem_code, problem_description)` for a device node status.
///
/// No problem code ⇒ `Working`, as in Device Manager ("This device is working properly"),
/// whether or not `DN_STARTED` is set (a few software-only devices are never "started").
/// A status that CfgMgr32 could not report is `ProblemDetected` without a code, with the
/// failure as description: it is never silently shown as working.
pub fn classify(
    status: &Result<DevNodeStatus, String>,
) -> (DeviceHealth, Option<u32>, Option<String>) {
    match status {
        Ok(DevNodeStatus {
            problem_code: None, ..
        }) => (DeviceHealth::Working, None, None),
        Ok(DevNodeStatus {
            problem_code: Some(code),
            ..
        }) => (
            health_for_problem(*code),
            Some(*code),
            Some(problem_description(*code)),
        ),
        Err(e) => (
            DeviceHealth::ProblemDetected,
            None,
            Some(format!(
                "Windows did not report the status of this device ({e})."
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(problem: Option<u32>) -> Result<DevNodeStatus, String> {
        Ok(DevNodeStatus {
            started: problem.is_none(),
            problem_code: problem,
        })
    }

    #[test]
    fn no_problem_is_working() {
        assert_eq!(classify(&st(None)), (DeviceHealth::Working, None, None));
        let not_started = Ok(DevNodeStatus {
            started: false,
            problem_code: None,
        });
        assert_eq!(classify(&not_started).0, DeviceHealth::Working);
    }

    #[test]
    fn disabled_is_not_a_malfunction() {
        let (h, c, d) = classify(&st(Some(22)));
        assert_eq!(h, DeviceHealth::Disabled);
        assert_eq!(c, Some(22));
        assert_eq!(d.as_deref(), Some("Code 22: This device is disabled."));
    }

    #[test]
    fn driver_missing_only_for_driver_association_codes() {
        for code in [1, 18, 28] {
            assert_eq!(
                health_for_problem(code),
                DeviceHealth::DriverMissing,
                "code {code}"
            );
        }
        // Load/start failures of an installed driver are problems, not "missing".
        for code in [10, 31, 37, 39, 43, 48, 52, 29, 45] {
            assert_eq!(
                health_for_problem(code),
                DeviceHealth::ProblemDetected,
                "code {code}"
            );
        }
    }

    #[test]
    fn descriptions_are_device_manager_style() {
        assert_eq!(
            problem_description(43),
            "Code 43: Windows has stopped this device because it has reported problems."
        );
        assert_eq!(
            problem_description(28),
            "Code 28: The drivers for this device are not installed."
        );
        assert_eq!(
            problem_description(999),
            "Code 999: Windows reports a problem with this device."
        );
    }

    #[test]
    fn description_table_is_complete_and_unique() {
        for code in 1..=57 {
            assert_eq!(
                DESCRIPTIONS.iter().filter(|(c, _)| *c == code).count(),
                1,
                "code {code}"
            );
        }
        assert!(DESCRIPTIONS.iter().all(|(_, t)| t.ends_with('.')));
    }

    #[test]
    fn unknown_status_is_never_working() {
        let (h, c, d) = classify(&Err("CONFIGRET 13".into()));
        assert_eq!(h, DeviceHealth::ProblemDetected);
        assert_eq!(c, None);
        assert!(d.is_some_and(|d| d.contains("CONFIGRET 13")));
    }
}
