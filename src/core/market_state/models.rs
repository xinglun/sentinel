use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleState {
    Startup,
    Transition,
    Ready,
}

impl Default for LifecycleState {
    fn default() -> Self {
        LifecycleState::Startup
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    NoTrade(Vec<String>), // List of reasons why trading is blocked
    Participate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakoutStatus {
    New,
    Removed,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakoutChange {
    pub symbol: String,
    pub status: BreakoutStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: LifecycleState,
    pub to: LifecycleState,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStateOutput {
    pub lifecycle: LifecycleState,
    pub action_status: ActionStatus,
    pub breakout_changes: Vec<BreakoutChange>,
    pub stability: f64,
    pub continuity_days: usize,
    pub has_mainline: bool,
}
