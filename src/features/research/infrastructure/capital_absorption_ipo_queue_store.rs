use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoSnapshot, CapitalAbsorptionIpoQueueHistoryPoint,
    CapitalAbsorptionIpoQueueItem, CapitalAbsorptionIpoQueueStatus,
    CapitalAbsorptionObservationEventType, CapitalAbsorptionPotentialSupplyPressureLevel,
};
use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const LEDGER_FILE: &str = "capital_absorption_ipo_queue_history.jsonl";
const LATEST_FILE: &str = "capital_absorption_ipo_queue_history_latest.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CapitalAbsorptionIpoQueueRecord {
    pub date: String,
    pub queue_count: usize,
    pub reported_count: usize,
    pub confirmed_count: usize,
    pub pressure: String,
    pub items: Vec<CapitalAbsorptionIpoQueueRecordItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CapitalAbsorptionIpoQueueRecordItem {
    pub issuer: String,
    pub ipo_stage: String,
    pub event_type: String,
    pub source_count: usize,
}

pub(crate) fn persist_and_replay_ipo_queue_history(
    save_dir: &Path,
    as_of_date: NaiveDate,
    snapshot: &mut CapitalAbsorptionAutoSnapshot,
) -> Result<()> {
    fs::create_dir_all(save_dir)
        .with_context(|| format!("Failed to create {}", save_dir.display()))?;
    let current = record_from_snapshot(as_of_date, snapshot);
    let mut records = load_ipo_queue_records(save_dir, as_of_date)?;
    records.retain(|record| record.date != current.date);
    records.push(current.clone());
    records.sort_by(|a, b| a.date.cmp(&b.date));
    snapshot.ipo_queue_history = history_from_records(&records, as_of_date);
    write_ipo_queue_record(save_dir, &current)?;
    Ok(())
}

pub(crate) fn load_ipo_queue_weekly_summary(
    save_dir: &Path,
    as_of_date: NaiveDate,
) -> serde_json::Value {
    let records = load_ipo_queue_records(save_dir, as_of_date).unwrap_or_default();
    let Some(latest) = records
        .iter()
        .filter_map(|record| {
            NaiveDate::parse_from_str(&record.date, "%Y-%m-%d")
                .ok()
                .map(|date| (date, record))
        })
        .max_by_key(|(date, _)| *date)
    else {
        return serde_json::json!({
            "configured": false
        });
    };
    let window_start = as_of_date - Duration::days(6);
    let weekly_queue_counts = records
        .iter()
        .filter_map(|record| {
            NaiveDate::parse_from_str(&record.date, "%Y-%m-%d")
                .ok()
                .map(|date| (date, record.queue_count))
        })
        .filter(|(date, _)| *date >= window_start && *date <= as_of_date)
        .map(|(_, queue_count)| queue_count)
        .collect::<Vec<_>>();
    serde_json::json!({
        "configured": true,
        "latest_date": latest.1.date,
        "queue_count_latest": latest.1.queue_count,
        "queue_count_min_7d": weekly_queue_counts.iter().min().copied().unwrap_or(latest.1.queue_count),
        "queue_count_max_7d": weekly_queue_counts.iter().max().copied().unwrap_or(latest.1.queue_count),
        "reported_count_latest": latest.1.reported_count,
        "confirmed_count_latest": latest.1.confirmed_count,
        "pressure_latest": latest.1.pressure
    })
}

fn load_ipo_queue_records(
    save_dir: &Path,
    as_of_date: NaiveDate,
) -> Result<Vec<CapitalAbsorptionIpoQueueRecord>> {
    let path = save_dir.join(LEDGER_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file =
        fs::File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line.with_context(|| format!("Failed to read {}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: CapitalAbsorptionIpoQueueRecord = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        let Ok(date) = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d") else {
            continue;
        };
        if date <= as_of_date {
            records.push(record);
        }
    }
    Ok(records)
}

fn write_ipo_queue_record(save_dir: &Path, record: &CapitalAbsorptionIpoQueueRecord) -> Result<()> {
    let latest = save_dir.join(LATEST_FILE);
    let daily = save_dir.join(format!(
        "capital_absorption_ipo_queue_history_{}.json",
        record.date
    ));
    let serialized = serde_json::to_string(record)?;
    fs::write(&latest, format!("{serialized}\n"))
        .with_context(|| format!("Failed to write {}", latest.display()))?;
    fs::write(&daily, format!("{serialized}\n"))
        .with_context(|| format!("Failed to write {}", daily.display()))?;

    // ledger の重複書き込みを防止するため、同一日付の既存レコードを上書きする
    let ledger = save_dir.join(LEDGER_FILE);
    let mut records = Vec::new();
    if ledger.exists() {
        let file = fs::File::open(&ledger)
            .with_context(|| format!("Failed to open {}", ledger.display()))?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.with_context(|| format!("Failed to read {}", ledger.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            let r: CapitalAbsorptionIpoQueueRecord = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse {}", ledger.display()))?;
            if r.date != record.date {
                records.push(r);
            }
        }
    }
    records.push(record.clone());
    records.sort_by(|a, b| a.date.cmp(&b.date));

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&ledger)
        .with_context(|| format!("Failed to open {}", ledger.display()))?;
    for r in records {
        let ser = serde_json::to_string(&r)?;
        writeln!(file, "{ser}").with_context(|| format!("Failed to write {}", ledger.display()))?;
    }
    Ok(())
}

fn record_from_snapshot(
    as_of_date: NaiveDate,
    snapshot: &CapitalAbsorptionAutoSnapshot,
) -> CapitalAbsorptionIpoQueueRecord {
    CapitalAbsorptionIpoQueueRecord {
        date: as_of_date.to_string(),
        queue_count: snapshot.potential_supply_pressure.queue_count,
        reported_count: snapshot.potential_supply_pressure.reported_count,
        confirmed_count: snapshot.potential_supply_pressure.confirmed_count,
        pressure: pressure_level_code(snapshot.potential_supply_pressure.level).to_string(),
        items: snapshot
            .ai_ipo_queue
            .iter()
            .filter(|item| item.source_count > 0)
            .map(record_item_from_queue_item)
            .collect(),
    }
}

fn record_item_from_queue_item(
    item: &CapitalAbsorptionIpoQueueItem,
) -> CapitalAbsorptionIpoQueueRecordItem {
    CapitalAbsorptionIpoQueueRecordItem {
        issuer: item.issuer.clone(),
        ipo_stage: ipo_stage_code(item.status).to_string(),
        event_type: event_type_code(item.event_type).to_string(),
        source_count: item.source_count,
    }
}

fn history_from_records(
    records: &[CapitalAbsorptionIpoQueueRecord],
    as_of_date: NaiveDate,
) -> Vec<CapitalAbsorptionIpoQueueHistoryPoint> {
    let first_date = as_of_date - Duration::days(29);
    (0..30)
        .map(|offset| first_date + Duration::days(offset))
        .map(|observed_at| {
            let queue_size = records
                .iter()
                .filter_map(|record| {
                    NaiveDate::parse_from_str(&record.date, "%Y-%m-%d")
                        .ok()
                        .map(|date| (date, record.queue_count))
                })
                .filter(|(date, _)| *date <= observed_at)
                .max_by_key(|(date, _)| *date)
                .map(|(_, queue_count)| queue_count)
                .unwrap_or(0);
            CapitalAbsorptionIpoQueueHistoryPoint {
                observed_at,
                queue_size,
            }
        })
        .collect()
}

fn ipo_stage_code(status: CapitalAbsorptionIpoQueueStatus) -> &'static str {
    match status {
        CapitalAbsorptionIpoQueueStatus::Rumor => "Rumor",
        CapitalAbsorptionIpoQueueStatus::Reported => "Reported",
        CapitalAbsorptionIpoQueueStatus::Preparation => "Preparation",
        CapitalAbsorptionIpoQueueStatus::PreIpo => "Pre-IPO",
        CapitalAbsorptionIpoQueueStatus::Filed => "Filed",
        CapitalAbsorptionIpoQueueStatus::Ipo => "IPO",
    }
}

fn event_type_code(event_type: CapitalAbsorptionObservationEventType) -> &'static str {
    match event_type {
        CapitalAbsorptionObservationEventType::Rumor => "Rumor",
        CapitalAbsorptionObservationEventType::Reported => "Reported",
        CapitalAbsorptionObservationEventType::Confirmed => "Confirmed",
    }
}

fn pressure_level_code(level: CapitalAbsorptionPotentialSupplyPressureLevel) -> &'static str {
    match level {
        CapitalAbsorptionPotentialSupplyPressureLevel::Low => "LOW",
        CapitalAbsorptionPotentialSupplyPressureLevel::Normal => "NORMAL",
        CapitalAbsorptionPotentialSupplyPressureLevel::Elevated => "ELEVATED",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::application::capital_absorption::{
        build_capital_absorption_snapshot_from_events, CapitalAbsorptionAutoConfidence,
        CapitalAbsorptionAutoEvent, CapitalAbsorptionAutoEventCategory,
        CapitalAbsorptionSourceHealth, CapitalAbsorptionSourceStatus, CapitalAbsorptionSupplyKind,
    };

    #[test]
    fn capital_absorption_ipo_queue_store_replays_thirty_day_history_without_future_records() {
        let tmp = tempfile::tempdir().expect("tempdir should be created");
        let ledger = tmp.path().join(LEDGER_FILE);
        fs::write(
            &ledger,
            r#"{"date":"2026-06-01","queue_count":1,"reported_count":1,"confirmed_count":0,"pressure":"NORMAL","items":[{"issuer":"SpaceX","ipo_stage":"Reported","event_type":"Reported","source_count":2}]}
{"date":"2026-06-20","queue_count":6,"reported_count":6,"confirmed_count":1,"pressure":"ELEVATED","items":[]}
"#,
        )
        .expect("ledger should be written");
        let mut snapshot = build_capital_absorption_snapshot_from_events(
            vec![event("OpenAI", "OpenAI IPO reported by multiple outlets")],
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        persist_and_replay_ipo_queue_history(
            tmp.path(),
            NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
            &mut snapshot,
        )
        .expect("queue history should be persisted");

        assert_eq!(snapshot.ipo_queue_history.len(), 30);
        assert_eq!(
            snapshot
                .ipo_queue_history
                .iter()
                .find(|point| point.observed_at == NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
                .map(|point| point.queue_size),
            Some(1)
        );
        assert_eq!(
            snapshot
                .ipo_queue_history
                .last()
                .map(|point| point.queue_size),
            Some(1)
        );
        let latest = fs::read_to_string(tmp.path().join(LATEST_FILE))
            .expect("latest record should be written");
        assert!(latest.contains(r#""date":"2026-06-10""#));
        assert!(latest.contains(r#""issuer":"OpenAI""#));
        assert!(!latest.contains("2026-06-20"));
    }

    #[test]
    fn capital_absorption_ipo_queue_store_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir should be created");
        let mut snapshot = build_capital_absorption_snapshot_from_events(
            vec![event("OpenAI", "OpenAI IPO reported by multiple outlets")],
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        // 1回目の書き込み
        persist_and_replay_ipo_queue_history(
            tmp.path(),
            NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
            &mut snapshot,
        )
        .expect("first write should succeed");

        // 2回目の書き込み（同日）
        persist_and_replay_ipo_queue_history(
            tmp.path(),
            NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
            &mut snapshot,
        )
        .expect("second write should succeed");

        // ledgerファイルに同一日付のレコードが1行だけ存在することを確認
        let ledger = tmp.path().join(LEDGER_FILE);
        let content = fs::read_to_string(&ledger).expect("ledger should be readable");
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains(r#""date":"2026-06-10""#));
    }

    fn event(subject: &str, description: &str) -> CapitalAbsorptionAutoEvent {
        CapitalAbsorptionAutoEvent {
            category: CapitalAbsorptionAutoEventCategory::IpoSupply,
            supply_kind: CapitalAbsorptionSupplyKind::Potential,
            event_type: CapitalAbsorptionObservationEventType::Reported,
            subject: subject.to_string(),
            description: description.to_string(),
            amount_usd_b: None,
            ai_capex_related: true,
            source_url: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
            source_count: 1,
            confidence: CapitalAbsorptionAutoConfidence::Low,
        }
    }
}
