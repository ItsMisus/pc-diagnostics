//! macOS providers. They sit behind the same return types from `models` as the Windows
//! providers in `windows/`; anything macOS does not expose stays an explained
//! `Unavailability`.

pub mod bench_env;
mod bench_iokit;
pub mod credentials;
pub mod locale;
pub mod process;
pub mod remediation;

pub mod cpu;
pub mod device;
pub mod gpu;
pub mod iokit;
pub mod live;
pub mod mem;
pub mod nvme_smart;
pub mod sensors;
pub mod storage;
pub mod time;
