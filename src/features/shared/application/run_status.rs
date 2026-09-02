use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use crate::features::shared::domain::reconciliation::{PositionMismatch, ReconciliationReport};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Succeeded,
    Failed {
        reason: String,
    },
    #[default]
    Skipped,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreflightResult {
    pub status: String,
    pub sub_quota_used: i32,
    pub sub_quota_total: i32,
    pub market_rights: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StateMachineSummary {
    pub from_state: String,
    pub to_state: String,
    pub reset_confirmed: bool,
    pub reset_blocked: bool,
    pub soft_reset_applied: bool,
    pub duration_locked: bool,
    pub defensive_override: bool,
    pub core_breakdown: bool,
    pub reconciliation_mismatch_count: usize,
    pub preflight_failed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GrayRhinoProviderStatus {
    pub status: String,
    pub accepted: u64,
    pub rejected: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GrayRhinoCollectionStatus {
    pub status: String,
    pub date: Option<String>,
    pub sec: GrayRhinoProviderStatus,
    pub finnhub: GrayRhinoProviderStatus,
    pub fred: GrayRhinoProviderStatus,
    pub failed_providers: Vec<String>,
}

/// 日次レポートを生成した実行環境の identity。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReportRuntimeIdentity {
    pub report_run_id: String,
    pub report_run_at: String,
    pub git_commit_sha: String,
    pub git_branch: String,
    pub binary_version: String,
    pub decision_snapshot_version: String,
    pub data_snapshot_id: String,
    pub data_snapshot_date: String,
    #[serde(default)]
    pub build_git_commit_sha: String,
    #[serde(default)]
    pub execution_git_commit_sha: String,
}

impl ReportRuntimeIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        report_run_id: impl Into<String>,
        report_run_at: impl Into<String>,
        git_commit_sha: impl Into<String>,
        git_branch: impl Into<String>,
        binary_version: impl Into<String>,
        decision_snapshot_version: impl Into<String>,
        data_snapshot_id: impl Into<String>,
        data_snapshot_date: impl Into<String>,
        build_git_commit_sha: impl Into<String>,
        execution_git_commit_sha: impl Into<String>,
    ) -> Self {
        Self {
            report_run_id: report_run_id.into(),
            report_run_at: report_run_at.into(),
            git_commit_sha: git_commit_sha.into(),
            git_branch: git_branch.into(),
            binary_version: binary_version.into(),
            decision_snapshot_version: decision_snapshot_version.into(),
            data_snapshot_id: data_snapshot_id.into(),
            data_snapshot_date: data_snapshot_date.into(),
            build_git_commit_sha: build_git_commit_sha.into(),
            execution_git_commit_sha: execution_git_commit_sha.into(),
        }
    }

    /// 実行環境の明示値を優先し、ローカル実行では build metadata に安全に戻す。
    pub fn from_environment(
        report_run_id: impl Into<String>,
        report_run_at: impl Into<String>,
        decision_snapshot_version: impl Into<String>,
        data_snapshot_id: impl Into<String>,
        data_snapshot_date: impl Into<String>,
    ) -> Self {
        let build_sha = option_env!("SENTINEL_BUILD_GIT_SHA")
            .unwrap_or("UNKNOWN")
            .to_string();
        let build_branch = option_env!("SENTINEL_BUILD_GIT_BRANCH")
            .unwrap_or("UNKNOWN")
            .to_string();
        let execution_sha = std::env::var("SENTINEL_EXECUTION_GIT_SHA")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| build_sha.clone());
        let branch = std::env::var("SENTINEL_EXECUTION_GIT_BRANCH")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(build_branch);
        let git_commit_sha = if is_known_revision(&execution_sha) {
            execution_sha.clone()
        } else {
            build_sha.clone()
        };

        Self::new(
            report_run_id,
            report_run_at,
            git_commit_sha,
            branch,
            env!("CARGO_PKG_VERSION"),
            decision_snapshot_version,
            data_snapshot_id,
            data_snapshot_date,
            build_sha,
            execution_sha,
        )
    }

    pub fn code_revision_known(&self) -> bool {
        is_known_revision(&self.git_commit_sha)
            && is_known_revision(&self.build_git_commit_sha)
            && is_known_revision(&self.execution_git_commit_sha)
    }

    pub fn revision_mismatch(&self) -> bool {
        is_known_revision(&self.build_git_commit_sha)
            && is_known_revision(&self.execution_git_commit_sha)
            && self.build_git_commit_sha != self.execution_git_commit_sha
    }
}

fn is_known_revision(value: &str) -> bool {
    !value.trim().is_empty() && !value.eq_ignore_ascii_case("UNKNOWN")
}

/// 一つの入力集合の provenance record。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataProvenance {
    pub status: String,
    pub source: String,
    #[serde(default)]
    pub as_of: Option<String>,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub snapshot_digest: Option<String>,
    #[serde(default)]
    pub record_count: usize,
    #[serde(default)]
    pub diagnostic: Option<String>,
}

impl Default for DataProvenance {
    fn default() -> Self {
        Self {
            status: "UNAVAILABLE".to_string(),
            source: "UNKNOWN".to_string(),
            as_of: None,
            snapshot_id: None,
            snapshot_digest: None,
            record_count: 0,
            diagnostic: Some("provenance_unavailable".to_string()),
        }
    }
}

impl DataProvenance {
    pub fn unavailable(source: impl Into<String>, diagnostic: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            diagnostic: Some(diagnostic.into()),
            ..Self::default()
        }
    }
}

/// 日次レポートが参照した入力集合を固定 key で保持する bundle。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DataProvenanceBundle {
    pub price_history: DataProvenance,
    pub benchmark_history: DataProvenance,
    pub relative_strength_input: DataProvenance,
    pub leadership_history: DataProvenance,
    pub market_change_baseline: DataProvenance,
    pub corporate_event_evidence: DataProvenance,
    pub expectation_data: DataProvenance,
    pub price_volume_history: DataProvenance,
}

impl DataProvenanceBundle {
    pub fn unavailable(source: impl Into<String>, diagnostic: impl Into<String>) -> Self {
        let source = source.into();
        let diagnostic = diagnostic.into();
        let record = || DataProvenance::unavailable(source.clone(), diagnostic.clone());
        Self {
            price_history: record(),
            benchmark_history: record(),
            relative_strength_input: record(),
            leadership_history: record(),
            market_change_baseline: record(),
            corporate_event_evidence: record(),
            expectation_data: record(),
            price_volume_history: record(),
        }
    }
}

/// Integrity の表示状態。取引判断へ渡してはならない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum RuntimeIntegrityStatus {
    Healthy,
    Degraded,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeIntegrity {
    pub status: RuntimeIntegrityStatus,
    pub decision_weight: u8,
    pub code_revision_known: bool,
    pub data_snapshot_known: bool,
    pub decision_snapshot_known: bool,
    pub rs_input_consistent: bool,
    pub leadership_snapshot_consistent: bool,
    pub report_artifact_matches_run: bool,
    pub diagnostics: Vec<String>,
}

impl RuntimeIntegrity {
    pub fn from_checks(
        identity: &ReportRuntimeIdentity,
        data_snapshot_known: bool,
        decision_snapshot_known: bool,
        rs_input_consistent: bool,
        leadership_snapshot_consistent: bool,
        report_artifact_matches_run: bool,
    ) -> Self {
        let code_revision_known = identity.code_revision_known();
        let mut diagnostics = Vec::new();
        if identity.revision_mismatch() {
            diagnostics.push("RUNTIME_MISMATCH".to_string());
        }
        if !code_revision_known {
            diagnostics.push("CODE_REVISION_UNAVAILABLE".to_string());
        }
        if !data_snapshot_known {
            diagnostics.push("DATA_SNAPSHOT_UNAVAILABLE".to_string());
        }
        if !decision_snapshot_known {
            diagnostics.push("DECISION_SNAPSHOT_UNAVAILABLE".to_string());
        }
        if !rs_input_consistent {
            diagnostics.push("RS_INPUT_DEGRADED".to_string());
        }
        if !leadership_snapshot_consistent {
            diagnostics.push("LEADERSHIP_SNAPSHOT_DEGRADED".to_string());
        }
        if !report_artifact_matches_run {
            diagnostics.push("REPORT_ARTIFACT_MISMATCH".to_string());
        }
        diagnostics.sort();
        let status = if !code_revision_known || !data_snapshot_known || !decision_snapshot_known {
            RuntimeIntegrityStatus::Unavailable
        } else if identity.revision_mismatch()
            || !rs_input_consistent
            || !leadership_snapshot_consistent
            || !report_artifact_matches_run
        {
            RuntimeIntegrityStatus::Degraded
        } else {
            RuntimeIntegrityStatus::Healthy
        };
        Self {
            status,
            decision_weight: 0,
            code_revision_known,
            data_snapshot_known,
            decision_snapshot_known,
            rs_input_consistent,
            leadership_snapshot_consistent,
            report_artifact_matches_run,
            diagnostics,
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.status == RuntimeIntegrityStatus::Healthy
    }
}

/// report artifact の生成と補送を区別する lifecycle。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReportLifecycleMode {
    #[default]
    Generated,
    Resent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReportLifecycle {
    pub mode: ReportLifecycleMode,
    #[serde(default)]
    pub original_generation_revision: Option<String>,
    #[serde(default)]
    pub original_report_run_id: Option<String>,
    #[serde(default)]
    pub resend_revision: Option<String>,
    #[serde(default)]
    pub resent_at: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// serde JSON の安定した内容 digest。record identity と conflict 検証に使う。
pub fn sha256_json<T: Serialize + ?Sized>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serializable JSON value");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RunOutcome {
    pub date: String,
    pub timestamp: String,
    pub preflight: Option<PreflightResult>,
    pub decisioning: DeliveryStatus,
    #[serde(default)]
    pub evidence_collection: DeliveryStatus,
    #[serde(default)]
    pub gray_rhino_collection: GrayRhinoCollectionStatus,
    #[serde(default)]
    pub gray_rhino_rendering: DeliveryStatus,
    pub archival: DeliveryStatus,
    pub notification: DeliveryStatus,
    pub execution: DeliveryStatus,
    pub reconciliation: DeliveryStatus,
    pub reconciliation_report: Option<ReconciliationReport>,
    pub state_machine: Option<StateMachineSummary>,
    pub data_quality: String,
    pub execution_details: Option<serde_json::Value>,
    #[serde(default)]
    pub runtime_identity: Option<ReportRuntimeIdentity>,
    #[serde(default)]
    pub data_provenance: Option<DataProvenanceBundle>,
    #[serde(default)]
    pub runtime_integrity: Option<RuntimeIntegrity>,
    #[serde(default)]
    pub report_lifecycle: Option<ReportLifecycle>,
}

#[cfg(test)]
mod tests {
    use super::{DeliveryStatus, GrayRhinoCollectionStatus, GrayRhinoProviderStatus, RunOutcome};

    #[test]
    fn run_outcome_defaults_missing_evidence_collection_for_legacy_json() {
        let legacy = r#"{
            "date": "2026-05-01",
            "timestamp": "2026-05-01T23:30:00+09:00",
            "preflight": null,
            "decisioning": "succeeded",
            "archival": "succeeded",
            "notification": "skipped",
            "execution": "skipped",
            "reconciliation": "skipped",
            "reconciliation_report": null,
            "state_machine": null,
            "data_quality": "OK",
            "execution_details": null
        }"#;

        let outcome: RunOutcome = serde_json::from_str(legacy).unwrap();
        assert_eq!(outcome.evidence_collection, DeliveryStatus::Skipped);
        assert_eq!(outcome.gray_rhino_collection.status, "");
        assert_eq!(outcome.gray_rhino_rendering, DeliveryStatus::Skipped);
    }

    #[test]
    fn gray_rhino_run_status_records_sensor_status() {
        let outcome = RunOutcome {
            gray_rhino_collection: GrayRhinoCollectionStatus {
                status: "partial_failure".to_string(),
                date: Some("2026-05-25".to_string()),
                fred: GrayRhinoProviderStatus {
                    status: "failed".to_string(),
                    accepted: 0,
                    rejected: 1,
                },
                failed_providers: vec!["fred".to_string()],
                ..Default::default()
            },
            gray_rhino_rendering: DeliveryStatus::Succeeded,
            ..Default::default()
        };
        let encoded = serde_json::to_string(&outcome).unwrap();

        assert!(encoded.contains("gray_rhino_collection"));
        assert!(encoded.contains("partial_failure"));
        assert!(encoded.contains("failed_providers"));
        assert!(encoded.contains("gray_rhino_rendering"));
    }

    #[test]
    fn gray_rhino_collection_status_preserves_partial_provider_coverage() {
        let status = GrayRhinoCollectionStatus {
            status: "partial_failure".to_string(),
            date: Some("2026-05-25".to_string()),
            sec: GrayRhinoProviderStatus {
                status: "succeeded".to_string(),
                accepted: 2,
                rejected: 0,
            },
            fred: GrayRhinoProviderStatus {
                status: "failed".to_string(),
                accepted: 0,
                rejected: 1,
            },
            failed_providers: vec!["fred".to_string()],
            ..Default::default()
        };

        assert_eq!(status.status, "partial_failure");
        assert_eq!(status.sec.accepted, 2);
        assert_eq!(status.fred.rejected, 1);
        assert_eq!(status.failed_providers, vec!["fred"]);
    }
}
