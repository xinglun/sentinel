use crate::features::backtest::domain::metrics::StateMachineMetrics;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BacktestDecisionClass {
    NoTrade,
    Probe,
    Ready,
}

#[derive(Debug, Clone)]
pub struct BacktestDecisionSnapshot {
    pub date: NaiveDate,
    pub decision_class: BacktestDecisionClass,
    pub decision_reasons: Vec<String>,
    pub gate_blocked: bool,
    pub classification_available: bool,
    pub decision_snapshot_version: String,
    pub universe_id: String,
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
    pub price: f64,
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

#[derive(Debug, Clone)]
pub struct BacktestReliabilityBucket {
    pub bucket: String,
    pub total: usize,
    pub correct: usize,
}

#[derive(Debug, Clone)]
pub struct BacktestRegimeAudit {
    pub state: String,
    pub total_signals: usize,
    pub correct_signals: usize,
    pub average_20d_return: f64,
    pub max_drawdown_20d: f64,
}

#[derive(Debug, Clone)]
pub struct BacktestSimulationReport {
    pub name: String,
    pub metrics: StateMachineMetrics,
    pub reliability: Vec<BacktestReliabilityBucket>,
    pub regime_audit: Vec<BacktestRegimeAudit>,
    pub validation: ValidationReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ValidationStatus {
    Pending,
    Partial,
    Complete,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationDecisionRecord {
    pub date: NaiveDate,
    pub symbol: String,
    pub decision_class: BacktestDecisionClass,
    pub decision_reasons: Vec<String>,
    pub gate_blocked: bool,
    pub classification_available: bool,
    pub decision_snapshot_version: String,
    pub universe_id: String,
    pub decision_session_index: usize,
    pub decision_close: f64,
    pub raw_candidate: bool,
    pub strength_date: Option<NaiveDate>,
    pub breakout_date: Option<NaiveDate>,
    pub ready_date: Option<NaiveDate>,
    pub strength_to_breakout_sessions: Option<usize>,
    pub breakout_to_ready_sessions: Option<usize>,
    pub strength_to_ready_sessions: Option<usize>,
    pub return_strength_to_ready: Option<f64>,
    pub return_breakout_to_ready: Option<f64>,
    pub max_move_strength_to_ready: Option<f64>,
    pub forward_return_5d: Option<f64>,
    pub forward_return_10d: Option<f64>,
    pub forward_return_20d: Option<f64>,
    pub mfe_5d: Option<f64>,
    pub mfe_10d: Option<f64>,
    pub mfe_20d: Option<f64>,
    pub mae_5d: Option<f64>,
    pub mae_10d: Option<f64>,
    pub mae_20d: Option<f64>,
    pub validation_status: ValidationStatus,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationClassOutcome {
    pub decision_class: Option<BacktestDecisionClass>,
    pub sample_count: usize,
    pub complete_5d: usize,
    pub complete_10d: usize,
    pub complete_20d: usize,
    pub average_5d_return: Option<f64>,
    pub average_10d_return: Option<f64>,
    pub average_20d_return: Option<f64>,
    pub average_mfe_20d: Option<f64>,
    pub average_mae_20d: Option<f64>,
    pub median_mae_20d: Option<f64>,
    pub p90_mae_20d: Option<f64>,
    pub p95_mae_20d: Option<f64>,
    pub average_positive_20d_return: Option<f64>,
    pub top_decile_missed_upside: Option<f64>,
    pub downside_20d_count: usize,
    pub positive_20d_count: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationBaselineComparison {
    pub raw_top3_sample_count: usize,
    pub ready_sample_count: usize,
    pub raw_top3_average_20d_return: Option<f64>,
    pub ready_average_20d_return: Option<f64>,
    pub raw_top3_average_20d_mfe: Option<f64>,
    pub ready_average_20d_mfe: Option<f64>,
    pub raw_top3_average_mae_20d: Option<f64>,
    pub ready_average_mae_20d: Option<f64>,
    pub return_difference: Option<f64>,
    pub mae_difference: Option<f64>,
    pub mfe_difference: Option<f64>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationUtility {
    pub blocked_candidate_count: usize,
    pub complete_20d_count: usize,
    pub downside_20d_count: usize,
    pub missed_upside_count: usize,
    pub average_mae_20d: Option<f64>,
    pub median_mae_20d: Option<f64>,
    pub p90_mae_20d: Option<f64>,
    pub p95_mae_20d: Option<f64>,
    pub average_mfe_20d: Option<f64>,
    pub average_positive_20d_return: Option<f64>,
    pub top_decile_missed_upside: Option<f64>,
    pub horizon_5d: ValidationHorizonUtility,
    pub horizon_10d: ValidationHorizonUtility,
    pub horizon_20d: ValidationHorizonUtility,
    pub reason_breakdown: Vec<ValidationReasonUtility>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationHorizonUtility {
    pub complete_sample_count: usize,
    pub downside_count: usize,
    pub missed_upside_count: usize,
    pub average_mae: Option<f64>,
    pub median_mae: Option<f64>,
    pub p90_mae: Option<f64>,
    pub p95_mae: Option<f64>,
    pub average_mfe: Option<f64>,
    pub average_positive_return: Option<f64>,
    pub top_decile_missed_upside: Option<f64>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationReasonUtility {
    pub reason: String,
    pub horizon_5d: ValidationHorizonUtility,
    pub horizon_10d: ValidationHorizonUtility,
    pub horizon_20d: ValidationHorizonUtility,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationPopulationAudit {
    pub classified_record_count: usize,
    pub gate_blocked_record_count: usize,
    pub raw_candidate_record_count: usize,
    pub raw_candidate_gate_blocked_record_count: usize,
    pub raw_candidate_gate_blocked_no_trade_record_count: usize,
    pub gate_blocked_non_candidate_record_count: usize,
    pub gate_blocked_non_candidate_reasons: Vec<ValidationPopulationReasonCount>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationPopulationReasonCount {
    pub reason: String,
    pub count: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ConfirmationCostSummary {
    pub episode_sample_count: usize,
    pub lifecycle_complete_episode_count: usize,
    pub average_strength_to_breakout_sessions: Option<f64>,
    pub average_breakout_to_ready_sessions: Option<f64>,
    pub average_strength_to_ready_sessions: Option<f64>,
    pub average_return_strength_to_ready: Option<f64>,
    pub average_return_lost_before_ready: Option<f64>,
    pub average_return_breakout_to_ready: Option<f64>,
    pub average_max_move_strength_to_ready: Option<f64>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationCohortReport {
    pub decision_snapshot_version: String,
    pub universe_id: String,
    pub outcomes: Vec<ValidationClassOutcome>,
    pub baseline: ValidationBaselineComparison,
    pub utility: ValidationUtility,
    pub population: ValidationPopulationAudit,
    pub confirmation_cost: ConfirmationCostSummary,
    pub net_decision_value: NetDecisionValue,
    pub sample_maturity: String,
    pub protection_sample_maturity: String,
    pub confirmation_sample_maturity: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NetDecisionValue {
    pub eligible_episode_count: usize,
    pub protection_episode_count: usize,
    pub confirmation_episode_count: usize,
    pub protection_benefit: Option<f64>,
    pub confirmation_cost: Option<f64>,
    pub net_value: Option<f64>,
    pub horizon_5d: NetDecisionHorizon,
    pub horizon_10d: NetDecisionHorizon,
    pub horizon_20d: NetDecisionHorizon,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NetDecisionHorizon {
    pub paired_episode_count: usize,
    pub unpaired_episode_count: usize,
    pub protection_benefit: Option<f64>,
    pub confirmation_cost: Option<f64>,
    pub adverse_waiting_return: Option<f64>,
    pub adverse_waiting_sample_count: usize,
    pub net_value: Option<f64>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationReport {
    pub records: Vec<ValidationDecisionRecord>,
    pub invalid_context_record_count: usize,
    pub outcomes: Vec<ValidationClassOutcome>,
    pub baseline: ValidationBaselineComparison,
    pub sample_maturity: String,
    pub cohorts: Vec<ValidationCohortReport>,
}

#[derive(Debug, Clone)]
pub struct BacktestComparisonReport {
    pub baseline: BacktestSimulationReport,
    pub enhanced: BacktestSimulationReport,
}
