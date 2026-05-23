use stock_sentinel::application::evidence::EvidenceRepository;
use stock_sentinel::core::evidence_store::EvidenceStore;
use stock_sentinel::domain::evidence::{AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType};
use tempfile::tempdir;

#[test]
fn evidence_store_implements_application_repository_port() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = EvidenceStore::new(dir.path());
    let repo: &dyn EvidenceRepository = &store;

    let record = AutomatedEvidenceRecord {
        source: EvidenceSourceType::Manual,
        evidence_type: EvidenceType::CapexPayoff,
        confidence: 0.8,
        description: "Capex payoff observed".to_string(),
        event_date: "2026-05-24".to_string(),
        symbol: Some("GOOG".to_string()),
        source_url: None,
        dedupe_key: "GOOG:2026-05-24:capex".to_string(),
    };

    assert_eq!(repo.save_records(std::slice::from_ref(&record))?, 1);
    assert_eq!(repo.save_records(std::slice::from_ref(&record))?, 0);

    let all = repo.load_all()?;
    assert_eq!(all, vec![record.clone()]);

    let goog = repo.find_by_symbol("GOOG")?;
    assert_eq!(goog, vec![record]);

    assert_eq!(repo.cleanup_old_records(30)?, 0);
    Ok(())
}
