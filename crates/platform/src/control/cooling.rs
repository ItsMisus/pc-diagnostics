//! OS access for the cooling center. Owned by the cooling feature.
//!
//! - Fans: macOS reads the SMC (`smc`, read only: forcing a fan needs root); Windows lists
//!   `Win32_Fan` through WMI (read only: Windows has no general fan API without vendor
//!   drivers). Elsewhere: not supported.
//! - The Windows "System cooling policy" power setting of the active scheme, per supply
//!   (read and write, no admin for a standard user unless a policy forbids it).
//!
//! Facts only: the control crate decides support and words the reasons.

#[cfg(target_os = "macos")]
mod smc;
#[cfg(windows)]
mod win;

/// Where fan readings come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanSource {
    AppleSmc,
    Wmi,
}

/// One fan as read from the system.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FanReading {
    pub id: String,
    /// Name the system gives the fan, if any.
    pub label: Option<String>,
    pub current_rpm: Option<f64>,
    pub min_rpm: Option<f64>,
    pub max_rpm: Option<f64>,
    pub target_rpm: Option<f64>,
    /// `Some(true)` when a fixed speed is forced.
    pub forced: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FanProbe {
    /// The source answered. `declared` is the fan count the source states (`FNum`), `None`
    /// when it states none.
    Read {
        source: FanSource,
        declared: Option<u32>,
        fans: Vec<FanReading>,
    },
    /// The source exists on this OS but could not be read.
    Unavailable { source: FanSource, detail: String },
    /// No fan source on this OS.
    NotSupported,
}

/// Raw values (0 passive, 1 active) of the cooling policy of the active scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PolicyValues {
    pub ac: Option<u32>,
    pub dc: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// Not a Windows computer.
    NotSupported,
    /// The setting does not exist in the active scheme.
    NotFound,
    /// `ERROR_ACCESS_DENIED`: a policy forbids changing power settings.
    AccessDenied,
    Failed(String),
}

/// Reads the fans. Blocking (SMC call or a WMI query, bounded).
pub fn probe_fans() -> FanProbe {
    #[cfg(target_os = "macos")]
    {
        probe_smc()
    }
    #[cfg(windows)]
    {
        win::probe_fans()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        FanProbe::NotSupported
    }
}

#[cfg(target_os = "macos")]
fn probe_smc() -> FanProbe {
    let smc = match smc::Smc::open() {
        Ok(s) => s,
        Err(detail) => {
            return FanProbe::Unavailable {
                source: FanSource::AppleSmc,
                detail,
            };
        }
    };
    let declared = match smc.read("FNum") {
        Ok(v) => v.as_f64().map(|n| n.clamp(0.0, 16.0) as u32),
        Err(smc::ReadError::Missing) => None,
        Err(smc::ReadError::Failed(detail)) => {
            return FanProbe::Unavailable {
                source: FanSource::AppleSmc,
                detail,
            };
        }
    };
    let fans = (0..declared.unwrap_or(0))
        .map(|n| {
            let rpm = |suffix: &str| {
                smc.read_f64(&format!("F{n}{suffix}"))
                    .filter(|v| (0.0..=20_000.0).contains(v))
            };
            FanReading {
                id: format!("smc-{n}"),
                label: None,
                current_rpm: rpm("Ac"),
                min_rpm: rpm("Mn"),
                max_rpm: rpm("Mx"),
                target_rpm: rpm("Tg"),
                forced: smc.read_f64(&format!("F{n}Md")).map(|m| m >= 1.0),
            }
        })
        .collect();
    FanProbe::Read {
        source: FanSource::AppleSmc,
        declared,
        fans,
    }
}

/// Whether this computer is a laptop, from the form factor detection (`None`: unknown).
pub fn is_laptop() -> Option<bool> {
    crate::form_factor::detect()
        .form_factor
        .value()
        .map(|f| *f == models::device::FormFactor::Laptop)
}

/// The cooling policy of the active power scheme.
pub fn read_cooling_policy() -> Result<PolicyValues, PolicyError> {
    #[cfg(windows)]
    {
        win::read_policy()
    }
    #[cfg(not(windows))]
    {
        Err(PolicyError::NotSupported)
    }
}

/// Writes the cooling policy of the active scheme for the given supplies (`None` leaves
/// that supply alone) and re-applies the scheme so it takes effect.
pub fn write_cooling_policy(values: PolicyValues) -> Result<(), PolicyError> {
    #[cfg(windows)]
    {
        win::write_policy(values)
    }
    #[cfg(not(windows))]
    {
        let _ = values;
        Err(PolicyError::NotSupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints what the backend reads on this computer (run with `--ignored --nocapture`).
    #[test]
    #[ignore = "prints host readings"]
    fn print_host_cooling() {
        println!("fans: {:#?}", probe_fans());
        println!("laptop: {:?}", is_laptop());
        println!("cooling policy: {:?}", read_cooling_policy());
    }

    #[test]
    fn non_windows_has_no_cooling_policy() {
        if cfg!(not(windows)) {
            assert_eq!(read_cooling_policy(), Err(PolicyError::NotSupported));
            assert_eq!(
                write_cooling_policy(PolicyValues::default()),
                Err(PolicyError::NotSupported)
            );
        }
    }
}
