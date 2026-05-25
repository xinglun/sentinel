use crate::features::evidence::domain::evidence::EvidenceType;
use anyhow::Result;
use chrono::NaiveDate;

/// Evidence BC が受け取る手動 ingestion command。
#[derive(Debug, Clone)]
pub struct ManualEvidenceCommand {
    pub evidence_type: EvidenceType,
    pub confidence: f64,
    pub description: String,
    pub event_date: String,
    pub symbol: Option<String>,
    pub source_url: Option<String>,
}

impl ManualEvidenceCommand {
    pub fn new(
        evidence_type: EvidenceType,
        confidence: f64,
        description: String,
        event_date: String,
        symbol: Option<String>,
        source_url: Option<String>,
    ) -> Result<Self> {
        validate_confidence(confidence)?;
        validate_event_date(&event_date)?;
        Ok(Self {
            evidence_type,
            confidence,
            description,
            event_date,
            symbol,
            source_url,
        })
    }
}

fn validate_event_date(value: &str) -> Result<()> {
    if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        return Err(anyhow::anyhow!(
            "Invalid date format: {}. Use YYYY-MM-DD",
            value
        ));
    }
    Ok(())
}

fn validate_confidence(value: f64) -> Result<()> {
    if !(0.0..=1.0).contains(&value) {
        return Err(anyhow::anyhow!(
            "Confidence must be between 0.0 and 1.0. Got: {}",
            value
        ));
    }
    Ok(())
}
