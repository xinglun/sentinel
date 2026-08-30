use crate::config::AppConfig;
use crate::features::research::application::corporate_event_evidence_resolver::{
    CorporateEventEvidenceResolution, CorporateEventEvidenceResolver,
    CorporateEventEvidenceResolverInput, ExternalCorporateEventEnrichment,
};
use crate::features::research::application::corporate_event_provider::{
    CorporateEventProviderReadModel, ExpectedCorporateEventProviderReadModel,
};
use crate::features::research::application::official_disclosure_provider::{
    CompanyIdentity, OfficialDisclosureProviderReadModel,
};
use chrono::NaiveDate;
use chrono::{DateTime, Utc};
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

/// Research 内の concrete provider を application Resolver へ束ねる唯一の入口。
pub(crate) fn resolve_corporate_event_evidence(
    app_config: &AppConfig,
    save_dir: &Path,
    market_date: NaiveDate,
    symbols: &[String],
    enrichments: &[ExternalCorporateEventEnrichment],
    external_diagnostic: Option<&str>,
    report_run_at: DateTime<Utc>,
) -> CorporateEventEvidenceResolution {
    let expected = load_alpha_vantage_expected_events(
        symbols,
        save_dir
            .join("corporate_event")
            .join("alpha_vantage_expected_events.json"),
    );
    let official = build_official_disclosure_read_model(app_config, save_dir, market_date, symbols);
    let aggregator = load_finnhub_corporate_events(app_config, market_date, symbols);
    CorporateEventEvidenceResolver::resolve(CorporateEventEvidenceResolverInput {
        subjects: symbols,
        expected: &expected,
        official: &official,
        aggregator: &aggregator,
        enrichments,
        external_diagnostic,
        report_run_at,
    })
}

fn build_official_disclosure_read_model(
    app_config: &AppConfig,
    save_dir: &Path,
    market_date: NaiveDate,
    symbols: &[String],
) -> OfficialDisclosureProviderReadModel {
    let subjects = symbols
        .iter()
        .map(|symbol| CompanyIdentity::new(symbol, None))
        .collect::<Vec<_>>();
    let provider =
        match super::official_disclosure_provider_factory::build_official_disclosure_provider(
            app_config, save_dir,
        ) {
            Ok(provider) => provider,
            Err(error) => {
                return OfficialDisclosureProviderReadModel::unavailable(None, error);
            }
        };
    provider.load_for_market_date(market_date, &subjects)
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
