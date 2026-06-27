use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceRecord, RedundancyEvidence,
};
use anyhow::{anyhow, Result};

/// Redundancy evidence の永続化 port。
pub trait RedundancyEvidenceRepository {
    fn save_redundancy_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool>;
    #[allow(dead_code)]
    fn load_redundancy_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedundancyEvidenceIngestionOutcome {
    pub saved: bool,
    pub record: GrayRhinoEvidenceRecord,
}

/// Redundancy evidence を contract validation 後に保存する。
pub fn ingest_redundancy_evidence(
    repository: &dyn RedundancyEvidenceRepository,
    evidence: RedundancyEvidence,
) -> Result<RedundancyEvidenceIngestionOutcome> {
    evidence
        .validate()
        .map_err(|err| anyhow!("Invalid redundancy evidence: {:?}", err))?;
    let record = evidence.to_record();
    let saved = repository.save_redundancy_evidence(&record)?;
    Ok(RedundancyEvidenceIngestionOutcome { saved, record })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceSourceType, GrayRhinoSourceReference, RedundancyMetrics,
    };
    use chrono::NaiveDate;
    use std::cell::RefCell;

    #[derive(Default)]
    struct InMemoryRedundancyEvidenceRepository {
        records: RefCell<Vec<GrayRhinoEvidenceRecord>>,
    }

    impl RedundancyEvidenceRepository for InMemoryRedundancyEvidenceRepository {
        fn save_redundancy_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
            self.records.borrow_mut().push(record.clone());
            Ok(true)
        }

        fn load_redundancy_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
            Ok(self.records.borrow().clone())
        }
    }

    #[test]
    fn saves_valid_redundancy_evidence_without_escalation_detection() {
        let repository = InMemoryRedundancyEvidenceRepository::default();
        let evidence = RedundancyEvidence {
            subject: "Example issuer".to_string(),
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::IndependentAudit,
                source_title: "Redundancy disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/redundancy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.84,
            extraction_note: "Redundancy disclosure identifies fallback capacity.".to_string(),
            structural_fact: "Alternative supplier and failover path are documented.".to_string(),
            metrics: RedundancyMetrics {
                fallback_available: Some(true),
                alternative_supplier_count: Some(2),
                redundancy_ratio: Some(0.6),
                recovery_path_disclosed: Some(true),
                failover_tested: Some(true),
            },
        };

        let outcome = ingest_redundancy_evidence(&repository, evidence).unwrap();

        assert!(outcome.saved);
        assert_eq!(repository.load_redundancy_evidence().unwrap().len(), 1);
    }
}
