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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrayRhinoRiskEffect {
    Amplifying,
    Mitigating,
    Neutral,
    #[default]
    Unclassified,
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
    #[serde(default)]
    pub risk_effect: GrayRhinoRiskEffect,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstitutionalMaturityMetrics {
    pub succession_structure_disclosed: Option<bool>,
    pub external_audit_present: Option<bool>,
    pub disclosure_quality_score: Option<f64>,
    pub oversight_evolution_disclosed: Option<bool>,
    pub compliance_maturity_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstitutionalMaturityEvidence {
    pub subject: String,
    pub source: GrayRhinoSourceReference,
    pub confidence: f64,
    pub extraction_note: String,
    pub structural_fact: String,
    pub metrics: InstitutionalMaturityMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedundancyMetrics {
    pub fallback_available: Option<bool>,
    pub alternative_supplier_count: Option<u32>,
    pub redundancy_ratio: Option<f64>,
    pub recovery_path_disclosed: Option<bool>,
    pub failover_tested: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedundancyEvidence {
    pub subject: String,
    pub source: GrayRhinoSourceReference,
    pub confidence: f64,
    pub extraction_note: String,
    pub structural_fact: String,
    pub metrics: RedundancyMetrics,
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
    MissingInstitutionalMetric,
    InvalidInstitutionalMetric,
    MissingRedundancyMetric,
    InvalidRedundancyMetric,
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
            risk_effect: self.metrics.risk_effect(),
            extraction_note: self.extraction_note.clone(),
            structural_fact: self.structural_fact.clone(),
        }
    }
}

impl GovernanceConcentrationMetrics {
    fn risk_effect(&self) -> GrayRhinoRiskEffect {
        let amplifying = self.founder_voting_power.is_some_and(|value| value >= 50.0)
            || self.dual_class_structure == Some(true)
            || self.super_voting_rights == Some(true)
            || self.succession_disclosure == Some(false);
        let mitigating = self
            .independent_board_ratio
            .is_some_and(|ratio| ratio >= 0.5)
            || self.succession_disclosure == Some(true);
        match (amplifying, mitigating) {
            (true, false) => GrayRhinoRiskEffect::Amplifying,
            (false, true) => GrayRhinoRiskEffect::Mitigating,
            _ => GrayRhinoRiskEffect::Neutral,
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
            risk_effect: self.metrics.risk_effect(),
            extraction_note: self.extraction_note.clone(),
            structural_fact: self.structural_fact.clone(),
        }
    }
}

impl DependencyConcentrationMetrics {
    fn risk_effect(&self) -> GrayRhinoRiskEffect {
        let amplifying = self.concentration_ratio.is_some_and(|ratio| ratio >= 0.5)
            || self.single_point_of_failure == Some(true)
            || self.fallback_disclosed == Some(false);
        let mitigating = self.fallback_disclosed == Some(true);
        match (amplifying, mitigating) {
            (true, false) => GrayRhinoRiskEffect::Amplifying,
            (false, true) => GrayRhinoRiskEffect::Mitigating,
            _ => GrayRhinoRiskEffect::Neutral,
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

impl InstitutionalMaturityEvidence {
    pub fn validate(&self) -> Result<(), GrayRhinoEvidenceRejection> {
        if self.subject.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingStructuralFact);
        }
        if !matches!(
            self.source.source_type,
            GrayRhinoEvidenceSourceType::RegulatoryFiling
                | GrayRhinoEvidenceSourceType::GovernanceDocument
                | GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::IndependentAudit
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ) {
            return Err(GrayRhinoEvidenceRejection::UnsupportedSourceType);
        }
        if !self.metrics.has_any_metric() {
            return Err(GrayRhinoEvidenceRejection::MissingInstitutionalMetric);
        }
        if !self.metrics.is_valid() {
            return Err(GrayRhinoEvidenceRejection::InvalidInstitutionalMetric);
        }
        self.to_record().validate()
    }

    pub fn to_record(&self) -> GrayRhinoEvidenceRecord {
        GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::InstitutionalMaturity,
            source: self.source.clone(),
            confidence: self.confidence,
            risk_effect: self.metrics.risk_effect(),
            extraction_note: self.extraction_note.clone(),
            structural_fact: self.structural_fact.clone(),
        }
    }
}

impl InstitutionalMaturityMetrics {
    fn risk_effect(&self) -> GrayRhinoRiskEffect {
        let mitigating = self.succession_structure_disclosed == Some(true)
            || self.external_audit_present == Some(true)
            || self
                .disclosure_quality_score
                .is_some_and(|score| score >= 0.6)
            || self.oversight_evolution_disclosed == Some(true);
        let amplifying = self.succession_structure_disclosed == Some(false)
            || self.external_audit_present == Some(false)
            || self
                .disclosure_quality_score
                .is_some_and(|score| score < 0.4);
        match (amplifying, mitigating) {
            (true, false) => GrayRhinoRiskEffect::Amplifying,
            (false, true) => GrayRhinoRiskEffect::Mitigating,
            _ => GrayRhinoRiskEffect::Neutral,
        }
    }
}

impl InstitutionalMaturityMetrics {
    fn has_any_metric(&self) -> bool {
        self.succession_structure_disclosed.is_some()
            || self.external_audit_present.is_some()
            || self.disclosure_quality_score.is_some()
            || self.oversight_evolution_disclosed.is_some()
            || self.compliance_maturity_level.is_some()
    }

    fn is_valid(&self) -> bool {
        self.disclosure_quality_score
            .is_none_or(|value| (0.0..=1.0).contains(&value))
            && self
                .compliance_maturity_level
                .as_ref()
                .is_none_or(|value| !value.trim().is_empty())
    }
}

impl RedundancyEvidence {
    pub fn validate(&self) -> Result<(), GrayRhinoEvidenceRejection> {
        if self.subject.trim().is_empty() {
            return Err(GrayRhinoEvidenceRejection::MissingStructuralFact);
        }
        if !matches!(
            self.source.source_type,
            GrayRhinoEvidenceSourceType::InfrastructureStatus
                | GrayRhinoEvidenceSourceType::SupplierDisclosure
                | GrayRhinoEvidenceSourceType::IndependentAudit
                | GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ) {
            return Err(GrayRhinoEvidenceRejection::UnsupportedSourceType);
        }
        if !self.metrics.has_any_metric() {
            return Err(GrayRhinoEvidenceRejection::MissingRedundancyMetric);
        }
        if !self.metrics.is_valid() {
            return Err(GrayRhinoEvidenceRejection::InvalidRedundancyMetric);
        }
        self.to_record().validate()
    }

    pub fn to_record(&self) -> GrayRhinoEvidenceRecord {
        GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::Redundancy,
            source: self.source.clone(),
            confidence: self.confidence,
            risk_effect: self.metrics.risk_effect(),
            extraction_note: self.extraction_note.clone(),
            structural_fact: self.structural_fact.clone(),
        }
    }
}

impl RedundancyMetrics {
    fn risk_effect(&self) -> GrayRhinoRiskEffect {
        let mitigating = self.fallback_available == Some(true)
            || self
                .alternative_supplier_count
                .is_some_and(|count| count >= 2)
            || self.redundancy_ratio.is_some_and(|ratio| ratio >= 0.5)
            || self.recovery_path_disclosed == Some(true)
            || self.failover_tested == Some(true);
        let amplifying = self.fallback_available == Some(false)
            || self
                .alternative_supplier_count
                .is_some_and(|count| count == 0)
            || self.redundancy_ratio.is_some_and(|ratio| ratio < 0.25)
            || self.recovery_path_disclosed == Some(false)
            || self.failover_tested == Some(false);
        match (amplifying, mitigating) {
            (true, false) => GrayRhinoRiskEffect::Amplifying,
            (false, true) => GrayRhinoRiskEffect::Mitigating,
            _ => GrayRhinoRiskEffect::Neutral,
        }
    }
}

impl RedundancyMetrics {
    fn has_any_metric(&self) -> bool {
        self.fallback_available.is_some()
            || self.alternative_supplier_count.is_some()
            || self.redundancy_ratio.is_some()
            || self.recovery_path_disclosed.is_some()
            || self.failover_tested.is_some()
    }

    fn is_valid(&self) -> bool {
        self.redundancy_ratio
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
            risk_effect: GrayRhinoRiskEffect::Amplifying,
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
            risk_effect: GrayRhinoRiskEffect::Amplifying,
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
            risk_effect: GrayRhinoRiskEffect::Neutral,
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
            risk_effect: GrayRhinoRiskEffect::Neutral,
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
        assert_eq!(
            evidence.to_record().risk_effect,
            GrayRhinoRiskEffect::Amplifying
        );
    }

    #[test]
    fn gray_rhino_reliability_governance_independence_is_mitigating() {
        let evidence = GovernanceConcentrationEvidence {
            subject: "Example issuer".to_string(),
            source: source(),
            confidence: 0.85,
            extraction_note: "Proxy statement discloses independent directors.".to_string(),
            structural_fact: "Eleven of twelve directors are independent.".to_string(),
            metrics: GovernanceConcentrationMetrics {
                founder_voting_power: None,
                independent_board_ratio: Some(11.0 / 12.0),
                dual_class_structure: None,
                super_voting_rights: None,
                succession_disclosure: Some(true),
            },
        };

        assert_eq!(evidence.validate(), Ok(()));
        assert_eq!(
            evidence.to_record().risk_effect,
            GrayRhinoRiskEffect::Mitigating
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
        assert_eq!(
            evidence.to_record().risk_effect,
            GrayRhinoRiskEffect::Amplifying
        );
    }

    #[test]
    fn institutional_evidence_requires_at_least_one_metric() {
        let evidence = InstitutionalMaturityEvidence {
            subject: "Example issuer".to_string(),
            source: source(),
            confidence: 0.8,
            extraction_note: "Annual report discloses institutional maturity signals.".to_string(),
            structural_fact: "Oversight maturity evidence is disclosed.".to_string(),
            metrics: InstitutionalMaturityMetrics {
                succession_structure_disclosed: None,
                external_audit_present: None,
                disclosure_quality_score: None,
                oversight_evolution_disclosed: None,
                compliance_maturity_level: None,
            },
        };

        assert_eq!(
            evidence.validate(),
            Err(GrayRhinoEvidenceRejection::MissingInstitutionalMetric)
        );
    }

    #[test]
    fn institutional_evidence_projects_to_gray_rhino_record() {
        let evidence = InstitutionalMaturityEvidence {
            subject: "Example issuer".to_string(),
            source: source(),
            confidence: 0.83,
            extraction_note: "Annual report discloses governance maturity controls.".to_string(),
            structural_fact: "Institutional oversight maturity is supported by disclosures."
                .to_string(),
            metrics: InstitutionalMaturityMetrics {
                succession_structure_disclosed: Some(true),
                external_audit_present: Some(true),
                disclosure_quality_score: Some(0.72),
                oversight_evolution_disclosed: Some(true),
                compliance_maturity_level: Some("developing".to_string()),
            },
        };

        assert_eq!(evidence.validate(), Ok(()));
        assert_eq!(
            evidence.to_record().category,
            GrayRhinoEvidenceCategory::InstitutionalMaturity
        );
        assert_eq!(
            evidence.to_record().risk_effect,
            GrayRhinoRiskEffect::Mitigating
        );
    }

    #[test]
    fn redundancy_evidence_requires_at_least_one_metric() {
        let evidence = RedundancyEvidence {
            subject: "Example issuer".to_string(),
            source: dependency_source(),
            confidence: 0.8,
            extraction_note: "Supplier disclosure identifies redundancy controls.".to_string(),
            structural_fact: "Fallback availability is disclosed.".to_string(),
            metrics: RedundancyMetrics {
                fallback_available: None,
                alternative_supplier_count: None,
                redundancy_ratio: None,
                recovery_path_disclosed: None,
                failover_tested: None,
            },
        };

        assert_eq!(
            evidence.validate(),
            Err(GrayRhinoEvidenceRejection::MissingRedundancyMetric)
        );
    }

    #[test]
    fn redundancy_evidence_projects_to_gray_rhino_record() {
        let evidence = RedundancyEvidence {
            subject: "Example issuer".to_string(),
            source: dependency_source(),
            confidence: 0.84,
            extraction_note: "Supplier disclosure identifies redundancy controls.".to_string(),
            structural_fact: "Fallback availability is disclosed.".to_string(),
            metrics: RedundancyMetrics {
                fallback_available: Some(true),
                alternative_supplier_count: Some(2),
                redundancy_ratio: Some(0.5),
                recovery_path_disclosed: Some(true),
                failover_tested: Some(false),
            },
        };

        assert_eq!(evidence.validate(), Ok(()));
        assert_eq!(
            evidence.to_record().category,
            GrayRhinoEvidenceCategory::Redundancy
        );
        assert_eq!(
            evidence.to_record().risk_effect,
            GrayRhinoRiskEffect::Neutral
        );
    }
}
