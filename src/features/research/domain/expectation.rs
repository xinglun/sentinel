use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ExpectationEventType {
    DeliveryConsensus,
    EarningsConsensus,
    RevenueConsensus,
    MarginConsensus,
    CloudGrowthConsensus,
    CapexConsensus,
    ProductEventExpectation,
    UserGrowthConsensus,
    ProcedureGrowthConsensus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RevisionDirection {
    Up,
    Down,
    Stable,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum SurpriseState {
    Above,
    InLine,
    Below,
    NotReleased,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ExpectationPressure {
    Low,
    Normal,
    High,
    Extreme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum SourceHealth {
    Succeeded,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ExpectationObservation {
    pub subject: String,
    pub period: String,
    pub as_of_date: NaiveDate,
    pub event_type: ExpectationEventType,
    pub expected_value: String,
    pub actual_value: String,
    pub unit: String,
    pub consensus_source: String,
    pub estimate_count: usize,
    pub estimate_high: Option<String>,
    pub estimate_low: Option<String>,
    pub estimate_median: Option<String>,
    pub estimate_average: Option<String>,
    pub revision_direction: RevisionDirection,
    pub surprise_state: SurpriseState,
    pub expectation_pressure: ExpectationPressure,
    pub confidence: Option<f64>,
    pub source_health: SourceHealth,
    pub interpretation: String,
    pub observed_at: NaiveDate,
}
