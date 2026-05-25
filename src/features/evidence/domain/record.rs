use crate::features::evidence::domain::evidence::{
    AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType,
};

/// Evidence BC 内で扱う evidence record。
#[derive(Debug, Clone)]
pub struct EvidenceRecord {
    pub source_type: EvidenceSourceType,
    pub evidence_type: EvidenceType,
    pub confidence: f64,
    pub description: String,
    pub event_date: String,
    pub symbol: Option<String>,
    pub source_url: Option<String>,
    pub dedupe_key: String,
}

impl EvidenceRecord {
    pub fn manual(
        evidence_type: EvidenceType,
        confidence: f64,
        description: String,
        event_date: String,
        symbol: Option<String>,
        source_url: Option<String>,
        dedupe_key: String,
    ) -> Self {
        Self {
            source_type: EvidenceSourceType::Manual,
            evidence_type,
            confidence,
            description,
            event_date,
            symbol,
            source_url,
            dedupe_key,
        }
    }
}

impl From<EvidenceRecord> for AutomatedEvidenceRecord {
    fn from(record: EvidenceRecord) -> Self {
        AutomatedEvidenceRecord::new(
            record.source_type,
            record.evidence_type,
            record.confidence,
            record.description,
            record.event_date,
            record.symbol,
            record.source_url,
            record.dedupe_key,
        )
    }
}
