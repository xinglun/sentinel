use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessment, GrayRhinoAssessmentSnapshot,
    GrayRhinoEscalationInput, GrayRhinoObservationSource,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceRecord, GrayRhinoEvidenceRejection,
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
}
