use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 追跡可能なイベント事実の最小証拠レコード。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceRecord {
    pub source: String,
    #[serde(default)]
    pub source_url: String,
    pub timestamp: String,
    #[serde(default)]
    pub source_published_at: String,
    pub event_type: String,
    pub subject: String,
    pub importance: String,
}

/// 事件事实与市场反应分离后的观测结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MarketReaction {
    pub observed_at: String,
    #[serde(default)]
    pub source_published_at: String,
    #[serde(default)]
    pub market_date: String,
    pub subject: String,
    pub observation: String,
    pub evidence: Vec<EvidenceRecord>,
}

#[cfg(test)]
mod signal_context_v1_tests {
    use super::{EvidenceRecord, MarketReaction};

    #[test]
    fn evidence_record_serializes_traceability_fields() {
        let evidence = EvidenceRecord {
            source: "official_calendar".to_string(),
            source_url: "https://example.test/employment".to_string(),
            timestamp: "2026-08-07T12:30:00Z".to_string(),
            source_published_at: "2026-08-07T12:30:00Z".to_string(),
            event_type: "EMPLOYMENT".to_string(),
            subject: "US Employment Report".to_string(),
            importance: "HIGH".to_string(),
        };
        let value = serde_json::to_value(evidence).unwrap();
        assert_eq!(value["event_type"], "EMPLOYMENT");
        assert_eq!(value["importance"], "HIGH");
    }

    #[test]
    fn market_reaction_keeps_evidence_as_a_separate_observation() {
        let reaction = MarketReaction {
            observed_at: "2026-08-07T16:00:00Z".to_string(),
            source_published_at: "2026-08-07T16:00:00Z".to_string(),
            market_date: "2026-08-07".to_string(),
            subject: "Nasdaq".to_string(),
            observation: "growth stocks stronger".to_string(),
            evidence: Vec::new(),
        };
        let value = serde_json::to_value(reaction).unwrap();
        assert!(value.get("observation").is_some());
        assert!(value.get("event_fact").is_none());
    }
}

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
    ActiveRepricing,
    Aftermath,
    Expired,
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

/// 既知イベントの発見事実。実績値の取得可否とは独立して保持する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EventDiscovery {
    pub event_id: String,
    pub event_name: String,
    pub event_date: NaiveDate,
    pub event_time: Option<String>,
    pub importance: MacroEventImportance,
}

/// イベント発生後に取得できた観測事実。欠損しても Discovery を消去しない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct EventObservation {
    pub expected_value: Option<String>,
    pub actual_value: Option<String>,
    pub surprise_state: MacroEventSurpriseState,
    pub status: String,
}

impl FutureCalendarObservation {
    pub(crate) fn discovery(&self) -> EventDiscovery {
        EventDiscovery {
            event_id: self.event_id.clone(),
            event_name: self.event_name.clone(),
            event_date: self.event_date,
            event_time: self.event_time.clone(),
            importance: self.importance,
        }
    }

    pub(crate) fn observation(&self) -> EventObservation {
        EventObservation {
            expected_value: self.expected_value.clone(),
            actual_value: self.actual_value.clone(),
            surprise_state: self.surprise_state,
            status: if self.actual_value.is_some() {
                "AVAILABLE".to_string()
            } else if self.lifecycle == MacroEventLifecycle::Released {
                "UNAVAILABLE".to_string()
            } else {
                "PENDING".to_string()
            },
        }
    }
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
