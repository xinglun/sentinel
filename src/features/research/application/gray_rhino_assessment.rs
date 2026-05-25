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
    let has_institutional =
        has_amplifying_category(records, GrayRhinoEvidenceCategory::InstitutionalMaturity);
    let has_redundancy = has_mitigating_category(records, GrayRhinoEvidenceCategory::Redundancy);
    let ready_count = [
        has_governance,
        has_dependency,
        has_institutional,
        has_redundancy,
    ]
    .into_iter()
    .filter(|ready| *ready)
    .count();
    let average_confidence =
        records.iter().map(|record| record.confidence).sum::<f64>() / records.len() as f64;
    let quality_ready = ready_count >= 2 && average_confidence >= 0.6;

    Some(GrayRhinoEscalationInput {
        risk_expansion_rate: if has_governance && has_dependency && quality_ready {
            RiskLevel::High
        } else if has_governance || has_dependency {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        constraint_growth_rate: if has_institutional {
            RiskLevel::Elevated
        } else {
            RiskLevel::Low
        },
        dependency_centralization: if has_dependency && quality_ready {
            RiskLevel::High
        } else if has_dependency {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        awareness_decay: if ready_count <= 1 {
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
            "Evidence-backed Gray Rhino assessment from validated records; amplifying categories: {ready_count}/4; average confidence: {average_confidence:.2}."
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
        assert!(input.notes[0].contains("amplifying categories: 1/4"));
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
}
