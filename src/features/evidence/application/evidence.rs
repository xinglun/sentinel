use crate::features::evidence::domain::evidence::AutomatedEvidenceRecord;
use crate::features::evidence::domain::ingestion_command::ManualEvidenceCommand;
use anyhow::Result;

/// 実体的証拠の永続化 port。
///
/// Application layer は具体的な JSONL / filesystem 実装を知らず、この port だけに依存する。
pub trait EvidenceRepository {
    fn load_all(&self) -> Result<Vec<AutomatedEvidenceRecord>>;
    fn save_records(&self, new_records: &[AutomatedEvidenceRecord]) -> Result<usize>;
    fn find_by_symbol(&self, symbol: &str) -> Result<Vec<AutomatedEvidenceRecord>>;
    fn cleanup_old_records(&self, max_age_days: i64) -> Result<usize>;
}

use crate::features::evidence::domain::evidence::EvidenceType;

/// 手動 evidence ingestion use case の入力。
#[derive(Debug, Clone)]
pub struct ManualEvidenceIngestionRequest {
    pub evidence_type: String,
    pub confidence: f64,
    pub description: String,
    pub event_date: Option<String>,
    pub symbol: Option<String>,
    pub source_url: Option<String>,
    pub fallback_date: String,
    pub retention_days: Option<i64>,
}

/// 手動 evidence ingestion use case の結果。
#[derive(Debug, Clone)]
pub struct ManualEvidenceIngestionOutcome {
    pub record: AutomatedEvidenceRecord,
    pub saved_count: usize,
    pub cleanup_count: usize,
}

/// 手動 evidence を検証し、repository へ保存する。
pub fn ingest_manual_evidence(
    repository: &dyn EvidenceRepository,
    request: ManualEvidenceIngestionRequest,
) -> Result<ManualEvidenceIngestionOutcome> {
    let evidence_type = parse_manual_evidence_type(&request.evidence_type)?;
    let event_date = request.event_date.unwrap_or(request.fallback_date);
    let command = ManualEvidenceCommand::new(
        evidence_type,
        request.confidence,
        request.description,
        event_date,
        request.symbol,
        request.source_url,
    )?;

    let mut cleanup_count = 0;
    if let Some(retention_days) = request.retention_days {
        cleanup_count = repository.cleanup_old_records(retention_days)?;
    }

    let dedupe_key = format!(
        "CLI:Manual:{}:{}:{}",
        command.symbol.as_deref().unwrap_or("GLOBAL"),
        request.evidence_type,
        command.event_date
    );
    let record = AutomatedEvidenceRecord::from(command.into_record(dedupe_key));

    let saved_count = repository.save_records(std::slice::from_ref(&record))?;
    Ok(ManualEvidenceIngestionOutcome {
        record,
        saved_count,
        cleanup_count,
    })
}

/// CLI の manual evidence type を domain enum へ変換する。
pub fn parse_manual_evidence_type(value: &str) -> Result<EvidenceType> {
    match value {
        "capex" => Ok(EvidenceType::CapexPayoff),
        "earnings" => Ok(EvidenceType::EarningsValidation),
        "order" => Ok(EvidenceType::OrderVisibility),
        "follow_through" => Ok(EvidenceType::FollowThrough),
        _ => Err(anyhow::anyhow!("Invalid evidence type: {}", value)),
    }
}
