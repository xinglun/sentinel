use std::collections::HashMap;
use stock_sentinel::features::evidence::acl::evidence_ingestion::{
    FixtureFetcher, RuleBasedExtractor,
};
use stock_sentinel::features::evidence::application::evidence_ingestion::{
    EvidenceExtractor, SourceDocument, SourceFetcher,
};
use stock_sentinel::features::evidence::domain::evidence::{EvidenceSourceType, EvidenceType};

#[test]
fn rule_based_extractor_works_through_application_port() {
    let extractor: &dyn EvidenceExtractor = &RuleBasedExtractor::new();
    let doc = SourceDocument {
        title: "MSFT earnings".to_string(),
        content: "revenue growth and margin expansion validate AI monetization".to_string(),
        url: "https://example.com/msft".to_string(),
        source_type: EvidenceSourceType::OfficialIR,
        symbol: "MSFT".to_string(),
        metadata: HashMap::new(),
    };

    let records = extractor.extract(&doc);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].evidence_type, EvidenceType::EarningsValidation);
    assert_eq!(records[0].symbol.as_deref(), Some("MSFT"));
}

#[test]
fn core_reexport_preserves_source_document_import_path() {
    let doc = stock_sentinel::features::evidence::application::evidence_ingestion::SourceDocument {
        title: "GOOG capex".to_string(),
        content: "capex payoff".to_string(),
        url: "https://example.com/goog".to_string(),
        source_type: EvidenceSourceType::NewsMedia,
        symbol: "GOOG".to_string(),
        metadata: HashMap::new(),
    };

    assert_eq!(doc.symbol, "GOOG");
    assert_eq!(doc.source_type, EvidenceSourceType::NewsMedia);
}

#[test]
fn infrastructure_extractor_matches_core_reexport_boundary() {
    let core_extractor: &dyn EvidenceExtractor = &RuleBasedExtractor::new();
    let infrastructure_extractor: &dyn EvidenceExtractor = &RuleBasedExtractor::new();
    let doc = SourceDocument {
        title: "NVDA order visibility".to_string(),
        content: "backlog and demand outstripping supply".to_string(),
        url: "https://example.com/nvda".to_string(),
        source_type: EvidenceSourceType::NewsMedia,
        symbol: "NVDA".to_string(),
        metadata: HashMap::new(),
    };

    let via_core = core_extractor.extract(&doc);
    let via_infra = infrastructure_extractor.extract(&doc);
    assert_eq!(via_core, via_infra);
    assert_eq!(via_infra[0].evidence_type, EvidenceType::OrderVisibility);
}

#[tokio::test]
async fn infrastructure_fetcher_matches_core_reexport_boundary() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("sample.txt"), "earnings validation")?;
    let base_path = dir.path().to_string_lossy();

    let core_fetcher: &dyn SourceFetcher = &FixtureFetcher::new(&base_path);
    let infrastructure_fetcher: &dyn SourceFetcher = &FixtureFetcher::new(&base_path);

    let via_core = core_fetcher
        .fetch("sample.txt", "MSFT", EvidenceSourceType::Manual, 1)
        .await?;
    let via_infra = infrastructure_fetcher
        .fetch("sample.txt", "MSFT", EvidenceSourceType::Manual, 1)
        .await?;

    assert_eq!(via_core.content, via_infra.content);
    assert_eq!(via_core.symbol, via_infra.symbol);
    assert_eq!(via_core.source_type, via_infra.source_type);
    Ok(())
}

#[tokio::test]
async fn evidence_collection_use_case_supports_dry_run_without_persistence() -> anyhow::Result<()> {
    use stock_sentinel::features::evidence::application::evidence_ingestion::{
        collect_evidence_from_source, CollectEvidenceRequest,
    };

    let dir = tempfile::tempdir()?;
    std::fs::write(
        dir.path().join("dry.txt"),
        "revenue growth and margin expansion",
    )?;
    let base_path = dir.path().to_string_lossy();
    let fetcher = FixtureFetcher::new(&base_path);
    let extractor = RuleBasedExtractor::new();

    let outcome = collect_evidence_from_source(
        &fetcher,
        &extractor,
        None,
        CollectEvidenceRequest {
            url: "dry.txt".to_string(),
            symbol: "MSFT".to_string(),
            source_type: EvidenceSourceType::NewsMedia,
            days: 1,
            persist: false,
            retention_days: Some(30),
        },
    )
    .await?;

    assert_eq!(outcome.saved_count, 0);
    assert_eq!(outcome.cleanup_count, 0);
    assert_eq!(outcome.records.len(), 1);
    assert!(outcome.records[0].dedupe_key().contains("AUTO:"));
    assert_eq!(outcome.document.symbol, "MSFT");
    Ok(())
}

#[tokio::test]
async fn evidence_collection_use_case_persists_through_repository_port() -> anyhow::Result<()> {
    use stock_sentinel::features::evidence::application::evidence::EvidenceRepository;
    use stock_sentinel::features::evidence::application::evidence_ingestion::{
        collect_evidence_from_source, CollectEvidenceRequest,
    };
    use stock_sentinel::features::evidence::infrastructure::evidence_store::EvidenceStore;

    let source_dir = tempfile::tempdir()?;
    std::fs::write(
        source_dir.path().join("persist.txt"),
        "backlog and demand outstripping supply",
    )?;
    let fetch_base_path = source_dir.path().to_string_lossy();
    let fetcher = FixtureFetcher::new(&fetch_base_path);
    let extractor = RuleBasedExtractor::new();
    let store_dir = tempfile::tempdir()?;
    let store = EvidenceStore::new(store_dir.path());
    let repository: &dyn EvidenceRepository = &store;

    let outcome = collect_evidence_from_source(
        &fetcher,
        &extractor,
        Some(repository),
        CollectEvidenceRequest {
            url: "persist.txt".to_string(),
            symbol: "NVDA".to_string(),
            source_type: EvidenceSourceType::OfficialIR,
            days: 1,
            persist: true,
            retention_days: Some(30),
        },
    )
    .await?;

    assert_eq!(outcome.saved_count, 1);
    let saved = repository.load_all()?;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].evidence_type, EvidenceType::OrderVisibility);
    assert!(saved[0].dedupe_key().contains("NVDA"));
    Ok(())
}

#[tokio::test]
async fn evidence_batch_collection_use_case_counts_success_and_failure() -> anyhow::Result<()> {
    use stock_sentinel::features::evidence::application::evidence_ingestion::{
        collect_evidence_batch, BatchCollectEvidenceRequest, BatchEvidenceTarget,
    };

    let dir = tempfile::tempdir()?;
    std::fs::write(
        dir.path().join("ok.txt"),
        "capex and infrastructure build-out",
    )?;
    let base_path = dir.path().to_string_lossy();
    let fetcher = FixtureFetcher::new(&base_path);
    let extractor = RuleBasedExtractor::new();

    let outcome = collect_evidence_batch(
        &fetcher,
        &extractor,
        None,
        BatchCollectEvidenceRequest {
            targets: vec![
                BatchEvidenceTarget {
                    symbol: "GOOG".to_string(),
                    url: "ok.txt".to_string(),
                },
                BatchEvidenceTarget {
                    symbol: "MSFT".to_string(),
                    url: "missing.txt".to_string(),
                },
            ],
            source_type: EvidenceSourceType::OfficialIR,
            days: 1,
            persist: false,
            retention_days: Some(30),
        },
    )
    .await?;

    assert_eq!(outcome.success_count, 1);
    assert_eq!(outcome.failure_count, 1);
    assert_eq!(outcome.failures[0].symbol, "MSFT");
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].evidence_type, EvidenceType::CapexPayoff);
    assert!(outcome.records[0].dedupe_key().contains("GOOG"));
    Ok(())
}

#[test]
fn evidence_cli_batch_fetcher_rejects_missing_finnhub_key_outside_dry_run() -> anyhow::Result<()> {
    use stock_sentinel::config::AppConfig;
    use stock_sentinel::features::evidence::infrastructure::evidence_fetcher_factory::build_batch_evidence_fetcher;

    let config_text = r#"
version = 1
provider = "yahoo"
watchlist = []

[output]
save_to = "./target/test-output"
timezone = "Asia/Tokyo"
format = "markdown"

[rules.trend]
lookback_days = 20
flat_threshold_pct = 0.5

[rules.deviation_bands]
overheat_2 = 30.0
optimal = -5.0

[rules.actions]
overheat_2 = "stop"
optimal = "buy"
fear = "fear"

[rules.market_state_engine]
continuity_threshold = 2
stability_threshold = 5.5
"#;
    let config: AppConfig = toml::from_str(config_text)?;

    let error = match build_batch_evidence_fetcher(&config, "finnhub", false) {
        Ok(_) => anyhow::bail!("missing Finnhub key should fail"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains("Finnhub API key is not configured"));
    Ok(())
}

#[test]
fn evidence_cli_url_fetcher_accepts_fixture_path_without_config() -> anyhow::Result<()> {
    use stock_sentinel::config::AppConfig;
    use stock_sentinel::features::evidence::infrastructure::evidence_fetcher_factory::build_url_evidence_fetcher;

    let config_text = r#"
version = 1
provider = "yahoo"
watchlist = []

[output]
save_to = "./target/test-output"
timezone = "Asia/Tokyo"
format = "markdown"

[rules.trend]
lookback_days = 20
flat_threshold_pct = 0.5

[rules.deviation_bands]
overheat_2 = 30.0
optimal = -5.0

[rules.actions]
overheat_2 = "stop"
optimal = "buy"
fear = "fear"

[rules.market_state_engine]
continuity_threshold = 2
stability_threshold = 5.5
"#;
    let config: AppConfig = toml::from_str(config_text)?;

    let _fetcher = build_url_evidence_fetcher(&config, "fixture.txt")?;
    Ok(())
}
