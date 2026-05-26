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
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord, GrayRhinoEvidenceRejection,
    GrayRhinoRiskEffect,
};
use crate::features::research::domain::gray_rhino_evidence_projection_policy;
use anyhow::Result;
use chrono::NaiveDate;

pub(crate) trait GrayRhinoDailyReportRepository {
    fn load_previous_snapshot(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Option<GrayRhinoAssessmentSnapshot>>;
    fn save_snapshot_if_changed(&self, snapshot: &GrayRhinoAssessmentSnapshot) -> Result<()>;
    fn load_evidence_read_model(&self, as_of_date: NaiveDate)
        -> Result<GrayRhinoEvidenceReadModel>;
    fn load_governance_audits(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Vec<GovernanceExtractionAuditRecord>>;
    fn load_persisted_candidates(
        &self,
        watch_symbols: &[String],
        as_of_date: NaiveDate,
    ) -> Result<Vec<GrayRhinoCandidate>>;
    fn load_backfill_ops_view(&self, as_of_date: NaiveDate) -> Option<BackfillOpsSummary>;
    fn load_discovery_ops_view(&self, as_of_date: NaiveDate) -> Option<DiscoveryOpsSummary>;
    fn load_refresh_status(&self, as_of_date: NaiveDate) -> Option<GrayRhinoRefreshStatus>;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrayRhinoEvidenceReadRejection {
    pub subject: String,
    pub category: Option<GrayRhinoEvidenceCategory>,
    pub source_title: String,
    pub reason: GrayRhinoEvidenceRejection,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GrayRhinoEvidenceReadModel {
    pub accepted_records: Vec<GrayRhinoEvidenceRecord>,
    pub rejected_records: Vec<GrayRhinoEvidenceReadRejection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrayRhinoSnapshotPersistence {
    SaveIfChanged,
    ReadOnly,
}

pub(crate) struct GrayRhinoDailyReportViewModel {
    pub assessment: Option<GrayRhinoAssessment>,
    pub evidence_records: Vec<GrayRhinoEvidenceRecord>,
    pub scoreable_evidence_records: Vec<GrayRhinoEvidenceRecord>,
    pub rejected_evidence_records: Vec<GrayRhinoEvidenceReadRejection>,
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
        let evidence_read_model = self.repository.load_evidence_read_model(as_of_date)?;
        let evidence_records = evidence_read_model.accepted_records;
        let rejected_evidence_records = evidence_read_model.rejected_records;
        let scoreable_evidence_records = scoreable_evidence_records(&evidence_records);
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
        let mut auto_candidates = self
            .repository
            .load_persisted_candidates(watch_symbols, as_of_date)?;
        auto_candidates.extend(
            gray_rhino_evidence_projection_policy::evidence_resolved_candidates(
                &evidence_records,
                &auto_candidates,
            ),
        );
        let display_candidates = dedupe_candidates(auto_candidates.clone());
        let monitoring_statuses =
            evaluate_gray_rhino_monitoring_states(&auto_candidates, as_of_date);
        Ok(GrayRhinoDailyReportViewModel {
            assessment,
            evidence_records,
            scoreable_evidence_records,
            rejected_evidence_records,
            unclassified_record_count,
            governance_audits: self.repository.load_governance_audits(as_of_date)?,
            display_candidates,
            monitoring_statuses,
            backfill_ops_view: self.repository.load_backfill_ops_view(as_of_date),
            discovery_ops_view: self.repository.load_discovery_ops_view(as_of_date),
            refresh_status: self.repository.load_refresh_status(as_of_date),
        })
    }
}

fn scoreable_evidence_records(records: &[GrayRhinoEvidenceRecord]) -> Vec<GrayRhinoEvidenceRecord> {
    records
        .iter()
        .filter(|record| {
            matches!(
                record.risk_effect,
                GrayRhinoRiskEffect::Amplifying | GrayRhinoRiskEffect::Mitigating
            )
        })
        .cloned()
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidateKind;
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceCategory, GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
    };

    #[test]
    fn gray_rhino_mitigating_evidence_projects_resolved_candidate() {
        let old_amplifying = GrayRhinoEvidenceRecord {
            subject: "GOOG".to_string(),
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Governance risk disclosure".to_string(),
                publisher: "GOOG".to_string(),
                source_url: Some("https://example.com/goog-old".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            },
            confidence: 0.9,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Governance risk is disclosed.".to_string(),
            structural_fact: "Founder voting control was concentrated.".to_string(),
        };
        let record = GrayRhinoEvidenceRecord {
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            source: GrayRhinoSourceReference {
                source_title: "Governance repair disclosure".to_string(),
                source_url: Some("https://example.com/goog".to_string()),
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                ..old_amplifying.source.clone()
            },
            extraction_note: "Governance remediation is disclosed.".to_string(),
            structural_fact: "Founder voting control has been remediated.".to_string(),
            ..old_amplifying.clone()
        };

        let candidates = gray_rhino_evidence_projection_policy::evidence_resolved_candidates(
            &[old_amplifying, record],
            &[],
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].subject, "GOOG");
        assert_eq!(candidates[0].state, GrayRhinoCandidateState::Resolved);
        assert_eq!(
            candidates[0].kind,
            GrayRhinoCandidateKind::GovernanceConcentration
        );
        assert_eq!(
            candidates[0].resolved_at,
            Some(NaiveDate::from_ymd_opt(2026, 5, 25).unwrap())
        );
    }

    #[test]
    fn gray_rhino_mitigating_evidence_without_prior_risk_does_not_project_resolved_candidate() {
        let record = GrayRhinoEvidenceRecord {
            subject: "GOOG".to_string(),
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Governance strength disclosure".to_string(),
                publisher: "GOOG".to_string(),
                source_url: Some("https://example.com/goog".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.9,
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            extraction_note: "Board independence is disclosed.".to_string(),
            structural_fact: "GOOG board independence is strong.".to_string(),
        };

        let candidates =
            gray_rhino_evidence_projection_policy::evidence_resolved_candidates(&[record], &[]);

        assert!(candidates.is_empty());
    }
}
