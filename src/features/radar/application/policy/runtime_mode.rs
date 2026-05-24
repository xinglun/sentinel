use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// 売買ロジックと execution を行わない（Radar / 観測 mode）。
    Disabled,
    /// 全ロジックと archive を実行するが、broker execution は bypass する。
    DryRun,
    /// 全ロジック、archive、live order dispatch を実行する。
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
