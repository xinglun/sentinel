use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrayRhinoEvidenceCategory {
    GovernanceConcentration,
    DependencyConcentration,
    InstitutionalMaturity,
    RiskNormalization,
    Redundancy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrayRhinoEvidenceSourceType {
    RegulatoryFiling,
    GovernanceDocument,
    CompanyDisclosure,
    IndependentAudit,
    InfrastructureStatus,
    SupplierDisclosure,
    MarketNarrativeCorpus,
    OperatorCuratedSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrayRhinoSourceReference {
    pub source_type: GrayRhinoEvidenceSourceType,
    pub source_title: String,
    pub publisher: String,
    pub source_url: Option<String>,
    pub repository_path: Option<String>,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrayRhinoEvidenceRecord {
    pub category: GrayRhinoEvidenceCategory,
    pub source: GrayRhinoSourceReference,
    pub confidence: f64,
    pub extraction_note: String,
    pub structural_fact: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GovernanceConcentrationMetrics {
    pub founder_voting_power: Option<f64>,
    pub independent_board_ratio: Option<f64>,
    pub dual_class_structure: Option<bool>,
    pub super_voting_rights: Option<bool>,
    pub succession_disclosure: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GovernanceConcentrationEvidence {
    pub subject: String,
    pub source: GrayRhinoSourceReference,
    pub confidence: f64,
    pub extraction_note: String,
    pub structural_fact: String,
    pub metrics: GovernanceConcentrationMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyConcentrationKind {
    Infrastructure,
    Compute,
    Cloud,
    Launch,
    Supplier,
    Ecosystem,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyConcentrationMetrics {
    pub dependency_kind: DependencyConcentrationKind,
    pub dependency_name: String,
    pub concentration_ratio: Option<f64>,
    pub single_point_of_failure: Option<bool>,
    pub fallback_disclosed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyConcentrationEvidence {
    pub subject: String,
    pub source: GrayRhinoSourceReference,
    pub confidence: f64,
    pub extraction_note: String,
    pub structural_fact: String,
    pub metrics: DependencyConcentrationMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrayRhinoEvidenceRejection {
    MissingSourceReference,
    MissingSourceTitle,
    MissingPublisher,
    MissingExtractionNote,
    MissingStructuralFact,
    ConfidenceOutOfRange,
    NarrativeOnly,
    ForbiddenBoundaryTerm,
    UnsupportedSourceType,
    MissingGovernanceMetric,
    InvalidGovernanceMetric,
    MissingDependencyMetric,
    InvalidDependencyMetric,
}

impl GrayRhinoEvidenceRecord {
    pub fn validate(&self) -> Result<(), GrayRhinoEvidenceRejection> {
        if self.source.source_url.is_none() && self.source.repository_path.is_none() {
            return Err(GrayRhinoEvidenceRejection::MissingSourceReference);
        }
        if self.source.source_title.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingSourceTitle);
        }
        if self.source.publisher.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingPublisher);
        }
        if self.extraction_note.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingExtractionNote);
        }
        if self.structural_fact.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingStructuralFact);
        }
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(GrayRhinoEvidenceRejection::ConfidenceOutOfRange);
        }
        if is_narrative_only(&self.structural_fact) || is_narrative_only(&self.extraction_note) {
            return Err(GrayRhinoEvidenceRejection::NarrativeOnly);
        }
        if contains_forbidden_boundary_term(&self.structural_fact)
            || contains_forbidden_boundary_term(&self.extraction_note)
        {
            return Err(GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm);
        }
        Ok(())
    }
}

impl GovernanceConcentrationEvidence {
    pub fn validate(&self) -> Result<(), GrayRhinoEvidenceRejection> {
        if self.subject.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingStructuralFact);
        }
        if !matches!(
            self.source.source_type,
            GrayRhinoEvidenceSourceType::RegulatoryFiling
                | GrayRhinoEvidenceSourceType::GovernanceDocument
                | GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ) {
            return Err(GrayRhinoEvidenceRejection::UnsupportedSourceType);
        }
        if !self.metrics.has_any_metric() {
            return Err(GrayRhinoEvidenceRejection::MissingGovernanceMetric);
        }
        if !self.metrics.is_valid() {
            return Err(GrayRhinoEvidenceRejection::InvalidGovernanceMetric);
        }
        self.to_record().validate()
    }

    pub fn to_record(&self) -> GrayRhinoEvidenceRecord {
        GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: self.source.clone(),
            confidence: self.confidence,
            extraction_note: self.extraction_note.clone(),
            structural_fact: self.structural_fact.clone(),
        }
    }
}

impl GovernanceConcentrationMetrics {
    fn has_any_metric(&self) -> bool {
        self.founder_voting_power.is_some()
            || self.independent_board_ratio.is_some()
            || self.dual_class_structure.is_some()
            || self.super_voting_rights.is_some()
            || self.succession_disclosure.is_some()
    }

    fn is_valid(&self) -> bool {
        self.founder_voting_power
            .is_none_or(|value| (0.0..=100.0).contains(&value))
            && self
                .independent_board_ratio
                .is_none_or(|value| (0.0..=1.0).contains(&value))
    }
}

impl DependencyConcentrationEvidence {
    pub fn validate(&self) -> Result<(), GrayRhinoEvidenceRejection> {
        if self.subject.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingStructuralFact);
        }
        if !matches!(
            self.source.source_type,
            GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::InfrastructureStatus
                | GrayRhinoEvidenceSourceType::SupplierDisclosure
                | GrayRhinoEvidenceSourceType::IndependentAudit
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ) {
            return Err(GrayRhinoEvidenceRejection::UnsupportedSourceType);
        }
        if !self.metrics.has_any_metric() {
            return Err(GrayRhinoEvidenceRejection::MissingDependencyMetric);
        }
        if !self.metrics.is_valid() {
            return Err(GrayRhinoEvidenceRejection::InvalidDependencyMetric);
        }
        self.to_record().validate()
    }

    pub fn to_record(&self) -> GrayRhinoEvidenceRecord {
        GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source: self.source.clone(),
            confidence: self.confidence,
            extraction_note: self.extraction_note.clone(),
            structural_fact: self.structural_fact.clone(),
        }
    }
}

impl DependencyConcentrationMetrics {
    fn has_any_metric(&self) -> bool {
        self.concentration_ratio.is_some()
            || self.single_point_of_failure.is_some()
            || self.fallback_disclosed.is_some()
    }

    fn is_valid(&self) -> bool {
        !self.dependency_name.trim().is_empty()
            && self
                .concentration_ratio
                .is_none_or(|value| (0.0..=1.0).contains(&value))
    }
}

fn is_narrative_only(value: &str) -> bool {
    let lower = value.to_lowercase();
    narrative_only_terms()
        .iter()
        .any(|term| lower.contains(term))
}

fn contains_forbidden_boundary_term(value: &str) -> bool {
    let lower = value.to_lowercase();
    forbidden_boundary_terms()
        .iter()
        .any(|term| lower.contains(term))
}

fn narrative_only_terms() -> &'static [&'static str] {
    &[
        "too successful to fail",
        "feels dangerous",
        "looks dangerous",
        "probably doomed",
        "危険そう",
        "成功しすぎている",
        "看起来危险",
        "太成功所以不会失败",
    ]
}

fn forbidden_boundary_terms() -> &'static [&'static str] {
    &[
        "buy",
        "sell",
        "gate",
        "execution",
        "trend_cohesion",
        "買入",
        "売却",
        "买入",
        "卖出",
        "人格",
        "政治",
        "陰謀",
        "阴谋",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> GrayRhinoSourceReference {
        GrayRhinoSourceReference {
            source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
            source_title: "Board governance disclosure".to_string(),
            publisher: "Example issuer".to_string(),
            source_url: Some("https://example.com/governance".to_string()),
            repository_path: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
        }
    }

    #[test]
    fn accepts_structural_evidence_with_traceable_source() {
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: source(),
            confidence: 0.8,
            extraction_note: "Voting control is disclosed in governance document.".to_string(),
            structural_fact: "Founder voting control exceeds ordinary common-share voting power."
                .to_string(),
        };

        assert_eq!(record.validate(), Ok(()));
    }

    #[test]
    fn rejects_record_without_source_reference() {
        let mut source = source();
        source.source_url = None;
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            source,
            confidence: 0.7,
            extraction_note: "Supplier disclosure identifies single-source dependency.".to_string(),
            structural_fact: "A critical supplier dependency has no disclosed fallback."
                .to_string(),
        };

        assert_eq!(
            record.validate(),
            Err(GrayRhinoEvidenceRejection::MissingSourceReference)
        );
    }

    #[test]
    fn rejects_narrative_only_record() {
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::RiskNormalization,
            source: source(),
            confidence: 0.6,
            extraction_note: "too successful to fail narrative".to_string(),
            structural_fact: "too successful to fail".to_string(),
        };

        assert_eq!(
            record.validate(),
            Err(GrayRhinoEvidenceRejection::NarrativeOnly)
        );
    }

    #[test]
    fn rejects_trading_boundary_terms() {
        let record = GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::RiskNormalization,
            source: source(),
            confidence: 0.6,
            extraction_note: "This should not become a sell signal.".to_string(),
            structural_fact: "Governance risk is connected to sell decision.".to_string(),
        };

        assert_eq!(
            record.validate(),
            Err(GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm)
        );
    }

    #[test]
    fn governance_evidence_requires_at_least_one_metric() {
        let evidence = GovernanceConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source: source(),
            confidence: 0.8,
            extraction_note: "Proxy statement discloses voting structure.".to_string(),
            structural_fact: "Founder voting control is disclosed.".to_string(),
            metrics: GovernanceConcentrationMetrics {
                founder_voting_power: None,
                independent_board_ratio: None,
                dual_class_structure: None,
                super_voting_rights: None,
                succession_disclosure: None,
            },
        };

        assert_eq!(
            evidence.validate(),
            Err(GrayRhinoEvidenceRejection::MissingGovernanceMetric)
        );
    }

    #[test]
    fn governance_evidence_projects_to_gray_rhino_record() {
        let evidence = GovernanceConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source: source(),
            confidence: 0.85,
            extraction_note: "Proxy statement discloses dual class shares.".to_string(),
            structural_fact: "Dual class structure grants unequal voting rights.".to_string(),
            metrics: GovernanceConcentrationMetrics {
                founder_voting_power: Some(61.2),
                independent_board_ratio: Some(0.42),
                dual_class_structure: Some(true),
                super_voting_rights: Some(true),
                succession_disclosure: Some(false),
            },
        };

        assert_eq!(evidence.validate(), Ok(()));
        assert_eq!(
            evidence.to_record().category,
            GrayRhinoEvidenceCategory::GovernanceConcentration
        );
    }

    fn dependency_source() -> GrayRhinoSourceReference {
        GrayRhinoSourceReference {
            source_type: GrayRhinoEvidenceSourceType::SupplierDisclosure,
            source_title: "Supplier dependency disclosure".to_string(),
            publisher: "Example issuer".to_string(),
            source_url: Some("https://example.com/dependency".to_string()),
            repository_path: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
        }
    }

    #[test]
    fn dependency_evidence_requires_at_least_one_dependency_metric() {
        let evidence = DependencyConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source: dependency_source(),
            confidence: 0.8,
            extraction_note: "Supplier disclosure identifies dependency concentration.".to_string(),
            structural_fact: "Critical supplier dependency has no disclosed fallback.".to_string(),
            metrics: DependencyConcentrationMetrics {
                dependency_kind: DependencyConcentrationKind::Supplier,
                dependency_name: "Example supplier".to_string(),
                concentration_ratio: None,
                single_point_of_failure: None,
                fallback_disclosed: None,
            },
        };

        assert_eq!(
            evidence.validate(),
            Err(GrayRhinoEvidenceRejection::MissingDependencyMetric)
        );
    }

    #[test]
    fn dependency_evidence_rejects_unsupported_source_type() {
        let mut source = dependency_source();
        source.source_type = GrayRhinoEvidenceSourceType::MarketNarrativeCorpus;
        let evidence = DependencyConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source,
            confidence: 0.8,
            extraction_note: "Supplier disclosure identifies dependency concentration.".to_string(),
            structural_fact: "Critical supplier dependency has no disclosed fallback.".to_string(),
            metrics: DependencyConcentrationMetrics {
                dependency_kind: DependencyConcentrationKind::Supplier,
                dependency_name: "Example supplier".to_string(),
                concentration_ratio: None,
                single_point_of_failure: Some(true),
                fallback_disclosed: Some(false),
            },
        };

        assert_eq!(
            evidence.validate(),
            Err(GrayRhinoEvidenceRejection::UnsupportedSourceType)
        );
    }

    #[test]
    fn dependency_evidence_projects_to_gray_rhino_record() {
        let evidence = DependencyConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source: dependency_source(),
            confidence: 0.82,
            extraction_note: "Supplier disclosure identifies dependency concentration.".to_string(),
            structural_fact: "Critical supplier dependency has no disclosed fallback.".to_string(),
            metrics: DependencyConcentrationMetrics {
                dependency_kind: DependencyConcentrationKind::Supplier,
                dependency_name: "Example supplier".to_string(),
                concentration_ratio: Some(0.74),
                single_point_of_failure: Some(true),
                fallback_disclosed: Some(false),
            },
        };

        assert_eq!(evidence.validate(), Ok(()));
        assert_eq!(
            evidence.to_record().category,
            GrayRhinoEvidenceCategory::DependencyConcentration
        );
    }
}
