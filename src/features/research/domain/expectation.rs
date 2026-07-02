use chrono::{Datelike, NaiveDate};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ExpectationLifecycleState {
    Upcoming,
    #[default]
    Pending,
    Released,
    Compared,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ExpectationResult {
    Beat,
    Miss,
    Inline,
}

pub(crate) fn derive_lifecycle_state(
    period: &str,
    as_of_date: NaiveDate,
    actual_value: &str,
    result: Option<ExpectationResult>,
    released_at: Option<NaiveDate>,
    archived_at: Option<NaiveDate>,
) -> ExpectationLifecycleState {
    if archived_at.is_some() {
        return ExpectationLifecycleState::Archived;
    }
    if released_at.is_some() && result.is_some() {
        return ExpectationLifecycleState::Compared;
    }
    if released_at.is_some() || is_released_value(actual_value) {
        return ExpectationLifecycleState::Released;
    }
    if is_future_period(period, as_of_date) {
        return ExpectationLifecycleState::Upcoming;
    }
    ExpectationLifecycleState::Pending
}

fn is_released_value(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed != "未発表" && trimmed != "UNAVAILABLE"
}

fn is_future_period(period: &str, as_of_date: NaiveDate) -> bool {
    if let Some(period_date) = parse_period_date(period) {
        return period_date > as_of_date;
    }

    if let Some((year, quarter)) = parse_quarter_label(period) {
        let current_quarter = (as_of_date.year(), quarter_index(as_of_date.month()));
        return (year, quarter) > current_quarter;
    }

    false
}

fn parse_period_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
}

fn parse_quarter_label(value: &str) -> Option<(i32, u32)> {
    let trimmed = value.trim();
    let (year_part, quarter_part) = trimmed.split_once('Q')?;
    let year = year_part.parse::<i32>().ok()?;
    let quarter = quarter_part.parse::<u32>().ok()?;
    (1..=4).contains(&quarter).then_some((year, quarter))
}

fn quarter_index(month: u32) -> u32 {
    match month {
        1..=3 => 1,
        4..=6 => 2,
        7..=9 => 3,
        _ => 4,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ExpectationObservation {
    pub subject: String,
    pub period: String,
    pub as_of_date: NaiveDate,
    pub event_type: ExpectationEventType,
    #[serde(default)]
    pub lifecycle_state: ExpectationLifecycleState,
    pub expected_value: String,
    pub actual_value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ExpectationResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surprise_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_reaction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<NaiveDate>,
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
