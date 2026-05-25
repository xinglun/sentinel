use crate::features::evidence::infrastructure::evidence_fetcher_factory::build_evidence_store;
use crate::features::evidence::infrastructure::evidence_store::EvidenceStore;
use std::path::Path;

pub type EvidenceStoreAdapter = EvidenceStore;

pub fn build_evidence_store_adapter(save_dir: &Path) -> EvidenceStoreAdapter {
    build_evidence_store(save_dir)
}
