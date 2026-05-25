use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceRecord, RedundancyEvidence,
};
use anyhow::{anyhow, Result};

/// Redundancy evidence の永続化 port。
#[allow(dead_code)]
pub trait RedundancyEvidenceRepository {
    fn save_redundancy_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool>;
    fn load_redundancy_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>>;
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct RedundancyEvidenceIngestionOutcome {
    pub saved: bool,
    pub record: GrayRhinoEvidenceRecord,
}

/// Redundancy evidence を contract validation 後に保存する。
#[allow(dead_code)]
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
