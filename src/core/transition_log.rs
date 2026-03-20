use crate::core::market_regime::MarketRegimeSnapshot;
use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use chrono::Local;

pub struct TransitionLogger {
    log_path: PathBuf,
    jsonl_path: PathBuf,
}

impl TransitionLogger {
    pub fn new(save_dir: &Path) -> Self {
        Self {
            log_path: save_dir.join("state_transitions.csv"),
            jsonl_path: save_dir.join("state_transitions.jsonl"),
        }
    }


    pub fn log_transition(
        &self,
        old_regime: Option<&MarketRegimeSnapshot>,
        new_regime: &MarketRegimeSnapshot,
    ) -> Result<()> {
        self.log_to_csv(old_regime, new_regime)?;
        self.log_to_jsonl(old_regime, new_regime)?;
        Ok(())
    }

    fn log_to_csv(
        &self,
        old_regime: Option<&MarketRegimeSnapshot>,
        new_regime: &MarketRegimeSnapshot,
    ) -> Result<()> {
        let is_new_file = !self.log_path.exists();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .context("Failed to open state_transitions.csv")?;

        if is_new_file {
            writeln!(file, "Timestamp,Prev_State,New_State,Risk_Overlay,Reasons")?;
        }

        let old_state = old_regime
            .map(|r| format!("{:?}", r.market_state))
            .unwrap_or_else(|| "START".to_string());
        
        let new_state = format!("{:?}", new_regime.market_state);
        let risk = format!("{:?}", new_regime.risk_overlay);
        let reasons = new_regime.reasons.join(" | ");

        writeln!(
            file,
            "{},\"{}\",\"{}\",\"{}\",\"{}\"",
            Local::now().to_rfc3339(),
            old_state,
            new_state,
            risk,
            reasons
        )?;

        Ok(())
    }

    fn log_to_jsonl(
        &self,
        old_regime: Option<&MarketRegimeSnapshot>,
        new_regime: &MarketRegimeSnapshot,
    ) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.jsonl_path)
            .context("Failed to open state_transitions.jsonl")?;

        let entry = serde_json::json!({
            "timestamp": Local::now().to_rfc3339(),
            "prev_state": old_regime.map(|r| format!("{:?}", r.market_state)).unwrap_or_else(|| "START".to_string()),
            "new_state": format!("{:?}", new_regime.market_state),
            "risk_overlay": format!("{:?}", new_regime.risk_overlay),
            "reasons": new_regime.reasons,
        });

        writeln!(file, "{}", entry)?;
        Ok(())
    }
}

