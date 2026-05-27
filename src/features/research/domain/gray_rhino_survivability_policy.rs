use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord, GrayRhinoRiskEffect,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurvivabilityLevel {
    Extreme,
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DependencyRiskLevel {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SurvivabilityDimension {
    pub level: SurvivabilityLevel,
    pub mitigating_count: usize,
    pub amplifying_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyRiskDimension {
    pub level: DependencyRiskLevel,
    pub mitigating_count: usize,
    pub amplifying_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrayRhinoSurvivabilitySummary {
    pub capital_access: SurvivabilityLevel,
    pub compute_control: SurvivabilityDimension,
    pub governance_resilience: SurvivabilityDimension,
    pub dependency_risk: DependencyRiskDimension,
    pub retry_capacity: SurvivabilityDimension,
}

pub(crate) fn build_survivability_summary(
    records: &[GrayRhinoEvidenceRecord],
) -> GrayRhinoSurvivabilitySummary {
    let dependency = count_by_category(records, GrayRhinoEvidenceCategory::DependencyConcentration);
    let institutional =
        count_by_category(records, GrayRhinoEvidenceCategory::InstitutionalMaturity);
    let redundancy = count_by_category(records, GrayRhinoEvidenceCategory::Redundancy);

    GrayRhinoSurvivabilitySummary {
        capital_access: SurvivabilityLevel::Unknown,
        compute_control: SurvivabilityDimension {
            level: resilience_level(dependency.mitigating_count, dependency.amplifying_count),
            mitigating_count: dependency.mitigating_count,
            amplifying_count: dependency.amplifying_count,
        },
        governance_resilience: SurvivabilityDimension {
            level: resilience_level(
                institutional.mitigating_count,
                institutional.amplifying_count,
            ),
            mitigating_count: institutional.mitigating_count,
            amplifying_count: institutional.amplifying_count,
        },
        dependency_risk: DependencyRiskDimension {
            level: dependency_risk_level(dependency.mitigating_count, dependency.amplifying_count),
            mitigating_count: dependency.mitigating_count,
            amplifying_count: dependency.amplifying_count,
        },
        retry_capacity: SurvivabilityDimension {
            level: retry_capacity_level(redundancy.mitigating_count, redundancy.amplifying_count),
            mitigating_count: redundancy.mitigating_count,
            amplifying_count: redundancy.amplifying_count,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    mitigating_count: usize,
    amplifying_count: usize,
}

fn count_by_category(
    records: &[GrayRhinoEvidenceRecord],
    category: GrayRhinoEvidenceCategory,
) -> EvidenceCounts {
    let mitigating_count = records
        .iter()
        .filter(|record| record.category == category)
        .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Mitigating)
        .count();
    let amplifying_count = records
        .iter()
        .filter(|record| record.category == category)
        .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Amplifying)
        .count();

    EvidenceCounts {
        mitigating_count,
        amplifying_count,
    }
}

fn resilience_level(mitigating_count: usize, amplifying_count: usize) -> SurvivabilityLevel {
    match (mitigating_count, amplifying_count) {
        (0, 0) => SurvivabilityLevel::Unknown,
        (mitigating, amplifying) if mitigating >= amplifying + 2 => SurvivabilityLevel::High,
        (mitigating, amplifying) if mitigating > amplifying => SurvivabilityLevel::Medium,
        (mitigating, amplifying) if mitigating == amplifying => SurvivabilityLevel::Medium,
        _ => SurvivabilityLevel::Low,
    }
}

fn retry_capacity_level(mitigating_count: usize, amplifying_count: usize) -> SurvivabilityLevel {
    match (mitigating_count, amplifying_count) {
        (0, 0) => SurvivabilityLevel::Unknown,
        (mitigating, 0) if mitigating >= 2 => SurvivabilityLevel::Extreme,
        (mitigating, amplifying) if mitigating >= amplifying + 2 => SurvivabilityLevel::High,
        (mitigating, amplifying) if mitigating >= amplifying => SurvivabilityLevel::Medium,
        _ => SurvivabilityLevel::Low,
    }
}

fn dependency_risk_level(mitigating_count: usize, amplifying_count: usize) -> DependencyRiskLevel {
    match (mitigating_count, amplifying_count) {
        (0, 0) => DependencyRiskLevel::Unknown,
        (mitigating, amplifying) if amplifying >= mitigating + 2 => DependencyRiskLevel::High,
        (mitigating, amplifying) if amplifying > mitigating => DependencyRiskLevel::Medium,
        (mitigating, amplifying) if mitigating == amplifying => DependencyRiskLevel::Medium,
        _ => DependencyRiskLevel::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
    };
    use chrono::NaiveDate;

    fn record(
        category: GrayRhinoEvidenceCategory,
        risk_effect: GrayRhinoRiskEffect,
    ) -> GrayRhinoEvidenceRecord {
        let observed_at = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        GrayRhinoEvidenceRecord {
            subject: "MSFT".to_string(),
            category,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::OperatorCuratedSource,
                source_title: "source".to_string(),
                publisher: "operator".to_string(),
                source_url: Some("https://example.com".to_string()),
                repository_path: None,
                observed_at,
                retrieved_at: observed_at,
            },
            confidence: 0.8,
            risk_effect,
            extraction_note: "note".to_string(),
            structural_fact: "fact".to_string(),
        }
    }

    #[test]
    fn survivability_keeps_capital_access_unknown_without_dedicated_source() {
        let summary = build_survivability_summary(&[]);

        assert_eq!(summary.capital_access, SurvivabilityLevel::Unknown);
        assert_eq!(summary.compute_control.level, SurvivabilityLevel::Unknown);
        assert_eq!(summary.dependency_risk.level, DependencyRiskLevel::Unknown);
    }

    #[test]
    fn survivability_derives_dependency_and_compute_from_existing_evidence() {
        let records = vec![
            record(
                GrayRhinoEvidenceCategory::DependencyConcentration,
                GrayRhinoRiskEffect::Amplifying,
            ),
            record(
                GrayRhinoEvidenceCategory::DependencyConcentration,
                GrayRhinoRiskEffect::Amplifying,
            ),
            record(
                GrayRhinoEvidenceCategory::DependencyConcentration,
                GrayRhinoRiskEffect::Mitigating,
            ),
        ];

        let summary = build_survivability_summary(&records);

        assert_eq!(summary.compute_control.level, SurvivabilityLevel::Low);
        assert_eq!(summary.dependency_risk.level, DependencyRiskLevel::Medium);
    }

    #[test]
    fn retry_capacity_is_extreme_when_redundancy_mitigation_is_repeated() {
        let records = vec![
            record(
                GrayRhinoEvidenceCategory::Redundancy,
                GrayRhinoRiskEffect::Mitigating,
            ),
            record(
                GrayRhinoEvidenceCategory::Redundancy,
                GrayRhinoRiskEffect::Mitigating,
            ),
        ];

        let summary = build_survivability_summary(&records);

        assert_eq!(summary.retry_capacity.level, SurvivabilityLevel::Extreme);
        assert_eq!(summary.retry_capacity.mitigating_count, 2);
    }
}
