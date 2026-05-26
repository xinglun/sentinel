use crate::features::evidence::acl::evidence_store_factory::{
    build_evidence_store_adapter, EvidenceStoreAdapter,
};
use std::path::Path;

pub type RadarEvidenceStore = EvidenceStoreAdapter;

pub fn build_radar_evidence_store(save_dir: &Path) -> RadarEvidenceStore {
    build_evidence_store_adapter(save_dir)
}
