use crate::features::research::infrastructure::gray_rhino_evidence_store::GrayRhinoEvidenceStore;
use std::path::Path;

/// Governance evidence store の infrastructure 実装を CLI から隠蔽する。
pub(crate) fn build_governance_evidence_store_adapter(save_dir: &Path) -> GrayRhinoEvidenceStore {
    GrayRhinoEvidenceStore::new(save_dir)
}
