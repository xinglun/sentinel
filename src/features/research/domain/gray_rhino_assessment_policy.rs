use crate::features::research::domain::gray_rhino::{GrayRhinoEscalationInput, RiskLevel};
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord, GrayRhinoRiskEffect,
};
use std::collections::{BTreeMap, BTreeSet};

/// 正式 evidence を subject と category 単位の最新状態へ畳み込む。
pub(crate) fn build_evidence_backed_gray_rhino_input(
    records: &[GrayRhinoEvidenceRecord],
) -> Option<GrayRhinoEscalationInput> {
    if records.is_empty() {
        return None;
    }
    let effective_records = latest_effective_subject_category_records(records);
    let has_governance = has_amplifying_category(
        &effective_records,
        GrayRhinoEvidenceCategory::GovernanceConcentration,
    );
    let has_dependency = has_amplifying_category(
        &effective_records,
        GrayRhinoEvidenceCategory::DependencyConcentration,
    );
    let has_institutional_gap = has_amplifying_category(
        &effective_records,
        GrayRhinoEvidenceCategory::InstitutionalMaturity,
    );
    let has_institutional_maturity = has_mitigating_category(
        &effective_records,
        GrayRhinoEvidenceCategory::InstitutionalMaturity,
    );
    let has_redundancy =
        has_mitigating_category(&effective_records, GrayRhinoEvidenceCategory::Redundancy);
    let has_unmitigated_dependency =
        has_dependency_without_same_subject_redundancy(&effective_records);
    let amplifying_count = [has_governance, has_dependency, has_institutional_gap]
        .into_iter()
        .filter(|ready| *ready)
        .count();
    let mitigating_count = [has_institutional_maturity, has_redundancy]
        .into_iter()
        .filter(|ready| *ready)
        .count();
    let classifiable_count = amplifying_count + mitigating_count;
    let unclassified_count = records
        .iter()
        .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Unclassified)
        .count();
    if classifiable_count == 0 {
        return None;
    }
    let scoreable_records: Vec<&GrayRhinoEvidenceRecord> = effective_records
        .iter()
        .copied()
        .filter(|record| {
            matches!(
                record.risk_effect,
                GrayRhinoRiskEffect::Amplifying | GrayRhinoRiskEffect::Mitigating
            )
        })
        .collect();
    let average_confidence = scoreable_records
        .iter()
        .map(|record| record.confidence)
        .sum::<f64>()
        / scoreable_records.len() as f64;
    let quality_ready = classifiable_count >= 2 && average_confidence >= 0.6;

    Some(GrayRhinoEscalationInput {
        risk_expansion_rate: if has_governance && has_dependency && quality_ready {
            RiskLevel::High
        } else if has_governance || has_dependency {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        constraint_growth_rate: if has_institutional_maturity {
            RiskLevel::Elevated
        } else if has_institutional_gap {
            RiskLevel::Low
        } else {
            RiskLevel::Moderate
        },
        dependency_centralization: if has_dependency && quality_ready {
            RiskLevel::High
        } else if has_dependency {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        awareness_decay: if classifiable_count <= 1 {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        narrative_overconfidence: RiskLevel::Moderate,
        single_point_fragility: if has_dependency {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        fallback_survivability_risk: if has_unmitigated_dependency && quality_ready {
            RiskLevel::Elevated
        } else {
            RiskLevel::Low
        },
        notes: vec![format!(
            "Evidence-backed Gray Rhino assessment from scoreable records; amplifying categories: {amplifying_count}/3; mitigating categories: {mitigating_count}/2; unclassified records: {unclassified_count}; scoreable average confidence: {average_confidence:.2}."
        )],
    })
}

fn latest_effective_subject_category_records(
    records: &[GrayRhinoEvidenceRecord],
) -> Vec<&GrayRhinoEvidenceRecord> {
    let mut latest =
        BTreeMap::<(String, GrayRhinoEvidenceCategory), &GrayRhinoEvidenceRecord>::new();
    for record in records {
        let subject = record.subject.trim();
        if subject.is_empty() {
            continue;
        }
        let subject = subject.to_ascii_uppercase();
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

fn has_amplifying_category(
    records: &[&GrayRhinoEvidenceRecord],
    category: GrayRhinoEvidenceCategory,
) -> bool {
    records.iter().any(|record| {
        record.category == category && record.risk_effect == GrayRhinoRiskEffect::Amplifying
    })
}

fn has_mitigating_category(
    records: &[&GrayRhinoEvidenceRecord],
    category: GrayRhinoEvidenceCategory,
) -> bool {
    records.iter().any(|record| {
        record.category == category && record.risk_effect == GrayRhinoRiskEffect::Mitigating
    })
}

fn has_dependency_without_same_subject_redundancy(records: &[&GrayRhinoEvidenceRecord]) -> bool {
    let dependency_subjects = subjects_with_effect(
        records,
        GrayRhinoEvidenceCategory::DependencyConcentration,
        GrayRhinoRiskEffect::Amplifying,
    );
    let redundancy_subjects = subjects_with_effect(
        records,
        GrayRhinoEvidenceCategory::Redundancy,
        GrayRhinoRiskEffect::Mitigating,
    );
    dependency_subjects
        .iter()
        .any(|subject| !redundancy_subjects.contains(subject))
}

fn subjects_with_effect(
    records: &[&GrayRhinoEvidenceRecord],
    category: GrayRhinoEvidenceCategory,
    effect: GrayRhinoRiskEffect,
) -> BTreeSet<String> {
    records
        .iter()
        .filter(|record| record.category == category && record.risk_effect == effect)
        .filter_map(|record| {
            let subject = record.subject.trim();
            if subject.is_empty() {
                None
            } else {
                Some(subject.to_ascii_uppercase())
            }
        })
        .collect()
}
