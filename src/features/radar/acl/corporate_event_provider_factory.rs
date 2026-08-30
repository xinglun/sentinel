use crate::config::AppConfig;
use crate::features::research::application::corporate_event_evidence_resolver::{
    CorporateEventEvidenceResolution, ExternalCorporateEventEnrichment,
};
use chrono::{DateTime, NaiveDate, Utc};
use std::path::Path;

/// Radar へは Resolver の canonical read model だけを返し、provider concrete 型を隠す。
pub(crate) fn load_corporate_event_evidence(
    app_config: &AppConfig,
    save_dir: &Path,
    market_date: NaiveDate,
    symbols: &[String],
    enrichments: &[ExternalCorporateEventEnrichment],
    external_diagnostic: Option<&str>,
    report_run_at: DateTime<Utc>,
) -> CorporateEventEvidenceResolution {
    crate::features::research::acl::corporate_event_provider_factory::resolve_corporate_event_evidence(
        app_config,
        save_dir,
        market_date,
        symbols,
        enrichments,
        external_diagnostic,
        report_run_at,
    )
}
