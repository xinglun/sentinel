use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceRecord, InstitutionalMaturityEvidence,
};
use anyhow::{anyhow, Result};

/// InstitutionalMaturity evidence の永続化 port。
pub trait InstitutionalEvidenceRepository {
    fn save_institutional_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool>;
    #[allow(dead_code)]
    fn load_institutional_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstitutionalEvidenceIngestionOutcome {
    pub saved: bool,
    pub record: GrayRhinoEvidenceRecord,
}

/// InstitutionalMaturity evidence を contract validation 後に保存する。
pub fn ingest_institutional_maturity_evidence(
    repository: &dyn InstitutionalEvidenceRepository,
    evidence: InstitutionalMaturityEvidence,
) -> Result<InstitutionalEvidenceIngestionOutcome> {
    evidence
        .validate()
        .map_err(|err| anyhow!("Invalid institutional evidence: {:?}", err))?;
    let record = evidence.to_record();
    let saved = repository.save_institutional_evidence(&record)?;
    Ok(InstitutionalEvidenceIngestionOutcome { saved, record })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceSourceType, GrayRhinoSourceReference, InstitutionalMaturityMetrics,
    };
    use chrono::NaiveDate;
    use std::cell::RefCell;

    #[derive(Default)]
    struct InMemoryInstitutionalEvidenceRepository {
        records: RefCell<Vec<GrayRhinoEvidenceRecord>>,
    }

    impl InstitutionalEvidenceRepository for InMemoryInstitutionalEvidenceRepository {
        fn save_institutional_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
            self.records.borrow_mut().push(record.clone());
            Ok(true)
        }

        fn load_institutional_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
            Ok(self.records.borrow().clone())
        }
    }

    #[test]
    fn saves_valid_institutional_evidence_without_escalation_detection() {
        let repository = InMemoryInstitutionalEvidenceRepository::default();
        let evidence = InstitutionalMaturityEvidence {
            subject: "Example issuer".to_string(),
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::CompanyDisclosure,
                source_title: "Institutional disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/institutional".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.88,
            extraction_note: "Institutional maturity disclosure is present.".to_string(),
            structural_fact: "External audit and succession planning are disclosed.".to_string(),
            metrics: InstitutionalMaturityMetrics {
                succession_structure_disclosed: Some(true),
                external_audit_present: Some(true),
                disclosure_quality_score: Some(0.9),
                oversight_evolution_disclosed: Some(true),
                compliance_maturity_level: Some("strong".to_string()),
            },
        };

        let outcome = ingest_institutional_maturity_evidence(&repository, evidence).unwrap();

        assert!(outcome.saved);
        assert_eq!(repository.load_institutional_evidence().unwrap().len(), 1);
    }
}
