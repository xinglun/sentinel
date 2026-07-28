use crate::features::radar::application::execution_gate::ExecutionResult;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::leader_persistence::LeaderObservation;
use crate::features::radar::domain::observation_timeline::{
    ObservationTimeline, ObservationTimelineEntry,
};
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviousSnapshotStatus {
    Available,
    BaselineUnavailable,
}

#[derive(Debug, Clone)]
pub struct PreviousSnapshotResolution {
    pub status: PreviousSnapshotStatus,
    pub current_market_date: chrono::NaiveDate,
    pub previous_market_date: Option<chrono::NaiveDate>,
    pub snapshot: Option<DecisionPacket>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservationHistoryState {
    pub count: usize,
    pub last_market_date: chrono::NaiveDate,
}

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
        let mut packets = self.load_all_packets()?;
        packets.retain(|existing| existing.date != packet.date);
        packets.push(packet.clone());
        packets.sort_by_key(|value| value.date);

        let mut file =
            File::create(&self.history_path).context("Failed to rewrite decision_history.jsonl")?;
        for value in packets {
            writeln!(file, "{}", serde_json::to_string(&value)?)
                .context("Failed to write packet to jsonl")?;
        }
        Ok(())
    }

    pub fn load_leader_observations(&self) -> Result<Vec<LeaderObservation>> {
        let path = self.save_dir.join("leader_observations.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(path).context("Failed to open leader_observations.jsonl")?;
        let mut by_date = std::collections::BTreeMap::new();
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let observation: LeaderObservation =
                serde_json::from_str(&line).context("Failed to deserialize leader observation")?;
            by_date.insert(observation.date, observation);
        }
        Ok(by_date.into_values().collect())
    }

    pub fn save_leader_observation(&self, observation: &LeaderObservation) -> Result<()> {
        let mut observations = self.load_leader_observations()?;
        observations.retain(|existing| existing.date != observation.date);
        observations.push(observation.clone());
        observations.sort_by_key(|value| value.date);
        let path = self.save_dir.join("leader_observations.jsonl");
        let mut file = File::create(path).context("Failed to rewrite leader_observations.jsonl")?;
        for value in observations {
            writeln!(file, "{}", serde_json::to_string(&value)?)?;
        }
        Ok(())
    }

    pub fn load_latest_observation_timeline(&self) -> Result<Option<ObservationTimeline>> {
        let path = self.save_dir.join("observation_timeline_latest.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)
            .context("Failed to read observation_timeline_latest.json")?;
        Ok(Some(
            serde_json::from_str(&content).context("Failed to deserialize observation timeline")?,
        ))
    }

    pub fn save_observation_timeline(
        &self,
        timeline: &ObservationTimeline,
        date: &str,
    ) -> Result<()> {
        let json = serde_json::to_string_pretty(timeline)
            .context("Failed to serialize observation timeline")?;
        std::fs::write(
            self.save_dir.join("observation_timeline_latest.json"),
            &json,
        )
        .context("Failed to write latest observation timeline")?;
        std::fs::write(
            self.save_dir
                .join(format!("observation_timeline_{date}.json")),
            &json,
        )
        .context("Failed to write dated observation timeline")?;
        let history_path = self.save_dir.join("observation_timeline.jsonl");
        let mut timelines = if history_path.exists() {
            BufReader::new(
                File::open(&history_path).context("Failed to open observation_timeline.jsonl")?,
            )
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<ObservationTimeline>(&line)
                    .context("Failed to deserialize observation timeline history")
            })
            .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        timelines.retain(|existing| {
            existing
                .entries
                .iter()
                .map(|entry| entry.date.to_string())
                .max()
                .as_deref()
                != Some(date)
        });
        timelines.push(timeline.clone());
        let mut file =
            File::create(history_path).context("Failed to rewrite observation_timeline.jsonl")?;
        for value in timelines {
            writeln!(file, "{}", serde_json::to_string(&value)?)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn save_observation_timeline_entry(
        &self,
        entry: ObservationTimelineEntry,
        expected_trading_dates: &[chrono::NaiveDate],
    ) -> Result<ObservationTimeline> {
        let mut entries = self.load_observation_history_entries()?;
        entries.retain(|existing| existing.date != entry.date);
        entries.push(entry);
        let timeline =
            crate::features::radar::domain::observation_timeline::build_observation_timeline(
                &entries,
                expected_trading_dates,
            );
        self.save_observation_timeline(&timeline, &entries.last().unwrap().date.to_string())?;
        Ok(timeline)
    }

    /// 正式な cycle history を trading date 単位で一意に読み込む。
    pub fn load_observation_history_entries(&self) -> Result<Vec<ObservationTimelineEntry>> {
        let path = self.save_dir.join("observation_timeline.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(path).context("Failed to open observation_timeline.jsonl")?;
        let mut by_date = std::collections::BTreeMap::new();
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let timeline: ObservationTimeline = serde_json::from_str(&line)
                .context("Failed to deserialize observation timeline history")?;
            for entry in timeline.entries {
                by_date.insert(entry.date, entry);
            }
        }
        Ok(by_date.into_values().collect())
    }

    pub fn load_observation_history_state(&self) -> Result<Option<ObservationHistoryState>> {
        let path = self.save_dir.join("observation_history_state.json");
        if !path.exists() {
            return Ok(None);
        }
        let value = serde_json::from_str(
            &std::fs::read_to_string(path).context("Failed to read observation history state")?,
        )
        .context("Failed to deserialize observation history state")?;
        Ok(Some(value))
    }

    pub fn save_observation_history_state(&self, state: &ObservationHistoryState) -> Result<()> {
        let path = self.save_dir.join("observation_history_state.json");
        let json = serde_json::to_string_pretty(state)
            .context("Failed to serialize observation history state")?;
        std::fs::write(path, json).context("Failed to write observation history state")?;
        Ok(())
    }

    pub fn load_latest_packet(&self) -> Result<Option<DecisionPacket>> {
        let recent = self.load_recent_packets(1)?;
        Ok(recent.into_iter().next())
    }

    /// 履歴 log から直近 N 件の packet を読み込む。
    /// packet は古い順の時系列で返す。
    pub fn load_recent_packets(&self, count: usize) -> Result<Vec<DecisionPacket>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let packets = self.load_all_packets()?;
        Ok(packets
            .into_iter()
            .rev()
            .take(count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect())
    }

    /// 指定日より前の packet を業務日順に返す。
    pub fn load_recent_packets_before(
        &self,
        as_of_date: chrono::NaiveDate,
        count: usize,
    ) -> Result<Vec<DecisionPacket>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let packets = self
            .load_all_packets()?
            .into_iter()
            .filter(|packet| packet.date < as_of_date)
            .collect::<Vec<_>>();
        Ok(packets
            .into_iter()
            .rev()
            .take(count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect())
    }

    /// 現在日付に対する前回有効スナップショットを一度だけ解決する。
    pub fn resolve_previous_snapshot(
        &self,
        current_market_date: chrono::NaiveDate,
        previous_market_date: Option<chrono::NaiveDate>,
    ) -> Result<PreviousSnapshotResolution> {
        let Some(previous_market_date) = previous_market_date else {
            return Ok(PreviousSnapshotResolution {
                status: PreviousSnapshotStatus::BaselineUnavailable,
                current_market_date,
                previous_market_date: None,
                snapshot: None,
                reason: Some("previous trading date is unavailable".to_string()),
            });
        };
        let snapshot = self
            .load_all_packets()?
            .into_iter()
            .find(|packet| packet.date == previous_market_date);
        let status = if snapshot.is_some() {
            PreviousSnapshotStatus::Available
        } else {
            PreviousSnapshotStatus::BaselineUnavailable
        };
        let reason = snapshot
            .as_ref()
            .is_none()
            .then(|| "previous trading-day snapshot is unavailable".to_string());
        Ok(PreviousSnapshotResolution {
            status,
            current_market_date,
            previous_market_date: Some(previous_market_date),
            snapshot,
            reason,
        })
    }

    fn load_all_packets(&self) -> Result<Vec<DecisionPacket>> {
        if !self.history_path.exists() {
            return Ok(Vec::new());
        }

        let file =
            File::open(&self.history_path).context("Failed to open decision_history.jsonl")?;
        let reader = BufReader::new(file);
        let mut by_date = std::collections::BTreeMap::new();
        for line in reader.lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let packet: DecisionPacket = serde_json::from_str(&line)
                .context("Failed to deserialize DecisionPacket from history")?;
            by_date.insert(packet.date, packet);
        }
        Ok(by_date.into_values().collect())
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

    pub fn save_portfolio_snapshot<T: serde::Serialize>(
        &self,
        snapshot: &T,
        date: &str,
    ) -> Result<()> {
        let filename = format!("portfolio_snapshot_{}.json", date);
        let path = self.save_dir.join(filename);
        let json = serde_json::to_string_pretty(snapshot)
            .context("Failed to serialize portfolio snapshot")?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn save_account_snapshot<T: serde::Serialize>(
        &self,
        snapshot: &T,
        date: &str,
    ) -> Result<()> {
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

    pub fn save_data_quality_log<T: serde::Serialize>(&self, log: &T) -> Result<()> {
        let path = self.save_dir.join("data_quality_log.jsonl");
        let json = serde_json::to_string(log).context("Failed to serialize data quality log")?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }

    pub fn save_telemetry(
        &self,
        row: &crate::features::radar::application::telemetry::TelemetryRow,
    ) -> Result<()> {
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

    pub fn save_run_status(
        &self,
        outcome: &crate::features::shared::application::run_status::RunOutcome,
    ) -> Result<()> {
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
    use crate::features::radar::application::execution_gate::ExecutionResult;
    use crate::features::radar::domain::action_matrix::AssetActionDecision;
    use crate::features::radar::domain::market_regime::{
        LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
    };
    use crate::features::radar::domain::portfolio_policy::PortfolioPolicy;
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
        let features = crate::features::radar::domain::features::MarketFeatures {
            date: Utc::now().date_naive(),
            regime_age: 1,
            potential_energy: 0.5,
            system_confidence: 80.0,
            ..crate::features::radar::domain::features::MarketFeatures::default()
        };
        let packet = DecisionPacket::new(
            Utc::now().date_naive(),
            features,
            market,
            None,
            policy,
            vec![],
            Vec::new(),
            false,
            crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot::default(),
            None,
            None,
        );

        layer.save_packet(&packet).unwrap();

        let loaded = layer.load_latest_packet().unwrap().unwrap();
        assert_eq!(loaded.market_regime.market_state, MarketState::ESTABLISHED);
        assert_eq!(loaded.date, packet.date);

        // 後片付け。
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn previous_snapshot_resolution_uses_previous_valid_trading_day() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_previous_snapshot_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let current = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();

        let resolution = layer
            .resolve_previous_snapshot(current, Some(NaiveDate::from_ymd_opt(2026, 7, 3).unwrap()))
            .unwrap();

        assert_eq!(resolution.current_market_date, current);
        assert_eq!(
            resolution.previous_market_date,
            Some(NaiveDate::from_ymd_opt(2026, 7, 3).unwrap())
        );
        assert_eq!(
            resolution.status,
            PreviousSnapshotStatus::BaselineUnavailable
        );
        assert!(resolution.snapshot.is_none());
    }

    #[test]
    fn decision_history_upserts_same_trading_date_without_duplicate_lines() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_packet_upsert_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();

        layer
            .save_packet(&DecisionPacket {
                date,
                top_tier_symbols: vec!["OLD".to_string()],
                ..Default::default()
            })
            .unwrap();
        layer
            .save_packet(&DecisionPacket {
                date,
                top_tier_symbols: vec!["NEW".to_string()],
                ..Default::default()
            })
            .unwrap();

        let lines = fs::read_to_string(temp_dir.join("decision_history.jsonl"))
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        assert_eq!(lines, 1);
        assert_eq!(
            layer
                .load_latest_packet()
                .unwrap()
                .unwrap()
                .top_tier_symbols,
            ["NEW"]
        );

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_markdown_report_saving() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_report_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
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
    fn leader_observation_upsert_deduplicates_date() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_leader_observation_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let first = LeaderObservation {
            date,
            leader: "GOOG".to_string(),
            confidence: Some(80.0),
            breadth: Some(70.0),
            relative_strength: Some(75.0),
            rotation_stability: Some(80.0),
            sector_or_index_rotation: None,
            supply_state: None,
        };
        let mut replacement = first.clone();
        replacement.leader = "MSFT".to_string();
        layer.save_leader_observation(&first).unwrap();
        layer.save_leader_observation(&replacement).unwrap();
        let loaded = layer.load_leader_observations().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].leader, "MSFT");
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn recent_packets_before_deduplicates_and_sorts_by_business_date() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_packet_date_order_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date_a = NaiveDate::from_ymd_opt(2026, 7, 8).unwrap();
        let date_b = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();
        let date_c = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        for date in [date_c, date_a, date_b, date_b] {
            layer
                .save_packet(&DecisionPacket {
                    date,
                    ..DecisionPacket::default()
                })
                .unwrap();
        }

        let packets = layer
            .load_recent_packets_before(date_c + chrono::Duration::days(1), 10)
            .unwrap();

        assert_eq!(
            packets.iter().map(|packet| packet.date).collect::<Vec<_>>(),
            vec![date_a, date_b, date_c]
        );
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn observation_timeline_writes_latest_dated_and_jsonl_artifacts() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_observation_timeline_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let timeline = ObservationTimeline {
            history_coverage:
                crate::features::radar::domain::observation_timeline::HistoryCoverage::Partial,
            entries: vec![ObservationTimelineEntry {
                date,
                primary_leader: "SPY".to_string(),
                secondary_leaders: vec![],
                breadth_score: 50.0,
                concentration_score: 70.0,
                rotation_score: 20.0,
                confidence_index: 55.0,
                market_state: "RANGE".to_string(),
                supply_phase: "WATCH".to_string(),
                risk_state: "NORMAL".to_string(),
                day_type: "NORMAL".to_string(),
            }],
            summary: "NO_STRUCTURAL_CHANGE".to_string(),
        };

        layer
            .save_observation_timeline(&timeline, &date.to_string())
            .unwrap();

        assert!(temp_dir.join("observation_timeline_latest.json").exists());
        assert!(temp_dir
            .join("observation_timeline_2026-07-10.json")
            .exists());
        assert!(temp_dir.join("observation_timeline.jsonl").exists());
        assert_eq!(
            layer
                .load_latest_observation_timeline()
                .unwrap()
                .unwrap()
                .entries
                .len(),
            1
        );
        let archive =
            fs::read_to_string(temp_dir.join("observation_timeline_latest.json")).unwrap();
        assert!(!archive.contains("过去 7 个交易日"));
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn observation_timeline_upserts_same_trading_date_without_duplicate_events() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_observation_timeline_upsert_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let timeline = ObservationTimeline {
            history_coverage:
                crate::features::radar::domain::observation_timeline::HistoryCoverage::Partial,
            entries: vec![ObservationTimelineEntry {
                date,
                primary_leader: "SPY".to_string(),
                secondary_leaders: vec![],
                breadth_score: 50.0,
                concentration_score: 70.0,
                rotation_score: 20.0,
                confidence_index: 55.0,
                market_state: "RANGE".to_string(),
                supply_phase: "IDLE".to_string(),
                risk_state: "NORMAL".to_string(),
                day_type: "NORMAL".to_string(),
            }],
            summary: "NO_STRUCTURAL_CHANGE".to_string(),
        };

        layer
            .save_observation_timeline(&timeline, &date.to_string())
            .unwrap();
        layer
            .save_observation_timeline(&timeline, &date.to_string())
            .unwrap();

        let lines = fs::read_to_string(temp_dir.join("observation_timeline.jsonl"))
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        assert_eq!(lines, 1);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn observation_history_keeps_all_valid_cycle_dates_beyond_display_window() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_timeline_full_history_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let start = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();

        for offset in 0..8 {
            let date = start + chrono::Duration::days(offset);
            layer
                .save_observation_timeline_entry(
                    ObservationTimelineEntry {
                        date,
                        primary_leader: format!("L{offset}"),
                        secondary_leaders: Vec::new(),
                        breadth_score: offset as f64,
                        concentration_score: 0.0,
                        rotation_score: 0.0,
                        confidence_index: 0.0,
                        market_state: "STARTUP".to_string(),
                        supply_phase: "UNAVAILABLE".to_string(),
                        risk_state: "NORMAL".to_string(),
                        day_type: "NORMAL".to_string(),
                    },
                    &[date],
                )
                .unwrap();
        }

        let history = layer.load_observation_history_entries().unwrap();
        assert_eq!(history.len(), 8);
        assert_eq!(history.first().unwrap().date, start);
        assert_eq!(
            history.last().unwrap().date,
            start + chrono::Duration::days(7)
        );

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_save_execution_gate_result_writes_noop_summary_when_audits_empty() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_gate_noop_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
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
            crate::features::radar::domain::features::MarketFeatures::default(),
            market.clone(),
            None,
            PortfolioPolicy::from_market_regime(&market),
            Vec::<AssetActionDecision>::new(),
            Vec::new(),
            false,
            crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot::default(),
            None,
            None,
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
