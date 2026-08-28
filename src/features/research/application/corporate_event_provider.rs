use chrono::NaiveDate;

/// 企業イベント Provider の取得状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CorporateEventProviderHealth {
    Healthy,
    #[default]
    Unavailable,
}

/// Finnhub が返す企業決算の発表時間帯。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CorporateEventReleaseWindow {
    BeforeMarketOpen,
    #[default]
    AfterMarketClose,
    DuringMarketHours,
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
    pub source: String,
    pub source_url: String,
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
            source: String::new(),
            source_url: String::new(),
            observed_at: String::new(),
        }
    }
}

/// 企業イベント Provider の監査可能な read model。
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CorporateEventProviderReadModel {
    pub health: CorporateEventProviderHealth,
    pub source: String,
    pub source_url: String,
    pub retrieved_at: String,
    pub diagnostic: Option<String>,
    pub events: Vec<CorporateEventObservation>,
}

impl CorporateEventProviderReadModel {
    pub(crate) fn unavailable(diagnostic: impl Into<String>) -> Self {
        Self {
            source: "finnhub-earnings-calendar".to_string(),
            source_url: "https://finnhub.io/api/v1/calendar/earnings".to_string(),
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
        CorporateEventReleaseWindow,
    };
    use chrono::NaiveDate;

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
        let read_model = CorporateEventProviderReadModel::unavailable("api key missing");

        assert_eq!(read_model.health, CorporateEventProviderHealth::Unavailable);
        assert!(read_model.events.is_empty());
        assert_eq!(read_model.diagnostic.as_deref(), Some("api key missing"));
        assert!(!read_model.source_url.contains("token"));
    }
}
