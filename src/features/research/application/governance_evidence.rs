use crate::features::research::domain::gray_rhino_evidence::{
    GovernanceConcentrationEvidence, GrayRhinoEvidenceRecord,
};
use anyhow::{anyhow, Result};

/// GovernanceConcentration evidence の永続化 port。
pub trait GovernanceEvidenceRepository {
    fn save_governance_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool>;
    fn load_governance_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct GovernanceEvidenceIngestionOutcome {
    pub saved: bool,
    pub record: GrayRhinoEvidenceRecord,
}

/// GovernanceConcentration evidence を contract validation 後に保存する。
pub fn ingest_governance_concentration_evidence(
    repository: &dyn GovernanceEvidenceRepository,
    evidence: GovernanceConcentrationEvidence,
) -> Result<GovernanceEvidenceIngestionOutcome> {
    evidence
        .validate()
        .map_err(|err| anyhow!("Invalid governance evidence: {:?}", err))?;
    let record = evidence.to_record();
    let saved = repository.save_governance_evidence(&record)?;
    Ok(GovernanceEvidenceIngestionOutcome { saved, record })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_evidence::{
        GovernanceConcentrationMetrics, GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
    };
    use chrono::NaiveDate;
    use std::cell::RefCell;

    #[derive(Default)]
    struct InMemoryGovernanceEvidenceRepository {
        records: RefCell<Vec<GrayRhinoEvidenceRecord>>,
    }

    impl GovernanceEvidenceRepository for InMemoryGovernanceEvidenceRepository {
        fn save_governance_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
            self.records.borrow_mut().push(record.clone());
            Ok(true)
        }

        fn load_governance_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
            Ok(self.records.borrow().clone())
        }
    }

    #[test]
    fn saves_valid_governance_evidence_without_escalation_detection() {
        let repository = InMemoryGovernanceEvidenceRepository::default();
        let evidence = GovernanceConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Proxy statement".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/proxy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.9,
            extraction_note: "Proxy statement discloses voting rights.".to_string(),
            structural_fact: "Dual class shares create unequal voting rights.".to_string(),
            metrics: GovernanceConcentrationMetrics {
                founder_voting_power: Some(61.2),
                independent_board_ratio: Some(0.5),
                dual_class_structure: Some(true),
                super_voting_rights: Some(true),
                succession_disclosure: Some(false),
            },
        };

        let outcome = ingest_governance_concentration_evidence(&repository, evidence).unwrap();

        assert!(outcome.saved);
        assert_eq!(repository.load_governance_evidence().unwrap().len(), 1);
    }
}
