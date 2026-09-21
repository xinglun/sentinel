//! Acceptance replay の side-effect boundary を定義する application policy。

use super::runtime_mode::ExecutionMode;

/// 不変入力の再計算が canonical publication に昇格しないことを表す policy。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptanceReplayPolicy {
    pub mode: ExecutionMode,
    pub run_purpose: &'static str,
    pub publication_scope: &'static str,
    pub canonical_write: bool,
    pub publishable: bool,
    pub decision_weight: i32,
    pub trade_signal: bool,
    pub gate_effect: &'static str,
}

impl AcceptanceReplayPolicy {
    /// canonical snapshot、data branch、freshness marker を持たない policy を返す。
    pub const fn isolated() -> Self {
        Self {
            mode: ExecutionMode::AcceptanceReplay,
            run_purpose: "ACCEPTANCE_REPLAY",
            publication_scope: "ISOLATED",
            canonical_write: false,
            publishable: false,
            decision_weight: 0,
            trade_signal: false,
            gate_effect: "none",
        }
    }
}
