//! OS access for the control centers. Each submodule is owned by one center and keeps its
//! macOS and Windows code behind `cfg`, with a portable fallback that reports the control as
//! unsupported, so the other centers keep working whatever one of them finds.

pub mod cooling;
pub mod gaming;
pub mod lighting;
pub mod performance;
