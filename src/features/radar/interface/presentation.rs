use crate::features::radar::interface::display::{
    RiskOpportunityViewModel, TacticalBucketViewModel, TopActionViewModel,
};
use crate::features::research::interface::macro_event_observation::{
    EvidenceRecord, MarketReaction,
};
use crate::features::shared::interface::i18n::Language;
use serde::{Deserialize, Serialize};

fn default_language() -> Language {
    Language::ZhCn
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct MacroDisplayContext {
    pub headline: String,
    pub summary: String,
    pub risk_label: String,
    pub bias_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DataAlertViewModel {
    pub prefix: String,
    pub label: String,
    pub message: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SignalSummaryViewModel {
    pub confidence_label: String,
    pub confidence_value: String,
    pub confidence_breakdown_label: String,
    pub confidence_breakdown_value: String,
    pub stability_label: String,
    pub stability_value: String,
    pub cohesion_label: String,
    pub cohesion_value: String,
    pub continuity_label: String,
    pub continuity_value: String,
    pub regime_age_label: String,
    pub regime_age_value: String,
    pub flow_label: String,
    pub flow_value: String,
    pub breadth_label: String,
    pub breadth_value: String,
    #[serde(default)]
    pub breadth_raw_label: String,
    #[serde(default)]
    pub breadth_raw_value: String,
    #[serde(default)]
    pub breadth_counts_label: String,
    #[serde(default)]
    pub breadth_counts_value: String,
    #[serde(default)]
    pub breadth_universe_label: String,
    #[serde(default)]
    pub breadth_universe_value: String,
    pub breadth_semantic_label: String,
    pub breadth_semantic_value: String,
    pub supply_phase_label: String,
    pub supply_phase_value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LeadershipSnapshotViewModel {
    pub title: String,
    pub primary_leader_label: String,
    pub primary_leader_value: String,
    pub secondary_leaders_label: String,
    pub secondary_leaders_values: Vec<String>,
    pub watchlist_leaders_label: String,
    pub watchlist_leaders_values: Vec<String>,
    pub watchlist_leaders_reasons: Vec<String>,
    pub leadership_confidence_label: String,
    pub leadership_confidence_value: String,
    pub leadership_conflict_label: String,
    pub leadership_conflict_value: String,
    pub boundary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LeaderPersistenceViewModel {
    pub title: String,
    pub primary_leader_label: String,
    pub primary_leader_value: String,
    pub persistence_label: String,
    pub persistence_value: String,
    pub persistence_days: usize,
    pub leader_absence_duration: usize,
    pub observed_days_label: String,
    pub observed_days_value: String,
    pub breakout_continuity_label: String,
    pub breakout_continuity_value: String,
    pub history_coverage_label: String,
    pub history_coverage_value: String,
    pub first_observed_at_value: Option<String>,
    pub previous_leader_value: Option<String>,
    #[serde(default)]
    pub previous_snapshot_leader_value: Option<String>,
    #[serde(default)]
    pub last_confirmed_leader_value: Option<String>,
    #[serde(default)]
    pub leader_absence_since_value: Option<String>,
    #[serde(default)]
    pub tactical_leadership_structure_value: String,
    pub history_note: Option<String>,
    pub leadership_score_label: String,
    pub leadership_score_value: String,
    pub leadership_score: f64,
    pub leader_state_label: String,
    pub leader_state_value: String,
    pub change_from_yesterday_label: String,
    pub change_from_yesterday_value: String,
    pub persistence_change_days: i32,
    pub score_change: f64,
    pub switch_history_label: String,
    pub switch_history_values: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CurrentRelativeStrengthItemViewModel {
    pub symbol: String,
    pub status: String,
    #[serde(default, alias = "recovery_state")]
    pub recovery_strength: String,
    pub relative_1d_vs_benchmark: Option<f64>,
    pub relative_5d_vs_benchmark: Option<f64>,
    pub price_position: Option<f64>,
    pub volume_participation: Option<f64>,
    #[serde(default)]
    pub conflict_code: Option<String>,
    #[serde(default)]
    pub recovery_watch: bool,
    #[serde(default)]
    pub recovery_explanation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CurrentRelativeStrengthViewModel {
    pub title: String,
    pub confirmed_leader: String,
    #[serde(default)]
    pub benchmark_symbol: String,
    pub items: Vec<CurrentRelativeStrengthItemViewModel>,
    pub boundary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketChangeLogViewModel {
    pub baseline_status: String,
    pub change_status: String,
    pub title: String,
    pub leader_label: String,
    pub leader_value: String,
    pub breadth_label: String,
    pub breadth_value: String,
    pub risk_label: String,
    pub risk_value: String,
    pub supply_phase_label: String,
    pub supply_phase_value: String,
    pub confidence_label: String,
    pub confidence_value: String,
    pub interpretation_label: String,
    pub interpretation_value: String,
    pub structural_change_label: String,
    pub structural_change_value: String,
    pub change_level: String,
    pub change_drivers: Vec<String>,
    pub unchanged_dimensions: Vec<String>,
    pub summary: String,
    pub summary_label: String,
    pub summary_values: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DecisionSummaryViewModel {
    pub is_no_trade: bool,
    pub section_title: String,
    pub trend_cohesion_label: String,
    pub trend_cohesion_value: String,
    pub trend_topology_label: String,
    pub trend_topology_value: String,
    pub gate_passed: bool,
    pub formation_conditions_label: String,
    pub unmet_conditions_label: String,
    pub formation_conditions: Vec<String>,
    pub unmet_conditions: Vec<String>,
    pub action_status_label: String,
    pub action_status_value: String,
    pub state_tag_label: String,
    pub state_tag_value: String,
    pub action_tag_label: String,
    pub action_tag_value: String,
    pub behavior_mode_label: String,
    pub behavior_mode_value: String,
    pub exposure_label: String,
    pub exposure_value: String,
    pub entry_cap_label: String,
    pub entry_cap_value: String,
    pub entry_cap_note: Option<String>,
    pub hard_rule_note: String,
    pub summary: String,
    pub readiness_reasons_label: String,
    pub readiness_reasons: Vec<String>,
    pub compact_stability_value: String,
    pub compact_continuity_value: String,
    pub candidate_only_note: Option<String>,
    pub market_board_label: String,
    pub market_board_value: String,
    pub opportunity_snapshot_label: String,
    pub opportunity_snapshot_value: String,
    pub risk_snapshot_label: String,
    pub risk_snapshot_value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionWindow {
    #[default]
    None,
    Limited,
    Open,
}
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParticipationMode {
    #[default]
    None,
    Probe,
    Add,
}
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionActionability {
    #[default]
    CandidateOnly,
    Executable,
}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FinalExecutionDecision {
    pub execution_window: ExecutionWindow,
    pub participation_mode: ParticipationMode,
    pub position_range: String,
    #[serde(default)]
    pub permission_position_range: String,
    #[serde(default)]
    pub eligible_asset_count: usize,
    pub actionability: ExecutionActionability,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RiskOpportunitySummaryViewModel {
    pub opportunity_label: String,
    pub opportunity_value: String,
    pub risk_label: String,
    pub risk_value: String,
    pub execution_risk_label: String,
    pub execution_risk_value: String,
    pub portfolio_risk_label: String,
    pub portfolio_risk_value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExitDisplayIntent {
    #[default]
    Hold,
    Trim,
    Exit,
    Watch,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ExitDecisionItemViewModel {
    pub symbol: String,
    pub intent: ExitDisplayIntent,
    pub intent_label: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ExitDecisionSummaryViewModel {
    pub title: String,
    #[serde(default)]
    pub signal_title: String,
    #[serde(default)]
    pub actual_action_title: String,
    #[serde(default)]
    pub empty_note: Option<String>,
    #[serde(default)]
    pub no_action_note: Option<String>,
    #[serde(default)]
    pub signal_items: Vec<ExitDecisionItemViewModel>,
    #[serde(default)]
    pub items: Vec<ExitDecisionItemViewModel>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakoutDisplayStatus {
    #[default]
    NoBreakout,
    EmergingBreakout,
    ConfirmedBreakout,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BreakoutItemViewModel {
    pub symbol: String,
    pub status: BreakoutDisplayStatus,
    pub status_label: String,
    pub reason: String,
    pub strength_value: String,
    pub quality_value: String,
    pub consecutive_days: usize,
    #[serde(default)]
    pub failed_risk_value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BreakoutSummaryViewModel {
    pub title: String,
    #[serde(default)]
    pub empty_note: Option<String>,
    #[serde(default)]
    pub items: Vec<BreakoutItemViewModel>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UnmetDiffViewModel {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub persisting: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrendBreadthMode {
    BroadExpansion,
    NarrowLeadership,
    #[default]
    FragileRotation,
    StructuralDefense,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketCyclePosition {
    EarlyFormation,
    MidConfirmation,
    LateAcceptance,
    CrowdedExpectation,
    DistributionWarning,
    #[default]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldingEfficiency {
    Efficient,
    #[default]
    Neutral,
    TimeCostRising,
    Overdiscounted,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StateTransitionViewModel {
    pub has_significant_change: bool,
    pub no_trade_persists: bool,
    pub market_state_change: Option<String>,
    pub risk_overlay_change: Option<String>,
    pub trend_cohesion_gate_change: Option<String>,
    pub trend_cohesion_gate_passed: bool,
    pub trend_unmet_diff: Option<UnmetDiffViewModel>,
    pub trend_cohesion_status_change: Option<String>,
    pub trend_cohesion_topology_change: Option<String>,
    pub breakout_changes: Vec<String>,
    #[serde(default)]
    pub risk_taxonomy: Vec<String>,
    #[serde(default)]
    pub scout_continuity: Option<String>,
    #[serde(default)]
    pub scout_expansion: Option<String>,
    #[serde(default)]
    pub scout_reset: Option<String>,
    #[serde(default)]
    pub trend_recognition_state: Option<String>,
    #[serde(default)]
    pub trend_recognition_diffusion_score: Option<f64>,
    #[serde(default)]
    pub trend_recognition_conviction_score: Option<f64>,
    #[serde(default)]
    pub trend_recognition_lag_state: Option<String>,
    #[serde(default)]
    pub trend_recognition_single_asset_decay: Option<String>,
    #[serde(default)]
    pub structural_strength: Option<String>,
    #[serde(default)]
    pub evidence_quality_summary: Option<String>,
    #[serde(default)]
    pub substantive_signals: Vec<String>,
    #[serde(default)]
    pub substantive_details: Vec<String>,
    #[serde(default)]
    pub strategic_context: Vec<String>,
    #[serde(default)]
    pub trend_breadth_mode: TrendBreadthMode,
    #[serde(default)]
    pub market_cycle_position: MarketCyclePosition,
    #[serde(default)]
    pub holding_efficiency: HoldingEfficiency,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpretationTrendState {
    #[default]
    Weak,
    Stable,
    PostRallyConsolidation,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpretationPattern {
    #[default]
    EventWaiting,
    FundamentalPricing,
    PostRallyConsolidation,
    SupplyPressure,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignalContextInformationContent {
    High,
    Medium,
    Low,
    #[default]
    #[serde(rename = "UNAVAILABLE", alias = "UNKNOWN")]
    Unknown,
}

/// Signal Context v1 のイベント分類。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignalContextType {
    #[default]
    ScheduledMacro,
    Corporate,
    Geopolitical,
    Commodity,
    RatesCredit,
    MarketStructure,
}

/// 市場情報量の判定値。データ未取得時は LOW と区別する。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignalContextInformationLevel {
    High,
    Medium,
    Low,
    #[default]
    Unavailable,
}

/// Context ソースの健全度。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignalContextSourceStatus {
    #[default]
    Healthy,
    Partial,
    Degraded,
    Unavailable,
}

/// イベントの市場影響ライフサイクル。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignalContextLifecycle {
    Upcoming,
    Released,
    ActiveRepricing,
    Aftermath,
    #[default]
    Expired,
}

/// 六つのイベント源を個別に監査可能にするカバレッジ。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct SignalContextCoverage {
    pub scheduled_macro: SignalContextSourceStatus,
    pub corporate: SignalContextSourceStatus,
    pub geopolitical: SignalContextSourceStatus,
    pub commodity: SignalContextSourceStatus,
    pub rates_credit: SignalContextSourceStatus,
    pub market_structure: SignalContextSourceStatus,
    pub overall: SignalContextSourceStatus,
}

impl Default for SignalContextCoverage {
    fn default() -> Self {
        Self {
            scheduled_macro: SignalContextSourceStatus::Unavailable,
            corporate: SignalContextSourceStatus::Unavailable,
            geopolitical: SignalContextSourceStatus::Unavailable,
            commodity: SignalContextSourceStatus::Unavailable,
            rates_credit: SignalContextSourceStatus::Unavailable,
            market_structure: SignalContextSourceStatus::Unavailable,
            overall: SignalContextSourceStatus::Unavailable,
        }
    }
}

/// Event Fact の一次表現。MarketReaction は別フィールドで保持する。
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(default)]
pub struct SignalContextItem {
    #[serde(rename = "type")]
    pub context_type: SignalContextType,
    pub title: String,
    pub information_content: SignalContextInformationLevel,
    pub market_relevance: SignalContextInformationLevel,
    pub evidence_quality: SignalContextInformationLevel,
    pub lifecycle: SignalContextLifecycle,
    pub event_fact: String,
    pub observed_at: String,
    pub source_published_at: String,
    pub market_date: String,
    pub evidence: Vec<EvidenceRecord>,
    #[serde(default)]
    pub expected_value: Option<String>,
    #[serde(default)]
    pub actual_value: Option<String>,
    #[serde(default)]
    pub surprise: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Signal Context v1 の読み取りモデル。取引判断への影響は常にゼロ。
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct SignalContextV1 {
    pub market_date: String,
    pub scheduled_macro: Vec<SignalContextItem>,
    pub corporate_events: Vec<SignalContextItem>,
    pub geopolitical_events: Vec<SignalContextItem>,
    pub commodity_events: Vec<SignalContextItem>,
    pub rates_credit_events: Vec<SignalContextItem>,
    pub market_structure_events: Vec<SignalContextItem>,
    pub primary_context: Option<SignalContextItem>,
    pub secondary_contexts: Vec<SignalContextItem>,
    pub overall_information_content: SignalContextInformationLevel,
    pub context_quality: SignalContextQuality,
    pub coverage: SignalContextCoverage,
    pub observed_market_reactions: Vec<MarketReaction>,
    pub event_time_utc: Option<String>,
    pub event_time_market_tz: Option<String>,
    pub report_generated_at: Option<String>,
    pub decision_weight: u8,
    pub trade_signal: bool,
    pub gate_effect: String,
    pub execution_effect: String,
    pub position_sizing_effect: String,
}

impl Default for SignalContextV1 {
    fn default() -> Self {
        Self {
            market_date: String::new(),
            scheduled_macro: Vec::new(),
            corporate_events: Vec::new(),
            geopolitical_events: Vec::new(),
            commodity_events: Vec::new(),
            rates_credit_events: Vec::new(),
            market_structure_events: Vec::new(),
            primary_context: None,
            secondary_contexts: Vec::new(),
            overall_information_content: SignalContextInformationLevel::Unavailable,
            context_quality: SignalContextQuality::Unavailable,
            coverage: SignalContextCoverage::default(),
            observed_market_reactions: Vec::new(),
            event_time_utc: None,
            event_time_market_tz: None,
            report_generated_at: None,
            decision_weight: 0,
            trade_signal: false,
            gate_effect: "none".to_string(),
            execution_effect: "none".to_string(),
            position_sizing_effect: "none".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignalContextPrimaryContext {
    QuarterEndRebalancing,
    MonthEndRebalancing,
    IndexReconstitution,
    EtfRebalance,
    HolidayLiquidity,
    PreEarningsWaiting,
    MajorEventWaiting,
    MacroEvent,
    #[default]
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignalContextQuality {
    High,
    Medium,
    Low,
    #[default]
    Unavailable,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpretationExpectationQuality {
    High,
    Medium,
    Low,
    #[default]
    Unavailable,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpretationExpectationQualityReason {
    MarketConsensusAvailable,
    MarketConsensusUnavailable,
    #[default]
    SystemUnavailable,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpretationGravityDataQuality {
    Ready,
    Partial,
    #[default]
    Unavailable,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpretationGravityDataQualityReason {
    #[default]
    ProviderUnavailable,
    HistoricalSnapshotMissing,
    ConsensusUnavailable,
    SourceTemporarilyUnavailable,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpretationQuality {
    High,
    Medium,
    #[default]
    Low,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InterpretationLayerViewModel {
    pub title: String,
    pub notice: String,
    pub current_decision_weight_label: String,
    pub current_decision_weight_value: String,
    pub signal_context_label: String,
    pub signal_context_information_content_label: String,
    pub signal_context_information_content_value: String,
    pub signal_context_primary_context_label: String,
    pub signal_context_primary_context_value: String,
    pub signal_context_quality_label: String,
    pub signal_context_quality_value: String,
    pub signal_context_event_fact_label: String,
    pub signal_context_event_fact_value: String,
    pub signal_context_source_diagnostics_label: String,
    pub signal_context_source_diagnostics_value: String,
    pub signal_context_source_diagnostics_appendix_label: String,
    pub signal_context_source_diagnostics_appendix_value: String,
    pub signal_context_interpretation_label: String,
    pub signal_context_interpretation_value: String,
    pub signal_context_next_observation_label: String,
    pub signal_context_next_observation_value: String,
    pub signal_context_boundary: String,
    #[serde(default)]
    pub signal_context_coverage: SignalContextCoverage,
    pub signal_context_lifecycle_label: String,
    pub signal_context_lifecycle_value: String,
    pub signal_context_expected_label: String,
    pub signal_context_expected_value: String,
    pub signal_context_actual_label: String,
    pub signal_context_actual_value: String,
    pub signal_context_surprise_label: String,
    pub signal_context_surprise_value: String,
    pub signal_context_reason_label: String,
    pub signal_context_reason_value: String,
    pub expectation_quality_label: String,
    pub expectation_quality_value: String,
    pub expectation_quality_reason_label: String,
    pub expectation_quality_reason_value: String,
    pub expectation_lifecycle_label: String,
    pub expectation_lifecycle_value: String,
    pub expectation_next_observation_label: String,
    pub expectation_next_observation_value: String,
    pub gravity_data_quality_label: String,
    pub gravity_data_quality_value: String,
    pub gravity_data_quality_reason_label: String,
    pub gravity_data_quality_reason_value: String,
    pub observation_health_label: String,
    pub observation_health_value: String,
    pub interpretation_quality_label: String,
    pub interpretation_quality_value: String,
    pub narrative_components_label: String,
    pub trend_label: String,
    pub trend_value: String,
    pub trend_confidence_label: String,
    pub trend_confidence_value: String,
    pub expectation_label: String,
    pub expectation_value: String,
    pub expectation_confidence_label: String,
    pub expectation_confidence_value: String,
    pub supply_label: String,
    pub supply_value: String,
    pub supply_confidence_label: String,
    pub supply_confidence_value: String,
    pub gravity_label: String,
    pub gravity_value: String,
    pub gravity_confidence_label: String,
    pub gravity_confidence_value: String,
    pub flow_label: String,
    pub flow_value: String,
    pub flow_confidence_label: String,
    pub flow_confidence_value: String,
    pub interpretation_label: String,
    pub interpretation_value: String,
    pub decision_explanation_label: String,
    pub decision_explanation_intro: String,
    pub decision_explanation_reasons: Vec<String>,
    pub decision_explanation_conclusion: String,
    pub subjects_label: String,
    pub subjects_value: String,
    pub boundary: String,
    pub todays_explanation_label: String,
    pub primary_driver_label: String,
    pub primary_driver_value: String,
    pub primary_driver_confidence: String,
    pub todays_explanation_navigation_value: String,
    pub secondary_drivers_label: String,
    pub secondary_drivers_values: Vec<String>,
    pub ignored_today_label: String,
    pub ignored_today_values: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketInterpretationViewModel {
    pub title: String,
    pub notice: String,
    pub current_decision_weight_label: String,
    pub current_decision_weight_value: String,
    pub narrative_label: String,
    pub narrative_values: Vec<String>,
    pub day_type_label: String,
    pub day_type_value: String,
    pub day_type_reason_label: String,
    pub day_type_reason_value: String,
    pub exceptional_factors_label: String,
    pub exceptional_factors_values: Vec<String>,
    pub leadership_label: String,
    pub leadership_classification_label: String,
    pub leadership_classification_value: String,
    pub primary_label: String,
    pub primary_values: Vec<String>,
    pub supporting_label: String,
    pub supporting_values: Vec<String>,
    pub weakening_label: String,
    pub weakening_values: Vec<String>,
    pub leadership_metrics_label: String,
    pub leadership_breadth_label: String,
    pub leadership_breadth_value: String,
    pub concentration_label: String,
    #[serde(default)]
    pub breadth_raw_label: String,
    #[serde(default)]
    pub breadth_raw_value: String,
    #[serde(default)]
    pub breadth_semantic_label: String,
    #[serde(default)]
    pub breadth_semantic_value: String,
    #[serde(default)]
    pub rs_recovery_breadth_label: String,
    #[serde(default)]
    pub rs_recovery_breadth_value: String,
    #[serde(default)]
    pub strong_moderate_recovery_label: String,
    #[serde(default)]
    pub strong_moderate_recovery_value: String,
    #[serde(default)]
    pub rs_diffusion_label: String,
    #[serde(default)]
    pub rs_diffusion_value: String,
    #[serde(default)]
    pub actionable_diffusion_label: String,
    #[serde(default)]
    pub actionable_diffusion_value: String,
    #[serde(default)]
    pub diffusion_reason_label: String,
    #[serde(default)]
    pub diffusion_reason_value: String,
    #[serde(default)]
    pub tactical_leadership_structure_value: String,
    #[serde(default)]
    pub leader_absence_duration: usize,
    pub breadth_score_label: String,
    pub breadth_score_value: String,
    pub concentration_score_label: String,
    pub concentration_score_value: String,
    pub rotation_score_label: String,
    pub rotation_score_value: String,
    pub rotation_label: String,
    pub rotation_type_value: String,
    pub rotation_from_label: String,
    pub rotation_from_values: Vec<String>,
    pub rotation_to_label: String,
    pub rotation_to_values: Vec<String>,
    pub rotation_interpretation_label: String,
    pub rotation_interpretation_value: String,
    pub confidence_label: String,
    pub trend_confidence_label: String,
    pub trend_confidence_value: String,
    pub macro_confidence_label: String,
    pub macro_confidence_value: String,
    pub supply_confidence_label: String,
    pub supply_confidence_value: String,
    pub expectation_confidence_label: String,
    pub expectation_confidence_value: String,
    pub gravity_confidence_label: String,
    pub gravity_confidence_value: String,
    pub flow_confidence_label: String,
    pub flow_confidence_value: String,
    pub overall_confidence_label: String,
    pub overall_confidence_value: String,
    pub interpretation_priority_label: String,
    pub interpretation_priority_values: Vec<String>,
    pub observation_only_label: String,
    pub observation_only_value: String,
    pub boundary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HypothesisLayerViewModel {
    pub title: String,
    pub notice: String,
    #[serde(default)]
    pub candidates: Vec<HypothesisCandidateViewModel>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HypothesisCandidateViewModel {
    pub title: String,
    pub hypothesis_type: String,
    pub summary: String,
    pub consensus_state: String,
    pub pricing_state: String,
    pub confidence: HypothesisConfidence,
    pub confidence_label: String,
    pub time_horizon: String,
    pub materialization_window: String,
    pub tactical_isolation_notice: String,
    pub narrative_saturation: String,
    pub reality_override_notice: String,
    pub reality_override_priority: String,
    pub confidence_decay_notice: String,
    #[serde(default)]
    pub age_days: Option<i64>,
    #[serde(default)]
    pub age_label: String,
    #[serde(default)]
    pub validation_summary: String,
    #[serde(default)]
    pub validation_checks: Vec<HypothesisValidationCheckViewModel>,
    #[serde(default)]
    pub evidence_chain: Vec<HypothesisEvidenceNodeViewModel>,
    #[serde(default)]
    pub candidate_beneficiaries: Vec<HypothesisBeneficiaryViewModel>,
    #[serde(default)]
    pub failure_risks: Vec<HypothesisFailureRiskViewModel>,
    pub responsibility_notice: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HypothesisValidationCheckViewModel {
    pub label: String,
    pub passed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum HypothesisConfidence {
    #[default]
    Exploratory,
    Early,
    Developing,
    Strengthening,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HypothesisEvidenceNodeViewModel {
    pub label: String,
    pub evidence_type: String,
    pub strength: String,
    pub source_layer: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HypothesisBeneficiaryViewModel {
    pub symbol: String,
    pub role: String,
    pub rationale: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HypothesisFailureRiskViewModel {
    pub label: String,
    pub description: String,
    pub severity: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PresentationPacket {
    pub date_str: String,
    #[serde(default = "default_language")]
    pub language: Language,
    pub macro_display: MacroDisplayContext,
    #[serde(default)]
    pub decision_summary: DecisionSummaryViewModel,
    #[serde(default)]
    pub final_execution_decision: FinalExecutionDecision,
    #[serde(default)]
    pub signal_summary: SignalSummaryViewModel,
    pub top_actions: Vec<TopActionViewModel>,
    #[serde(default)]
    pub exit_summary: ExitDecisionSummaryViewModel,
    #[serde(default)]
    pub breakout_summary: BreakoutSummaryViewModel,
    #[serde(default)]
    pub tactical_buckets: Vec<TacticalBucketViewModel>,
    #[serde(default)]
    pub risk_opportunity_summary: RiskOpportunitySummaryViewModel,
    #[serde(default)]
    pub risk_opportunities: Vec<RiskOpportunityViewModel>,
    #[serde(default)]
    pub leadership_snapshot: Option<LeadershipSnapshotViewModel>,
    #[serde(default)]
    pub leader_persistence: Option<LeaderPersistenceViewModel>,
    #[serde(default)]
    pub current_relative_strength: Option<CurrentRelativeStrengthViewModel>,
    #[serde(default)]
    pub market_change_log: Option<MarketChangeLogViewModel>,
    #[serde(default)]
    pub notices: Vec<String>,
    #[serde(default)]
    pub data_alert: Option<DataAlertViewModel>,
    #[serde(default)]
    pub transition_evidence: Option<StateTransitionViewModel>,
    #[serde(default)]
    pub interpretation_layer: Option<InterpretationLayerViewModel>,
    #[serde(default)]
    pub market_interpretation: Option<MarketInterpretationViewModel>,
    #[serde(default)]
    pub hypothesis_layer: Option<HypothesisLayerViewModel>,
    // terminal table と archival markdown 用。
    pub terminal_rows: Vec<TerminalRowViewModel>,
    pub state_code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TerminalRowViewModel {
    pub symbol: String,
    pub state_label: String,
    pub intent_label: String,
    pub action_label: String,
    pub owner_dev_label: String,
    pub strength_z_label: String,
}

#[cfg(test)]
mod tests {
    use super::BreakoutItemViewModel;

    #[test]
    fn signal_context_v1_serializes_all_context_groups_and_zero_decision_weight() {
        let context = super::SignalContextV1::default();
        let value = serde_json::to_value(context).unwrap();
        assert!(value.get("scheduled_macro").is_some());
        assert!(value.get("corporate_events").is_some());
        assert!(value.get("geopolitical_events").is_some());
        assert!(value.get("commodity_events").is_some());
        assert!(value.get("rates_credit_events").is_some());
        assert!(value.get("market_structure_events").is_some());
        assert_eq!(value["decision_weight"], 0);
        assert_eq!(value["trade_signal"], false);
    }

    #[test]
    fn signal_context_v1_keeps_event_fact_and_market_reaction_separate() {
        let item = super::SignalContextItem::default();
        let reaction = super::MarketReaction::default();
        let context = super::SignalContextV1 {
            primary_context: Some(item),
            observed_market_reactions: vec![reaction],
            ..Default::default()
        };
        let value = serde_json::to_value(context).unwrap();
        assert!(value["primary_context"].is_object());
        assert!(value["observed_market_reactions"].is_array());
        assert!(value["primary_context"].get("market_reaction").is_none());
    }

    #[test]
    fn breakout_item_keeps_consecutive_days_in_serialized_read_model() {
        let item = BreakoutItemViewModel {
            symbol: "SPY".to_string(),
            consecutive_days: 3,
            ..Default::default()
        };
        let value = serde_json::to_value(item).unwrap();
        assert_eq!(value["consecutive_days"], 3);
    }
}
