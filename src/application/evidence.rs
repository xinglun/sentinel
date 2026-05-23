use crate::domain::evidence::AutomatedEvidenceRecord;
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
