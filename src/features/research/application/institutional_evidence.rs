use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceRecord, InstitutionalMaturityEvidence,
};
use anyhow::{anyhow, Result};

/// InstitutionalMaturity evidence の永続化 port。
#[allow(dead_code)]
pub trait InstitutionalEvidenceRepository {
    fn save_institutional_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool>;
    fn load_institutional_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>>;
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct InstitutionalEvidenceIngestionOutcome {
    pub saved: bool,
    pub record: GrayRhinoEvidenceRecord,
}

/// InstitutionalMaturity evidence を contract validation 後に保存する。
#[allow(dead_code)]
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
