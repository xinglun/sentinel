use crate::config::AppConfig;
use crate::features::research::application::corporate_event_provider::{
    CorporateEventProviderReadModel, CorporateEventSource,
};
use chrono::NaiveDate;

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
