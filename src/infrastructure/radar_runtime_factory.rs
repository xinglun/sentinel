use crate::infrastructure::evidence_fetcher_factory::build_evidence_store;
use crate::infrastructure::evidence_store::EvidenceStore;
use crate::infrastructure::persistence::PersistenceLayer;
use crate::infrastructure::transition_log::TransitionLogger;
use std::path::Path;

pub struct RadarRuntimeServices {
    pub persistence: PersistenceLayer,
    pub transition_logger: TransitionLogger,
    pub evidence_store: EvidenceStore,
}

pub fn build_radar_runtime_services(save_dir: &Path) -> RadarRuntimeServices {
    RadarRuntimeServices {
        persistence: PersistenceLayer::new(save_dir),
        transition_logger: TransitionLogger::new(save_dir),
        evidence_store: build_evidence_store(save_dir),
    }
}
