use crate::features::research::domain::gray_rhino_evidence::{
    DependencyConcentrationEvidence, GrayRhinoEvidenceRecord,
};
use anyhow::{anyhow, Result};

/// DependencyConcentration evidence の永続化 port。
///
/// Phase 3-B 初期段階では collector をまだ接続しないため、port 定義を先行させる。
#[allow(dead_code)]
pub trait DependencyEvidenceRepository {
    fn save_dependency_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool>;
    fn load_dependency_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>>;
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct DependencyEvidenceIngestionOutcome {
    pub saved: bool,
    pub record: GrayRhinoEvidenceRecord,
}

/// DependencyConcentration evidence を contract validation 後に保存する。
#[allow(dead_code)]
pub fn ingest_dependency_concentration_evidence(
    repository: &dyn DependencyEvidenceRepository,
    evidence: DependencyConcentrationEvidence,
) -> Result<DependencyEvidenceIngestionOutcome> {
    evidence
        .validate()
        .map_err(|err| anyhow!("Invalid dependency evidence: {:?}", err))?;
    let record = evidence.to_record();
    let saved = repository.save_dependency_evidence(&record)?;
    Ok(DependencyEvidenceIngestionOutcome { saved, record })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_evidence::{
        DependencyConcentrationEvidence, DependencyConcentrationKind,
        DependencyConcentrationMetrics, GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
    };
    use chrono::NaiveDate;
    use std::cell::RefCell;

    #[derive(Default)]
    struct InMemoryDependencyEvidenceRepository {
        records: RefCell<Vec<GrayRhinoEvidenceRecord>>,
    }

    impl DependencyEvidenceRepository for InMemoryDependencyEvidenceRepository {
        fn save_dependency_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
            self.records.borrow_mut().push(record.clone());
            Ok(true)
        }

        fn load_dependency_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
            Ok(self.records.borrow().clone())
        }
    }

    #[test]
    fn saves_valid_dependency_evidence_without_escalation_detection() {
        let repository = InMemoryDependencyEvidenceRepository::default();
        let evidence = DependencyConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "Supplier dependency disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/supplier".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.86,
            extraction_note: "Supplier disclosure identifies dependency concentration.".to_string(),
            structural_fact: "Critical supplier dependency has no disclosed fallback.".to_string(),
            metrics: DependencyConcentrationMetrics {
                dependency_kind: DependencyConcentrationKind::Supplier,
                dependency_name: "Example supplier".to_string(),
                concentration_ratio: Some(0.7),
                single_point_of_failure: Some(true),
                fallback_disclosed: Some(false),
            },
        };

        let outcome = ingest_dependency_concentration_evidence(&repository, evidence).unwrap();

        assert!(outcome.saved);
        assert_eq!(repository.load_dependency_evidence().unwrap().len(), 1);
    }
}
