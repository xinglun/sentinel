use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// No trading logic or execution (Radar/Observation mode).
    Disabled,
    /// Full logic and archival, but broker execution is bypassed.
    DryRun,
    /// Full logic, archival, and live order dispatch.
    Live,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Disabled => write!(f, "DISABLED"),
            ExecutionMode::DryRun => write!(f, "DRY-RUN"),
            ExecutionMode::Live => write!(f, "LIVE"),
        }
    }
}
