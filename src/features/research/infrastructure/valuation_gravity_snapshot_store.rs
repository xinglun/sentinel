use crate::features::research::application::valuation_gravity::{
    ValuationGravitySnapshot, ValuationGravitySnapshotRepository,
};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::Path;
use std::path::PathBuf;

pub(crate) struct FileValuationGravitySnapshotRepository {
    save_dir: PathBuf,
}

impl FileValuationGravitySnapshotRepository {
    pub(crate) fn new(save_dir: impl Into<PathBuf>) -> Self {
        Self {
            save_dir: save_dir.into(),
        }
    }
}

impl ValuationGravitySnapshotRepository for FileValuationGravitySnapshotRepository {
    fn load(&self, as_of_date: NaiveDate) -> Result<Option<ValuationGravitySnapshot>, String> {
        load_valuation_gravity_snapshot(&self.save_dir, as_of_date)
            .map_err(|error| error.to_string())
    }

    fn save(&self, snapshot: &ValuationGravitySnapshot) -> Result<(), String> {
        persist_valuation_gravity_snapshot(&self.save_dir, snapshot)
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn persist_valuation_gravity_snapshot(
    save_dir: &Path,
    snapshot: &ValuationGravitySnapshot,
) -> Result<()> {
    std::fs::create_dir_all(save_dir)
        .with_context(|| format!("Failed to create output directory: {}", save_dir.display()))?;
    let encoded = serde_json::to_string_pretty(snapshot)?;
    write_atomic(
        &save_dir.join(format!("valuation_gravity_{}.json", snapshot.as_of_date)),
        &encoded,
    )?;
    write_atomic(&save_dir.join("valuation_gravity_latest.json"), &encoded)?;
    Ok(())
}

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, contents)
        .with_context(|| format!("Failed to write {}", temporary.display()))?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).with_context(|| format!("Failed to replace {}", path.display()));
    }
    Ok(())
}

pub(crate) fn load_valuation_gravity_snapshot(
    save_dir: &Path,
    as_of_date: NaiveDate,
) -> Result<Option<ValuationGravitySnapshot>> {
    let path = save_dir.join(format!("valuation_gravity_{as_of_date}.json"));
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let snapshot: ValuationGravitySnapshot = serde_json::from_str(&raw)?;
    snapshot
        .validate_for_replay(as_of_date)
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("Invalid valuation snapshot: {}", path.display()))?;
    Ok(Some(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::application::valuation_gravity::{
        GravityStatus, ValuationConfidence, ValuationDataQualityReason,
        ValuationGravityAssetSnapshot, ValuationSource, ValuationSourceHealth,
    };

    #[test]
    fn snapshot_store_writes_latest_and_date_files() {
        let dir = tempfile::tempdir().unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        let snapshot = ValuationGravitySnapshot {
            as_of_date: date,
            assets: vec![ValuationGravityAssetSnapshot {
                symbol: "MSFT".to_string(),
                gravity: Some(GravityStatus::Fair),
                confidence: Some(ValuationConfidence::Low),
                source: Some(ValuationSource::MarketMultiple),
                provider: "Finnhub".to_string(),
                as_of_date: date,
                source_health: ValuationSourceHealth::Partial,
                quality_reason: ValuationDataQualityReason::MarketMultipleFallback,
                evidence_count: 5,
                relative_ratio: Some(1.0),
                message: "fixture".to_string(),
            }],
            observation_only: true,
        };

        persist_valuation_gravity_snapshot(dir.path(), &snapshot).unwrap();

        assert!(dir.path().join("valuation_gravity_latest.json").exists());
        assert_eq!(
            load_valuation_gravity_snapshot(dir.path(), date).unwrap(),
            Some(snapshot)
        );
    }

    #[test]
    fn repository_returns_filesystem_write_failure() {
        let dir = tempfile::tempdir().unwrap();
        let blocked_path = dir.path().join("blocked");
        std::fs::write(&blocked_path, "not a directory").unwrap();
        let repository = FileValuationGravitySnapshotRepository::new(blocked_path);
        let date = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        let snapshot = ValuationGravitySnapshot {
            as_of_date: date,
            assets: Vec::new(),
            observation_only: true,
        };

        let error = repository.save(&snapshot).unwrap_err();

        assert!(error.contains("Failed to create output directory"));
    }

    #[test]
    fn snapshot_store_rejects_deserializable_snapshots_that_break_domain_invariants() {
        let dir = tempfile::tempdir().unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        let path = dir.path().join("valuation_gravity_2026-06-18.json");
        let invalid_snapshots = [
            serde_json::json!({
                "as_of_date": "2026-06-17", "assets": [], "observation_only": true
            }),
            serde_json::json!({
                "as_of_date": "2026-06-18", "assets": [], "observation_only": false
            }),
            serde_json::json!({
                "as_of_date": "2026-06-18",
                "observation_only": true,
                "assets": [{
                    "symbol": "MSFT", "gravity": "FAIR", "confidence": null,
                    "source": "MARKET_MULTIPLE", "provider": "Finnhub",
                    "as_of_date": "2026-06-19", "source_health": "SUCCEEDED",
                    "quality_reason": "PRICE_TARGET_CONSENSUS", "evidence_count": 0,
                    "relative_ratio": 1.0, "message": "corrupt fixture"
                }]
            }),
        ];

        for invalid in invalid_snapshots {
            std::fs::write(&path, serde_json::to_string(&invalid).unwrap()).unwrap();
            let error = load_valuation_gravity_snapshot(dir.path(), date).unwrap_err();
            assert!(error.to_string().contains("Invalid valuation snapshot"));
        }
    }
}
