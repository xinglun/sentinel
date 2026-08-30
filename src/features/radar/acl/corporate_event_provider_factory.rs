use crate::config::AppConfig;
use crate::features::research::application::corporate_event_provider::{
    CorporateEventProviderReadModel, CorporateEventSource,
};
use chrono::NaiveDate;

/// Research の企業イベント Provider を Radar が利用できる ACL 境界へ投影する。
pub(crate) fn load_corporate_event_provider(
    app_config: &AppConfig,
    market_date: NaiveDate,
    symbols: &[String],
) -> CorporateEventProviderReadModel {
    crate::features::research::acl::corporate_event_provider_factory::load_finnhub_corporate_events(
        app_config,
        market_date,
        symbols,
    )
}

/// Radar の fallback が利用する source metadata を Research ACL から受け取る。
pub(crate) fn corporate_event_provider_source(market_date: NaiveDate) -> CorporateEventSource {
    crate::features::research::acl::corporate_event_provider_factory::finnhub_corporate_event_source(
        market_date,
    )
}
