use crate::features::research::application::dependency_evidence::DependencyEvidenceRepository;
use crate::features::research::application::governance_evidence::GovernanceEvidenceRepository;
use crate::features::research::application::governance_source_pipeline::GovernanceSourceAuditRepository;
use crate::features::research::application::gray_rhino_daily_report::{
    BackfillOpsSummary, DiscoveryOpsSummary, GrayRhinoDailyReportRepository, GrayRhinoRefreshStatus,
};
use crate::features::research::application::institutional_evidence::InstitutionalEvidenceRepository;
use crate::features::research::application::redundancy_evidence::RedundancyEvidenceRepository;
use crate::features::research::domain::governance_source::GovernanceExtractionAuditRecord;
use crate::features::research::domain::gray_rhino::GrayRhinoAssessmentSnapshot;
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateScope,
};
use crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord;
use crate::features::research::infrastructure::gray_rhino_candidate_store::GrayRhinoCandidateStore;
use crate::features::research::infrastructure::gray_rhino_evidence_store::GrayRhinoEvidenceStore;
use crate::features::research::infrastructure::gray_rhino_snapshot_store::GrayRhinoSnapshotStore;
use anyhow::Result;
use chrono::NaiveDate;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) struct FileGrayRhinoDailyReportRepository {
    save_dir: PathBuf,
}

impl FileGrayRhinoDailyReportRepository {
    pub(crate) fn new(save_dir: &Path) -> Self {
        Self {
            save_dir: save_dir.to_path_buf(),
        }
    }
}

impl GrayRhinoDailyReportRepository for FileGrayRhinoDailyReportRepository {
    fn load_previous_snapshot(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Option<GrayRhinoAssessmentSnapshot>> {
        GrayRhinoSnapshotStore::new(&self.save_dir).load_latest_before(as_of_date)
    }

    fn save_snapshot_if_changed(&self, snapshot: &GrayRhinoAssessmentSnapshot) -> Result<()> {
        GrayRhinoSnapshotStore::new(&self.save_dir).save_if_changed(snapshot)
    }

    fn load_evidence_records(&self, as_of_date: NaiveDate) -> Result<Vec<GrayRhinoEvidenceRecord>> {
        let store = GrayRhinoEvidenceStore::new(&self.save_dir);
        let mut records = store.load_governance_evidence()?;
        records.extend(store.load_dependency_evidence()?);
        records.extend(store.load_institutional_evidence()?);
        records.extend(store.load_redundancy_evidence()?);
        Ok(records
            .into_iter()
            .filter(|record| {
                record.source.observed_at <= as_of_date && record.source.retrieved_at <= as_of_date
            })
            .collect())
    }

    fn load_governance_audits(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Vec<GovernanceExtractionAuditRecord>> {
        Ok(GrayRhinoEvidenceStore::new(&self.save_dir)
            .load_governance_extraction_audits()?
            .into_iter()
            .filter(|record| record.observed_at <= as_of_date)
            .collect())
    }

    fn load_persisted_candidates(
        &self,
        watch_symbols: &[String],
        as_of_date: NaiveDate,
    ) -> Result<Vec<GrayRhinoCandidate>> {
        let candidates = GrayRhinoCandidateStore::new(&self.save_dir)
            .load_candidates()?
            .into_iter()
            .filter(|candidate| {
                candidate_in_current_report_scope(candidate, watch_symbols)
                    && candidate.last_confirmed_at() <= as_of_date
                    && candidate.observed_at <= as_of_date
            })
            .collect();
        Ok(candidates)
    }

    fn load_backfill_ops_view(&self, as_of_date: NaiveDate) -> Option<BackfillOpsSummary> {
        let value = load_latest_jsonl_value_as_of(
            &self.save_dir.join("gray_rhino_backfill_runs.jsonl"),
            as_of_date,
        )?;
        Some(BackfillOpsSummary {
            run_id: value
                .get("run_id")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string(),
            source_count: value
                .get("source_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            rejected: value
                .get("rejected")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            stale_sources: value
                .get("stale_sources")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            drift_sources: value
                .get("drift_sources")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
        })
    }

    fn load_discovery_ops_view(&self, as_of_date: NaiveDate) -> Option<DiscoveryOpsSummary> {
        let value = load_latest_jsonl_value_as_of(
            &self.save_dir.join("gray_rhino_discovery_runs.jsonl"),
            as_of_date,
        )?;
        Some(DiscoveryOpsSummary {
            run_id: value
                .get("run_id")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string(),
            source_count: value
                .get("source_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            candidate_count: value
                .get("candidate_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
        })
    }

    fn load_refresh_status(&self, as_of_date: NaiveDate) -> Option<GrayRhinoRefreshStatus> {
        let value = load_latest_json_value_as_of(
            &self.save_dir.join("gray_rhino_refresh_status_latest.json"),
            as_of_date,
        )?;
        Some(GrayRhinoRefreshStatus {
            status: string_field(&value, "status"),
            sec: string_field(&value, "sec"),
            finnhub: string_field(&value, "finnhub"),
            fred: string_field(&value, "fred"),
            sec_accepted: u64_field(&value, "sec_accepted"),
            sec_rejected: u64_field(&value, "sec_rejected"),
            finnhub_accepted: u64_field(&value, "finnhub_accepted"),
            finnhub_rejected: u64_field(&value, "finnhub_rejected"),
            fred_accepted: u64_field(&value, "fred_accepted"),
            fred_rejected: u64_field(&value, "fred_rejected"),
            failed_providers: string_field(&value, "failed_providers"),
            reason: value
                .get("reason")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            date: value
                .get("date")
                .and_then(|value| value.as_str())
                .map(str::to_string),
        })
    }
}

fn candidate_in_current_report_scope(
    candidate: &GrayRhinoCandidate,
    watch_symbols: &[String],
) -> bool {
    candidate.scope == GrayRhinoCandidateScope::Market
        || watch_symbols
            .iter()
            .any(|symbol| symbol.eq_ignore_ascii_case(&candidate.subject))
}

fn load_latest_jsonl_value_as_of(path: &Path, as_of_date: NaiveDate) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value_date(value).is_none_or(|date| date <= as_of_date))
}

fn load_latest_json_value_as_of(path: &Path, as_of_date: NaiveDate) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<Value>(&raw).ok()?;
    value_date(&value)
        .is_none_or(|date| date <= as_of_date)
        .then_some(value)
}

fn value_date(value: &Value) -> Option<NaiveDate> {
    ["as_of_date", "date", "run_date"]
        .iter()
        .filter_map(|key| value.get(key).and_then(|value| value.as_str()))
        .find_map(|raw| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
        .or_else(|| {
            value
                .get("run_id")
                .and_then(|value| value.as_str())
                .and_then(extract_date)
        })
}

fn extract_date(raw: &str) -> Option<NaiveDate> {
    raw.split(|ch: char| !(ch.is_ascii_digit() || ch == '-'))
        .find_map(|part| NaiveDate::parse_from_str(part, "%Y-%m-%d").ok())
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(|value| value.as_u64()).unwrap_or(0)
}
