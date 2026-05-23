pub use crate::application::evidence_ingestion::{
    EvidenceExtractor, SourceDocument, SourceFetcher,
};
pub use crate::infrastructure::evidence_ingestion::{
    FinnhubFetcher, FixtureFetcher, RuleBasedExtractor, SECEDGARFetcher, WebFetcher,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence::{EvidenceSourceType, EvidenceType};
    use std::collections::HashMap;

    #[test]
    fn test_rule_based_extractor_goog() {
        let doc = SourceDocument {
            title: "GOOG Q1 Results".to_string(),
            content: "AI infrastructure is starting to see significant capex payoff".to_string(),
            url: "http://example.com".to_string(),
            source_type: EvidenceSourceType::OfficialIR,
            symbol: "GOOG".to_string(),
            metadata: HashMap::new(),
        };

        let extractor = RuleBasedExtractor::new();
        let records = extractor.extract(&doc);

        assert!(!records.is_empty());
        assert_eq!(records[0].evidence_type, EvidenceType::CapexPayoff);
        assert_eq!(records[0].symbol, Some("GOOG".to_string()));
    }

    #[test]
    fn test_rule_based_extractor_sec_form() {
        let mut metadata = HashMap::new();
        metadata.insert("form_type".to_string(), "8-K".to_string());
        let doc = SourceDocument {
            title: "SEC Filing 8-K for AAPL".to_string(),
            content: "Item 2.02 Results of Operations and Financial Condition... revenue growth was strong".to_string(),
            url: "https://www.sec.gov/Archives/...".to_string(),
            source_type: EvidenceSourceType::OfficialIR,
            symbol: "AAPL".to_string(),
            metadata,
        };

        let extractor = RuleBasedExtractor::new();
        let records = extractor.extract(&doc);

        assert!(!records.is_empty());
        // 8-K で Item 2.02 + revenue growth があれば EarningsValidation
        assert!(records
            .iter()
            .any(|r| r.evidence_type == EvidenceType::EarningsValidation));
        assert!(records.iter().any(|r| r.confidence >= 0.90));
    }

    #[test]
    fn test_rule_based_extractor_sec_10q() {
        let mut metadata = HashMap::new();
        metadata.insert("form_type".to_string(), "10-Q".to_string());
        let doc = SourceDocument {
            title: "SEC Filing 10-Q for MSFT".to_string(),
            content: "backlog is increasing... order visibility is high".to_string(),
            url: "https://www.sec.gov/Archives/...".to_string(),
            source_type: EvidenceSourceType::OfficialIR,
            symbol: "MSFT".to_string(),
            metadata,
        };

        let extractor = RuleBasedExtractor::new();
        let records = extractor.extract(&doc);

        assert!(!records.is_empty());
        assert!(records
            .iter()
            .any(|r| r.evidence_type == EvidenceType::OrderVisibility));
        // SEC 10-Q should have 0.90 confidence
        assert!(records.iter().any(|r| r.confidence >= 0.90));
    }

    #[test]
    fn test_rule_based_extractor_uses_metadata_date() {
        let mut metadata = HashMap::new();
        metadata.insert("filing_date".to_string(), "2024-01-01".to_string());
        let doc = SourceDocument {
            title: "Old News".to_string(),
            content: "capex payoff happened long ago".to_string(),
            url: "http://example.com".to_string(),
            source_type: EvidenceSourceType::NewsMedia,
            symbol: "AAPL".to_string(),
            metadata,
        };

        let extractor = RuleBasedExtractor::new();
        let records = extractor.extract(&doc);

        assert!(!records.is_empty());
        // The record should have the date from metadata, not today's date
        assert_eq!(records[0].event_date, "2024-01-01");
    }
}
