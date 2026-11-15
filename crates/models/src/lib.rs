//! Shared data contracts.
//!
//! Every type that crosses the IPC boundary lives here and derives `TS`, so the
//! TypeScript definitions in `src/types/generated` are generated, never hand-written.
//! Regenerate with `npm run gen:types` (runs `cargo test -p models`).

#![forbid(unsafe_code)]

pub mod app;
pub mod assistant;
pub mod benchmark;
pub mod benchmark_run;
pub mod component;
pub mod control;
pub mod device;
pub mod diagnostics;
pub mod drivers;
pub mod error;
pub mod hardware;
pub mod metric;
pub mod monitor;
pub mod process;
pub mod settings;
pub mod system;
pub mod time;

pub use error::{AppError, ErrorCode};
pub use metric::{Metric, Source, Unavailability, Unit};
