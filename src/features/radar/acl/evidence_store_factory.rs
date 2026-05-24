use crate::features::evidence::infrastructure::evidence_fetcher_factory::build_evidence_store;
use crate::features::evidence::infrastructure::evidence_store::EvidenceStore;
use std::path::Path;

pub type RadarEvidenceStore = EvidenceStore;

pub fn build_radar_evidence_store(save_dir: &Path) -> RadarEvidenceStore {
    build_evidence_store(save_dir)
}
