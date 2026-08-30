use chrono::NaiveDate;

/// 企業イベント Provider の取得状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CorporateEventProviderHealth {
    Healthy,
    #[default]
    Unavailable,
}

/// 企業イベントの provider 中立な source 種別。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CorporateEventSourceKind {
    EarningsCalendar,
    OfficialFiling,
    CompanyIr,
    NewsAggregator,
    ExternalFixture,
}

/// 企業イベントを取得した source の application contract 表現。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorporateEventSource {
    pub provider_id: String,
    pub source_kind: CorporateEventSourceKind,
    pub source_url: Option<String>,
}

impl Default for CorporateEventSource {
    fn default() -> Self {
        Self {
            provider_id: String::new(),
            source_kind: CorporateEventSourceKind::ExternalFixture,
            source_url: None,
        }
    }
}

impl CorporateEventSource {
    pub(crate) fn is_empty(&self) -> bool {
        self.provider_id.trim().is_empty() && self.source_url.is_none()
    }
}

/// Provider に依存しない企業イベントの発表時間帯。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CorporateEventReleaseWindow {
    BeforeMarketOpen,
    DuringMarketHours,
    AfterMarketClose,
    #[default]
    Unknown,
}

/// Provider 固有の payload を Signal Context から分離した正規化済み事実。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CorporateEventObservation {
    pub symbol: String,
    pub market_date: NaiveDate,
    pub market_timezone: String,
    pub release_window: CorporateEventReleaseWindow,
    pub fiscal_quarter: u8,
    pub fiscal_year: i32,
    pub eps_actual: Option<f64>,
    pub eps_estimate: Option<f64>,
    pub revenue_actual: Option<f64>,
    pub revenue_estimate: Option<f64>,
    pub source: CorporateEventSource,
    pub observed_at: String,
}

impl Default for CorporateEventObservation {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            market_date: NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid default date"),
            market_timezone: "America/New_York".to_string(),
            release_window: CorporateEventReleaseWindow::default(),
            fiscal_quarter: 0,
            fiscal_year: 0,
            eps_actual: None,
            eps_estimate: None,
            revenue_actual: None,
            revenue_estimate: None,
            source: CorporateEventSource::default(),
            observed_at: String::new(),
        }
    }
}

/// 企業イベント Provider の監査可能な read model。
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CorporateEventProviderReadModel {
    pub health: CorporateEventProviderHealth,
    pub source: CorporateEventSource,
    pub retrieved_at: String,
    pub diagnostic: Option<String>,
    pub events: Vec<CorporateEventObservation>,
}

impl CorporateEventProviderReadModel {
    pub(crate) fn unavailable(source: CorporateEventSource, diagnostic: impl Into<String>) -> Self {
        Self {
            source,
            retrieved_at: String::new(),
            diagnostic: Some(diagnostic.into()),
            ..Self::default()
        }
    }
}

/// 企業イベントを market date 単位で取得する provider port。
pub(crate) trait CorporateEventProvider {
    fn load_for_market_date(
        &self,
        market_date: NaiveDate,
        symbols: &[String],
    ) -> CorporateEventProviderReadModel;
}

#[cfg(test)]
mod tests {
    use super::{
        CorporateEventObservation, CorporateEventProviderHealth, CorporateEventProviderReadModel,
        CorporateEventReleaseWindow, CorporateEventSource, CorporateEventSourceKind,
    };
    use chrono::NaiveDate;

    #[test]
    fn source_object_preserves_provider_kind_and_optional_url() {
        let source = CorporateEventSource {
            provider_id: "fixture".to_string(),
            source_kind: CorporateEventSourceKind::ExternalFixture,
            source_url: Some("tests/fixtures/corporate_events/example.json".to_string()),
        };

        assert_eq!(source.provider_id, "fixture");
        assert_eq!(
            source.source_kind,
            CorporateEventSourceKind::ExternalFixture
        );
        assert_eq!(
            source.source_url.as_deref(),
            Some("tests/fixtures/corporate_events/example.json")
        );
    }

    #[test]
    fn release_window_default_is_unknown() {
        assert_eq!(
            CorporateEventReleaseWindow::default(),
            CorporateEventReleaseWindow::Unknown
        );
    }

    #[test]
    fn normalized_earnings_observation_preserves_provider_facts() {
        let observation = CorporateEventObservation {
            symbol: "NVDA".to_string(),
            market_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            release_window: CorporateEventReleaseWindow::AfterMarketClose,
            fiscal_quarter: 2,
            fiscal_year: 2027,
            eps_actual: None,
            eps_estimate: None,
            revenue_actual: Some(96_200_000_000.0),
            revenue_estimate: Some(95_000_000_000.0),
            ..Default::default()
        };

        assert_eq!(observation.symbol, "NVDA");
        assert_eq!(observation.market_date.to_string(), "2026-08-27");
        assert_eq!(
            observation.release_window,
            CorporateEventReleaseWindow::AfterMarketClose
        );
        assert_eq!(observation.fiscal_quarter, 2);
        assert_eq!(observation.fiscal_year, 2027);
        assert_eq!(observation.revenue_actual, Some(96_200_000_000.0));
    }

    #[test]
    fn unavailable_read_model_has_no_events_and_a_safe_diagnostic() {
        let source = CorporateEventSource {
            provider_id: "fixture".to_string(),
            source_kind: CorporateEventSourceKind::ExternalFixture,
            source_url: None,
        };
        let read_model =
            CorporateEventProviderReadModel::unavailable(source.clone(), "api key missing");

        assert_eq!(read_model.health, CorporateEventProviderHealth::Unavailable);
        assert!(read_model.events.is_empty());
        assert_eq!(read_model.diagnostic.as_deref(), Some("api key missing"));
        assert_eq!(read_model.source, source);
        assert!(read_model.source.source_url.is_none());
    }
}
