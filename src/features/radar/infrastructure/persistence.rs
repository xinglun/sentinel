use crate::features::radar::application::execution_gate::ExecutionResult;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::leader_persistence::LeaderObservation;
use crate::features::radar::domain::observation_timeline::{
    ObservationTimeline, ObservationTimelineEntry,
};
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviousSnapshotStatus {
    Available,
    BaselineUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingDaySnapshotWriteDisposition {
    Created,
    SameDayRerun,
}

#[derive(Debug, Clone)]
pub struct PreviousSnapshotResolution {
    pub status: PreviousSnapshotStatus,
    pub current_market_date: chrono::NaiveDate,
    pub previous_market_date: Option<chrono::NaiveDate>,
    pub previous_snapshot_id: Option<String>,
    pub gap_type: Option<String>,
    pub is_same_cycle: bool,
    pub snapshot: Option<DecisionPacket>,
    pub reason: Option<String>,
    pub formal_snapshot: Option<TradingDaySnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradingDaySnapshot {
    pub schema_version: String,
    pub market_date: chrono::NaiveDate,
    pub report_date: chrono::NaiveDate,
    pub as_of_date: chrono::NaiveDate,
    pub generated_at: String,
    pub run_id: String,
    pub cycle_id: String,
    pub snapshot_id: String,
    pub is_valid_trading_day: bool,
    pub source_status: String,
    pub market_state: String,
    pub decision_state: String,
    pub new_position_limit: f64,
    pub breadth: f64,
    pub confidence: f64,
    pub supply_phase: String,
    pub risk_state: String,
    pub primary_leader: Option<String>,
    pub secondary_leaders: Vec<String>,
    pub breakouts: serde_json::Value,
    pub stability: f64,
    pub continuity: usize,
    pub cycle_length_days: usize,
    pub reset_event: Option<String>,
    pub data_quality: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservationHistoryState {
    pub count: usize,
    pub last_market_date: chrono::NaiveDate,
    #[serde(default)]
    pub cycle_id: String,
}

#[derive(Clone)]
pub struct PersistenceLayer {
    history_path: PathBuf,
    save_dir: PathBuf,
}

pub(crate) struct HistoryWriteTransaction {
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
    committed: bool,
}

/// 一時ファイルを同じディレクトリに作成し、完成後に対象へ原子的に置換する。
fn write_file_atomically(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("原子的書き込み先の作成に失敗: {parent:?}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("原子的書き込み用一時ファイルの作成に失敗: {path:?}"))?;
    temporary
        .write_all(content)
        .with_context(|| format!("原子的書き込み用一時ファイルへの書き込みに失敗: {path:?}"))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("原子的書き込み用一時ファイルの同期に失敗: {path:?}"))?;
    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .with_context(|| format!("原子的書き込み先の権限設定に失敗: {path:?}"))?;
    }
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
        .with_context(|| format!("原子的書き込み先の置換に失敗: {path:?}"))
}

impl HistoryWriteTransaction {
    pub(crate) fn commit(mut self) {
        self.committed = true;
    }

    fn rollback(&self) -> Result<()> {
        for (path, content) in self.files.iter().rev() {
            match content {
                Some(content) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("履歴ロールバック先の作成に失敗: {parent:?}")
                        })?;
                    }
                    std::fs::write(path, content)
                        .with_context(|| format!("履歴ファイルの復元に失敗: {path:?}"))?;
                }
                None => match std::fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("履歴ファイルの削除に失敗: {path:?}"));
                    }
                },
            }
        }
        let known_paths = self
            .files
            .iter()
            .map(|(path, _)| path)
            .collect::<HashSet<_>>();
        for entry in std::fs::read_dir(self.files[0].0.parent().unwrap_or(Path::new(".")))
            .context("履歴ロールバック対象の確認に失敗")?
        {
            let path = entry
                .context("履歴ロールバック対象の読み込みに失敗")?
                .path();
            let is_new_legacy_transition = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("state_transitions_legacy_") && name.ends_with(".csv")
                });
            if is_new_legacy_transition && !known_paths.contains(&path) {
                std::fs::remove_file(&path)
                    .with_context(|| format!("新規 legacy 履歴の削除に失敗: {path:?}"))?;
            }
        }
        Ok(())
    }
}

impl Drop for HistoryWriteTransaction {
    fn drop(&mut self) {
        if !self.committed {
            if let Err(error) = self.rollback() {
                eprintln!("履歴トランザクションのロールバックに失敗: {error:#}");
            }
        }
    }
}

impl PersistenceLayer {
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

    /// formal snapshot が未作成で、日付付き legacy packet がある時だけ移行する。
    pub fn legacy_history_migration_needed(&self) -> Result<bool> {
        if !self.load_trading_day_snapshots()?.is_empty() {
            return Ok(false);
        }
        Ok(!self.load_dated_legacy_packets()?.is_empty())
    }

    pub fn save_observation_history_state(&self, state: &ObservationHistoryState) -> Result<()> {
        let path = self.save_dir.join("observation_history_state.json");
        let json = serde_json::to_string_pretty(state)
            .context("Failed to serialize observation history state")?;
        write_file_atomically(&path, json.as_bytes())
            .context("Failed to write observation history state")?;
        Ok(())
    }

    /// 日付付き legacy packet を formal snapshot と観測履歴 state へ安全に移行する。
    pub fn migrate_legacy_history(&self) -> Result<Option<ObservationHistoryState>> {
        let packets = self.load_dated_legacy_packets()?;
        let Some((&first_date, _)) = packets.first_key_value() else {
            return Ok(None);
        };
        let (&last_date, _) = packets
            .last_key_value()
            .expect("legacy packet map is not empty");
        let state_before = self.load_observation_history_state()?;
        let cycle_id = state_before
            .as_ref()
            .filter(|state| !state.cycle_id.is_empty())
            .map(|state| state.cycle_id.clone())
            .unwrap_or_else(|| format!("legacy-{first_date}-{last_date}"));
        let snapshots = packets
            .values()
            .cloned()
            .map(|packet| self.project_legacy_packet_snapshot(packet, &cycle_id))
            .collect::<Vec<_>>();
        let dispositions = snapshots
            .iter()
            .map(|snapshot| self.validate_trading_day_snapshot_conflict(snapshot))
            .collect::<Result<Vec<_>>>()?;
        let (history_packets, snapshot_packets) = self.load_formal_packet_artifacts()?;
        self.validate_legacy_packet_publication(&packets, &history_packets, &snapshot_packets)?;
        let (timeline, timeline_needs_publish) = self.project_legacy_timeline(&packets)?;
        let timeline_date = timeline_needs_publish.then_some(
            timeline
                .entries
                .last()
                .expect("merged timeline contains legacy packet dates")
                .date,
        );
        let transaction = self.begin_legacy_migration_transaction(
            packets.keys().copied().collect::<Vec<_>>().as_slice(),
            &snapshots,
            timeline_date,
        )?;

        for (snapshot, disposition) in snapshots.iter().zip(dispositions) {
            if disposition == TradingDaySnapshotWriteDisposition::Created {
                self.save_trading_day_snapshot(snapshot)?;
            }
        }
        self.publish_legacy_packets(&packets, &history_packets, &snapshot_packets)?;
        if let Some(date) = timeline_date {
            self.save_observation_timeline(&timeline, &date.to_string())?;
        }
        let entries = self.load_observation_history_entries()?;
        let last_market_date = entries
            .last()
            .map(|entry| entry.date)
            .context("Migrated observation timeline is unexpectedly empty")?;
        let state = ObservationHistoryState {
            count: entries.len(),
            last_market_date,
            cycle_id,
        };
        if state_before.as_ref().is_none_or(|existing| {
            existing.count != state.count
                || existing.last_market_date != state.last_market_date
                || existing.cycle_id != state.cycle_id
        }) {
            self.save_observation_history_state(&state)?;
        }
        transaction.commit();
        Ok(Some(state))
    }

    /// legacy packet を既存の正式 packet artifact と意味的に照合する。
    fn validate_legacy_packet_publication(
        &self,
        packets: &std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
        history_packets: &std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
        snapshot_packets: &std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
    ) -> Result<()> {
        for (date, packet) in packets {
            for existing in [history_packets.get(date), snapshot_packets.get(date)]
                .into_iter()
                .flatten()
            {
                if !Self::decision_packet_semantics_match(existing, packet)? {
                    bail!("DECISION_PACKET_CONFLICT");
                }
            }
        }
        Ok(())
    }

    /// legacy packet を正式な snapshot と JSONL history の双方へ追加する。
    fn publish_legacy_packets(
        &self,
        packets: &std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
        history_packets: &std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
        snapshot_packets: &std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
    ) -> Result<()> {
        let dir = self.save_dir.join("decision_snapshots");
        std::fs::create_dir_all(&dir).context("Failed to create decision snapshot directory")?;
        for (date, packet) in packets {
            if !snapshot_packets.contains_key(date) {
                let json = serde_json::to_string_pretty(packet)
                    .context("Failed to serialize migrated decision packet snapshot")?;
                write_file_atomically(&dir.join(format!("{date}.json")), json.as_bytes())
                    .context("Failed to write migrated decision packet snapshot")?;
            }
        }

        let packets_to_append = packets
            .iter()
            .filter(|(date, _)| !history_packets.contains_key(date))
            .collect::<Vec<_>>();
        if packets_to_append.is_empty() {
            return Ok(());
        }
        let mut content = self
            .history_path
            .exists()
            .then(|| std::fs::read(&self.history_path))
            .transpose()
            .context("Failed to read decision_history.jsonl for migration")?
            .unwrap_or_default();
        if !content.is_empty() && !content.ends_with(b"\n") {
            content.push(b'\n');
        }
        for (_, packet) in packets_to_append {
            serde_json::to_writer(&mut content, packet)
                .context("Failed to serialize migrated decision packet history")?;
            content.push(b'\n');
        }
        write_file_atomically(&self.history_path, &content)
            .context("Failed to write migrated decision packet history")?;
        Ok(())
    }

    /// 正式 packet artifact を日付ごとに読み、同日内容の矛盾を拒否する。
    fn load_formal_packet_artifacts(
        &self,
    ) -> Result<(
        std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
        std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
    )> {
        let history_packets = self.load_packet_history_artifact()?;
        let snapshot_packets = self.load_packet_snapshot_artifact()?;
        for (date, history_packet) in &history_packets {
            if let Some(snapshot_packet) = snapshot_packets.get(date) {
                if !Self::decision_packet_semantics_match(history_packet, snapshot_packet)? {
                    bail!("DECISION_PACKET_CONFLICT");
                }
            }
        }
        Ok((history_packets, snapshot_packets))
    }

    fn load_packet_history_artifact(
        &self,
    ) -> Result<std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>> {
        let mut packets = std::collections::BTreeMap::new();
        if !self.history_path.exists() {
            return Ok(packets);
        }
        let file =
            File::open(&self.history_path).context("Failed to open decision_history.jsonl")?;
        for line in BufReader::new(file).lines() {
            let line = line.context("Failed to read decision_history.jsonl")?;
            if line.trim().is_empty() {
                continue;
            }
            let packet: DecisionPacket = serde_json::from_str(&line)
                .context("Failed to deserialize DecisionPacket from history")?;
            Self::insert_packet_artifact(&mut packets, packet)?;
        }
        Ok(packets)
    }

    fn load_packet_snapshot_artifact(
        &self,
    ) -> Result<std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>> {
        let mut packets = std::collections::BTreeMap::new();
        let dir = self.save_dir.join("decision_snapshots");
        if !dir.exists() {
            return Ok(packets);
        }
        for file in std::fs::read_dir(dir).context("Failed to read decision snapshots")? {
            let path = file?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let packet: DecisionPacket = serde_json::from_str(
                &std::fs::read_to_string(&path).context("Failed to read decision snapshot")?,
            )
            .context("Failed to deserialize decision snapshot")?;
            Self::insert_packet_artifact(&mut packets, packet)?;
        }
        Ok(packets)
    }

    fn insert_packet_artifact(
        packets: &mut std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
        packet: DecisionPacket,
    ) -> Result<()> {
        if let Some(existing) = packets.get(&packet.date) {
            if !Self::decision_packet_semantics_match(existing, &packet)? {
                bail!("DECISION_PACKET_CONFLICT");
            }
            return Ok(());
        }
        packets.insert(packet.date, packet);
        Ok(())
    }

    fn decision_packet_semantics_match(
        left: &DecisionPacket,
        right: &DecisionPacket,
    ) -> Result<bool> {
        Ok(
            serde_json::to_value(left).context("Failed to serialize decision packet semantics")?
                == serde_json::to_value(right)
                    .context("Failed to serialize decision packet semantics")?,
        )
    }

    /// 既存 timeline を保持して legacy entry を日付単位で追加する計画を検証する。
    fn project_legacy_timeline(
        &self,
        packets: &std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>,
    ) -> Result<(ObservationTimeline, bool)> {
        let mut entries = self
            .load_observation_history_entries()?
            .into_iter()
            .map(|entry| (entry.date, entry))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut needs_publish = false;
        for packet in packets.values() {
            if let std::collections::btree_map::Entry::Vacant(entry) = entries.entry(packet.date) {
                entry.insert(self.project_legacy_packet_timeline_entry(packet));
                needs_publish = true;
            }
        }
        let expected_trading_dates = entries.keys().copied().collect::<Vec<_>>();
        let timeline =
            crate::features::radar::domain::observation_timeline::build_observation_timeline(
                &entries.into_values().collect::<Vec<_>>(),
                &expected_trading_dates,
            );
        if timeline.entries.len() != expected_trading_dates.len() {
            bail!("TIMELINE_PROJECTION_CONFLICT");
        }
        Ok((timeline, needs_publish))
    }

    /// migration の途中失敗時に既存 artifact を戻すための rollback 記録を作る。
    fn begin_legacy_migration_transaction(
        &self,
        packet_dates: &[chrono::NaiveDate],
        snapshots: &[TradingDaySnapshot],
        timeline_date: Option<chrono::NaiveDate>,
    ) -> Result<HistoryWriteTransaction> {
        let mut paths = vec![
            self.history_path.clone(),
            self.save_dir.join("observation_history_state.json"),
        ];
        paths.extend(packet_dates.iter().map(|date| {
            self.save_dir
                .join("decision_snapshots")
                .join(format!("{date}.json"))
        }));
        paths.extend(snapshots.iter().map(|snapshot| {
            self.save_dir.join("snapshots").join(format!(
                "{}_{}.json",
                snapshot.cycle_id, snapshot.market_date
            ))
        }));
        if let Some(date) = timeline_date {
            paths.extend([
                self.save_dir.join("observation_timeline.jsonl"),
                self.save_dir.join("observation_timeline_latest.json"),
                self.save_dir
                    .join(format!("observation_timeline_{date}.json")),
                self.save_dir
                    .join("timeline_snapshots")
                    .join(format!("{date}.json")),
            ]);
        }
        let files = paths
            .into_iter()
            .map(|path| {
                let content = path
                    .exists()
                    .then(|| std::fs::read(&path))
                    .transpose()
                    .with_context(|| format!("移行ロールバック対象の取得に失敗: {path:?}"))?;
                Ok((path, content))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(HistoryWriteTransaction {
            files,
            committed: false,
        })
    }

    /// save_dir 直下の dated legacy packet を日付ごとに一意に読み込む。
    fn load_dated_legacy_packets(
        &self,
    ) -> Result<std::collections::BTreeMap<chrono::NaiveDate, DecisionPacket>> {
        let mut packets = std::collections::BTreeMap::new();
        if !self.save_dir.exists() {
            return Ok(packets);
        }

        let mut paths = std::fs::read_dir(&self.save_dir)
            .context("Failed to read legacy decision packet directory")?
            .map(|entry| {
                entry
                    .context("Failed to read legacy decision packet entry")
                    .map(|entry| entry.path())
            })
            .collect::<Result<Vec<_>>>()?;
        paths.sort();

        for path in paths {
            let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(date) = filename
                .strip_prefix("decision_packet_")
                .and_then(|name| name.strip_suffix(".json"))
            else {
                continue;
            };
            let filename_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .with_context(|| format!("Invalid legacy decision packet date in {filename}"))?;
            let packet: DecisionPacket = serde_json::from_str(
                &std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read legacy decision packet: {path:?}"))?,
            )
            .with_context(|| format!("Failed to deserialize legacy decision packet: {path:?}"))?;
            if packet.date != filename_date {
                bail!("Legacy decision packet date does not match filename: {path:?}");
            }
            packets.entry(packet.date).or_insert(packet);
        }
        Ok(packets)
    }

    /// legacy packet の確認可能な市場事実だけを安全側の formal snapshot へ写像する。
    fn project_legacy_packet_snapshot(
        &self,
        packet: DecisionPacket,
        cycle_id: &str,
    ) -> TradingDaySnapshot {
        let breadth = if packet.market_features.total_count == 0 {
            0.0
        } else {
            packet.market_features.up_count as f64 / packet.market_features.total_count as f64
                * 100.0
        };
        TradingDaySnapshot {
            schema_version: "1".to_string(),
            market_date: packet.date,
            report_date: packet.date,
            as_of_date: packet.date,
            generated_at: "UNAVAILABLE".to_string(),
            run_id: "UNAVAILABLE".to_string(),
            cycle_id: cycle_id.to_string(),
            snapshot_id: format!("{cycle_id}-{}", packet.date),
            is_valid_trading_day: true,
            source_status: "degraded".to_string(),
            market_state: format!("{:?}", packet.market_regime.market_state),
            decision_state: "NO_TRADE".to_string(),
            new_position_limit: 0.0,
            breadth,
            confidence: packet.market_features.system_confidence,
            supply_phase: "UNAVAILABLE".to_string(),
            risk_state: format!("{:?}", packet.market_regime.risk_overlay),
            primary_leader: None,
            secondary_leaders: Vec::new(),
            breakouts: serde_json::json!({}),
            stability: packet.market_features.stability_score,
            continuity: 0,
            cycle_length_days: 0,
            reset_event: Some("MIGRATED_LEGACY".to_string()),
            data_quality: serde_json::json!({"history": "MIGRATED_LEGACY"}),
        }
    }

    /// legacy packet から証明できる観測履歴だけを安全側の timeline entry へ写像する。
    fn project_legacy_packet_timeline_entry(
        &self,
        packet: &DecisionPacket,
    ) -> ObservationTimelineEntry {
        let breadth_score = if packet.market_features.total_count == 0 {
            0.0
        } else {
            packet.market_features.up_count as f64 / packet.market_features.total_count as f64
                * 100.0
        };
        ObservationTimelineEntry {
            date: packet.date,
            primary_leader: String::new(),
            secondary_leaders: Vec::new(),
            breadth_score,
            concentration_score: 0.0,
            rotation_score: 0.0,
            confidence_index: 0.0,
            market_state: "UNAVAILABLE".to_string(),
            supply_phase: "UNAVAILABLE".to_string(),
            risk_state: "UNAVAILABLE".to_string(),
            day_type: "UNAVAILABLE".to_string(),
        }
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
