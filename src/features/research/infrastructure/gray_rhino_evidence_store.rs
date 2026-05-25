use crate::features::research::application::governance_evidence::GovernanceEvidenceRepository;
use crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord;
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 灰色のサイ evidence を追記型 JSONL として保存する。
pub(crate) struct GrayRhinoEvidenceStore {
    path: PathBuf,
}

impl GrayRhinoEvidenceStore {
    pub(crate) fn new(save_dir: &Path) -> Self {
        Self {
            path: save_dir.join("gray_rhino_evidence.jsonl"),
        }
    }
}

impl GovernanceEvidenceRepository for GrayRhinoEvidenceStore {
    fn save_governance_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
        let records = self.load_governance_evidence()?;
        if records.iter().any(|existing| existing == record) {
            return Ok(false);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("Failed to open {}", self.path.display()))?;
        writeln!(file, "{}", serde_json::to_string(record)?)?;
        Ok(true)
    }

    fn load_governance_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read {}", self.path.display()))?;
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line).with_context(|| {
                    format!(
                        "Failed to parse gray rhino evidence in {}",
                        self.path.display()
                    )
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceCategory, GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
    };
    use chrono::NaiveDate;
    use tempfile::tempdir;

    fn record() -> GrayRhinoEvidenceRecord {
        GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Proxy statement".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/proxy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.9,
            extraction_note: "Proxy statement discloses voting rights.".to_string(),
            structural_fact: "Dual class shares create unequal voting rights.".to_string(),
        }
    }

    #[test]
    fn appends_governance_evidence_without_duplicates() {
        let dir = tempdir().unwrap();
        let store = GrayRhinoEvidenceStore::new(dir.path());
        let record = record();

        assert!(store.save_governance_evidence(&record).unwrap());
        assert!(!store.save_governance_evidence(&record).unwrap());

        assert_eq!(store.load_governance_evidence().unwrap(), vec![record]);
    }
}
