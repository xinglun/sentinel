use crate::application::evidence::EvidenceRepository;
use crate::domain::evidence::AutomatedEvidenceRecord;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// 実質性証拠（Substantive Evidence）の永続化を担当する。
/// dedupe_key に基づく重複排除をサポートする。
pub struct EvidenceStore {
    path: PathBuf,
}

impl EvidenceStore {
    pub fn new(save_dir: &Path) -> Self {
        Self {
            path: save_dir.join("evidence_records.jsonl"),
        }
    }

    /// すべての証拠レコードを読み込む。
    pub fn load_all(&self) -> Result<Vec<AutomatedEvidenceRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path).context("Failed to open evidence_records.jsonl")?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<AutomatedEvidenceRecord>(&line) {
                Ok(record) => records.push(record),
                Err(e) => {
                    // 読み込みエラーをログに記録して続行（破損した行をスキップ）
                    eprintln!("Warning: Failed to deserialize evidence record: {}", e);
                }
            }
        }

        Ok(records)
    }

    /// 新しいレコードを保存する。重複（dedupe_key の一致）がある場合はスキップする。
    pub fn save_records(&self, new_records: &[AutomatedEvidenceRecord]) -> Result<usize> {
        if new_records.is_empty() {
            return Ok(0);
        }

        // 現在の全レコードを読み込んで重複チェック
        let existing = self.load_all()?;
        let mut known_keys: HashSet<String> = existing.into_iter().map(|r| r.dedupe_key).collect();
        let mut saved_count = 0;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create evidence save directory")?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .context("Failed to open evidence_records.jsonl for appending")?;

        for record in new_records {
            if known_keys.insert(record.dedupe_key.clone()) {
                let json = serde_json::to_string(record)?;
                writeln!(file, "{json}")?;
                saved_count += 1;
            }
        }

        Ok(saved_count)
    }

    pub fn find_by_symbol(&self, symbol: &str) -> Result<Vec<AutomatedEvidenceRecord>> {
        let all = self.load_all()?;
        Ok(all
            .into_iter()
            .filter(|r| r.symbol.as_deref() == Some(symbol))
            .collect())
    }

    /// 指定された保持期間を超えた古いレコードをクリーンアップする。
    pub fn cleanup_old_records(&self, max_age_days: i64) -> Result<usize> {
        if !self.path.exists() {
            return Ok(0);
        }

        let all_records = self.load_all()?;
        let initial_count = all_records.len();
        let now = chrono::Local::now().naive_local().date();

        let filtered_records: Vec<_> = all_records
            .into_iter()
            .filter(|r| {
                if let Ok(event_date) = chrono::NaiveDate::parse_from_str(&r.event_date, "%Y-%m-%d")
                {
                    let age = now - event_date;
                    age.num_days() <= max_age_days
                } else {
                    // 日付フォーマットが不正なものは安全のため維持
                    true
                }
            })
            .collect();

        let removed_count = initial_count - filtered_records.len();
        if removed_count > 0 {
            let file = File::create(&self.path)
                .context("Failed to rewrite evidence_records.jsonl during cleanup")?;
            let mut writer = std::io::BufWriter::new(file);
            for record in filtered_records {
                let json = serde_json::to_string(&record)?;
                writeln!(writer, "{json}")?;
            }
            writer.flush()?;
        }

        Ok(removed_count)
    }
}

impl EvidenceRepository for EvidenceStore {
    fn load_all(&self) -> Result<Vec<AutomatedEvidenceRecord>> {
        EvidenceStore::load_all(self)
    }

    fn save_records(&self, new_records: &[AutomatedEvidenceRecord]) -> Result<usize> {
        EvidenceStore::save_records(self, new_records)
    }

    fn find_by_symbol(&self, symbol: &str) -> Result<Vec<AutomatedEvidenceRecord>> {
        EvidenceStore::find_by_symbol(self, symbol)
    }

    fn cleanup_old_records(&self, max_age_days: i64) -> Result<usize> {
        EvidenceStore::cleanup_old_records(self, max_age_days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::trend_cohesion::{EvidenceSourceType, EvidenceType};
    use tempfile::tempdir;

    #[test]
    fn test_evidence_store_save_and_load() -> Result<()> {
        let dir = tempdir()?;
        let store = EvidenceStore::new(dir.path());

        let record1 = AutomatedEvidenceRecord {
            dedupe_key: "key1".to_string(),
            symbol: Some("AAPL".to_string()),
            evidence_type: EvidenceType::FollowThrough,
            source: EvidenceSourceType::PriceAction,
            confidence: 1.0,
            event_date: "2024-05-01".to_string(),
            description: "Breakout".to_string(),
            source_url: None,
        };

        let record2 = AutomatedEvidenceRecord {
            dedupe_key: "key2".to_string(),
            symbol: Some("GOOG".to_string()),
            evidence_type: EvidenceType::EarningsValidation,
            source: EvidenceSourceType::Manual,
            confidence: 0.8,
            event_date: "2024-05-01".to_string(),
            description: "Strong earnings".to_string(),
            source_url: None,
        };

        // Save records
        let saved = store.save_records(&[record1.clone(), record2.clone()])?;
        assert_eq!(saved, 2);

        // Load all
        let all = store.load_all()?;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].dedupe_key, "key1");
        assert_eq!(all[1].dedupe_key, "key2");

        // Duplicate save
        let saved_again = store.save_records(std::slice::from_ref(&record1))?;
        assert_eq!(saved_again, 0); // Should be deduped

        // Filter by symbol
        let aapl = store.find_by_symbol("AAPL")?;
        assert_eq!(aapl.len(), 1);
        assert_eq!(aapl[0].symbol.as_deref(), Some("AAPL"));

        Ok(())
    }

    #[test]
    fn test_evidence_store_cleanup() -> Result<()> {
        let dir = tempdir()?;
        let store = EvidenceStore::new(dir.path());

        let now = chrono::Local::now().naive_local().date();
        let old_date = (now - chrono::Duration::days(31))
            .format("%Y-%m-%d")
            .to_string();
        let recent_date = (now - chrono::Duration::days(10))
            .format("%Y-%m-%d")
            .to_string();

        let records = vec![
            AutomatedEvidenceRecord {
                source: EvidenceSourceType::Manual,
                evidence_type: EvidenceType::CapexPayoff,
                confidence: 1.0,
                description: "Old".to_string(),
                event_date: old_date,
                symbol: Some("AAPL".to_string()),
                source_url: None,
                dedupe_key: "old".to_string(),
            },
            AutomatedEvidenceRecord {
                source: EvidenceSourceType::Manual,
                evidence_type: EvidenceType::CapexPayoff,
                confidence: 1.0,
                description: "Recent".to_string(),
                event_date: recent_date,
                symbol: Some("AAPL".to_string()),
                source_url: None,
                dedupe_key: "recent".to_string(),
            },
        ];

        store.save_records(&records)?;
        assert_eq!(store.load_all()?.len(), 2);

        let removed = store.cleanup_old_records(30)?;
        assert_eq!(removed, 1);

        let remaining = store.load_all()?;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].dedupe_key, "recent");

        Ok(())
    }

    #[test]
    fn test_evidence_store_corrupted_line() -> Result<()> {
        let dir = tempdir()?;
        let store = EvidenceStore::new(dir.path());
        let file_path = dir.path().join("evidence_records.jsonl");

        // Write a valid line and an invalid line
        let mut file = File::create(&file_path)?;
        let valid_record = AutomatedEvidenceRecord {
            dedupe_key: "valid".to_string(),
            ..Default::default()
        };
        writeln!(file, "{}", serde_json::to_string(&valid_record)?)?;
        writeln!(file, "{{ corrupted json }}")?;
        writeln!(file)?; // Empty line

        let all = store.load_all()?;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].dedupe_key, "valid");

        Ok(())
    }

    #[test]
    fn test_evidence_store_dedupes_same_batch() -> Result<()> {
        let dir = tempdir()?;
        let store = EvidenceStore::new(dir.path());

        let first = AutomatedEvidenceRecord {
            dedupe_key: "same-key".to_string(),
            symbol: Some("AAPL".to_string()),
            evidence_type: EvidenceType::FollowThrough,
            source: EvidenceSourceType::PriceAction,
            confidence: 0.9,
            event_date: "2024-05-01".to_string(),
            description: "First".to_string(),
            source_url: None,
        };
        let duplicate = AutomatedEvidenceRecord {
            description: "Duplicate".to_string(),
            ..first.clone()
        };

        let saved = store.save_records(&[first, duplicate])?;
        assert_eq!(saved, 1);
        assert_eq!(store.load_all()?.len(), 1);

        Ok(())
    }
}
