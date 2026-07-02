use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FutureCalendarKind {
    IndexReconstitution,
    EtfRebalance,
    HolidayLiquidity,
    PreEarningsWaiting,
    MajorEventWaiting,
    #[default]
    MacroEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroEventInformationContent {
    High,
    Medium,
    Low,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroEventImportance {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroEventLifecycle {
    #[default]
    Upcoming,
    Released,
    Compared,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroEventSurpriseState {
    Above,
    InLine,
    Below,
    #[default]
    NotAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroEventSourceHealth {
    Succeeded,
    Partial,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroEventType {
    Cpi,
    CoreCpi,
    Ppi,
    Pce,
    CorePce,
    NonfarmPayrolls,
    UnemploymentRate,
    Jolts,
    Gdp,
    FomcRateDecision,
    FomcMinutes,
    FedChairSpeech,
    TreasuryAuction,
    IsmManufacturing,
    IsmServices,
    RetailSales,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FutureCalendarObservation {
    #[serde(default)]
    pub kind: FutureCalendarKind,
    pub event_id: String,
    pub as_of_date: NaiveDate,
    pub event_date: NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_time: Option<String>,
    pub timezone: String,
    pub country: String,
    pub event_type: MacroEventType,
    pub event_name: String,
    pub source: String,
    pub source_url: String,
    pub importance: MacroEventImportance,
    pub lifecycle: MacroEventLifecycle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub surprise_state: MacroEventSurpriseState,
    pub information_content: MacroEventInformationContent,
    pub source_health: MacroEventSourceHealth,
    pub observed_at: NaiveDate,
}

impl From<MacroEventObservation> for FutureCalendarObservation {
    fn from(value: MacroEventObservation) -> Self {
        Self {
            kind: FutureCalendarKind::MacroEvent,
            event_id: value.event_id,
            as_of_date: value.as_of_date,
            event_date: value.event_date,
            event_time: value.event_time,
            timezone: value.timezone,
            country: value.country,
            event_type: value.event_type,
            event_name: value.event_name,
            source: value.source,
            source_url: value.source_url,
            importance: value.importance,
            lifecycle: value.lifecycle,
            expected_value: value.expected_value,
            actual_value: value.actual_value,
            previous_value: value.previous_value,
            unit: value.unit,
            surprise_state: value.surprise_state,
            information_content: value.information_content,
            source_health: value.source_health,
            observed_at: value.observed_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MacroEventObservation {
    pub event_id: String,
    pub as_of_date: NaiveDate,
    pub event_date: NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_time: Option<String>,
    pub timezone: String,
    pub country: String,
    pub event_type: MacroEventType,
    pub event_name: String,
    pub source: String,
    pub source_url: String,
    pub importance: MacroEventImportance,
    pub lifecycle: MacroEventLifecycle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub surprise_state: MacroEventSurpriseState,
    pub information_content: MacroEventInformationContent,
    pub source_health: MacroEventSourceHealth,
    pub observed_at: NaiveDate,
}
