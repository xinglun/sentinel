use crate::features::radar::infrastructure::persistence::PersistenceLayer;
use crate::features::radar::infrastructure::transition_log::TransitionLogger;
use std::path::Path;

pub struct RadarRuntimeServices {
    pub persistence: PersistenceLayer,
    pub transition_logger: TransitionLogger,
}

pub fn build_radar_runtime_services(save_dir: &Path) -> RadarRuntimeServices {
    RadarRuntimeServices {
        persistence: PersistenceLayer::new(save_dir),
        transition_logger: TransitionLogger::new(save_dir),
    }
}
