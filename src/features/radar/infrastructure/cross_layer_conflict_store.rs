use crate::features::radar::domain::cross_layer_conflict::CrossLayerConflictObservation;
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 層間衝突の跨日観測を追記型 JSONL として保存する。遡及書き込みはしない。
pub struct CrossLayerConflictStore {
    path: PathBuf,
}

impl CrossLayerConflictStore {
    pub fn new(save_dir: &Path) -> Self {
        Self {
            path: save_dir.join("cross_layer_conflict_observations.jsonl"),
        }
    }

    pub fn record_observations(
        &self,
        observations: &[CrossLayerConflictObservation],
    ) -> Result<usize> {
        let mut recorded = 0;
        for observation in observations {
            if self.record_observation(observation)? {
                recorded += 1;
            }
        }
        Ok(recorded)
    }

    pub fn load_observations(&self) -> Result<Vec<CrossLayerConflictObservation>> {
        load_jsonl(&self.path)
    }

    fn record_observation(&self, observation: &CrossLayerConflictObservation) -> Result<bool> {
        let records = self.load_observations()?;
        if records.iter().any(|existing| existing == observation) {
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
        writeln!(file, "{}", serde_json::to_string(observation)?)?;
        Ok(true)
    }
}

fn load_jsonl<T>(path: &Path) -> Result<Vec<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .with_context(|| format!("Failed to parse JSONL in {}", path.display()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::cross_layer_conflict::{
        CrossLayerDirectionPair, CrossLayerPair,
    };
    use chrono::NaiveDate;
    use tempfile::tempdir;

    fn observation(symbol: &str, observed_at: NaiveDate) -> CrossLayerConflictObservation {
        CrossLayerConflictObservation {
            symbol: symbol.to_string(),
            layer_pair: CrossLayerPair::ValuationVsTrend,
            direction_pair: CrossLayerDirectionPair::BullishValuationWeakeningTrend,
            observed_at,
        }
    }

    #[test]
    fn records_new_observations_and_ignores_exact_duplicates() {
        let dir = tempdir().unwrap();
        let store = CrossLayerConflictStore::new(dir.path());
        let day1 = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();

        let saved = store
            .record_observations(&[observation("NVDA", day1)])
            .unwrap();
        assert_eq!(saved, 1);

        let saved_again = store
            .record_observations(&[observation("NVDA", day1)])
            .unwrap();
        assert_eq!(saved_again, 0);

        let loaded = store.load_observations().unwrap();
        assert_eq!(loaded, vec![observation("NVDA", day1)]);
    }

    #[test]
    fn appends_distinct_dates_without_overwriting_history() {
        let dir = tempdir().unwrap();
        let store = CrossLayerConflictStore::new(dir.path());
        let day1 = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();

        store
            .record_observations(&[observation("NVDA", day1)])
            .unwrap();
        store
            .record_observations(&[observation("NVDA", day2)])
            .unwrap();

        let loaded = store.load_observations().unwrap();
        assert_eq!(
            loaded,
            vec![observation("NVDA", day1), observation("NVDA", day2)]
        );
    }

    #[test]
    fn missing_store_file_loads_as_empty_without_fabricating_history() {
        let dir = tempdir().unwrap();
        let store = CrossLayerConflictStore::new(dir.path());
        assert_eq!(store.load_observations().unwrap(), Vec::new());
    }
}
