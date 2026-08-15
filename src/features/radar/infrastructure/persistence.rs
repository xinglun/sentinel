mod atomic;
mod migration;
mod model;

use atomic::{write_file_atomically, HistoryWriteTransaction};
pub(crate) use model::PriceVolumeObservationRecord;
pub use model::{
    ObservationHistoryState, PreviousSnapshotResolution, PreviousSnapshotStatus,
    TradingDaySnapshot, TradingDaySnapshotWriteDisposition,
};

use crate::features::radar::application::execution_gate::ExecutionResult;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::leader_persistence::LeaderObservation;
use crate::features::radar::domain::observation_timeline::{
    ObservationTimeline, ObservationTimelineEntry,
};
use crate::features::radar::domain::price_volume_structure::PriceVolumeStructure;
use anyhow::{bail, Context, Result};
use chrono::Datelike;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct PersistenceLayer {
    history_path: PathBuf,
    save_dir: PathBuf,
}

impl PersistenceLayer {
    /// 同じ構造が相隣取引日に続く日数だけを数え、欠損や構造変更は継続とみなさない。
    pub(crate) fn next_price_volume_persistence_days(
        &self,
        market_date: chrono::NaiveDate,
        symbol: &str,
        structure: PriceVolumeStructure,
    ) -> Result<u8> {
        let mut records = self
            .load_price_volume_observations()?
            .into_iter()
            .filter(|record| record.symbol == symbol && record.market_date < market_date)
            .collect::<Vec<_>>();
        records.sort_by_key(|record| std::cmp::Reverse(record.market_date));
        let mut previous_date = market_date;
        let mut days = 1_u8;
        for record in records {
            if !follows_previous_trading_day(record.market_date, previous_date) {
                break;
            }
            if record.assessment.structure != structure {
                break;
            }
            days = days.saturating_add(1);
            previous_date = record.market_date;
        }
        Ok(days)
    }

    /// 直近の吸収観測の翌取引日に分配構造へ移った場合だけ、吸収失敗を解釈として残す。
    pub(crate) fn is_accumulation_failed(
        &self,
        market_date: chrono::NaiveDate,
        symbol: &str,
        structure: PriceVolumeStructure,
    ) -> Result<bool> {
        if structure != PriceVolumeStructure::Distribution {
            return Ok(false);
        }
        Ok(self
            .load_price_volume_observations()?
            .into_iter()
            .filter(|record| record.symbol == symbol && record.market_date < market_date)
            .max_by_key(|record| record.market_date)
            .is_some_and(|record| {
                follows_previous_trading_day(record.market_date, market_date)
                    && record.assessment.structure == PriceVolumeStructure::Accumulation
            }))
    }

    pub(crate) fn load_price_volume_observations(
        &self,
    ) -> Result<Vec<PriceVolumeObservationRecord>> {
        let path = self.save_dir.join("price_volume_observations.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut records = std::collections::BTreeMap::new();
        for line in
            BufReader::new(File::open(path).context("Failed to open price volume observations")?)
                .lines()
                .map_while(Result::ok)
        {
            if line.trim().is_empty() {
                continue;
            }
            let record: PriceVolumeObservationRecord = serde_json::from_str(&line)
                .context("Failed to deserialize price volume observation")?;
            records.insert((record.market_date, record.symbol.clone()), record);
        }
        Ok(records.into_values().collect())
    }

    pub(crate) fn save_price_volume_observations(
        &self,
        observations: &[PriceVolumeObservationRecord],
    ) -> Result<()> {
        let mut records = self
            .load_price_volume_observations()?
            .into_iter()
            .map(|record| ((record.market_date, record.symbol.clone()), record))
            .collect::<std::collections::BTreeMap<_, _>>();
        for observation in observations {
            records.insert(
                (observation.market_date, observation.symbol.clone()),
                observation.clone(),
            );
        }
        let content = records
            .into_values()
            .map(|record| serde_json::to_string(&record))
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n");
        let content = if content.is_empty() {
            content
        } else {
            format!("{content}\n")
        };
        write_file_atomically(
            &self.save_dir.join("price_volume_observations.jsonl"),
            content.as_bytes(),
        )
        .context("Failed to save price volume observations")
    }
    /// 市場事実として保存された取引日快照は、基線可用性が低くても次回の比較対象にする。
    fn is_historical_snapshot(snapshot: &TradingDaySnapshot) -> bool {
        snapshot.is_valid_trading_day
            && matches!(snapshot.source_status.as_str(), "complete" | "degraded")
    }

    fn trading_day_snapshot_semantics(snapshot: &TradingDaySnapshot) -> serde_json::Value {
        let mut value = serde_json::to_value(snapshot).expect("TradingDaySnapshot is serializable");
        if let Some(object) = value.as_object_mut() {
            object.remove("generated_at");
            object.remove("run_id");
            object.remove("snapshot_id");
        }
        value
    }

    pub fn new(save_dir: &Path) -> Self {
        Self {
            history_path: save_dir.join("decision_history.jsonl"),
            save_dir: save_dir.to_path_buf(),
        }
    }

    pub(crate) fn begin_history_write_transaction(
        &self,
        market_date: chrono::NaiveDate,
        cycle_id: &str,
    ) -> Result<HistoryWriteTransaction> {
        let date = market_date.to_string();
        let mut paths = vec![
            self.save_dir.join("decision_history.jsonl"),
            self.save_dir.join("state_transitions.csv"),
            self.save_dir.join("state_transitions.jsonl"),
            self.save_dir
                .join("decision_snapshots")
                .join(format!("{date}.json")),
            self.save_dir.join("leader_observations.jsonl"),
            self.save_dir
                .join("leader_snapshots")
                .join(format!("{date}.json")),
            self.save_dir.join("observation_timeline.jsonl"),
            self.save_dir.join("observation_timeline_latest.json"),
            self.save_dir
                .join(format!("observation_timeline_{date}.json")),
            self.save_dir
                .join("timeline_snapshots")
                .join(format!("{date}.json")),
            self.save_dir.join("observation_history_state.json"),
            self.save_dir
                .join("snapshots")
                .join(format!("{cycle_id}_{date}.json")),
        ];
        if self.save_dir.is_dir() {
            for entry in
                std::fs::read_dir(&self.save_dir).context("履歴 legacy 文件の読み込みに失敗")?
            {
                let path = entry.context("履歴 legacy 文件の読み込みに失敗")?.path();
                let is_legacy_transition = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("state_transitions_legacy_") && name.ends_with(".csv")
                    });
                if is_legacy_transition {
                    paths.push(path);
                }
            }
        }
        let files = paths
            .into_iter()
            .map(|path| {
                let content = path
                    .exists()
                    .then(|| std::fs::read(&path))
                    .transpose()
                    .with_context(|| format!("履歴トランザクションの状態取得に失敗: {path:?}"))?;
                Ok((path, content))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(HistoryWriteTransaction {
            files,
            committed: false,
        })
    }

    pub fn save_packet(&self, packet: &DecisionPacket) -> Result<()> {
        let dir = self.save_dir.join("decision_snapshots");
        std::fs::create_dir_all(&dir).context("Failed to create decision snapshot directory")?;
        let snapshot_path = dir.join(format!("{}.json", packet.date));
        let json = serde_json::to_string_pretty(packet)
            .context("Failed to serialize decision packet snapshot")?;
        std::fs::write(snapshot_path, json).context("Failed to write decision packet snapshot")?;

        if !self.legacy_packet_dates()?.contains(&packet.date) {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.history_path)
                .context("Failed to open decision_history.jsonl")?;
            writeln!(file, "{}", serde_json::to_string(packet)?)
                .context("Failed to append packet to jsonl")?;
        }
        Ok(())
    }

    fn legacy_packet_dates(&self) -> Result<std::collections::BTreeSet<chrono::NaiveDate>> {
        if !self.history_path.exists() {
            return Ok(std::collections::BTreeSet::new());
        }
        let file =
            File::open(&self.history_path).context("Failed to open decision_history.jsonl")?;
        let mut dates = std::collections::BTreeSet::new();
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let packet: DecisionPacket = serde_json::from_str(&line)
                .context("Failed to deserialize DecisionPacket from history")?;
            dates.insert(packet.date);
        }
        Ok(dates)
    }

    pub fn load_leader_observations(&self) -> Result<Vec<LeaderObservation>> {
        let path = self.save_dir.join("leader_observations.jsonl");
        let mut by_date = std::collections::BTreeMap::new();
        if path.exists() {
            let file = File::open(path).context("Failed to open leader_observations.jsonl")?;
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                let observation: LeaderObservation = serde_json::from_str(&line)
                    .context("Failed to deserialize leader observation")?;
                by_date.insert(observation.date, observation);
            }
        }
        let dir = self.save_dir.join("leader_snapshots");
        if dir.exists() {
            for file in std::fs::read_dir(dir).context("Failed to read leader snapshots")? {
                let path = file?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("json") {
                    continue;
                }
                let observation: LeaderObservation = serde_json::from_str(
                    &std::fs::read_to_string(path).context("Failed to read leader snapshot")?,
                )
                .context("Failed to deserialize leader snapshot")?;
                by_date.insert(observation.date, observation);
            }
        }
        Ok(by_date.into_values().collect())
    }

    pub fn save_leader_observation(&self, observation: &LeaderObservation) -> Result<()> {
        let dir = self.save_dir.join("leader_snapshots");
        std::fs::create_dir_all(&dir).context("Failed to create leader snapshot directory")?;
        let json = serde_json::to_string_pretty(observation)
            .context("Failed to serialize leader snapshot")?;
        std::fs::write(dir.join(format!("{}.json", observation.date)), json)
            .context("Failed to write leader snapshot")?;

        let path = self.save_dir.join("leader_observations.jsonl");
        let has_date = if path.exists() {
            BufReader::new(File::open(&path).context("Failed to open leader_observations.jsonl")?)
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| serde_json::from_str::<LeaderObservation>(&line).ok())
                .any(|value| value.date == observation.date)
        } else {
            false
        };
        if !has_date {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .context("Failed to open leader_observations.jsonl")?;
            writeln!(file, "{}", serde_json::to_string(observation)?)?;
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
        write_file_atomically(
            &self.save_dir.join("observation_timeline_latest.json"),
            json.as_bytes(),
        )
        .context("Failed to write latest observation timeline")?;
        write_file_atomically(
            &self
                .save_dir
                .join(format!("observation_timeline_{date}.json")),
            json.as_bytes(),
        )
        .context("Failed to write dated observation timeline")?;
        let dir = self.save_dir.join("timeline_snapshots");
        std::fs::create_dir_all(&dir).context("Failed to create timeline snapshot directory")?;
        write_file_atomically(&dir.join(format!("{date}.json")), json.as_bytes())
            .context("Failed to write timeline snapshot")?;

        let history_path = self.save_dir.join("observation_timeline.jsonl");
        if !self.legacy_timeline_dates()?.contains(date) {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(history_path)
                .context("Failed to open observation_timeline.jsonl")?;
            writeln!(file, "{}", serde_json::to_string(timeline)?)?;
        }
        Ok(())
    }

    fn legacy_timeline_dates(&self) -> Result<std::collections::BTreeSet<String>> {
        let path = self.save_dir.join("observation_timeline.jsonl");
        if !path.exists() {
            return Ok(std::collections::BTreeSet::new());
        }
        let file = File::open(path).context("Failed to open observation_timeline.jsonl")?;
        let mut dates = std::collections::BTreeSet::new();
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let timeline: ObservationTimeline = serde_json::from_str(&line)
                .context("Failed to deserialize observation timeline history")?;
            if let Some(date) = timeline.entries.iter().map(|entry| entry.date).max() {
                dates.insert(date.to_string());
            }
        }
        Ok(dates)
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
        let mut by_date = std::collections::BTreeMap::new();
        let legacy_path = self.save_dir.join("observation_timeline.jsonl");
        if legacy_path.exists() {
            let file =
                File::open(legacy_path).context("Failed to open observation_timeline.jsonl")?;
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
        }
        let dir = self.save_dir.join("timeline_snapshots");
        if dir.exists() {
            for file in std::fs::read_dir(dir).context("Failed to read timeline snapshots")? {
                let path = file?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("json") {
                    continue;
                }
                let timeline: ObservationTimeline = serde_json::from_str(
                    &std::fs::read_to_string(path).context("Failed to read timeline snapshot")?,
                )
                .context("Failed to deserialize timeline snapshot")?;
                for entry in timeline.entries {
                    by_date.insert(entry.date, entry);
                }
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

    pub fn save_trading_day_snapshot(
        &self,
        snapshot: &TradingDaySnapshot,
    ) -> Result<TradingDaySnapshotWriteDisposition> {
        let dir = self.save_dir.join("snapshots");
        std::fs::create_dir_all(&dir).context("Failed to create trading-day snapshot directory")?;
        let path = dir.join(format!(
            "{}_{}.json",
            snapshot.cycle_id, snapshot.market_date
        ));
        let disposition = self.validate_trading_day_snapshot_conflict(snapshot)?;
        let json = serde_json::to_string_pretty(snapshot)
            .context("Failed to serialize trading-day snapshot")?;
        std::fs::write(path, json).context("Failed to write trading-day snapshot")?;
        Ok(disposition)
    }

    /// 正式履歴を書き込む前に、同一キーの快照が意味的に一致するか検証する。
    pub fn validate_trading_day_snapshot_conflict(
        &self,
        snapshot: &TradingDaySnapshot,
    ) -> Result<TradingDaySnapshotWriteDisposition> {
        let path = self.save_dir.join("snapshots").join(format!(
            "{}_{}.json",
            snapshot.cycle_id, snapshot.market_date
        ));
        if path.exists() {
            let existing: TradingDaySnapshot = serde_json::from_str(
                &std::fs::read_to_string(&path).context("Failed to read trading-day snapshot")?,
            )
            .context("Failed to deserialize existing trading-day snapshot")?;
            if Self::trading_day_snapshot_semantics(&existing)
                != Self::trading_day_snapshot_semantics(snapshot)
            {
                bail!("SNAPSHOT_CONFLICT");
            }
            return Ok(TradingDaySnapshotWriteDisposition::SameDayRerun);
        }
        Ok(TradingDaySnapshotWriteDisposition::Created)
    }

    pub fn load_trading_day_snapshots(&self) -> Result<Vec<TradingDaySnapshot>> {
        let dir = self.save_dir.join("snapshots");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut snapshots = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(dir).context("Failed to read trading-day snapshots")? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let snapshot: TradingDaySnapshot = serde_json::from_str(
                &std::fs::read_to_string(path).context("Failed to read trading-day snapshot")?,
            )
            .context("Failed to deserialize trading-day snapshot")?;
            let key = (snapshot.cycle_id.clone(), snapshot.market_date);
            if let Some(existing) = snapshots.insert(key, snapshot.clone()) {
                if Self::trading_day_snapshot_semantics(&existing)
                    != Self::trading_day_snapshot_semantics(&snapshot)
                {
                    bail!("SNAPSHOT_CONFLICT");
                }
            }
        }
        Ok(snapshots.into_values().collect())
    }

    pub fn latest_cycle_id_before(
        &self,
        current_market_date: chrono::NaiveDate,
    ) -> Result<Option<String>> {
        Ok(self
            .load_trading_day_snapshots()?
            .into_iter()
            .filter(|snapshot| {
                Self::is_historical_snapshot(snapshot)
                    && snapshot.market_date < current_market_date
                    && !snapshot.cycle_id.is_empty()
            })
            .max_by_key(|snapshot| snapshot.market_date)
            .map(|snapshot| snapshot.cycle_id))
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
                previous_snapshot_id: None,
                gap_type: None,
                is_same_cycle: false,
                snapshot: None,
                reason: Some("previous trading date is unavailable".to_string()),
                formal_snapshot: None,
            });
        };
        let formal_snapshot = self
            .load_trading_day_snapshots()?
            .into_iter()
            .find(|snapshot| {
                snapshot.market_date == previous_market_date
                    && Self::is_historical_snapshot(snapshot)
            });
        let snapshot = formal_snapshot.as_ref().and_then(|formal| {
            self.load_all_packets()
                .ok()?
                .into_iter()
                .find(|packet| packet.date == formal.market_date)
        });
        Ok(PreviousSnapshotResolution {
            status: if formal_snapshot.is_some() {
                PreviousSnapshotStatus::Available
            } else {
                PreviousSnapshotStatus::BaselineUnavailable
            },
            current_market_date,
            previous_market_date: Some(previous_market_date),
            previous_snapshot_id: None,
            gap_type: Some("TRADING_DAY_GAP".to_string()),
            is_same_cycle: false,
            reason: formal_snapshot
                .as_ref()
                .is_none()
                .then(|| "previous trading-day snapshot is unavailable".to_string()),
            snapshot,
            formal_snapshot,
        })
    }

    pub fn resolve_previous_snapshot_from_history(
        &self,
        current_market_date: chrono::NaiveDate,
        current_cycle_id: Option<&str>,
    ) -> Result<PreviousSnapshotResolution> {
        let snapshot_history = self.load_trading_day_snapshots()?;
        let previous_snapshot = snapshot_history
            .iter()
            .filter(|snapshot| {
                Self::is_historical_snapshot(snapshot)
                    && snapshot.market_date < current_market_date
                    && current_cycle_id.is_some_and(|cycle_id| snapshot.cycle_id == cycle_id)
            })
            .max_by_key(|snapshot| snapshot.market_date);
        let previous_market_date = previous_snapshot.map(|snapshot| snapshot.market_date);
        let Some(previous_market_date) = previous_market_date else {
            return Ok(PreviousSnapshotResolution {
                status: PreviousSnapshotStatus::BaselineUnavailable,
                current_market_date,
                previous_market_date: None,
                previous_snapshot_id: None,
                gap_type: None,
                is_same_cycle: false,
                snapshot: None,
                reason: Some("previous trading date is unavailable".to_string()),
                formal_snapshot: None,
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
            previous_snapshot_id: previous_snapshot.map(|snapshot| snapshot.snapshot_id.clone()),
            gap_type: Some("TRADING_DAY_GAP".to_string()),
            is_same_cycle: current_cycle_id.is_some(),
            snapshot,
            reason,
            formal_snapshot: previous_snapshot.cloned(),
        })
    }

    fn load_all_packets(&self) -> Result<Vec<DecisionPacket>> {
        let mut by_date = std::collections::BTreeMap::new();
        if self.history_path.exists() {
            let file =
                File::open(&self.history_path).context("Failed to open decision_history.jsonl")?;
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                let packet: DecisionPacket = serde_json::from_str(&line)
                    .context("Failed to deserialize DecisionPacket from history")?;
                by_date.insert(packet.date, packet);
            }
        }
        let dir = self.save_dir.join("decision_snapshots");
        if dir.exists() {
            for file in std::fs::read_dir(dir).context("Failed to read decision snapshots")? {
                let path = file?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("json") {
                    continue;
                }
                let packet: DecisionPacket = serde_json::from_str(
                    &std::fs::read_to_string(path).context("Failed to read decision snapshot")?,
                )
                .context("Failed to deserialize decision snapshot")?;
                by_date.insert(packet.date, packet);
            }
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

/// 連続とは暦日連番、または金曜日の次の月曜日だけを指す。
fn follows_previous_trading_day(previous: chrono::NaiveDate, current: chrono::NaiveDate) -> bool {
    let elapsed_days = (current - previous).num_days();
    elapsed_days == 1
        || (previous.weekday() == chrono::Weekday::Fri
            && current.weekday() == chrono::Weekday::Mon
            && elapsed_days == 3)
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
    use crate::features::radar::domain::price_volume_structure::{
        BaselineType, ParticipationQuality, PriceVolumeAssessment, PriceVolumeObservationBoundary,
        PriceVolumeStructure, StructurePersistence, SupplyAbsorption, VolumeDataQuality,
    };
    use crate::features::shared::domain::supply_event_context::ObservationEffect;
    use chrono::NaiveDate;
    use chrono::Utc;
    use std::fs;

    #[test]
    fn persistence_model_boundary_preserves_history_state_contract() {
        let state = ObservationHistoryState {
            count: 3,
            last_market_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            cycle_id: "cycle-1".to_string(),
        };
        let encoded = serde_json::to_string(&state).unwrap();
        let restored: ObservationHistoryState = serde_json::from_str(&encoded).unwrap();

        assert_eq!(restored.count, state.count);
        assert_eq!(restored.last_market_date, state.last_market_date);
        assert_eq!(restored.cycle_id, state.cycle_id);
    }

    #[test]
    fn atomic_write_replaces_json_without_leaving_temporary_files() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_atomic_write_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("observation_timeline_2026-07-29.json");
        fs::write(&path, b"{\"stale\":true}\n").unwrap();

        write_file_atomically(&path, b"{\"history_coverage\":\"Partial\"}\n").unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{\"history_coverage\":\"Partial\"}\n"
        );
        assert_eq!(fs::read_dir(&temp_dir).unwrap().count(), 1);
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn price_volume_observations_upsert_by_market_date_and_symbol() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_price_volume_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 8, 7).unwrap();
        let assessment = |structure| PriceVolumeAssessment {
            structure,
            participation: ParticipationQuality::Neutral,
            supply_absorption: SupplyAbsorption::None,
            quality: VolumeDataQuality::Healthy,
            persistence: StructurePersistence::Candidate,
            persistence_days: 1,
            metrics: None,
            boundary: PriceVolumeObservationBoundary {
                decision_weight_percent: 0,
                trade_signal: false,
                gate_effect: ObservationEffect::None,
                execution_effect: ObservationEffect::None,
                position_sizing_effect: ObservationEffect::None,
            },
            secondary_metrics: None,
            observation_confidence: Default::default(),
            eligibility: Default::default(),
            primary_baseline: Default::default(),
            secondary_baseline: None,
            lifecycle: Default::default(),
            unavailable_reason: None,
            next_eligibility_condition: None,
        };
        layer
            .save_price_volume_observations(&[PriceVolumeObservationRecord {
                market_date: date,
                symbol: "MSFT".to_string(),
                assessment: assessment(PriceVolumeStructure::Neutral),
                supply_context: None,
                price_position: None,
                accumulation_failed: false,
            }])
            .unwrap();
        layer
            .save_price_volume_observations(&[PriceVolumeObservationRecord {
                market_date: date,
                symbol: "MSFT".to_string(),
                assessment: assessment(PriceVolumeStructure::ExhaustedAdvance),
                supply_context: None,
                price_position: None,
                accumulation_failed: false,
            }])
            .unwrap();
        let records = layer.load_price_volume_observations().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].assessment.structure,
            PriceVolumeStructure::ExhaustedAdvance
        );
        assert_eq!(records[0].supply_context, None);
        assert_eq!(records[0].price_position, None);
        assert!(!records[0].accumulation_failed);
        assert_eq!(records[0].assessment.boundary.decision_weight_percent, 0);
        assert!(!records[0].assessment.boundary.trade_signal);
        assert_eq!(
            records[0].assessment.boundary.gate_effect,
            ObservationEffect::None
        );
        assert_eq!(
            records[0].assessment.boundary.execution_effect,
            ObservationEffect::None
        );
        assert_eq!(
            records[0].assessment.boundary.position_sizing_effect,
            ObservationEffect::None
        );
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn price_volume_persistence_counts_only_adjacent_same_structure_days() {
        let temp_dir = tempfile::tempdir().unwrap();
        let layer = PersistenceLayer::new(temp_dir.path());
        let assessment = |structure| PriceVolumeAssessment {
            structure,
            participation: ParticipationQuality::Neutral,
            supply_absorption: SupplyAbsorption::None,
            quality: VolumeDataQuality::Healthy,
            persistence: StructurePersistence::Candidate,
            persistence_days: 1,
            metrics: None,
            boundary: PriceVolumeObservationBoundary {
                decision_weight_percent: 0,
                trade_signal: false,
                gate_effect: ObservationEffect::None,
                execution_effect: ObservationEffect::None,
                position_sizing_effect: ObservationEffect::None,
            },
            secondary_metrics: None,
            observation_confidence: Default::default(),
            eligibility: Default::default(),
            primary_baseline: Default::default(),
            secondary_baseline: None,
            lifecycle: Default::default(),
            unavailable_reason: None,
            next_eligibility_condition: None,
        };
        let date = |day| NaiveDate::from_ymd_opt(2026, 8, day).unwrap();
        layer
            .save_price_volume_observations(&[
                PriceVolumeObservationRecord {
                    market_date: date(3),
                    symbol: "MSFT".to_string(),
                    assessment: assessment(PriceVolumeStructure::ExhaustedAdvance),
                    supply_context: None,
                    price_position: None,
                    accumulation_failed: false,
                },
                PriceVolumeObservationRecord {
                    market_date: date(4),
                    symbol: "MSFT".to_string(),
                    assessment: assessment(PriceVolumeStructure::ExhaustedAdvance),
                    supply_context: None,
                    price_position: None,
                    accumulation_failed: false,
                },
            ])
            .unwrap();
        assert_eq!(
            layer
                .next_price_volume_persistence_days(
                    date(5),
                    "MSFT",
                    PriceVolumeStructure::ExhaustedAdvance
                )
                .unwrap(),
            3
        );
        assert_eq!(
            layer
                .next_price_volume_persistence_days(date(5), "MSFT", PriceVolumeStructure::Neutral)
                .unwrap(),
            1
        );
        assert_eq!(
            layer
                .next_price_volume_persistence_days(
                    date(10),
                    "MSFT",
                    PriceVolumeStructure::ExhaustedAdvance
                )
                .unwrap(),
            1
        );
        assert_eq!(
            layer
                .next_price_volume_persistence_days(
                    date(6),
                    "MSFT",
                    PriceVolumeStructure::ExhaustedAdvance
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn accumulation_followed_by_distribution_is_recorded_as_failed_observation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let layer = PersistenceLayer::new(temp_dir.path());
        let assessment = |structure| PriceVolumeAssessment {
            structure,
            participation: ParticipationQuality::Neutral,
            supply_absorption: SupplyAbsorption::None,
            quality: VolumeDataQuality::Healthy,
            persistence: StructurePersistence::Candidate,
            persistence_days: 1,
            metrics: None,
            boundary: PriceVolumeObservationBoundary {
                decision_weight_percent: 0,
                trade_signal: false,
                gate_effect: ObservationEffect::None,
                execution_effect: ObservationEffect::None,
                position_sizing_effect: ObservationEffect::None,
            },
            secondary_metrics: None,
            observation_confidence: Default::default(),
            eligibility: Default::default(),
            primary_baseline: Default::default(),
            secondary_baseline: None,
            lifecycle: Default::default(),
            unavailable_reason: None,
            next_eligibility_condition: None,
        };
        layer
            .save_price_volume_observations(&[PriceVolumeObservationRecord {
                market_date: NaiveDate::from_ymd_opt(2026, 8, 6).unwrap(),
                symbol: "SPCX".to_string(),
                assessment: assessment(PriceVolumeStructure::Accumulation),
                supply_context: None,
                price_position: None,
                accumulation_failed: false,
            }])
            .unwrap();

        assert!(layer
            .is_accumulation_failed(
                NaiveDate::from_ymd_opt(2026, 8, 7).unwrap(),
                "SPCX",
                PriceVolumeStructure::Distribution,
            )
            .unwrap());
        assert!(!layer
            .is_accumulation_failed(
                NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
                "SPCX",
                PriceVolumeStructure::Distribution,
            )
            .unwrap());
    }

    #[test]
    fn price_volume_record_schema_accepts_missing_supply_context() {
        let value: PriceVolumeObservationRecord = serde_json::from_str(r#"{"market_date":"2026-08-07","symbol":"X","assessment":{"structure":"Neutral","participation":"Neutral","supply_absorption":"None","quality":"Healthy","persistence":"Candidate","persistence_days":1,"metrics":null,"boundary":{"decision_weight_percent":0,"trade_signal":false,"gate_effect":"None","execution_effect":"None","position_sizing_effect":"None"}}}"#).unwrap();
        assert!(value.supply_context.is_none());
    }

    #[test]
    fn test_persistence_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_persist_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
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
    fn legacy_migration_module_preserves_empty_history_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_legacy_migration_boundary_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);

        assert!(!layer.legacy_history_migration_needed().unwrap());
        assert!(layer.migrate_legacy_history().unwrap().is_none());

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
    fn history_write_transaction_restores_partial_files_when_not_committed() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_history_transaction_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let history_path = temp_dir.join("decision_history.jsonl");
        fs::write(&history_path, "before\n").unwrap();

        {
            let _transaction = layer
                .begin_history_write_transaction(date, "cycle-1")
                .unwrap();
            fs::write(&history_path, "partial\n").unwrap();
            fs::create_dir_all(temp_dir.join("snapshots")).unwrap();
            fs::write(
                temp_dir.join("snapshots/cycle-1_2026-07-27.json"),
                "partial",
            )
            .unwrap();
        }

        assert_eq!(fs::read_to_string(history_path).unwrap(), "before\n");
        assert!(!temp_dir.join("snapshots/cycle-1_2026-07-27.json").exists());
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn history_write_transaction_includes_transition_logs() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_history_transition_transaction_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        fs::write(temp_dir.join("state_transitions.csv"), "header\nold\n").unwrap();
        fs::write(temp_dir.join("state_transitions.jsonl"), "old\n").unwrap();

        {
            let _transaction = layer
                .begin_history_write_transaction(
                    NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
                    "cycle-1",
                )
                .unwrap();
            fs::write(temp_dir.join("state_transitions.csv"), "header\npartial\n").unwrap();
            fs::write(temp_dir.join("state_transitions.jsonl"), "partial\n").unwrap();
        }

        assert_eq!(
            fs::read_to_string(temp_dir.join("state_transitions.csv")).unwrap(),
            "header\nold\n"
        );
        assert_eq!(
            fs::read_to_string(temp_dir.join("state_transitions.jsonl")).unwrap(),
            "old\n"
        );
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn history_write_transaction_keeps_files_after_commit() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_history_transaction_commit_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let history_path = temp_dir.join("decision_history.jsonl");
        let transaction = layer
            .begin_history_write_transaction(date, "cycle-1")
            .unwrap();
        fs::write(&history_path, "committed\n").unwrap();
        transaction.commit();

        assert_eq!(fs::read_to_string(history_path).unwrap(), "committed\n");
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn formal_snapshot_resolution_does_not_fallback_to_legacy_packets() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_formal_baseline_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        layer
            .save_packet(&DecisionPacket {
                date: NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(),
                ..Default::default()
            })
            .unwrap();

        let resolution = layer
            .resolve_previous_snapshot_from_history(
                NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
                Some("cycle-1"),
            )
            .unwrap();
        assert_eq!(
            resolution.status,
            PreviousSnapshotStatus::BaselineUnavailable
        );
        assert!(resolution.snapshot.is_none());
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn legacy_packet_not_formed_status_is_read_as_dispersed() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_legacy_not_formed_status_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        let mut value = serde_json::to_value(DecisionPacket {
            date,
            ..Default::default()
        })
        .unwrap();
        value["trend_cohesion"]["status"] = serde_json::json!("NotFormed");
        fs::write(
            temp_dir.join("decision_packet_2026-04-15.json"),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let packets = layer.load_dated_legacy_packets().unwrap();
        assert_eq!(
            packets.get(&date).unwrap().trend_cohesion.status,
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed
        );
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn formal_snapshot_resolution_exposes_the_formal_snapshot_as_baseline() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_formal_snapshot_object_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let snapshot = TradingDaySnapshot {
            schema_version: "1".to_string(),
            market_date: NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(),
            report_date: NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
            as_of_date: NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(),
            generated_at: "2026-07-27T05:30:00+09:00".to_string(),
            run_id: "run-formal".to_string(),
            cycle_id: "cycle-1".to_string(),
            snapshot_id: "snapshot-formal".to_string(),
            is_valid_trading_day: true,
            source_status: "complete".to_string(),
            market_state: "STARTUP".to_string(),
            decision_state: "NO_TRADE".to_string(),
            new_position_limit: 0.0,
            breadth: 35.0,
            breadth_classification: Some("Very Narrow".to_string()),
            confidence: 56.7,
            supply_phase: "ACCUMULATING".to_string(),
            risk_state: "NORMAL".to_string(),
            primary_leader: Some("TSLA".to_string()),
            secondary_leaders: vec!["ISRG".to_string()],
            breakouts: serde_json::json!({}),
            stability: 1.1,
            continuity: 1,
            cycle_length_days: 1,
            reset_event: None,
            data_quality: serde_json::json!({"history": "HEALTHY"}),
        };
        layer.save_trading_day_snapshot(&snapshot).unwrap();

        let resolution = layer
            .resolve_previous_snapshot_from_history(
                NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
                Some("cycle-1"),
            )
            .unwrap();

        assert_eq!(
            resolution.formal_snapshot.unwrap().snapshot_id,
            "snapshot-formal"
        );
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn formal_snapshot_without_breadth_classification_remains_readable() {
        let snapshot = TradingDaySnapshot {
            schema_version: "1".to_string(),
            market_date: NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(),
            report_date: NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(),
            as_of_date: NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(),
            generated_at: "2026-07-24T00:00:00+00:00".to_string(),
            run_id: "run-legacy".to_string(),
            cycle_id: "cycle-1".to_string(),
            snapshot_id: "snapshot-legacy".to_string(),
            is_valid_trading_day: true,
            source_status: "complete".to_string(),
            market_state: "RANGE".to_string(),
            decision_state: "NO_TRADE".to_string(),
            new_position_limit: 0.0,
            breadth: 35.0,
            breadth_classification: Some("Very Narrow".to_string()),
            confidence: 53.2,
            supply_phase: "IDLE".to_string(),
            risk_state: "NORMAL".to_string(),
            primary_leader: Some("TSLA".to_string()),
            secondary_leaders: Vec::new(),
            breakouts: serde_json::json!({}),
            stability: 0.9,
            continuity: 2,
            cycle_length_days: 2,
            reset_event: None,
            data_quality: serde_json::json!({}),
        };
        let mut encoded = serde_json::to_value(snapshot).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .remove("breadth_classification");

        let restored: TradingDaySnapshot = serde_json::from_value(encoded).unwrap();

        assert_eq!(restored.breadth_classification, None);
    }

    #[test]
    fn formal_snapshot_preserves_structured_breadth_observation() {
        let snapshot = TradingDaySnapshot {
            schema_version: "1".to_string(),
            market_date: NaiveDate::from_ymd_opt(2026, 8, 13).unwrap(),
            report_date: NaiveDate::from_ymd_opt(2026, 8, 13).unwrap(),
            as_of_date: NaiveDate::from_ymd_opt(2026, 8, 13).unwrap(),
            generated_at: "2026-08-13T00:00:00+00:00".to_string(),
            run_id: "run-breadth".to_string(),
            cycle_id: "cycle-breadth".to_string(),
            snapshot_id: "snapshot-breadth".to_string(),
            is_valid_trading_day: true,
            source_status: "complete".to_string(),
            market_state: "RANGE".to_string(),
            decision_state: "NO_TRADE".to_string(),
            new_position_limit: 0.0,
            breadth: 50.0,
            breadth_classification: Some("Narrow".to_string()),
            confidence: 50.0,
            supply_phase: "IDLE".to_string(),
            risk_state: "NORMAL".to_string(),
            primary_leader: None,
            secondary_leaders: Vec::new(),
            breakouts: serde_json::json!({}),
            stability: 0.0,
            continuity: 1,
            cycle_length_days: 1,
            reset_event: None,
            data_quality: serde_json::json!({
                "breadth_observation": {
                    "raw_percent": 50.0,
                    "up_count": 5,
                    "flat_count": 1,
                    "down_count": 4,
                    "total_count": 10,
                    "universe_integrity": 1.0,
                    "classification": "Narrow"
                }
            }),
        };
        let value = serde_json::to_value(snapshot).unwrap();
        assert_eq!(
            value["data_quality"]["breadth_observation"]["total_count"],
            serde_json::json!(10)
        );
    }

    #[test]
    fn formal_snapshot_resolution_accepts_degraded_snapshot_as_historical_fact() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_degraded_formal_baseline_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let snapshot = TradingDaySnapshot {
            schema_version: "1".to_string(),
            market_date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            report_date: NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
            as_of_date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            generated_at: "2026-07-29T05:30:00+09:00".to_string(),
            run_id: "run-degraded".to_string(),
            cycle_id: "cycle-1".to_string(),
            snapshot_id: "snapshot-degraded".to_string(),
            is_valid_trading_day: true,
            source_status: "degraded".to_string(),
            market_state: "IGNITION".to_string(),
            decision_state: "NO_TRADE".to_string(),
            new_position_limit: 0.0,
            breadth: 35.0,
            breadth_classification: None,
            confidence: 20.0,
            supply_phase: "UNAVAILABLE".to_string(),
            risk_state: "NORMAL".to_string(),
            primary_leader: Some("U".to_string()),
            secondary_leaders: vec![],
            breakouts: serde_json::json!({"U": 96.0}),
            stability: 1.1,
            continuity: 1,
            cycle_length_days: 1,
            reset_event: None,
            data_quality: serde_json::json!({"history": "UNAVAILABLE"}),
        };
        assert!(PersistenceLayer::is_historical_snapshot(&snapshot));
        layer.save_trading_day_snapshot(&snapshot).unwrap();
        let loaded = layer.load_trading_day_snapshots().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].market_date, snapshot.market_date);
        assert_eq!(loaded[0].cycle_id, "cycle-1");
        layer
            .save_packet(&DecisionPacket {
                date: snapshot.market_date,
                ..Default::default()
            })
            .unwrap();

        let resolution = layer
            .resolve_previous_snapshot_from_history(
                NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
                Some("cycle-1"),
            )
            .unwrap();

        assert_eq!(resolution.status, PreviousSnapshotStatus::Available);
        assert_eq!(resolution.previous_market_date, Some(snapshot.market_date));
        assert_eq!(
            resolution.formal_snapshot.unwrap().snapshot_id,
            "snapshot-degraded"
        );
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn latest_cycle_id_before_date_is_recovered_from_formal_snapshot() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_cycle_recovery_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        layer
            .save_trading_day_snapshot(&TradingDaySnapshot {
                schema_version: "1".to_string(),
                market_date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
                report_date: NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
                as_of_date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
                generated_at: "2026-07-29T05:30:00+09:00".to_string(),
                run_id: "run-recovery".to_string(),
                cycle_id: "cycle-recovered".to_string(),
                snapshot_id: "cycle-recovered-2026-07-28".to_string(),
                is_valid_trading_day: true,
                source_status: "degraded".to_string(),
                market_state: "IGNITION".to_string(),
                decision_state: "NO_TRADE".to_string(),
                new_position_limit: 0.0,
                breadth: 35.0,
                breadth_classification: None,
                confidence: 20.0,
                supply_phase: "UNAVAILABLE".to_string(),
                risk_state: "NORMAL".to_string(),
                primary_leader: Some("U".to_string()),
                secondary_leaders: vec![],
                breakouts: serde_json::json!({"U": 96.0}),
                stability: 1.1,
                continuity: 1,
                cycle_length_days: 1,
                reset_event: None,
                data_quality: serde_json::json!({"history": "UNAVAILABLE"}),
            })
            .unwrap();

        assert_eq!(
            layer
                .latest_cycle_id_before(NaiveDate::from_ymd_opt(2026, 7, 29).unwrap())
                .unwrap(),
            Some("cycle-recovered".to_string())
        );
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn legacy_migration_is_skipped_when_formal_snapshots_already_exist() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_legacy_migration_skip_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&packet).unwrap(),
        )
        .unwrap();
        layer
            .save_trading_day_snapshot(
                &layer.project_legacy_packet_snapshot(packet, "formal-cycle"),
            )
            .unwrap();

        assert!(!layer.legacy_history_migration_needed().unwrap());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_projects_one_packet_to_degraded_formal_snapshot() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_history_one_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&packet).unwrap(),
        )
        .unwrap();
        fs::write(
            temp_dir.join("migration-notes.txt"),
            "legacy migration input boundary",
        )
        .unwrap();

        layer.migrate_legacy_history().unwrap();
        let snapshots = layer.load_trading_day_snapshots().unwrap();
        let state = layer
            .load_observation_history_state()
            .unwrap()
            .expect("migration should persist observation history state");

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].source_status, "degraded");
        assert_eq!(snapshots[0].decision_state, "NO_TRADE");
        assert_eq!(snapshots[0].new_position_limit, 0.0);
        assert_eq!(snapshots[0].reset_event.as_deref(), Some("MIGRATED_LEGACY"));
        assert_eq!(snapshots[0].data_quality["history"], "MIGRATED_LEGACY");
        assert_eq!(snapshots[0].generated_at, "UNAVAILABLE");
        assert_eq!(snapshots[0].run_id, "UNAVAILABLE");
        assert_eq!(snapshots[0].supply_phase, "UNAVAILABLE");
        assert!(snapshots[0].primary_leader.is_none());
        assert!(snapshots[0].secondary_leaders.is_empty());
        assert_eq!(snapshots[0].breakouts, serde_json::json!({}));
        assert_eq!(state.count, 1);
        assert_eq!(state.last_market_date, packet.date);
        assert_eq!(state.cycle_id, "legacy-2026-07-28-2026-07-28");
        assert_eq!(snapshots[0].cycle_id, state.cycle_id);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_publishes_packets_for_history_resolution() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_packet_history_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&packet).unwrap(),
        )
        .unwrap();

        let state = layer.migrate_legacy_history().unwrap().unwrap();
        let resolution = layer
            .resolve_previous_snapshot_from_history(
                NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
                Some(&state.cycle_id),
            )
            .unwrap();
        let packets = layer
            .load_recent_packets_before(NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(), 1)
            .unwrap();

        assert_eq!(resolution.status, PreviousSnapshotStatus::Available);
        assert_eq!(resolution.snapshot.unwrap().date, packet.date);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].date, packet.date);
        assert!(temp_dir.join("decision_snapshots/2026-07-28.json").exists());
        assert!(temp_dir.join("decision_history.jsonl").exists());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_accepts_jsonl_only_input() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_history_jsonl_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_history.jsonl"),
            format!("{}\n", serde_json::to_string(&packet).unwrap()),
        )
        .unwrap();

        let state = layer.migrate_legacy_history().unwrap().unwrap();
        assert_eq!(state.count, 1);
        assert_eq!(layer.load_trading_day_snapshots().unwrap().len(), 1);
        let loaded_packets = layer
            .load_recent_packets_before(packet.date.succ_opt().unwrap(), 1)
            .unwrap();
        assert_eq!(loaded_packets.len(), 1);
        assert_eq!(loaded_packets[0].date, packet.date);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn legacy_price_volume_metrics_without_baseline_fields_remain_readable() {
        let value: PriceVolumeObservationRecord = serde_json::from_str(
            r#"{
                "market_date":"2026-08-07",
                "symbol":"X",
                "assessment":{
                    "structure":"Neutral",
                    "participation":"Neutral",
                    "supply_absorption":"None",
                    "quality":"Healthy",
                    "persistence":"Candidate",
                    "persistence_days":1,
                    "metrics":{
                        "return_1d":0.0,
                        "return_5d":0.0,
                        "return_10d":0.0,
                        "return_20d":0.0,
                        "rvol_5":1.0,
                        "rvol_20":1.0,
                        "average_volume_5":1.0,
                        "average_volume_20":1.0,
                        "up_day_average_volume":1.0,
                        "down_day_average_volume":1.0,
                        "distance_from_20d_high":0.0,
                        "distance_from_20d_low":0.0,
                        "new_high":false,
                        "new_low":false,
                        "atr_normalized_move":null,
                        "body_ratio":null,
                        "upper_wick_ratio":null,
                        "lower_wick_ratio":null,
                        "gap_percent":null
                    },
                    "boundary":{
                        "decision_weight_percent":0,
                        "trade_signal":false,
                        "gate_effect":"None",
                        "execution_effect":"None",
                        "position_sizing_effect":"None"
                    }
                }
            }"#,
        )
        .unwrap();

        let metrics = value.assessment.metrics.unwrap();
        assert_eq!(metrics.baseline_days, 0);
        assert_eq!(metrics.baseline_type, BaselineType::Unavailable);
        assert_eq!(metrics.relative_volume, 0.0);
        assert!(metrics.relative_volume_label.is_empty());
    }

    #[test]
    fn migrate_legacy_history_sorts_dates_and_reuses_one_stable_cycle() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_history_order_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        for date in [
            NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
        ] {
            let packet = DecisionPacket {
                date,
                ..Default::default()
            };
            fs::write(
                temp_dir.join(format!("decision_packet_{date}.json")),
                serde_json::to_string_pretty(&packet).unwrap(),
            )
            .unwrap();
        }

        layer.migrate_legacy_history().unwrap();
        let snapshots = layer.load_trading_day_snapshots().unwrap();
        let entries = layer.load_observation_history_entries().unwrap();
        let state = layer
            .load_observation_history_state()
            .unwrap()
            .expect("migration should persist observation history state");

        assert_eq!(state.count, 2);
        assert_eq!(snapshots.len(), 2);
        assert_eq!(entries.len(), state.count);
        assert_eq!(
            entries.iter().map(|entry| entry.date).collect::<Vec<_>>(),
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
            ]
        );
        assert_eq!(state.cycle_id, "legacy-2026-07-28-2026-07-29");
        assert_eq!(snapshots[0].cycle_id, state.cycle_id);
        assert_eq!(snapshots[1].cycle_id, state.cycle_id);
        assert_eq!(
            state.last_market_date,
            NaiveDate::from_ymd_opt(2026, 7, 29).unwrap()
        );

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_writes_safe_timeline_entries_for_every_packet() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_history_timeline_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        for date in [
            NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
        ] {
            let packet = DecisionPacket {
                date,
                ..Default::default()
            };
            fs::write(
                temp_dir.join(format!("decision_packet_{date}.json")),
                serde_json::to_string_pretty(&packet).unwrap(),
            )
            .unwrap();
        }

        let state = layer.migrate_legacy_history().unwrap().unwrap();
        let entries = layer.load_observation_history_entries().unwrap();

        assert_eq!(entries.len(), state.count);
        assert_eq!(
            entries.iter().map(|entry| entry.date).collect::<Vec<_>>(),
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
            ]
        );
        assert!(entries.iter().all(|entry| {
            entry.primary_leader.is_empty()
                && entry.secondary_leaders.is_empty()
                && entry.concentration_score == 0.0
                && entry.rotation_score == 0.0
                && entry.confidence_index == 0.0
                && entry.market_state == "UNAVAILABLE"
                && entry.supply_phase == "UNAVAILABLE"
                && entry.risk_state == "UNAVAILABLE"
                && entry.day_type == "UNAVAILABLE"
        }));

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_derives_state_from_merged_timeline_entries() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_merged_timeline_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let existing_date = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let legacy_date = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();
        layer
            .save_observation_timeline_entry(
                ObservationTimelineEntry {
                    date: existing_date,
                    primary_leader: "EXISTING".to_string(),
                    secondary_leaders: Vec::new(),
                    breadth_score: 42.0,
                    concentration_score: 1.0,
                    rotation_score: 2.0,
                    confidence_index: 3.0,
                    market_state: "RANGE".to_string(),
                    supply_phase: "WATCH".to_string(),
                    risk_state: "NORMAL".to_string(),
                    day_type: "NORMAL".to_string(),
                    ..Default::default()
                },
                &[existing_date],
            )
            .unwrap();
        let packet = DecisionPacket {
            date: legacy_date,
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&packet).unwrap(),
        )
        .unwrap();

        let state = layer.migrate_legacy_history().unwrap().unwrap();
        let entries = layer.load_observation_history_entries().unwrap();

        assert_eq!(state.count, entries.len());
        assert_eq!(state.count, 2);
        assert_eq!(state.last_market_date, legacy_date);
        assert_eq!(
            entries.iter().map(|entry| entry.date).collect::<Vec<_>>(),
            vec![existing_date, legacy_date]
        );

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_rejects_malformed_packet_without_publishing_outputs() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_history_malformed_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&packet).unwrap(),
        )
        .unwrap();
        fs::write(
            temp_dir.join("decision_packet_2026-07-29.json"),
            "{ malformed packet",
        )
        .unwrap();

        assert!(layer.migrate_legacy_history().is_err());
        assert!(!temp_dir.join("snapshots").exists());
        assert!(layer.load_trading_day_snapshots().unwrap().is_empty());
        assert!(layer.load_observation_history_state().unwrap().is_none());
        assert!(!temp_dir.join("observation_history_state.json").exists());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_reuses_identical_snapshot_and_existing_state_cycle() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_history_idempotent_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&packet).unwrap(),
        )
        .unwrap();

        let state = ObservationHistoryState {
            count: 1,
            last_market_date: packet.date,
            cycle_id: "existing-cycle".to_string(),
        };
        layer.save_observation_history_state(&state).unwrap();
        let state_path = temp_dir.join("observation_history_state.json");
        let original_state = fs::read_to_string(&state_path).unwrap();
        let mut existing = layer.project_legacy_packet_snapshot(packet, &state.cycle_id);
        existing.generated_at = "2026-07-30T00:00:00+09:00".to_string();
        existing.run_id = "preserved-run".to_string();
        existing.snapshot_id = "preserved-snapshot".to_string();
        layer.save_trading_day_snapshot(&existing).unwrap();
        let snapshot_path = temp_dir.join("snapshots/existing-cycle_2026-07-28.json");
        let original_snapshot = fs::read_to_string(&snapshot_path).unwrap();

        let first_state = layer.migrate_legacy_history().unwrap().unwrap();
        let packet_snapshot_path = temp_dir.join("decision_snapshots/2026-07-28.json");
        let history_path = temp_dir.join("decision_history.jsonl");
        let original_packet_snapshot = fs::read_to_string(&packet_snapshot_path).unwrap();
        let original_history = fs::read_to_string(&history_path).unwrap();
        assert_eq!(first_state.cycle_id, "existing-cycle");
        assert_eq!(first_state.count, 1);
        assert_eq!(
            fs::read_to_string(&snapshot_path).unwrap(),
            original_snapshot
        );
        assert_eq!(
            layer
                .load_observation_history_state()
                .unwrap()
                .unwrap()
                .cycle_id,
            "existing-cycle"
        );
        assert_eq!(fs::read_to_string(&state_path).unwrap(), original_state);

        let second_state = layer.migrate_legacy_history().unwrap().unwrap();
        assert_eq!(second_state.count, first_state.count);
        assert_eq!(second_state.last_market_date, first_state.last_market_date);
        assert_eq!(second_state.cycle_id, first_state.cycle_id);
        assert_eq!(
            fs::read_to_string(&snapshot_path).unwrap(),
            original_snapshot
        );
        assert_eq!(fs::read_to_string(&state_path).unwrap(), original_state);
        assert_eq!(
            fs::read_to_string(&packet_snapshot_path).unwrap(),
            original_packet_snapshot
        );
        assert_eq!(fs::read_to_string(&history_path).unwrap(), original_history);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn migrate_legacy_history_rejects_semantic_conflict_without_changing_existing_history() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_migrate_legacy_history_conflict_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            temp_dir.join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&packet).unwrap(),
        )
        .unwrap();
        let expected_state = ObservationHistoryState {
            count: 7,
            last_market_date: NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
            cycle_id: "existing-cycle".to_string(),
        };
        layer
            .save_observation_history_state(&expected_state)
            .unwrap();
        let mut conflicting_snapshot =
            layer.project_legacy_packet_snapshot(packet, &expected_state.cycle_id);
        conflicting_snapshot.market_state = "CONFLICTING_MARKET_STATE".to_string();
        layer
            .save_trading_day_snapshot(&conflicting_snapshot)
            .unwrap();
        let snapshot_path = temp_dir.join("snapshots/existing-cycle_2026-07-28.json");
        let original_snapshot = fs::read_to_string(&snapshot_path).unwrap();

        let error = layer.migrate_legacy_history().unwrap_err();
        let error_chain = format!("{error:#}");
        let error_contract = error_chain
            .rsplit(": ")
            .next()
            .expect("anyhow error chain must have a final error contract");
        assert_eq!(error_contract, "SNAPSHOT_CONFLICT");
        let state_after = layer
            .load_observation_history_state()
            .unwrap()
            .expect("conflict must preserve the existing state");
        assert_eq!(state_after.count, expected_state.count);
        assert_eq!(
            state_after.last_market_date,
            expected_state.last_market_date
        );
        assert_eq!(state_after.cycle_id, expected_state.cycle_id);
        let snapshots = layer.load_trading_day_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].market_state, "CONFLICTING_MARKET_STATE");
        assert_eq!(
            fs::read_to_string(&snapshot_path).unwrap(),
            original_snapshot
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
        assert!(temp_dir.join("leader_snapshots/2026-07-10.json").exists());
        assert_eq!(
            fs::read_to_string(temp_dir.join("leader_observations.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
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
                ..Default::default()
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
        for path in [
            temp_dir.join("observation_timeline_latest.json"),
            temp_dir.join("observation_timeline_2026-07-10.json"),
            temp_dir.join("timeline_snapshots").join("2026-07-10.json"),
        ] {
            let payload = fs::read_to_string(path).unwrap();
            serde_json::from_str::<ObservationTimeline>(&payload).unwrap();
        }
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
                ..Default::default()
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
                        ..Default::default()
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
    fn trading_day_snapshot_is_upserted_by_cycle_and_market_date() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sentinel_trading_day_snapshot_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let layer = PersistenceLayer::new(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let snapshot = TradingDaySnapshot {
            schema_version: "1".to_string(),
            market_date: date,
            report_date: date,
            as_of_date: date,
            generated_at: "2026-07-28T05:30:00+09:00".to_string(),
            run_id: "run-1".to_string(),
            cycle_id: "default".to_string(),
            snapshot_id: "default-2026-07-27".to_string(),
            is_valid_trading_day: true,
            source_status: "complete".to_string(),
            market_state: "STARTUP".to_string(),
            decision_state: "NO_TRADE".to_string(),
            new_position_limit: 0.0,
            breadth: 35.0,
            breadth_classification: Some("Very Narrow".to_string()),
            confidence: 56.7,
            supply_phase: "ACCUMULATING".to_string(),
            risk_state: "NORMAL".to_string(),
            primary_leader: Some("TSLA".to_string()),
            secondary_leaders: vec!["ISRG".to_string()],
            breakouts: serde_json::json!({}),
            stability: 1.1,
            continuity: 1,
            cycle_length_days: 1,
            reset_event: None,
            data_quality: serde_json::json!({"history": "UNAVAILABLE"}),
        };
        assert_eq!(
            layer.save_trading_day_snapshot(&snapshot).unwrap(),
            TradingDaySnapshotWriteDisposition::Created
        );
        let mut revised = snapshot.clone();
        revised.generated_at = "2026-07-28T05:31:00+09:00".to_string();
        assert_eq!(
            layer.save_trading_day_snapshot(&revised).unwrap(),
            TradingDaySnapshotWriteDisposition::SameDayRerun
        );
        let mut conflict = revised.clone();
        conflict.decision_state = "OBSERVE".to_string();
        let error = layer.save_trading_day_snapshot(&conflict).unwrap_err();
        assert_eq!(error.to_string(), "SNAPSHOT_CONFLICT");

        let loaded = layer.load_trading_day_snapshots().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].generated_at, "2026-07-28T05:31:00+09:00");
        let encoded =
            fs::read_to_string(temp_dir.join("snapshots/default_2026-07-27.json")).unwrap();
        for key in [
            "decision_state",
            "new_position_limit",
            "market_date",
            "report_date",
            "as_of_date",
        ] {
            assert!(encoded.contains(&format!("\"{key}\"")));
        }

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
