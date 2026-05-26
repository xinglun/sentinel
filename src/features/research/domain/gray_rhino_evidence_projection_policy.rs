use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord, GrayRhinoRiskEffect,
};

/// evidence の緩和記録を lifecycle 上の解決候補へ投影する domain policy。
pub(crate) fn evidence_resolved_candidates(
    records: &[GrayRhinoEvidenceRecord],
    persisted_candidates: &[GrayRhinoCandidate],
) -> Vec<GrayRhinoCandidate> {
    latest_effective_evidence(records)
        .into_iter()
        .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Mitigating)
        .filter_map(|record| {
            let kind = evidence_category_candidate_kind(record.category)?;
            let subject = record.subject.trim();
            if subject.is_empty() {
                return None;
            }
            if !has_prior_resolvable_candidate(persisted_candidates, subject, kind, record)
                && !has_prior_amplifying_evidence(records, subject, record.category, record)
            {
                return None;
            }
            let scope = if subject.eq_ignore_ascii_case("Market") {
                GrayRhinoCandidateScope::Market
            } else {
                GrayRhinoCandidateScope::Company
            };
            Some(GrayRhinoCandidate {
                scope,
                kind,
                subject: subject.to_string(),
                state: GrayRhinoCandidateState::Resolved,
                evidence: vec![record.structural_fact.clone()],
                watch_triggers: vec!["mitigating evidence".to_string()],
                source_title: record.source.source_title.clone(),
                observed_at: record.source.observed_at,
                source_published_at: Some(record.source.observed_at),
                last_confirmed_at: Some(record.source.observed_at),
                resolved_at: Some(record.source.observed_at),
            })
        })
        .collect()
}

fn latest_effective_evidence(records: &[GrayRhinoEvidenceRecord]) -> Vec<&GrayRhinoEvidenceRecord> {
    let mut latest = std::collections::BTreeMap::<
        (String, GrayRhinoEvidenceCategory),
        &GrayRhinoEvidenceRecord,
    >::new();
    for record in records {
        let subject = record.subject.trim().to_ascii_uppercase();
        if subject.is_empty() {
            continue;
        }
        latest
            .entry((subject, record.category))
            .and_modify(|existing| {
                if (record.source.observed_at, record.source.retrieved_at)
                    > (existing.source.observed_at, existing.source.retrieved_at)
                {
                    *existing = record;
                }
            })
            .or_insert(record);
    }
    latest.into_values().collect()
}

fn has_prior_resolvable_candidate(
    candidates: &[GrayRhinoCandidate],
    subject: &str,
    kind: GrayRhinoCandidateKind,
    record: &GrayRhinoEvidenceRecord,
) -> bool {
    candidates.iter().any(|candidate| {
        candidate.subject.eq_ignore_ascii_case(subject)
            && candidate.kind == kind
            && matches!(
                candidate.state,
                GrayRhinoCandidateState::Visible
                    | GrayRhinoCandidateState::Expanding
                    | GrayRhinoCandidateState::Critical
            )
            && candidate.last_confirmed_at() < record.source.observed_at
    })
}

fn has_prior_amplifying_evidence(
    records: &[GrayRhinoEvidenceRecord],
    subject: &str,
    category: GrayRhinoEvidenceCategory,
    record: &GrayRhinoEvidenceRecord,
) -> bool {
    records.iter().any(|candidate| {
        candidate.subject.eq_ignore_ascii_case(subject)
            && candidate.category == category
            && candidate.risk_effect == GrayRhinoRiskEffect::Amplifying
            && (candidate.source.observed_at, candidate.source.retrieved_at)
                < (record.source.observed_at, record.source.retrieved_at)
    })
}

fn evidence_category_candidate_kind(
    category: GrayRhinoEvidenceCategory,
) -> Option<GrayRhinoCandidateKind> {
    match category {
        GrayRhinoEvidenceCategory::GovernanceConcentration => {
            Some(GrayRhinoCandidateKind::GovernanceConcentration)
        }
        GrayRhinoEvidenceCategory::DependencyConcentration => {
            Some(GrayRhinoCandidateKind::DependencyConcentration)
        }
        GrayRhinoEvidenceCategory::InstitutionalMaturity => {
            Some(GrayRhinoCandidateKind::InstitutionalMaturityGap)
        }
        GrayRhinoEvidenceCategory::Redundancy => Some(GrayRhinoCandidateKind::RedundancyGap),
        GrayRhinoEvidenceCategory::RiskNormalization => None,
    }
}
