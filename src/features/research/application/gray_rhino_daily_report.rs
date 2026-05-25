use crate::features::research::application::gray_rhino_assessment::{
    build_evidence_backed_gray_rhino_assessment, build_gray_rhino_assessment,
};
use crate::features::research::application::gray_rhino_monitoring_state::{
    evaluate_gray_rhino_monitoring_states, GrayRhinoMonitoringStatus,
};
use crate::features::research::domain::governance_source::GovernanceExtractionAuditRecord;
use crate::features::research::domain::gray_rhino::{
    GrayRhinoAssessment, GrayRhinoAssessmentSnapshot, GrayRhinoEscalationInput,
};
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateState,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceRecord, GrayRhinoRiskEffect,
};
use anyhow::Result;
use chrono::NaiveDate;

pub(crate) trait GrayRhinoDailyReportRepository {
    fn load_previous_snapshot(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Option<GrayRhinoAssessmentSnapshot>>;
    fn save_snapshot_if_changed(&self, snapshot: &GrayRhinoAssessmentSnapshot) -> Result<()>;
    fn load_evidence_records(&self, as_of_date: NaiveDate) -> Result<Vec<GrayRhinoEvidenceRecord>>;
    fn load_governance_audits(&self) -> Result<Vec<GovernanceExtractionAuditRecord>>;
    fn load_persisted_candidates(
        &self,
        watch_symbols: &[String],
        as_of_date: NaiveDate,
    ) -> Result<Vec<GrayRhinoCandidate>>;
    fn load_backfill_ops_view(&self) -> Option<BackfillOpsSummary>;
    fn load_discovery_ops_view(&self) -> Option<DiscoveryOpsSummary>;
    fn load_refresh_status(&self) -> Option<GrayRhinoRefreshStatus>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackfillOpsSummary {
    pub run_id: String,
    pub source_count: u64,
    pub rejected: u64,
    pub stale_sources: u64,
    pub drift_sources: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscoveryOpsSummary {
    pub run_id: String,
    pub source_count: u64,
    pub candidate_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrayRhinoRefreshStatus {
    pub status: String,
    pub sec: String,
    pub finnhub: String,
    pub fred: String,
    pub sec_accepted: u64,
    pub sec_rejected: u64,
    pub finnhub_accepted: u64,
    pub finnhub_rejected: u64,
    pub fred_accepted: u64,
    pub fred_rejected: u64,
    pub failed_providers: String,
    pub reason: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrayRhinoSnapshotPersistence {
    SaveIfChanged,
    ReadOnly,
}

pub(crate) struct GrayRhinoDailyReportViewModel {
    pub assessment: Option<GrayRhinoAssessment>,
    pub evidence_records: Vec<GrayRhinoEvidenceRecord>,
    pub unclassified_record_count: usize,
    pub governance_audits: Vec<GovernanceExtractionAuditRecord>,
    pub display_candidates: Vec<GrayRhinoCandidate>,
    pub monitoring_statuses: Vec<GrayRhinoMonitoringStatus>,
    pub backfill_ops_view: Option<BackfillOpsSummary>,
    pub discovery_ops_view: Option<DiscoveryOpsSummary>,
    pub refresh_status: Option<GrayRhinoRefreshStatus>,
}

pub(crate) struct GrayRhinoDailyReportUseCase<'a, R: GrayRhinoDailyReportRepository> {
    repository: &'a R,
}

impl<'a, R: GrayRhinoDailyReportRepository> GrayRhinoDailyReportUseCase<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub(crate) fn build(
        &self,
        manual_input: Option<GrayRhinoEscalationInput>,
        watch_symbols: &[String],
        as_of_date: NaiveDate,
        snapshot_persistence: GrayRhinoSnapshotPersistence,
    ) -> Result<GrayRhinoDailyReportViewModel> {
        let previous = self.repository.load_previous_snapshot(as_of_date)?;
        let evidence_records = self.repository.load_evidence_records(as_of_date)?;
        let unclassified_record_count = evidence_records
            .iter()
            .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Unclassified)
            .count();
        let assessment = build_evidence_backed_gray_rhino_assessment(
            &evidence_records,
            as_of_date,
            previous.clone(),
        )
        .or_else(|| {
            manual_input.map(|input| build_gray_rhino_assessment(input, as_of_date, previous))
        });
        if snapshot_persistence == GrayRhinoSnapshotPersistence::SaveIfChanged {
            if let Some(assessment) = &assessment {
                self.repository
                    .save_snapshot_if_changed(&assessment.current)?;
            }
        }
        let auto_candidates = self
            .repository
            .load_persisted_candidates(watch_symbols, as_of_date)?;
        let display_candidates = dedupe_candidates(auto_candidates.clone());
        let monitoring_statuses =
            evaluate_gray_rhino_monitoring_states(&auto_candidates, as_of_date);
        Ok(GrayRhinoDailyReportViewModel {
            assessment,
            evidence_records,
            unclassified_record_count,
            governance_audits: self.repository.load_governance_audits()?,
            display_candidates,
            monitoring_statuses,
            backfill_ops_view: self.repository.load_backfill_ops_view(),
            discovery_ops_view: self.repository.load_discovery_ops_view(),
            refresh_status: self.repository.load_refresh_status(),
        })
    }
}

fn dedupe_candidates(candidates: Vec<GrayRhinoCandidate>) -> Vec<GrayRhinoCandidate> {
    let mut latest = std::collections::BTreeMap::<String, GrayRhinoCandidate>::new();
    for candidate in candidates {
        let key = format!(
            "{}::{:?}::{:?}",
            candidate.subject, candidate.scope, candidate.kind
        );
        latest
            .entry(key)
            .and_modify(|existing| {
                if candidate_is_newer_for_display(&candidate, existing) {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    latest.into_values().collect()
}

fn candidate_is_newer_for_display(
    candidate: &GrayRhinoCandidate,
    existing: &GrayRhinoCandidate,
) -> bool {
    if candidate.state == GrayRhinoCandidateState::Resolved
        && candidate.last_confirmed_at() >= existing.last_confirmed_at()
    {
        return true;
    }
    if existing.state == GrayRhinoCandidateState::Resolved
        && existing.last_confirmed_at() >= candidate.last_confirmed_at()
    {
        return false;
    }
    candidate
        .last_confirmed_at()
        .cmp(&existing.last_confirmed_at())
        .then_with(|| state_rank(candidate.state).cmp(&state_rank(existing.state)))
        .is_gt()
}

fn state_rank(state: GrayRhinoCandidateState) -> u8 {
    match state {
        GrayRhinoCandidateState::Background => 0,
        GrayRhinoCandidateState::Resolved => 0,
        GrayRhinoCandidateState::Cooling => 1,
        GrayRhinoCandidateState::Visible => 2,
        GrayRhinoCandidateState::Expanding => 3,
        GrayRhinoCandidateState::Critical => 4,
    }
}
