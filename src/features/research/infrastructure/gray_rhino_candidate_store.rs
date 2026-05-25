use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidate;
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 自動発見された Gray Rhino candidate を追記型 JSONL として保存する。
pub(crate) struct GrayRhinoCandidateStore {
    path: PathBuf,
}

impl GrayRhinoCandidateStore {
    pub(crate) fn new(save_dir: &Path) -> Self {
        Self {
            path: save_dir.join("gray_rhino_candidates.jsonl"),
        }
    }

    pub(crate) fn save_candidates(&self, candidates: &[GrayRhinoCandidate]) -> Result<usize> {
        let mut saved = 0;
        for candidate in candidates {
            if self.save_candidate(candidate)? {
                saved += 1;
            }
        }
        Ok(saved)
    }

    pub(crate) fn load_candidates(&self) -> Result<Vec<GrayRhinoCandidate>> {
        load_jsonl(&self.path)
    }

    fn save_candidate(&self, candidate: &GrayRhinoCandidate) -> Result<bool> {
        let records = self.load_candidates()?;
        if records.iter().any(|existing| existing == candidate) {
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
        writeln!(file, "{}", serde_json::to_string(candidate)?)?;
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
    use crate::features::research::domain::gray_rhino_candidate::{
        GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
    };
    use chrono::NaiveDate;
    use tempfile::tempdir;

    fn candidate() -> GrayRhinoCandidate {
        GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::GovernanceConcentration,
            subject: "TSLA".to_string(),
            state: GrayRhinoCandidateState::Expanding,
            evidence: vec!["Founder control detected.".to_string()],
            watch_triggers: vec!["voting terms".to_string()],
            source_title: "Proxy statement".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            source_published_at: Some(NaiveDate::from_ymd_opt(2026, 5, 25).unwrap()),
            last_confirmed_at: Some(NaiveDate::from_ymd_opt(2026, 5, 25).unwrap()),
            resolved_at: None,
        }
    }

    #[test]
    fn saves_candidates_without_duplicate_records() {
        let dir = tempdir().unwrap();
        let store = GrayRhinoCandidateStore::new(dir.path());
        let candidate = candidate();

        assert_eq!(
            store
                .save_candidates(std::slice::from_ref(&candidate))
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .save_candidates(std::slice::from_ref(&candidate))
                .unwrap(),
            0
        );
        assert_eq!(store.load_candidates().unwrap(), vec![candidate]);
    }
}
