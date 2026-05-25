use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessment, GrayRhinoAssessmentSnapshot,
    GrayRhinoEscalationInput, GrayRhinoObservationSource, RiskLevel,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord, GrayRhinoEvidenceRejection,
    GrayRhinoRiskEffect,
};
use chrono::NaiveDate;

/// 灰色のサイ観測入力を、日次監査可能な snapshot へ変換する。
pub(crate) fn build_gray_rhino_assessment(
    input: GrayRhinoEscalationInput,
    as_of_date: NaiveDate,
    previous: Option<GrayRhinoAssessmentSnapshot>,
) -> GrayRhinoAssessment {
    GrayRhinoAssessment {
        current: GrayRhinoAssessmentSnapshot {
            schema_version: 1,
            as_of_date,
            source: GrayRhinoObservationSource::ManualConfiguration,
            escalation: evaluate_gray_rhino_escalation(input),
        },
        previous,
    }
}

/// evidence store の category coverage から Gray Rhino escalation input を構成する。
pub(crate) fn build_evidence_backed_gray_rhino_input(
    records: &[GrayRhinoEvidenceRecord],
) -> Option<GrayRhinoEscalationInput> {
    if records.is_empty() {
        return None;
    }
    let has_governance =
        has_amplifying_category(records, GrayRhinoEvidenceCategory::GovernanceConcentration);
    let has_dependency =
        has_amplifying_category(records, GrayRhinoEvidenceCategory::DependencyConcentration);
    let has_institutional_gap =
        has_amplifying_category(records, GrayRhinoEvidenceCategory::InstitutionalMaturity);
    let has_institutional_maturity =
        has_mitigating_category(records, GrayRhinoEvidenceCategory::InstitutionalMaturity);
    let has_redundancy = has_mitigating_category(records, GrayRhinoEvidenceCategory::Redundancy);
    let amplifying_count = [has_governance, has_dependency, has_institutional_gap]
        .into_iter()
        .filter(|ready| *ready)
        .count();
    let mitigating_count = [has_institutional_maturity, has_redundancy]
        .into_iter()
        .filter(|ready| *ready)
        .count();
    let average_confidence =
        records.iter().map(|record| record.confidence).sum::<f64>() / records.len() as f64;
    let classifiable_count = amplifying_count + mitigating_count;
    let unclassified_count = records
        .iter()
        .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Unclassified)
        .count();
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
        fallback_survivability_risk: if has_dependency && !has_redundancy && quality_ready {
            RiskLevel::Elevated
        } else {
            RiskLevel::Low
        },
        notes: vec![format!(
            "Evidence-backed Gray Rhino assessment from validated records; amplifying categories: {amplifying_count}/3; mitigating categories: {mitigating_count}/2; unclassified records: {unclassified_count}; average confidence: {average_confidence:.2}."
        )],
    })
}

fn has_amplifying_category(
    records: &[GrayRhinoEvidenceRecord],
    category: GrayRhinoEvidenceCategory,
) -> bool {
    records.iter().any(|record| {
        record.category == category && record.risk_effect == GrayRhinoRiskEffect::Amplifying
    })
}

fn has_mitigating_category(
    records: &[GrayRhinoEvidenceRecord],
    category: GrayRhinoEvidenceCategory,
) -> bool {
    records.iter().any(|record| {
        record.category == category && record.risk_effect == GrayRhinoRiskEffect::Mitigating
    })
}

pub(crate) fn build_evidence_backed_gray_rhino_assessment(
    records: &[GrayRhinoEvidenceRecord],
    as_of_date: NaiveDate,
    previous: Option<GrayRhinoAssessmentSnapshot>,
) -> Option<GrayRhinoAssessment> {
    let input = build_evidence_backed_gray_rhino_input(records)?;
    Some(GrayRhinoAssessment {
        current: GrayRhinoAssessmentSnapshot {
            schema_version: 1,
            as_of_date,
            source: GrayRhinoObservationSource::EvidenceStore,
            escalation: evaluate_gray_rhino_escalation(input),
        },
        previous,
    })
}

/// 将来の自動収集 adapter が満たすべき evidence ingestion 境界。
///
/// 現時点では escalation 判定へ接続せず、contract 違反の早期検出だけを行う。
#[allow(dead_code)]
pub(crate) fn validate_gray_rhino_evidence_contract(
    record: &GrayRhinoEvidenceRecord,
) -> Result<(), GrayRhinoEvidenceRejection> {
    record.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceCategory, GrayRhinoEvidenceSourceType, GrayRhinoRiskEffect,
        GrayRhinoSourceReference,
    };

    #[test]
    fn evidence_contract_validation_is_not_escalation_detection() {
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::Redundancy,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::IndependentAudit,
                source_title: "Redundancy audit".to_string(),
                publisher: "Example auditor".to_string(),
                source_url: Some("https://example.com/audit".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.9,
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            extraction_note: "Audit identifies fallback provider availability.".to_string(),
            structural_fact: "Fallback provider is documented for critical dependency.".to_string(),
        };

        assert_eq!(validate_gray_rhino_evidence_contract(&record), Ok(()));
    }

    #[test]
    fn evidence_records_build_evidence_backed_escalation_input() {
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "Dependency disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/dependency".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.86,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Supplier disclosure identifies dependency concentration.".to_string(),
            structural_fact: "Critical supplier dependency has no disclosed fallback.".to_string(),
        };

        let input = build_evidence_backed_gray_rhino_input(&[record]).unwrap();

        assert_eq!(input.dependency_centralization, RiskLevel::Elevated);
        assert_eq!(input.fallback_survivability_risk, RiskLevel::Low);
        assert!(input.notes[0].contains("amplifying categories: 1/3"));
    }

    #[test]
    fn gray_rhino_reliability_mitigating_governance_does_not_raise_risk() {
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Proxy disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/proxy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.88,
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            extraction_note: "Proxy statement reports eleven of twelve independent directors."
                .to_string(),
            structural_fact: "Board independence is strong and succession is disclosed."
                .to_string(),
        };

        let input = build_evidence_backed_gray_rhino_input(&[record]).unwrap();

        assert_eq!(input.risk_expansion_rate, RiskLevel::Moderate);
        assert_eq!(input.awareness_decay, RiskLevel::Elevated);
    }

    #[test]
    fn gray_rhino_completion_institutional_maturity_reduces_constraint_risk() {
        let mature = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::InstitutionalMaturity,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::IndependentAudit,
                source_title: "Institutional maturity disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/institutional".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.88,
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            extraction_note: "External audit and succession structure are disclosed.".to_string(),
            structural_fact: "Institutional oversight maturity is supported.".to_string(),
        };
        let gap = GrayRhinoEvidenceRecord {
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Succession structure and audit controls are missing.".to_string(),
            structural_fact: "Institutional constraints are weak.".to_string(),
            ..mature.clone()
        };

        let mature_input = build_evidence_backed_gray_rhino_input(&[mature]).unwrap();
        let gap_input = build_evidence_backed_gray_rhino_input(&[gap]).unwrap();

        assert_eq!(mature_input.constraint_growth_rate, RiskLevel::Elevated);
        assert_eq!(gap_input.constraint_growth_rate, RiskLevel::Low);
    }

    #[test]
    fn gray_rhino_completion_unclassified_evidence_is_reported_not_scored() {
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "Legacy dependency disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/legacy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.86,
            risk_effect: GrayRhinoRiskEffect::Unclassified,
            extraction_note: "Legacy record does not include risk effect.".to_string(),
            structural_fact: "Dependency is mentioned without directional classification."
                .to_string(),
        };

        let input = build_evidence_backed_gray_rhino_input(&[record]).unwrap();

        assert_eq!(input.dependency_centralization, RiskLevel::Moderate);
        assert!(input.notes[0].contains("unclassified records: 1"));
    }
}
