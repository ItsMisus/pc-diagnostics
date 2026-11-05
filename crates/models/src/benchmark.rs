//! Benchmark contracts. The engine (crate `bench`, phase 5) implements them; the
//! state machine lives here so the backend controller and the UI agree on legal states.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::component::ComponentKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BenchmarkKind {
    CpuSingleCore,
    CpuMultiCore,
    CpuSustained,
    MemoryBandwidth,
    MemoryLatency,
    StorageSequential,
    StorageRandom,
    GpuCompute,
}

impl BenchmarkKind {
    pub fn component(self) -> ComponentKind {
        match self {
            Self::CpuSingleCore | Self::CpuMultiCore | Self::CpuSustained => ComponentKind::Cpu,
            Self::MemoryBandwidth | Self::MemoryLatency => ComponentKind::Memory,
            Self::StorageSequential | Self::StorageRandom => ComponentKind::Storage,
            Self::GpuCompute => ComponentKind::Gpu,
        }
    }
}

/// Lifecycle of one benchmark run.
///
/// ```text
/// Idle -> Preparing -> Running -> Completed
///             |           |
///             +-----------+--> Cancelling -> Cancelled
///             +-----------+--> Failed
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BenchmarkState {
    Idle,
    Preparing,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl BenchmarkState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }

    /// True while the run holds the exclusive heavy-benchmark slot.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Preparing | Self::Running | Self::Cancelling)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use BenchmarkState::*;
        matches!(
            (self, next),
            (Idle, Preparing)
                | (Preparing, Running)
                | (Preparing | Running, Cancelling)
                | (Preparing | Running | Cancelling, Failed)
                | (Running, Completed)
                | (Cancelling, Cancelled)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::BenchmarkState::*;
    use super::*;

    const ALL: [BenchmarkState; 7] = [
        Idle, Preparing, Running, Cancelling, Completed, Cancelled, Failed,
    ];

    #[test]
    fn happy_path_is_legal() {
        assert!(Idle.can_transition_to(Preparing));
        assert!(Preparing.can_transition_to(Running));
        assert!(Running.can_transition_to(Completed));
    }

    #[test]
    fn cancel_path_is_legal_from_preparing_and_running() {
        assert!(Preparing.can_transition_to(Cancelling));
        assert!(Running.can_transition_to(Cancelling));
        assert!(Cancelling.can_transition_to(Cancelled));
    }

    #[test]
    fn terminal_states_never_transition() {
        for from in ALL.iter().copied().filter(|s| s.is_terminal()) {
            for to in ALL {
                assert!(!from.can_transition_to(to), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn cannot_complete_while_cancelling() {
        assert!(!Cancelling.can_transition_to(Completed));
        assert!(!Idle.can_transition_to(Running));
    }

    #[test]
    fn active_and_terminal_are_disjoint() {
        for s in ALL {
            assert!(!(s.is_active() && s.is_terminal()));
        }
    }

    #[test]
    fn kinds_map_to_components() {
        assert_eq!(BenchmarkKind::CpuSustained.component(), ComponentKind::Cpu);
        assert_eq!(
            BenchmarkKind::StorageRandom.component(),
            ComponentKind::Storage
        );
        assert_eq!(BenchmarkKind::GpuCompute.component(), ComponentKind::Gpu);
    }
}
