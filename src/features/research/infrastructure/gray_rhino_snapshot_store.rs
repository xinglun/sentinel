use crate::features::research::domain::gray_rhino::GrayRhinoAssessmentSnapshot;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 灰色のサイの日次評価を追記型 JSONL として保持する。
pub(crate) struct GrayRhinoSnapshotStore {
    path: PathBuf,
}

impl GrayRhinoSnapshotStore {
    pub(crate) fn new(save_dir: &Path) -> Self {
        Self {
            path: save_dir.join("gray_rhino_snapshots.jsonl"),
        }
    }

    pub(crate) fn load_latest_before(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Option<GrayRhinoAssessmentSnapshot>> {
        let snapshots = self.load_all()?;
        Ok(snapshots
            .into_iter()
            .filter(|snapshot| snapshot.as_of_date < as_of_date)
            .max_by_key(|snapshot| snapshot.as_of_date))
    }

    pub(crate) fn save_if_changed(&self, snapshot: &GrayRhinoAssessmentSnapshot) -> Result<()> {
        if self.load_all()?.iter().any(|existing| existing == snapshot) {
            return Ok(());
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
        writeln!(file, "{}", serde_json::to_string(snapshot)?)?;
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<GrayRhinoAssessmentSnapshot>> {
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
                        "Failed to parse gray rhino snapshot in {}",
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
    use crate::features::research::domain::gray_rhino::{
        evaluate_gray_rhino_escalation, GrayRhinoEscalationInput, GrayRhinoObservationSource,
        RiskLevel,
    };
    use tempfile::tempdir;

    fn snapshot(date: NaiveDate, risk: RiskLevel) -> GrayRhinoAssessmentSnapshot {
        GrayRhinoAssessmentSnapshot {
            schema_version: 1,
            as_of_date: date,
            source: GrayRhinoObservationSource::ManualConfiguration,
            escalation: evaluate_gray_rhino_escalation(GrayRhinoEscalationInput {
                risk_expansion_rate: risk,
                constraint_growth_rate: RiskLevel::Moderate,
                dependency_centralization: RiskLevel::Low,
                awareness_decay: RiskLevel::Low,
                narrative_overconfidence: RiskLevel::Low,
                single_point_fragility: RiskLevel::Low,
                fallback_survivability_risk: RiskLevel::Low,
                notes: Vec::new(),
            }),
        }
    }

    #[test]
    fn snapshot_store_preserves_prior_business_date_without_duplicate_write() {
        let dir = tempdir().unwrap();
        let store = GrayRhinoSnapshotStore::new(dir.path());
        let first = snapshot(
            NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
            RiskLevel::Low,
        );
        let next = snapshot(
            NaiveDate::from_ymd_opt(2026, 5, 22).unwrap(),
            RiskLevel::Elevated,
        );

        store.save_if_changed(&first).unwrap();
        store.save_if_changed(&first).unwrap();
        store.save_if_changed(&next).unwrap();

        let previous = store
            .load_latest_before(NaiveDate::from_ymd_opt(2026, 5, 22).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(previous, first);
        let lines = fs::read_to_string(dir.path().join("gray_rhino_snapshots.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 2);
    }
}
