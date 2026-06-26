use stock_sentinel::features::evidence::application::evidence::EvidenceRepository;
use stock_sentinel::features::evidence::domain::evidence::{
    AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType,
};
use stock_sentinel::features::evidence::infrastructure::evidence_store::EvidenceStore;
use tempfile::tempdir;

#[test]
fn evidence_store_implements_application_repository_port() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = EvidenceStore::new(dir.path());
    let repo: &dyn EvidenceRepository = &store;
    let recent_date = chrono::Local::now()
        .naive_local()
        .date()
        .format("%Y-%m-%d")
        .to_string();
    let dedupe_key = format!("GOOG:{recent_date}:capex");

    let record = AutomatedEvidenceRecord::new(
        EvidenceSourceType::Manual,
        EvidenceType::CapexPayoff,
        0.8,
        "Capex payoff observed".to_string(),
        recent_date,
        Some("GOOG".to_string()),
        None,
        dedupe_key,
    );

    assert_eq!(repo.save_records(std::slice::from_ref(&record))?, 1);
    assert_eq!(repo.save_records(std::slice::from_ref(&record))?, 0);

    let all = repo.load_all()?;
    assert_eq!(all, vec![record.clone()]);

    let goog = repo.find_by_symbol("GOOG")?;
    assert_eq!(goog, vec![record]);

    assert_eq!(repo.cleanup_old_records(30)?, 0);
    Ok(())
}

#[test]
fn manual_evidence_ingestion_use_case_validates_and_persists() -> anyhow::Result<()> {
    use stock_sentinel::features::evidence::application::evidence::{
        ingest_manual_evidence, EvidenceRepository, ManualEvidenceIngestionRequest,
    };
    use stock_sentinel::features::evidence::domain::evidence::EvidenceType;

    let dir = tempfile::tempdir()?;
    let store = EvidenceStore::new(dir.path());
    let repository: &dyn EvidenceRepository = &store;

    let outcome = ingest_manual_evidence(
        repository,
        ManualEvidenceIngestionRequest {
            evidence_type: "earnings".to_string(),
            confidence: 0.8,
            description: "manual earnings check".to_string(),
            event_date: Some("2026-05-24".to_string()),
            symbol: Some("MSFT".to_string()),
            source_url: Some("https://example.com/msft".to_string()),
            fallback_date: "2026-05-23".to_string(),
            retention_days: Some(30),
        },
    )?;

    assert_eq!(outcome.saved_count, 1);
    assert_eq!(
        outcome.record.evidence_type,
        EvidenceType::EarningsValidation
    );
    assert!(outcome
        .record
        .dedupe_key()
        .contains("MSFT:earnings:2026-05-24"));
    assert_eq!(repository.load_all()?.len(), 1);
    Ok(())
}

#[test]
fn manual_evidence_ingestion_use_case_rejects_invalid_date() -> anyhow::Result<()> {
    use stock_sentinel::features::evidence::application::evidence::{
        ingest_manual_evidence, EvidenceRepository, ManualEvidenceIngestionRequest,
    };

    let dir = tempfile::tempdir()?;
    let store = EvidenceStore::new(dir.path());
    let repository: &dyn EvidenceRepository = &store;

    let error = ingest_manual_evidence(
        repository,
        ManualEvidenceIngestionRequest {
            evidence_type: "capex".to_string(),
            confidence: 0.8,
            description: "manual capex check".to_string(),
            event_date: Some("2026/05/24".to_string()),
            symbol: Some("GOOG".to_string()),
            source_url: None,
            fallback_date: "2026-05-23".to_string(),
            retention_days: Some(30),
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("Invalid date format"));
    Ok(())
}
