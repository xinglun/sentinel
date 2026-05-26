use crate::features::radar::domain::transition_log::{BreakoutTransition, StateTransitionLog};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TransitionAuditSummary {
    pub market_state_changed: bool,
    pub risk_overlay_changed: bool,
    pub trend_gate_changed: bool,
    pub trend_unmet_changed: bool,
    pub breakout_change_count: usize,
    pub opportunity_mode_changed: bool,
    pub scout_reset_triggered: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TransitionAuditRecord {
    pub schema_version: u32,
    pub event_type: String,
    pub timestamp: String,
    pub date: String,
    pub transition: StateTransitionLog,
    #[serde(rename = "log")]
    pub legacy_log: StateTransitionLog,
    pub summary: TransitionAuditSummary,
}

impl TransitionAuditRecord {
    fn from_log(now: DateTime<Local>, business_date: NaiveDate, log: &StateTransitionLog) -> Self {
        let transition = log.clone();
        let summary = TransitionAuditSummary {
            market_state_changed: transition.market_state.changed,
            risk_overlay_changed: transition.risk_overlay.changed,
            trend_gate_changed: transition.trend_cohesion_gate.from
                != transition.trend_cohesion_gate.to,
            trend_unmet_changed: transition.trend_cohesion_gate.unmet_conditions_changed,
            breakout_change_count: transition.breakout_changes.len(),
            opportunity_mode_changed: transition.opportunity_mode.changed,
            scout_reset_triggered: transition.scout_reset_triggered,
        };

        Self {
            schema_version: 3,
            event_type: "state_transition".to_string(),
            timestamp: now.to_rfc3339(),
            date: business_date.to_string(),
            transition: transition.clone(),
            legacy_log: transition,
            summary,
        }
    }
}

/// 日次の市場状態遷移を監査用ファイルへ永続化する。
///
/// - `state_transitions.jsonl`: 構造化された監査ログ。
/// - `state_transitions.csv`: 人間が確認しやすい集計ビュー。
pub struct TransitionLogger {
    log_path: PathBuf,
    jsonl_path: PathBuf,
}

impl TransitionLogger {
    pub const CSV_HEADER: &'static str = "Timestamp,No_Trade_Persists,Opportunity_Mode_From,Opportunity_Mode_To,Scout_Days_Without_Expansion,Scout_Abort_Days,Scout_Reset_Triggered,Breakout_Active_Count,Market_State_From,Market_State_To,Risk_Overlay_From,Risk_Overlay_To,Trend_Gate_From,Trend_Gate_To,Trend_Status_From,Trend_Status_To,Topology_From,Topology_To,Trend_New_Unmet,Trend_Resolved_Unmet,Trend_Stay_Unmet,Breakout_1,Breakout_2,Breakout_3,Breakout_Summary";

    pub fn new(save_dir: &Path) -> Self {
        Self {
            log_path: save_dir.join("state_transitions.csv"),
            jsonl_path: save_dir.join("state_transitions.jsonl"),
        }
    }

    pub fn log_transition(&self, business_date: NaiveDate, log: &StateTransitionLog) -> Result<()> {
        self.log_to_csv(log)?;
        self.log_to_jsonl(business_date, log)?;
        Ok(())
    }

    fn log_to_csv(&self, log: &StateTransitionLog) -> Result<()> {
        let is_new_file = self.ensure_csv_schema_compatible()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .context("Failed to open state_transitions.csv")?;

        if is_new_file {
            writeln!(file, "{}", Self::CSV_HEADER)?;
        }

        let format_breakout = |b: &BreakoutTransition| {
            let status_change = if b.status_changed {
                format!("{:?}->{:?}", b.from_status, b.to_status)
            } else {
                format!("{:?}", b.to_status)
            };
            let risk_mark = if b.risk_changed { "(risk+)" } else { "" };
            format!("{}:{}{}", b.symbol, status_change, risk_mark)
        };

        let b1 = log
            .breakout_changes
            .first()
            .map(format_breakout)
            .unwrap_or_default();
        let b2 = log
            .breakout_changes
            .get(1)
            .map(format_breakout)
            .unwrap_or_default();
        let b3 = log
            .breakout_changes
            .get(2)
            .map(format_breakout)
            .unwrap_or_default();

        let breakouts_summary = log
            .breakout_changes
            .iter()
            .skip(3)
            .map(format_breakout)
            .collect::<Vec<_>>()
            .join(" | ");

        writeln!(
            file,
            "{},{},{:?},{:?},{},{},{},{},{:?},{:?},{:?},{:?},{},{},{:?},{:?},{:?},{:?},\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
            Local::now().to_rfc3339(),
            log.no_trade_persists,
            log.opportunity_mode.from,
            log.opportunity_mode.to,
            log.scout_days_without_expansion,
            log.scout_abort_days,
            log.scout_reset_triggered,
            log.breakout_active_count,
            log.market_state.from,
            log.market_state.to,
            log.risk_overlay.from,
            log.risk_overlay.to,
            log.trend_cohesion_gate.from,
            log.trend_cohesion_gate.to,
            log.trend_cohesion_status.from,
            log.trend_cohesion_status.to,
            log.trend_cohesion_topology.from,
            log.trend_cohesion_topology.to,
            log.trend_cohesion_gate.added.join("-"),
            log.trend_cohesion_gate.removed.join("-"),
            log.trend_cohesion_gate.persisting.join("-"),
            b1, b2, b3,
            breakouts_summary
        )?;

        Ok(())
    }

    fn ensure_csv_schema_compatible(&self) -> Result<bool> {
        if !self.log_path.exists() {
            return Ok(true);
        }

        let first_line = std::fs::read_to_string(&self.log_path)
            .ok()
            .and_then(|raw| raw.lines().next().map(|s| s.trim().to_string()))
            .unwrap_or_default();

        if first_line == Self::CSV_HEADER {
            return Ok(false);
        }

        let parent = self.log_path.parent().unwrap_or(Path::new("."));
        let legacy_name = format!(
            "state_transitions_legacy_{}.csv",
            Local::now().format("%Y%m%d_%H%M%S")
        );
        let legacy_path = parent.join(legacy_name);
        std::fs::rename(&self.log_path, &legacy_path).with_context(|| {
            format!(
                "Failed to rotate incompatible state_transitions.csv to {}",
                legacy_path.display()
            )
        })?;
        Ok(true)
    }

    fn log_to_jsonl(&self, business_date: NaiveDate, log: &StateTransitionLog) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.jsonl_path)
            .context("Failed to open state_transitions.jsonl")?;

        let record = TransitionAuditRecord::from_log(Local::now(), business_date, log);
        writeln!(file, "{}", serde_json::to_string(&record)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::action_matrix::AssetActionDecision;
    use crate::features::radar::domain::breakout_detection::{BreakoutSnapshot, BreakoutStatus};
    use crate::features::radar::domain::decision::DecisionPacket;
    use crate::features::radar::domain::market_regime::MarketState;
    use tempfile::tempdir;

    fn mock_packet(state: MarketState, trend_gate_passed: bool) -> DecisionPacket {
        let mut curr = DecisionPacket::default();
        curr.market_regime.market_state = state;
        curr.trend_cohesion.gate_passed = trend_gate_passed;
        curr
    }

    #[test]
    fn transition_logger_writes_versioned_structured_jsonl() {
        let dir = tempdir().unwrap();
        let logger = TransitionLogger::new(dir.path());
        let prev = mock_packet(MarketState::DEFENSIVE, false);
        let mut curr = mock_packet(MarketState::IGNITION, true);
        curr.assets.push(AssetActionDecision {
            symbol: "NVDA".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                ..Default::default()
            },
            ..Default::default()
        });
        let log = StateTransitionLog::compare(Some(&prev), &curr);

        let business_date = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();
        logger.log_transition(business_date, &log).unwrap();

        let jsonl = std::fs::read_to_string(dir.path().join("state_transitions.jsonl")).unwrap();
        let line = jsonl.lines().next().unwrap();
        let record: TransitionAuditRecord = serde_json::from_str(line).unwrap();

        assert_eq!(record.schema_version, 3);
        assert_eq!(record.event_type, "state_transition");
        assert_eq!(record.date, "2026-05-22");
        assert_eq!(record.transition, log);
        assert_eq!(record.legacy_log, log);
        assert_eq!(
            record.summary.market_state_changed,
            record.transition.market_state.changed
        );
        assert_eq!(
            record.summary.breakout_change_count,
            record.transition.breakout_changes.len()
        );
        assert_eq!(
            record.summary.opportunity_mode_changed,
            record.transition.opportunity_mode.changed
        );
        assert_eq!(
            record.summary.scout_reset_triggered,
            record.transition.scout_reset_triggered
        );
    }

    #[test]
    fn transition_logger_rotates_incompatible_csv_header() {
        let dir = tempdir().unwrap();
        let csv_path = dir.path().join("state_transitions.csv");
        std::fs::write(
            &csv_path,
            "Timestamp,No_Trade_Persists,Participation_From,Participation_To\nlegacy_row\n",
        )
        .unwrap();

        let logger = TransitionLogger::new(dir.path());
        let prev = mock_packet(MarketState::DEFENSIVE, false);
        let curr = mock_packet(MarketState::IGNITION, true);
        let log = StateTransitionLog::compare(Some(&prev), &curr);
        logger
            .log_transition(NaiveDate::from_ymd_opt(2026, 5, 22).unwrap(), &log)
            .unwrap();

        let new_csv = std::fs::read_to_string(&csv_path).unwrap();
        let first = new_csv.lines().next().unwrap_or_default();
        assert_eq!(first, TransitionLogger::CSV_HEADER);

        let legacy_exists = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("state_transitions_legacy_")
            });
        assert!(legacy_exists);
    }
}
