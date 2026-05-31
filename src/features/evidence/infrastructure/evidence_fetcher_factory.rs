use crate::config::AppConfig;
use crate::features::evidence::application::evidence_ingestion::SourceFetcher;
use crate::features::evidence::infrastructure::evidence_ingestion::{
    FinnhubFetcher, FixtureFetcher, RuleBasedExtractor, SECEDGARFetcher, WebFetcher,
};
use crate::features::evidence::infrastructure::evidence_store::EvidenceStore;
use anyhow::{anyhow, Result};
use std::path::Path;

pub fn build_evidence_store(save_dir: &Path) -> EvidenceStore {
    EvidenceStore::new(save_dir)
}

pub fn build_evidence_extractor() -> RuleBasedExtractor {
    RuleBasedExtractor::new()
}

/// 単一 URL ingest 用の fetcher を選択する。
pub fn build_url_evidence_fetcher(
    app_config: &AppConfig,
    url: &str,
) -> Result<Box<dyn SourceFetcher>> {
    if url == "finnhub" {
        let api_key = app_config
            .finnhub
            .as_ref()
            .map(|f| f.finnhub_api_key.clone())
            .ok_or_else(|| {
                anyhow!("Finnhub API key is not configured. Set FINNHUB_API_KEY env or config.toml")
            })?;
        Ok(Box::new(FinnhubFetcher::new(api_key)))
    } else if url.starts_with("sec://") {
        let user_agent = app_config
            .sec
            .as_ref()
            .map(|s| s.user_agent.clone())
            .ok_or_else(|| {
                anyhow!("SEC user_agent is not configured. Set SEC_USER_AGENT env or config.toml")
            })?;
        Ok(Box::new(SECEDGARFetcher::new(user_agent)))
    } else if url.starts_with("http://") || url.starts_with("https://") {
        Ok(Box::new(WebFetcher))
    } else {
        Ok(Box::new(FixtureFetcher::new(".")))
    }
}

/// batch evidence collection 用の fetcher を選択する。
pub fn build_batch_evidence_fetcher(
    app_config: &AppConfig,
    source_provider: &str,
    dry_run: bool,
) -> Result<Box<dyn SourceFetcher>> {
    if source_provider == "sec" {
        let user_agent = app_config
            .sec
            .as_ref()
            .map(|s| s.user_agent.clone())
            .ok_or_else(|| {
                anyhow!("SEC user_agent is not configured. Set SEC_USER_AGENT env or config.toml")
            })?;
        return Ok(Box::new(SECEDGARFetcher::new(user_agent)));
    }

    match app_config
        .finnhub
        .as_ref()
        .map(|f| f.finnhub_api_key.clone())
    {
        Some(key) => Ok(Box::new(FinnhubFetcher::new(key))),
        None if dry_run => {
            println!(
                "  [INFO] Finnhub API key not found. Falling back to Fixture mode for dry-run."
            );
            Ok(Box::new(FixtureFetcher::new(".")))
        }
        None => Err(anyhow!(
            "Finnhub API key is not configured. Set FINNHUB_API_KEY env or config.toml"
        )),
    }
}
