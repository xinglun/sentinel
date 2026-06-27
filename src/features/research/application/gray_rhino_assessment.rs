use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessment, GrayRhinoAssessmentSnapshot,
    GrayRhinoEscalationInput, GrayRhinoObservationSource,
};
use crate::features::research::domain::gray_rhino_assessment_policy;
use crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord;
#[cfg(test)]
use crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRejection;
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
    gray_rhino_assessment_policy::build_evidence_backed_gray_rhino_input(records)
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
#[cfg(test)]
pub(crate) fn validate_gray_rhino_evidence_contract(
    record: &GrayRhinoEvidenceRecord,
) -> Result<(), GrayRhinoEvidenceRejection> {
    record.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino::RiskLevel;
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceCategory, GrayRhinoEvidenceSourceType, GrayRhinoRiskEffect,
        GrayRhinoSourceReference,
    };

    #[test]
    fn evidence_contract_validation_is_not_escalation_detection() {
        let record = GrayRhinoEvidenceRecord {
            subject: "Example issuer".to_string(),
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
            subject: "Example issuer".to_string(),
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
    fn gray_rhino_scoring_unclassified_confidence_does_not_raise_formal_score() {
        let governance = GrayRhinoEvidenceRecord {
            subject: "Example issuer".to_string(),
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Low confidence governance disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/governance".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.4,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Governance concentration is partially disclosed.".to_string(),
            structural_fact: "Founder voting control is present.".to_string(),
        };
        let dependency = GrayRhinoEvidenceRecord {
            subject: "Example issuer".to_string(),
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "Low confidence dependency disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/dependency".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.4,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Dependency concentration is partially disclosed.".to_string(),
            structural_fact: "Single supplier dependency is present.".to_string(),
        };
        let legacy = GrayRhinoEvidenceRecord {
            subject: "Example issuer".to_string(),
            category: GrayRhinoEvidenceCategory::Redundancy,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::IndependentAudit,
                source_title: "Legacy high confidence record".to_string(),
                publisher: "Example auditor".to_string(),
                source_url: Some("https://example.com/legacy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 1.0,
            risk_effect: GrayRhinoRiskEffect::Unclassified,
            extraction_note: "Legacy record lacks risk_effect.".to_string(),
            structural_fact: "Legacy record should be reported but not scored.".to_string(),
        };

        let input =
            build_evidence_backed_gray_rhino_input(&[governance, dependency, legacy]).unwrap();

        assert_eq!(input.risk_expansion_rate, RiskLevel::Elevated);
        assert_eq!(input.dependency_centralization, RiskLevel::Elevated);
        assert_eq!(input.fallback_survivability_risk, RiskLevel::Low);
        assert!(input.notes[0].contains("unclassified records: 1"));
        assert!(input.notes[0].contains("scoreable average confidence: 0.40"));
    }

    #[test]
    fn gray_rhino_reliability_mitigating_governance_does_not_raise_risk() {
        let record = GrayRhinoEvidenceRecord {
            subject: "Example issuer".to_string(),
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

        assert!(build_evidence_backed_gray_rhino_input(&[record]).is_none());
    }

    #[test]
    fn gray_rhino_mitigating_evidence_closes_old_amplifying() {
        let old_amplifying = GrayRhinoEvidenceRecord {
            subject: "GOOG".to_string(),
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Old proxy disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/old-proxy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            },
            confidence: 0.9,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Founder control was disclosed.".to_string(),
            structural_fact: "Founder voting control exceeded majority.".to_string(),
        };
        let newer_mitigating = GrayRhinoEvidenceRecord {
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            source: GrayRhinoSourceReference {
                source_title: "New governance repair disclosure".to_string(),
                source_url: Some("https://example.com/new-proxy".to_string()),
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                ..old_amplifying.source.clone()
            },
            extraction_note: "Voting control remediation is disclosed.".to_string(),
            structural_fact: "Dual-class voting control has been collapsed.".to_string(),
            ..old_amplifying.clone()
        };

        assert!(
            build_evidence_backed_gray_rhino_input(&[old_amplifying, newer_mitigating]).is_none()
        );
    }

    #[test]
    fn gray_rhino_subject_mitigation_does_not_cross_close() {
        let goog_amplifying = GrayRhinoEvidenceRecord {
            subject: "GOOG".to_string(),
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "GOOG proxy disclosure".to_string(),
                publisher: "GOOG".to_string(),
                source_url: Some("https://example.com/goog-proxy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            },
            confidence: 0.9,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Founder control was disclosed for GOOG.".to_string(),
            structural_fact: "GOOG founder voting control exceeded majority.".to_string(),
        };
        let tsla_mitigating = GrayRhinoEvidenceRecord {
            subject: "TSLA".to_string(),
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            source: GrayRhinoSourceReference {
                source_title: "TSLA governance repair disclosure".to_string(),
                publisher: "TSLA".to_string(),
                source_url: Some("https://example.com/tsla-proxy".to_string()),
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                ..goog_amplifying.source.clone()
            },
            extraction_note: "Voting control remediation is disclosed for TSLA.".to_string(),
            structural_fact: "TSLA dual-class voting control has been collapsed.".to_string(),
            ..goog_amplifying.clone()
        };

        let input =
            build_evidence_backed_gray_rhino_input(&[goog_amplifying, tsla_mitigating]).unwrap();

        assert_eq!(input.risk_expansion_rate, RiskLevel::Elevated);
        assert!(input.notes[0].contains("amplifying categories: 1/3"));
    }

    #[test]
    fn gray_rhino_redundancy_mitigation_does_not_cross_subject_dependency() {
        let governance = GrayRhinoEvidenceRecord {
            subject: "GOOG".to_string(),
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "GOOG governance disclosure".to_string(),
                publisher: "GOOG".to_string(),
                source_url: Some("https://example.com/goog-governance".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.9,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "GOOG governance control is concentrated.".to_string(),
            structural_fact: "GOOG founder voting control remains concentrated.".to_string(),
        };
        let dependency = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "GOOG dependency disclosure".to_string(),
                source_url: Some("https://example.com/goog-dependency".to_string()),
                ..governance.source.clone()
            },
            extraction_note: "GOOG critical supplier dependency is disclosed.".to_string(),
            structural_fact: "GOOG has no disclosed fallback for a critical supplier.".to_string(),
            ..governance.clone()
        };
        let tsla_redundancy = GrayRhinoEvidenceRecord {
            subject: "TSLA".to_string(),
            category: GrayRhinoEvidenceCategory::Redundancy,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::IndependentAudit,
                source_title: "TSLA redundancy audit".to_string(),
                publisher: "TSLA".to_string(),
                source_url: Some("https://example.com/tsla-redundancy".to_string()),
                ..governance.source.clone()
            },
            risk_effect: GrayRhinoRiskEffect::Mitigating,
            extraction_note: "TSLA redundancy is independently audited.".to_string(),
            structural_fact: "TSLA has tested failover for its critical supplier.".to_string(),
            ..governance.clone()
        };

        let input =
            build_evidence_backed_gray_rhino_input(&[governance, dependency, tsla_redundancy])
                .unwrap();

        assert_eq!(input.fallback_survivability_risk, RiskLevel::Elevated);
    }

    #[test]
    fn gray_rhino_legacy_subjectless_evidence_is_not_scoreable() {
        let record = GrayRhinoEvidenceRecord {
            subject: String::new(),
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "Legacy dependency disclosure".to_string(),
                publisher: "Legacy issuer".to_string(),
                source_url: Some("https://example.com/legacy-dependency".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.95,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Legacy record predates subject preservation.".to_string(),
            structural_fact: "Legacy dependency risk should remain display-only.".to_string(),
        };

        assert!(build_evidence_backed_gray_rhino_input(&[record]).is_none());
    }

    #[test]
    fn gray_rhino_assessment_note_reports_category_coverage() {
        let record = GrayRhinoEvidenceRecord {
            subject: "GOOG".to_string(),
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "Dependency disclosure".to_string(),
                publisher: "GOOG".to_string(),
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

        assert!(input.notes[0].contains("amplifying categories:"));
        assert!(!input.notes[0].contains("subject-categories"));
    }

    #[test]
    fn gray_rhino_completion_institutional_maturity_reduces_constraint_risk() {
        let mature = GrayRhinoEvidenceRecord {
            subject: "Example issuer".to_string(),
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
            subject: "Example issuer".to_string(),
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

        assert!(build_evidence_backed_gray_rhino_input(&[record]).is_none());
    }

    #[test]
    fn gray_rhino_final_neutral_only_evidence_does_not_build_assessment() {
        let record = GrayRhinoEvidenceRecord {
            subject: "Example issuer".to_string(),
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
                source_title: "Neutral dependency disclosure".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/neutral".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.86,
            risk_effect: GrayRhinoRiskEffect::Neutral,
            extraction_note: "Dependency is disclosed without direction.".to_string(),
            structural_fact: "Dependency exists but no directional risk effect is classified."
                .to_string(),
        };

        assert!(build_evidence_backed_gray_rhino_input(&[record]).is_none());
    }

    #[test]
    fn manual_assessment_marks_manual_configuration_source() {
        let input = GrayRhinoEscalationInput {
            risk_expansion_rate: RiskLevel::Low,
            constraint_growth_rate: RiskLevel::Low,
            dependency_centralization: RiskLevel::Low,
            awareness_decay: RiskLevel::Low,
            narrative_overconfidence: RiskLevel::Low,
            single_point_fragility: RiskLevel::Low,
            fallback_survivability_risk: RiskLevel::Low,
            notes: vec!["all clear".to_string()],
        };

        let assessment =
            build_gray_rhino_assessment(input, NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(), None);

        assert_eq!(
            assessment.current.source,
            GrayRhinoObservationSource::ManualConfiguration
        );
        assert_eq!(assessment.current.schema_version, 1);
        assert_eq!(
            assessment.current.escalation.notes,
            vec!["all clear".to_string()]
        );
    }
}
