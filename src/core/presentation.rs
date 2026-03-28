use crate::core::display::{RiskOpportunityViewModel, TacticalBucketViewModel, TopActionViewModel};
use crate::core::i18n::Language;
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
    pub participation_label: String,
    pub participation_value: String,
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
    pub action_status_label: String,
    pub action_status_value: String,
    pub behavior_mode_label: String,
    pub behavior_mode_value: String,
    pub exposure_label: String,
    pub exposure_value: String,
    pub summary: String,
    pub readiness_reasons_label: String,
    pub readiness_reasons: Vec<String>,
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
    pub tactical_buckets: Vec<TacticalBucketViewModel>,
    #[serde(default)]
    pub risk_opportunity_summary: RiskOpportunitySummaryViewModel,
    #[serde(default)]
    pub risk_opportunities: Vec<RiskOpportunityViewModel>,
    #[serde(default)]
    pub notices: Vec<String>,
    #[serde(default)]
    pub data_alert: Option<DataAlertViewModel>,
    // For the terminal table and archival markdown
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
