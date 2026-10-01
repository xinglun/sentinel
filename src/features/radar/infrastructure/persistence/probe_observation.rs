//! Probe の監査 sidecar。既存 decision snapshot を変更しない。
use super::PersistenceLayer;
use crate::features::radar::domain::probe_eligibility_observation::ProbeFact;
use anyhow::{Context, Result};

impl PersistenceLayer {
    pub fn save_probe_observation_archive<T: serde::Serialize>(
        &self,
        run_id: &str,
        value: &T,
    ) -> Result<()> {
        anyhow::ensure!(
            !run_id.is_empty()
                && run_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "invalid observation run identity"
        );
        let dir = self.save_dir.join("probe_observation_archives");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{run_id}.json"));
        if path.exists() {
            let existing: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
            anyhow::ensure!(
                existing == serde_json::to_value(value)?,
                "conflicting observation archive identity"
            );
            return Ok(());
        }

        super::atomic::write_file_atomically(&path, serde_json::to_string_pretty(value)?.as_bytes())
    }

    pub fn save_probe_fact(&self, fact: &ProbeFact) -> Result<()> {
        anyhow::ensure!(
            !fact.report_run_id.is_empty()
                && fact
                    .report_run_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "invalid observation run identity"
        );
        let dir = self.save_dir.join("probe_observations");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}-{}.json", fact.market_date, fact.report_run_id));
        if path.exists() {
            let existing: ProbeFact = serde_json::from_slice(&std::fs::read(&path)?)?;
            anyhow::ensure!(existing == *fact, "conflicting observation run identity");
            return Ok(());
        }
        super::atomic::write_file_atomically(&path, serde_json::to_string_pretty(fact)?.as_bytes())
    }
    pub fn load_probe_facts(&self) -> Result<Vec<ProbeFact>> {
        let dir = self.save_dir.join("probe_observations");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut facts = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                facts.push(
                    serde_json::from_slice(&std::fs::read(path)?)
                        .context("invalid Probe observation history")?,
                );
            }
        }
        Ok(facts)
    }
}
#[cfg(test)]
mod tests {
    use super::super::PersistenceLayer;
    use crate::features::radar::domain::probe_eligibility_observation::{Permission, ProbeFact};
    #[test]
    fn archive_keeps_complete_windows_and_episode_values() {
        let dir =
            std::env::temp_dir().join(format!("sentinel-probe-archive-{}", uuid::Uuid::new_v4()));
        let layer = PersistenceLayer::new(&dir);
        let value = serde_json::json!({"session_20":{"episodes":[{"episode_id":"probe-2026-09-01"}]},"session_60":{},"all_history":{},"decision_weight":0,"trade_signal":false});
        layer
            .save_probe_observation_archive("run-1", &value)
            .unwrap();
        let archived: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join("probe_observation_archives/run-1.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(archived, value);
        layer
            .save_probe_observation_archive("run-1", &value)
            .unwrap();
        assert!(layer
            .save_probe_observation_archive("run-1", &serde_json::json!({"conflict":true}))
            .is_err());
        let original: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join("probe_observation_archives/run-1.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(original, value);

        assert!(layer
            .save_probe_observation_archive("../bad", &value)
            .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn roundtrip_preserves_missing_eligibility_and_rejects_conflicting_identity() {
        let dir = std::env::temp_dir().join(format!("sentinel-probe-{}", uuid::Uuid::new_v4()));
        let layer = PersistenceLayer::new(&dir);
        let fact = ProbeFact {
            market_date: chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
            permission: Permission::Probe,
            eligible_asset_count: None,
            report_run_id: "run-1".into(),
            observed_at: "2026-09-30T21:00:00Z".into(),
            provenance: "canonical-final-execution-v1".into(),
        };
        let mut legacy = serde_json::to_value(&fact).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("eligible_asset_count");
        let missing: ProbeFact = serde_json::from_value(legacy).unwrap();
        assert_eq!(missing.eligible_asset_count, None);
        layer.save_probe_fact(&fact).unwrap();
        layer.save_probe_fact(&fact).unwrap();
        assert_eq!(layer.load_probe_facts().unwrap(), vec![fact.clone()]);
        let mut other = fact;
        other.eligible_asset_count = Some(0);
        assert!(layer.save_probe_fact(&other).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
