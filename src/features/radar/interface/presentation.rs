use crate::features::radar::interface::display::{
    RiskOpportunityViewModel, TacticalBucketViewModel, TopActionViewModel,
};
use crate::features::shared::interface::i18n::Language;
use serde::{Deserialize, Serialize};

fn default_language() -> Language {
    Language::ZhCn
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RiskOpportunitySummaryViewModel {
    pub opportunity_label: String,
    pub opportunity_value: String,
    pub risk_label: String,
    pub risk_value: String,
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
    pub empty_note: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InterpretationLayerViewModel {
    pub title: String,
    pub notice: String,
    pub current_decision_weight_label: String,
    pub current_decision_weight_value: String,
    pub expectation_quality_label: String,
    pub expectation_quality_value: String,
    pub expectation_quality_reason_label: String,
    pub expectation_quality_reason_value: String,
    pub gravity_data_quality_label: String,
    pub gravity_data_quality_value: String,
    pub gravity_data_quality_reason_label: String,
    pub gravity_data_quality_reason_value: String,
    pub narrative_pattern_label: String,
    pub narrative_pattern_value: String,
    pub subjects_label: String,
    pub subjects_value: String,
    pub narrative_summary_label: String,
    pub narrative_summary_value: String,
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
    pub notices: Vec<String>,
    #[serde(default)]
    pub data_alert: Option<DataAlertViewModel>,
    #[serde(default)]
    pub transition_evidence: Option<StateTransitionViewModel>,
    #[serde(default)]
    pub interpretation_layer: Option<InterpretationLayerViewModel>,
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
