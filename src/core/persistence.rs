use crate::core::decision::DecisionPacket;
use crate::core::execution_gate::ExecutionResult;
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct PersistenceLayer {
    history_path: PathBuf,
    save_dir: PathBuf,
}

impl PersistenceLayer {
    pub fn new(save_dir: &Path) -> Self {
        Self {
            history_path: save_dir.join("decision_history.jsonl"),
            save_dir: save_dir.to_path_buf(),
        }
    }

    pub fn save_packet(&self, packet: &DecisionPacket) -> Result<()> {
        let json = serde_json::to_string(packet).context("Failed to serialize DecisionPacket")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_path)
            .context("Failed to open decision_history.jsonl for appending")?;

        writeln!(file, "{}", json).context("Failed to write packet to jsonl")?;
        Ok(())
    }

    pub fn load_latest_packet(&self) -> Result<Option<DecisionPacket>> {
        let recent = self.load_recent_packets(1)?;
        Ok(recent.into_iter().next())
    }

    /// Loads the most recent N packets from the history log.
    /// Packets are returned in chronological order (oldest first).
    pub fn load_recent_packets(&self, count: usize) -> Result<Vec<DecisionPacket>> {
        if count == 0 || !self.history_path.exists() {
            return Ok(Vec::new());
        }

        let file =
            File::open(&self.history_path).context("Failed to open decision_history.jsonl")?;
        let reader = BufReader::new(file);

        // For small history files, we can just read all and take last N.
        // For very large files, this would need a tail-like implementation.
        // Given this is a local tool, reading the whole file is usually fine for a few hundred days of history.
        let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

        let recent_lines = if lines.len() > count {
            &lines[lines.len() - count..]
        } else {
            &lines[..]
        };

        let mut packets = Vec::with_capacity(recent_lines.len());
        for line in recent_lines {
            if line.trim().is_empty() {
                continue;
            }
            let packet: DecisionPacket = serde_json::from_str(line)
                .context("Failed to deserialize DecisionPacket from history")?;
            packets.push(packet);
        }

        Ok(packets)
    }

    pub fn save_daily_packet(&self, packet: &DecisionPacket) -> Result<()> {
        let filename = format!("decision_packet_{}.json", packet.date);
        let path = self.save_dir.join(filename);
        let json = serde_json::to_string_pretty(packet)
            .context("Failed to serialize DecisionPacket for daily output")?;
        std::fs::write(path, json).context("Failed to write daily decision packet")?;
        Ok(())
    }

    pub fn save_execution_gate_log(&self, log: &serde_json::Value) -> Result<()> {
        let path = self.save_dir.join("execution_gate_log.jsonl");
        let json = serde_json::to_string(log).context("Failed to serialize execution gate log")?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }

    pub fn save_execution_gate_result(
        &self,
        packet: &DecisionPacket,
        result: &ExecutionResult,
    ) -> Result<()> {
        if result.audits.is_empty() {
            let summary = serde_json::json!({
                "event": "execution_gate_noop",
                "date": packet.date.to_string(),
                "market_state": format!("{:?}", packet.market_regime.market_state),
                "risk_overlay": format!("{:?}", packet.market_regime.risk_overlay),
                "asset_count": packet.assets.len(),
                "trade_count": result.trades.len(),
                "audit_count": 0,
                "reason": "No eligible ACCUMULATE/REDUCE signals reached the execution gate"
            });
            return self.save_execution_gate_log(&summary);
        }

        for audit in &result.audits {
            let log_entry = serde_json::to_value(audit)?;
            self.save_execution_gate_log(&log_entry)?;
        }

        Ok(())
    }

    pub fn save_portfolio_snapshot(&self, snapshot: &serde_json::Value, date: &str) -> Result<()> {
        let filename = format!("portfolio_snapshot_{}.json", date);
        let path = self.save_dir.join(filename);
        let json = serde_json::to_string_pretty(snapshot)
            .context("Failed to serialize portfolio snapshot")?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn save_account_snapshot(&self, snapshot: &serde_json::Value, date: &str) -> Result<()> {
        let filename = format!("account_snapshot_{}.json", date);
        let path = self.save_dir.join(filename);
        let json = serde_json::to_string_pretty(snapshot)
            .context("Failed to serialize account snapshot")?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn save_markdown_report(&self, content: &str, date: &str) -> Result<()> {
        let filename = format!("{}.md", date);
        let path = self.save_dir.join(filename);
        std::fs::write(path, content).context("Failed to write daily markdown report")?;
        Ok(())
    }

    pub fn save_data_quality_log(&self, log: &serde_json::Value) -> Result<()> {
        let path = self.save_dir.join("data_quality_log.jsonl");
        let json = serde_json::to_string(log).context("Failed to serialize data quality log")?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }

    pub fn save_telemetry(&self, row: &crate::core::telemetry::TelemetryRow) -> Result<()> {
        let path = self.save_dir.join("telemetry.csv");
        let is_new = !path.exists();
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

        if is_new {
            writeln!(file, "timestamp,date,provider,market_state,risk_overlay,system_confidence,stability_score,dominance_margin,potential_energy,regime_age,up_count,flat_count,down_count,total_count,up_weight,flat_weight,down_weight,total_weight,config_hash,data_quality_status")?;
        }

        writeln!(
            file,
            "{},{},{},{:?},{:?},{:.2},{:.2},{:.4},{:.4},{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{},{}",
            row.timestamp,
            row.date,
            row.provider,
            row.market_state,
            row.risk_overlay,
            row.system_confidence,
            row.stability_score,
            row.dominance_margin,
            row.potential_energy,
            row.regime_age,
            row.up_count,
            row.flat_count,
            row.down_count,
            row.total_count,
            row.up_weight,
            row.flat_weight,
            row.down_weight,
            row.total_weight,
            row.config_hash,
            row.data_quality_status
        )?;

        Ok(())
    }

    pub fn save_run_status(&self, outcome: &crate::core::run_status::RunOutcome) -> Result<()> {
        let filename = format!("run_status_{}.json", outcome.date);
        let path = self.save_dir.join(filename);
        let json =
            serde_json::to_string_pretty(outcome).context("Failed to serialize run outcome")?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::market_regime::{
        LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
    };
    use crate::core::portfolio_policy::PortfolioPolicy;
    use crate::core::{action_matrix::AssetActionDecision, execution_gate::ExecutionResult};
    use chrono::NaiveDate;
    use chrono::Utc;
    use std::fs;

    #[test]
    fn test_persistence_roundtrip() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_sentinel_persist_{}", Utc::now().timestamp()));
        fs::create_dir_all(&temp_dir).unwrap();

        let layer = PersistenceLayer::new(&temp_dir);

        let market = MarketRegimeSnapshot {
            market_state: MarketState::ESTABLISHED,
            lifecycle_state: LifecycleState::ESTABLISHED,
            risk_overlay: RiskOverlay::NORMAL,
            reasons: vec!["Test reason".to_string()],
            low_stability_streak: 0,
            duration_in_state: 1,
            transition_audit: None,
        };
        let policy = PortfolioPolicy::from_market_regime(&market);
        let features = crate::core::features::MarketFeatures {
            date: Utc::now().date_naive(),
            regime_age: 1,
            potential_energy: 0.5,
            system_confidence: 80.0,
            ..crate::core::features::MarketFeatures::default()
        };
        let packet = DecisionPacket::new(Utc::now().date_naive(), features, market, policy, vec![]);

        layer.save_packet(&packet).unwrap();

        let loaded = layer.load_latest_packet().unwrap().unwrap();
        assert_eq!(loaded.market_regime.market_state, MarketState::ESTABLISHED);
        assert_eq!(loaded.date, packet.date);

        // Cleanup
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_markdown_report_saving() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_sentinel_report_{}", Utc::now().timestamp()));
        fs::create_dir_all(&temp_dir).unwrap();

        let layer = PersistenceLayer::new(&temp_dir);
        let content = "# Test Report\nContent";
        let date = "2023-01-01";

        layer.save_markdown_report(content, date).unwrap();

        let report_path = temp_dir.join("2023-01-01.md");
        assert!(report_path.exists());
        let saved_content = fs::read_to_string(report_path).unwrap();
        assert_eq!(saved_content, content);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_save_execution_gate_result_writes_noop_summary_when_audits_empty() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_gate_noop_{}",
            Utc::now().timestamp()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        let layer = PersistenceLayer::new(&temp_dir);
        let market = MarketRegimeSnapshot {
            market_state: MarketState::IGNITION,
            lifecycle_state: LifecycleState::IGNITION,
            risk_overlay: RiskOverlay::NORMAL,
            reasons: vec![],
            low_stability_streak: 0,
            duration_in_state: 1,
            transition_audit: None,
        };
        let packet = DecisionPacket::new(
            NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            crate::core::features::MarketFeatures::default(),
            market.clone(),
            PortfolioPolicy::from_market_regime(&market),
            Vec::<AssetActionDecision>::new(),
        );
        let result = ExecutionResult {
            trades: vec![],
            audits: vec![],
        };

        layer.save_execution_gate_result(&packet, &result).unwrap();

        let path = temp_dir.join("execution_gate_log.jsonl");
        assert!(path.exists());
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("execution_gate_noop"));

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
