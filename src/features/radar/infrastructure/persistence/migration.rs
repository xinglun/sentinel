use super::atomic::{write_file_atomically, HistoryWriteTransaction};
use super::model::{
    ObservationHistoryState, TradingDaySnapshot, TradingDaySnapshotWriteDisposition,
};
use super::PersistenceLayer;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::observation_timeline::{
    derive_breadth_facts, ObservationTimeline, ObservationTimelineEntry,
};
use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};

impl PersistenceLayer {
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
    pub(super) fn load_dated_legacy_packets(
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
        if packets.is_empty() && self.history_path.exists() {
            let file = File::open(&self.history_path)
                .context("Failed to open legacy decision history JSONL")?;
            for line in BufReader::new(file).lines() {
                let line = line.context("Failed to read legacy decision history JSONL")?;
                if line.trim().is_empty() {
                    continue;
                }
                let packet: DecisionPacket = serde_json::from_str(&line)
                    .context("Failed to deserialize legacy decision history JSONL")?;
                packets.entry(packet.date).or_insert(packet);
            }
        }
        Ok(packets)
    }

    /// legacy packet の確認可能な市場事実だけを安全側の formal snapshot へ写像する。
    pub(super) fn project_legacy_packet_snapshot(
        &self,
        packet: DecisionPacket,
        cycle_id: &str,
    ) -> TradingDaySnapshot {
        let breadth_facts = derive_breadth_facts(
            packet.market_features.up_count,
            packet.market_features.flat_count,
            packet.market_features.down_count,
            packet.market_features.total_count,
        );
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
            breadth: breadth_facts.raw_percent,
            breadth_classification: Some(breadth_facts.label),
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
            ..Default::default()
        }
    }

    /// legacy packet から証明できる観測履歴だけを安全側の timeline entry へ写像する。
    fn project_legacy_packet_timeline_entry(
        &self,
        packet: &DecisionPacket,
    ) -> ObservationTimelineEntry {
        let facts = derive_breadth_facts(
            packet.market_features.up_count,
            packet.market_features.flat_count,
            packet.market_features.down_count,
            packet.market_features.total_count,
        );
        ObservationTimelineEntry {
            date: packet.date,
            primary_leader: String::new(),
            secondary_leaders: Vec::new(),
            breadth_score: facts.classification_score,
            breadth_raw_percent: facts.raw_percent,
            breadth_up_count: packet.market_features.up_count,
            breadth_flat_count: packet.market_features.flat_count,
            breadth_down_count: packet.market_features.down_count,
            breadth_total_count: packet.market_features.total_count,
            breadth_universe_integrity: packet.market_features.universe_integrity,
            concentration_score: 0.0,
            rotation_score: 0.0,
            confidence_index: 0.0,
            market_state: "UNAVAILABLE".to_string(),
            supply_phase: "UNAVAILABLE".to_string(),
            risk_state: "UNAVAILABLE".to_string(),
            day_type: "UNAVAILABLE".to_string(),
        }
    }
}
