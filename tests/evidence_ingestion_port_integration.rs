use std::collections::HashMap;
use stock_sentinel::application::evidence_ingestion::{EvidenceExtractor, SourceDocument};
use stock_sentinel::core::evidence_ingestion::RuleBasedExtractor;
use stock_sentinel::domain::evidence::{EvidenceSourceType, EvidenceType};
use stock_sentinel::infrastructure::evidence_ingestion::RuleBasedExtractor as InfrastructureRuleBasedExtractor;

#[test]
fn rule_based_extractor_works_through_application_port() {
    let extractor: &dyn EvidenceExtractor = &RuleBasedExtractor::new();
    let doc = SourceDocument {
        title: "MSFT earnings".to_string(),
        content: "revenue growth and margin expansion validate AI monetization".to_string(),
        url: "https://example.com/msft".to_string(),
        source_type: EvidenceSourceType::OfficialIR,
        symbol: "MSFT".to_string(),
        metadata: HashMap::new(),
    };

    let records = extractor.extract(&doc);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].evidence_type, EvidenceType::EarningsValidation);
    assert_eq!(records[0].symbol.as_deref(), Some("MSFT"));
}

#[test]
fn core_reexport_preserves_source_document_import_path() {
    let doc = stock_sentinel::core::evidence_ingestion::SourceDocument {
        title: "GOOG capex".to_string(),
        content: "capex payoff".to_string(),
        url: "https://example.com/goog".to_string(),
        source_type: EvidenceSourceType::NewsMedia,
        symbol: "GOOG".to_string(),
        metadata: HashMap::new(),
    };

    assert_eq!(doc.symbol, "GOOG");
    assert_eq!(doc.source_type, EvidenceSourceType::NewsMedia);
}

#[test]
fn infrastructure_extractor_matches_core_reexport_boundary() {
    let core_extractor: &dyn EvidenceExtractor = &RuleBasedExtractor::new();
    let infrastructure_extractor: &dyn EvidenceExtractor = &InfrastructureRuleBasedExtractor::new();
    let doc = SourceDocument {
        title: "NVDA order visibility".to_string(),
        content: "backlog and demand outstripping supply".to_string(),
        url: "https://example.com/nvda".to_string(),
        source_type: EvidenceSourceType::NewsMedia,
        symbol: "NVDA".to_string(),
        metadata: HashMap::new(),
    };

    let via_core = core_extractor.extract(&doc);
    let via_infra = infrastructure_extractor.extract(&doc);
    assert_eq!(via_core, via_infra);
    assert_eq!(via_infra[0].evidence_type, EvidenceType::OrderVisibility);
}
