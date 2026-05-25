use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessment, GrayRhinoAssessmentSnapshot,
    GrayRhinoEscalationInput, GrayRhinoObservationSource, RiskLevel,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord, GrayRhinoEvidenceRejection,
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
    let has_governance = records
        .iter()
        .any(|record| record.category == GrayRhinoEvidenceCategory::GovernanceConcentration);
    let has_dependency = records
        .iter()
        .any(|record| record.category == GrayRhinoEvidenceCategory::DependencyConcentration);
    let has_institutional = records
        .iter()
        .any(|record| record.category == GrayRhinoEvidenceCategory::InstitutionalMaturity);
    let has_redundancy = records
        .iter()
        .any(|record| record.category == GrayRhinoEvidenceCategory::Redundancy);

    Some(GrayRhinoEscalationInput {
        risk_expansion_rate: if has_governance || has_dependency {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        constraint_growth_rate: if has_institutional {
            RiskLevel::Elevated
        } else {
            RiskLevel::Low
        },
        dependency_centralization: if has_dependency {
            RiskLevel::High
        } else {
            RiskLevel::Moderate
        },
        awareness_decay: RiskLevel::Moderate,
        narrative_overconfidence: RiskLevel::Moderate,
        single_point_fragility: if has_dependency {
            RiskLevel::Elevated
        } else {
            RiskLevel::Moderate
        },
        fallback_survivability_risk: if has_dependency && !has_redundancy {
            RiskLevel::Elevated
        } else {
            RiskLevel::Low
        },
        notes: vec!["Evidence-backed Gray Rhino assessment from validated records.".to_string()],
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
        GrayRhinoEvidenceCategory, GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
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
            extraction_note: "Supplier disclosure identifies dependency concentration.".to_string(),
            structural_fact: "Critical supplier dependency has no disclosed fallback.".to_string(),
        };

        let input = build_evidence_backed_gray_rhino_input(&[record]).unwrap();

        assert_eq!(input.dependency_centralization, RiskLevel::High);
        assert_eq!(input.fallback_survivability_risk, RiskLevel::Elevated);
    }
}
