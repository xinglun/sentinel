use crate::config::AppConfig;
use crate::features::research::application::corporate_event_provider::{
    CorporateEventProviderReadModel, CorporateEventSource, ExpectedCorporateEventProviderReadModel,
};
use chrono::NaiveDate;
use std::path::Path;

/// Finnhub の concrete adapter を Research 内の ACL で application contract へ接続する。
pub(crate) fn load_finnhub_corporate_events(
    app_config: &AppConfig,
    market_date: NaiveDate,
    symbols: &[String],
) -> CorporateEventProviderReadModel {
    crate::features::research::infrastructure::finnhub_corporate_event_provider::load_finnhub_corporate_events(
        app_config,
        market_date,
        symbols,
    )
}

/// Finnhub adapter が所有する source metadata を ACL 経由で公開する。
pub(crate) fn finnhub_corporate_event_source(market_date: NaiveDate) -> CorporateEventSource {
    crate::features::research::infrastructure::finnhub_corporate_event_provider::finnhub_source(
        market_date,
    )
}

/// Alpha Vantage の expected event adapter を Research ACL 経由で公開する。
#[allow(dead_code)]
pub(crate) fn load_alpha_vantage_expected_events(
    symbols: &[String],
    cache_path: impl AsRef<Path>,
) -> ExpectedCorporateEventProviderReadModel {
    crate::features::research::infrastructure::alpha_vantage_earnings_calendar_provider::load_alpha_vantage_expected_events(
        symbols,
        cache_path,
    )
}

#[cfg(test)]
mod tests {
    use super::load_alpha_vantage_expected_events;
    use crate::features::research::application::corporate_event_provider::ExpectedCorporateEventProviderHealth;

    #[test]
    fn alpha_vantage_acl_keeps_empty_universe_unavailable_without_network() {
        let directory = tempfile::tempdir().expect("temporary directory must be created");
        let read_model = load_alpha_vantage_expected_events(
            &[],
            directory.path().join("alpha-vantage-cache.json"),
        );

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
    }
}
