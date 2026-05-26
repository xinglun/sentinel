use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRejection, GrayRhinoEvidenceSourceType,
};

/// evidence category ごとの許可 source type を判定する domain policy。
pub(crate) fn validate_source_type_for_category(
    category: GrayRhinoEvidenceCategory,
    source_type: GrayRhinoEvidenceSourceType,
) -> Result<(), GrayRhinoEvidenceRejection> {
    if source_type_allowed_for_category(category, source_type) {
        Ok(())
    } else {
        Err(GrayRhinoEvidenceRejection::UnsupportedSourceType)
    }
}

fn source_type_allowed_for_category(
    category: GrayRhinoEvidenceCategory,
    source_type: GrayRhinoEvidenceSourceType,
) -> bool {
    match category {
        GrayRhinoEvidenceCategory::GovernanceConcentration => matches!(
            source_type,
            GrayRhinoEvidenceSourceType::RegulatoryFiling
                | GrayRhinoEvidenceSourceType::GovernanceDocument
                | GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ),
        GrayRhinoEvidenceCategory::DependencyConcentration => matches!(
            source_type,
            GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::InfrastructureStatus
                | GrayRhinoEvidenceSourceType::SupplierDisclosure
                | GrayRhinoEvidenceSourceType::IndependentAudit
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ),
        GrayRhinoEvidenceCategory::InstitutionalMaturity => matches!(
            source_type,
            GrayRhinoEvidenceSourceType::RegulatoryFiling
                | GrayRhinoEvidenceSourceType::GovernanceDocument
                | GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::IndependentAudit
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ),
        GrayRhinoEvidenceCategory::Redundancy => matches!(
            source_type,
            GrayRhinoEvidenceSourceType::InfrastructureStatus
                | GrayRhinoEvidenceSourceType::SupplierDisclosure
                | GrayRhinoEvidenceSourceType::IndependentAudit
                | GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ),
        GrayRhinoEvidenceCategory::RiskNormalization => matches!(
            source_type,
            GrayRhinoEvidenceSourceType::MarketNarrativeCorpus
                | GrayRhinoEvidenceSourceType::RegulatoryFiling
                | GrayRhinoEvidenceSourceType::GovernanceDocument
                | GrayRhinoEvidenceSourceType::CompanyDisclosure
                | GrayRhinoEvidenceSourceType::OperatorCuratedSource
        ),
    }
}
