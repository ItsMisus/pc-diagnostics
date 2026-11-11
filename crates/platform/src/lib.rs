//! OS access layer.
//!
//! Cross-platform facts come from `sysinfo`; Windows-specific providers (PDH, DXGI,
//! `CallNtPowerInformation`, Toolhelp, WMI, NVML) live under `windows/` behind the same
//! return types from `models`. On other platforms those values are `NotSupported`.
//!
//! - [`inventory::collect_inventory`]: slow hardware inventory (cache it).
//! - [`live::LiveCollector`]: cheap per-tick sampling; lives on one thread.
//! - [`overhead::OverheadTracker`]: the app's own CPU/RAM footprint.
//! - [`bench_env`]: read-only run-environment facts for the benchmark engine.
//! - [`gpu_bench`]: GPU benchmark support (adapter list, NVML sensor, worker job object).
//! - [`locale`]: the Windows display language (for `LanguagePreference::System`).
//! - [`credentials`]: the assistant's API key in Windows Credential Manager.
//! - [`remediation`]: validation and native execution of the assistant's whitelisted
//!   actions (`models::assistant::RemediationAction`).

pub mod bench_env;
pub mod convert;
pub mod credentials;
pub mod devices;
pub mod gpu_bench;
pub mod inventory;
pub mod live;
pub mod locale;
pub mod overhead;
pub mod process;
pub mod remediation;
pub mod system;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;
