//! macOS providers. They sit behind the same return types from `models` as the Windows
//! providers in `windows/`; anything macOS does not expose stays an explained
//! `Unavailability`.

pub mod process;
