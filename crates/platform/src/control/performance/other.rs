//! Other systems: nothing is reachable; every control is reported unavailable.

use models::control::PowerSupply;
use models::control::performance::{PerformanceControl, PerformanceKeepAwake, PerformanceOsSupply};
use models::{AppError, ErrorCode};

use super::{Access, OsPowerState, PerformanceAccess, WriteFailure};

pub fn current_supply() -> (PowerSupply, bool) {
    (PowerSupply::Ac, false)
}

pub fn read_state() -> OsPowerState {
    OsPowerState {
        supply: PowerSupply::Ac,
        has_battery: false,
        ac: PerformanceOsSupply::default(),
        battery: None,
        scheme_id: None,
        scheme_name: None,
        access: PerformanceAccess {
            mode: Access::NotOnThisOs,
            keep_awake: Access::NotOnThisOs,
            processor_state: Access::NotOnThisOs,
            boost_mode: Access::NotOnThisOs,
            display_timeout: Access::NotOnThisOs,
            sleep_timeout: Access::NotOnThisOs,
        },
    }
}

pub fn write_settings(
    _scheme_id: Option<&str>,
    _current: PowerSupply,
    _ac: &PerformanceOsSupply,
    _battery: Option<&PerformanceOsSupply>,
) -> Vec<WriteFailure> {
    vec![WriteFailure {
        control: PerformanceControl::Mode,
        denied: false,
        detail: "power settings are not supported on this OS".to_owned(),
    }]
}

pub fn open_power_settings() -> Result<(), AppError> {
    Err(AppError::new(
        ErrorCode::NotSupported,
        "no power settings on this OS",
    ))
}

#[derive(Default)]
pub struct KeepAwakeImpl;

impl KeepAwakeImpl {
    pub fn set(&mut self, mode: PerformanceKeepAwake) -> Result<(), AppError> {
        match mode {
            PerformanceKeepAwake::Off => Ok(()),
            _ => Err(AppError::new(
                ErrorCode::NotSupported,
                "keep-awake is not supported on this OS",
            )),
        }
    }
}
