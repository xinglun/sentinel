use crate::features::shared::domain::market_data::DailyBar;
use chrono::NaiveDate;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct BacktestTickerHistory<'a> {
    pub symbol: String,
    pub bars: Cow<'a, [DailyBar]>,
    pub total_trading_days: usize,
}

#[derive(Debug, Clone)]
pub struct BacktestWatchlistEntry {
    pub symbol: String,
    pub enable: bool,
}

#[derive(Debug, Clone)]
pub struct BacktestRules {
    pub optimal_threshold: f64,
}

#[derive(Debug, Clone)]
pub struct BacktestDecisionSnapshot {
    pub date: NaiveDate,
    pub market_state: String,
    pub trend_gate_passed: bool,
    pub trend_status: BacktestTrendStatus,
    pub trend_topology: BacktestTrendTopology,
    pub transition_audit: Option<BacktestTransitionAudit>,
    pub potential_energy: f64,
    pub system_confidence: f64,
    pub assets: Vec<BacktestAssetSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktestTrendStatus {
    Dispersed,
    Forming,
    Formed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktestTrendTopology {
    NoLeader,
    SingleLeader,
    FragmentedLeaders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktestBreakoutStatus {
    NoBreakout,
    EmergingBreakout,
    ConfirmedBreakout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktestAssetState {
    Other,
    Optimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktestAssetAction {
    Other,
    Reduce,
    Freeze,
    Avoid,
}

#[derive(Debug, Clone)]
pub struct BacktestAssetSnapshot {
    pub symbol: String,
    pub action: BacktestAssetAction,
    pub deviation: Option<f64>,
    pub asset_state: BacktestAssetState,
    pub breakout_eligible: bool,
    pub breakout_status: BacktestBreakoutStatus,
    pub breakout_failed_risk: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BacktestTransitionAudit {
    pub from: String,
    pub to: String,
    pub is_reset_blocked: bool,
    pub is_downgrade_clamped: bool,
    pub duration_locked: bool,
    pub soft_reset_applied: bool,
    pub defensive_override: bool,
}
