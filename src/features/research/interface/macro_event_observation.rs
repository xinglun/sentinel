use chrono::{DateTime, NaiveDate, Utc};
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
    #[serde(default)]
    pub observation_id: String,
    pub observed_at: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub venue: String,
    #[serde(default)]
    pub instrument: String,
    #[serde(default)]
    pub source_published_at: String,
    #[serde(default)]
    pub market_date: String,
    pub subject: String,
    pub observation: String,
    pub evidence: Vec<EvidenceRecord>,
}

/// MarketReaction の互換名。新規コードでは MarketObservation として扱う。
pub type MarketObservation = MarketReaction;

/// Event と MarketObservation の時間関係を保持する canonical read model。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TemporalBinding {
    pub event_id: String,
    pub observation_id: String,
    pub source_published_at: String,
    pub observed_at: String,
    pub temporal_eligible: bool,
    pub reason: String,
}

/// report run と observation window の時間コンテキスト。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SignalContextTemporalContext {
    #[serde(default)]
    pub report_run_at: Option<String>,
    #[serde(default)]
    pub observation_window_start: Option<String>,
    #[serde(default)]
    pub observation_window_end: Option<String>,
}

/// source 公開時刻と observation 時刻を UTC で比較する。parse 失敗は false にする。
pub(crate) fn build_temporal_binding(
    event_id: &str,
    source_published_at: &str,
    observation: &MarketObservation,
) -> TemporalBinding {
    let temporal_eligible = match (
        parse_timestamp(source_published_at),
        parse_timestamp(&observation.observed_at),
    ) {
        (Some(source_published_at), Some(observed_at)) => source_published_at <= observed_at,
        _ => false,
    };
    let reason = if event_id.trim().is_empty() || observation.observation_id.trim().is_empty() {
        "invalid event_id or observation_id".to_string()
    } else if parse_timestamp(source_published_at).is_none()
        || parse_timestamp(&observation.observed_at).is_none()
    {
        "invalid or missing timestamp".to_string()
    } else if temporal_eligible {
        "source_published_at is before or equal to observed_at".to_string()
    } else {
        "source_published_at is after observed_at".to_string()
    };
    TemporalBinding {
        event_id: event_id.to_string(),
        observation_id: observation.observation_id.clone(),
        source_published_at: source_published_at.to_string(),
        observed_at: observation.observed_at.clone(),
        temporal_eligible,
        reason,
    }
}

/// accepted_at が report run までに確定しているかを検証する。
pub(crate) fn event_visible_at(accepted_at: &str, report_run_at: &str) -> bool {
    match (parse_timestamp(accepted_at), parse_timestamp(report_run_at)) {
        (Some(accepted_at), Some(report_run_at)) => accepted_at <= report_run_at,
        _ => false,
    }
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

/// Research interface が同一 feature の ACL を呼び出す facade。
pub(crate) async fn load_macro_signal_context(
    app_config: &crate::config::AppConfig,
    market_date: NaiveDate,
    report_run_at: DateTime<Utc>,
) -> MacroSignalContextReadModel {
    crate::features::research::acl::macro_signal_context_provider_factory::load_macro_signal_context(
        app_config,
        market_date,
        report_run_at,
    )
    .await
}

/// Radar が解釈する前の macro source の中立 read model。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MacroSignalContextReadModel {
    pub market_date: NaiveDate,
    #[serde(default)]
    pub temporal_context: SignalContextTemporalContext,
    pub rates_credit: MacroSignalContextSource,
    pub commodity: MacroSignalContextSource,
    pub geopolitical: MacroSignalContextSource,
    pub observed_market_reactions: Vec<MarketReaction>,
}

/// 単一 macro source の event と取得状態を保持する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct MacroSignalContextSource {
    pub status: MacroSignalContextSourceStatus,
    pub events: Vec<MacroSignalContextEvent>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroSignalContextSourceStatus {
    Healthy,
    Partial,
    Degraded,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroSignalContextInformationLevel {
    High,
    Medium,
    Low,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum MacroSignalContextLifecycle {
    Released,
    ActiveRepricing,
    Aftermath,
    #[default]
    Expired,
}

/// Provider が観測した event fact。取引判断には渡さない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct MacroSignalContextEvent {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub accepted_at: String,
    pub title: String,
    pub information_content: MacroSignalContextInformationLevel,
    pub market_relevance: MacroSignalContextInformationLevel,
    pub evidence_quality: MacroSignalContextInformationLevel,
    pub lifecycle: MacroSignalContextLifecycle,
    pub event_fact: String,
    pub observed_at: String,
    pub source_published_at: String,
    pub market_date: String,
    pub evidence: Vec<EvidenceRecord>,
    pub expected_value: Option<String>,
    pub actual_value: Option<String>,
    pub surprise: Option<String>,
    pub reason: Option<String>,
}

#[cfg(test)]
mod signal_context_v1_tests {
    use super::{build_temporal_binding, event_visible_at, EvidenceRecord, MarketReaction};

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
            observation_id: "obs-payroll".to_string(),
            observed_at: "2026-08-07T16:00:00Z".to_string(),
            session: "CORE".to_string(),
            venue: "NASDAQ".to_string(),
            instrument: "NASDAQ".to_string(),
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

    #[test]
    fn temporal_binding_allows_only_publication_before_observation() {
        let reaction = MarketReaction {
            observed_at: "2026-09-09T22:00:00Z".to_string(),
            observation_id: "obs-spy-overnight".to_string(),
            ..Default::default()
        };
        let binding = build_temporal_binding("event-jordan", "2026-09-09T21:42:00Z", &reaction);

        assert!(binding.temporal_eligible);
        assert_eq!(binding.event_id, "event-jordan");
        assert_eq!(binding.observation_id, "obs-spy-overnight");
    }

    #[test]
    fn temporal_binding_fails_closed_for_invalid_publication_or_observation() {
        let reaction = MarketReaction {
            observed_at: "not-a-timestamp".to_string(),
            observation_id: "obs-invalid".to_string(),
            ..Default::default()
        };
        let binding = build_temporal_binding("event-invalid", "", &reaction);

        assert!(!binding.temporal_eligible);
        assert!(binding.reason.contains("invalid"));
    }

    #[test]
    fn accepted_at_is_a_report_visibility_boundary() {
        assert!(event_visible_at(
            "2026-09-09T21:42:00Z",
            "2026-09-10T00:00:00Z"
        ));
        assert!(!event_visible_at(
            "2026-09-10T00:00:01Z",
            "2026-09-10T00:00:00Z"
        ));
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
