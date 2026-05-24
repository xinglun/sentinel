use crate::features::radar::acl::evidence_store_factory::{
    build_radar_evidence_store, RadarEvidenceStore,
};
use crate::features::radar::infrastructure::persistence::PersistenceLayer;
use crate::features::radar::infrastructure::transition_log::TransitionLogger;
use std::path::Path;

pub struct RadarRuntimeServices {
    pub persistence: PersistenceLayer,
    pub transition_logger: TransitionLogger,
    pub evidence_store: RadarEvidenceStore,
}

pub fn build_radar_runtime_services(save_dir: &Path) -> RadarRuntimeServices {
    RadarRuntimeServices {
        persistence: PersistenceLayer::new(save_dir),
        transition_logger: TransitionLogger::new(save_dir),
        evidence_store: build_radar_evidence_store(save_dir),
    }
}
