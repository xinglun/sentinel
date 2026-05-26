use crate::config::AppConfig;
use crate::features::research::infrastructure::sec_governance_source_adapter::GovernanceDocumentSourceAdapter;
use std::path::Path;

/// Governance source adapter の infrastructure 実装を CLI から隠蔽する。
pub(crate) fn build_governance_source_adapter(
    app_config: &AppConfig,
    save_dir: &Path,
) -> GovernanceDocumentSourceAdapter {
    GovernanceDocumentSourceAdapter::new(
        app_config.sec.as_ref().map(|sec| sec.user_agent.clone()),
        save_dir,
    )
}
