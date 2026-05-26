#[derive(Default, Debug)]
pub(crate) struct RegimeStats {
    pub(crate) total_signals: usize,
    pub(crate) correct_signals: usize,
    pub(crate) sum_20d_return: f64,
    pub(crate) sum_max_drawdown_20d: f64,
    pub(crate) count_drawdowns: usize,
}

#[derive(Default, Debug, serde::Serialize, Clone)]
pub struct StateMachineMetrics {
    pub reset_count: usize,
    pub blocked_reset_count: usize,
    pub multi_step_downgrade_attempt_count: usize,
    pub duration_lock_count: usize,
    pub soft_reset_count: usize,
    pub defensive_override_count: usize,
    pub state_flip_count_5d: usize,
    // Asset-level stability metrics (V1.4)
    pub top_actions_turnover_sum: f64,
    pub core_asset_protection_hits: usize,
    pub weak_asset_promotion_cap_hits: usize,
    // Behavior Calibration Proxy Metrics (V1.4+)
    pub total_raw_vs_actual_divergence_days: usize,
    pub total_raw_optimal_suppression_days: usize,
    pub total_initial_top_actions_latency_days: usize,
    pub total_overstay_events: usize,
    pub total_recovery_events: usize,
    pub total_days: usize,
    pub evaluated_asset_days: usize,
    pub breakout_eligible_asset_days: usize,
    pub trend_gate_blocked_days: usize,
    pub trend_status_dispersed_days: usize,
    pub trend_status_forming_days: usize,
    pub trend_status_formed_days: usize,
    pub topology_no_leader_days: usize,
    pub topology_single_leader_days: usize,
    pub topology_fragmented_leaders_days: usize,
    pub breakout_no_breakout_count: usize,
    pub breakout_emerging_count: usize,
    pub breakout_confirmed_count: usize,
    pub breakout_failed_risk_count: usize,
}

#[derive(Debug)]
pub struct BacktestRunArtifacts {
    pub metrics: StateMachineMetrics,
    pub summary_markdown: String,
    pub state_machine_metrics_markdown: String,
    pub state_machine_metrics_json: String,
}
