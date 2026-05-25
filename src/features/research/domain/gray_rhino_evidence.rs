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
}
