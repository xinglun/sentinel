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
use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidate;
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceRecord, GrayRhinoRiskEffect,
};
use anyhow::Result;
use chrono::NaiveDate;
use serde_json::Value;

pub(crate) trait GrayRhinoDailyReportRepository {
    fn load_previous_snapshot(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Option<GrayRhinoAssessmentSnapshot>>;
    fn save_snapshot_if_changed(&self, snapshot: &GrayRhinoAssessmentSnapshot) -> Result<()>;
    fn load_evidence_records(&self) -> Result<Vec<GrayRhinoEvidenceRecord>>;
    fn load_governance_audits(&self) -> Result<Vec<GovernanceExtractionAuditRecord>>;
    fn collect_auto_candidates(
        &self,
        watch_symbols: &[String],
        as_of_date: NaiveDate,
    ) -> Result<Vec<GrayRhinoCandidate>>;
    fn load_backfill_ops_view(&self) -> Option<Value>;
    fn load_discovery_ops_view(&self) -> Option<Value>;
}

pub(crate) struct GrayRhinoDailyReportViewModel {
    pub assessment: Option<GrayRhinoAssessment>,
    pub evidence_records: Vec<GrayRhinoEvidenceRecord>,
    pub unclassified_record_count: usize,
    pub governance_audits: Vec<GovernanceExtractionAuditRecord>,
    pub display_candidates: Vec<GrayRhinoCandidate>,
    pub monitoring_statuses: Vec<GrayRhinoMonitoringStatus>,
    pub backfill_ops_view: Option<Value>,
    pub discovery_ops_view: Option<Value>,
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
    ) -> Result<GrayRhinoDailyReportViewModel> {
        let previous = self.repository.load_previous_snapshot(as_of_date)?;
        let evidence_records = self.repository.load_evidence_records()?;
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
        if let Some(assessment) = &assessment {
            self.repository
                .save_snapshot_if_changed(&assessment.current)?;
        }
        let auto_candidates = self
            .repository
            .collect_auto_candidates(watch_symbols, as_of_date)?;
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
        })
    }
}

fn dedupe_candidates(candidates: Vec<GrayRhinoCandidate>) -> Vec<GrayRhinoCandidate> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();
    for candidate in candidates {
        let key = format!(
            "{}::{:?}::{:?}",
            candidate.subject, candidate.scope, candidate.kind
        );
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }
    deduped
}
