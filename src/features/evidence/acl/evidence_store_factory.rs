use crate::config::AppConfig;
use crate::features::evidence::application::evidence_ingestion::SourceFetcher;
use crate::features::evidence::infrastructure::evidence_fetcher_factory::{
    build_batch_evidence_fetcher, build_evidence_extractor, build_evidence_store,
    build_url_evidence_fetcher,
};
use crate::features::evidence::infrastructure::evidence_store::EvidenceStore;
use anyhow::Result;
use std::path::Path;

pub type EvidenceStoreAdapter = EvidenceStore;

pub fn build_evidence_store_adapter(save_dir: &Path) -> EvidenceStoreAdapter {
    build_evidence_store(save_dir)
}

pub fn build_evidence_extractor_adapter(
) -> crate::features::evidence::acl::evidence_ingestion::RuleBasedExtractor {
    build_evidence_extractor()
}

pub fn build_url_evidence_fetcher_adapter(
    app_config: &AppConfig,
    url: &str,
) -> Result<Box<dyn SourceFetcher>> {
    build_url_evidence_fetcher(app_config, url)
}

pub fn build_batch_evidence_fetcher_adapter(
    app_config: &AppConfig,
    source_provider: &str,
    dry_run: bool,
) -> Result<Box<dyn SourceFetcher>> {
    build_batch_evidence_fetcher(app_config, source_provider, dry_run)
}
