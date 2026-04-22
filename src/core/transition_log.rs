use crate::core::breakout_detection::BreakoutStatus;
use crate::core::decision::DecisionPacket;
use crate::core::market_regime::{MarketState, RiskOverlay};
use crate::core::trend_cohesion::{TrendCohesionStatus, TrendCohesionTopology};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct GateTransition {
    pub from: bool,
    pub to: bool,
    pub unmet_conditions_changed: bool,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub persisting: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct StatusTransition<T: Serialize + PartialEq> {
    pub from: T,
    pub to: T,
    pub changed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct BreakoutTransition {
    pub symbol: String,
    pub from_status: BreakoutStatus,
    pub to_status: BreakoutStatus,
    pub status_changed: bool,
    pub risk_changed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct StateTransitionLog {
    pub no_trade_persists: bool,
    pub market_state: StatusTransition<MarketState>,
    pub risk_overlay: StatusTransition<RiskOverlay>,
    pub trend_cohesion_gate: GateTransition,
    pub trend_cohesion_status: StatusTransition<TrendCohesionStatus>,
    pub trend_cohesion_topology: StatusTransition<TrendCohesionTopology>,
    pub breakout_changes: Vec<BreakoutTransition>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TransitionAuditSummary {
    pub market_state_changed: bool,
    pub risk_overlay_changed: bool,
    pub trend_gate_changed: bool,
    pub trend_unmet_changed: bool,
    pub breakout_change_count: usize,
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
    fn from_log(now: DateTime<Local>, log: &StateTransitionLog) -> Self {
        let transition = log.clone();
        let summary = TransitionAuditSummary {
            market_state_changed: transition.market_state.changed,
            risk_overlay_changed: transition.risk_overlay.changed,
            trend_gate_changed: transition.trend_cohesion_gate.from
                != transition.trend_cohesion_gate.to,
            trend_unmet_changed: transition.trend_cohesion_gate.unmet_conditions_changed,
            breakout_change_count: transition.breakout_changes.len(),
        };

        Self {
            schema_version: 2,
            event_type: "state_transition".to_string(),
            timestamp: now.to_rfc3339(),
            date: now.date_naive().to_string(),
            transition: transition.clone(),
            legacy_log: transition,
            summary,
        }
    }
}

impl StateTransitionLog {
    pub fn compare(prev: Option<&DecisionPacket>, curr: &DecisionPacket) -> Self {
        let no_trade_prev = prev.map(|p| !p.trend_cohesion.gate_passed).unwrap_or(true);
        let no_trade_curr = !curr.trend_cohesion.gate_passed;

        let market_state = StatusTransition {
            from: prev
                .map(|p| p.market_regime.market_state)
                .unwrap_or_default(),
            to: curr.market_regime.market_state,
            changed: prev
                .map(|p| p.market_regime.market_state != curr.market_regime.market_state)
                .unwrap_or(true),
        };

        let risk_overlay = StatusTransition {
            from: prev
                .map(|p| p.market_regime.risk_overlay)
                .unwrap_or_default(),
            to: curr.market_regime.risk_overlay,
            changed: prev
                .map(|p| p.market_regime.risk_overlay != curr.market_regime.risk_overlay)
                .unwrap_or(true),
        };

        let (t_added, t_removed, t_persisting) = Self::diff_reasons(
            prev.map(|p| {
                p.trend_cohesion
                    .unmet_conditions
                    .iter()
                    .map(|c| format!("{:?}", c))
                    .collect()
            })
            .unwrap_or_default(),
            curr.trend_cohesion
                .unmet_conditions
                .iter()
                .map(|c| format!("{:?}", c))
                .collect(),
        );
        let trend_cohesion_gate = GateTransition {
            from: prev.map(|p| p.trend_cohesion.gate_passed).unwrap_or(false),
            to: curr.trend_cohesion.gate_passed,
            unmet_conditions_changed: !t_added.is_empty() || !t_removed.is_empty(),
            added: t_added,
            removed: t_removed,
            persisting: t_persisting,
        };

        let trend_cohesion_status = StatusTransition {
            from: prev.map(|p| p.trend_cohesion.status).unwrap_or_default(),
            to: curr.trend_cohesion.status,
            changed: prev
                .map(|p| p.trend_cohesion.status != curr.trend_cohesion.status)
                .unwrap_or(true),
        };

        let trend_cohesion_topology = StatusTransition {
            from: prev.map(|p| p.trend_cohesion.topology).unwrap_or_default(),
            to: curr.trend_cohesion.topology,
            changed: prev
                .map(|p| p.trend_cohesion.topology != curr.trend_cohesion.topology)
                .unwrap_or(true),
        };

        let mut breakout_changes = Vec::new();
        for curr_asset in &curr.assets {
            let prev_asset =
                prev.and_then(|p| p.assets.iter().find(|a| a.symbol == curr_asset.symbol));

            let from_status = prev_asset.map(|a| a.breakout.status).unwrap_or_default();
            let to_status = curr_asset.breakout.status;
            let status_changed = from_status != to_status;

            let prev_risk = prev_asset
                .map(|a| a.breakout.failed_breakout_risk)
                .unwrap_or(0.0);
            let curr_risk = curr_asset.breakout.failed_breakout_risk;
            let risk_changed = (prev_risk - curr_risk).abs() > 0.1;

            if status_changed || risk_changed {
                breakout_changes.push(BreakoutTransition {
                    symbol: curr_asset.symbol.clone(),
                    from_status,
                    to_status,
                    status_changed,
                    risk_changed,
                });
            }
        }

        Self {
            no_trade_persists: no_trade_prev && no_trade_curr,
            market_state,
            risk_overlay,
            trend_cohesion_gate,
            trend_cohesion_status,
            trend_cohesion_topology,
            breakout_changes,
        }
    }

    fn diff_reasons(
        prev: Vec<String>,
        curr: Vec<String>,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let curr_set: HashSet<String> = curr.into_iter().collect();
        let prev_set: HashSet<String> = prev.into_iter().collect();

        let added = curr_set
            .difference(&prev_set)
            .cloned()
            .collect::<Vec<String>>();
        let removed = prev_set
            .difference(&curr_set)
            .cloned()
            .collect::<Vec<String>>();
        let persisting = curr_set
            .intersection(&prev_set)
            .cloned()
            .collect::<Vec<String>>();

        (added, removed, persisting)
    }
}

/// Persists daily market state transitions for auditing.
///
/// - `state_transitions.jsonl`: Full structured audit trail (Audit-Grade).
/// - `state_transitions.csv`: Tabular summary view for human analysis.
pub struct TransitionLogger {
    log_path: PathBuf,
    jsonl_path: PathBuf,
}

impl TransitionLogger {
    const CSV_HEADER: &'static str = "Timestamp,No_Trade_Persists,Market_State_From,Market_State_To,Risk_Overlay_From,Risk_Overlay_To,Trend_Gate_From,Trend_Gate_To,Trend_Status_From,Trend_Status_To,Topology_From,Topology_To,Trend_New_Unmet,Trend_Resolved_Unmet,Trend_Stay_Unmet,Breakout_1,Breakout_2,Breakout_3,Breakout_Summary";

    pub fn new(save_dir: &Path) -> Self {
        Self {
            log_path: save_dir.join("state_transitions.csv"),
            jsonl_path: save_dir.join("state_transitions.jsonl"),
        }
    }

    /// Logs a transition to both summary and structural files.
    pub fn log_transition(&self, log: &StateTransitionLog) -> Result<()> {
        self.log_to_csv(log)?;
        self.log_to_jsonl(log)?;
        Ok(())
    }

    /// Generates a tabular summary view (Human-friendly).
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
            "{},{},{:?},{:?},{:?},{:?},{},{},{:?},{:?},{:?},{:?},\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
            Local::now().to_rfc3339(),
            log.no_trade_persists,
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

    /// Generates a structural audit trail (System-friendly, Audit-Grade).
    fn log_to_jsonl(&self, log: &StateTransitionLog) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.jsonl_path)
            .context("Failed to open state_transitions.jsonl")?;

        let record = TransitionAuditRecord::from_log(Local::now(), log);
        writeln!(file, "{}", serde_json::to_string(&record)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::action_matrix::AssetActionDecision;
    use crate::core::breakout_detection::BreakoutSnapshot;
    use tempfile::tempdir;

    fn mock_packet(state: MarketState, trend_gate_passed: bool) -> DecisionPacket {
        let mut curr = DecisionPacket::default();
        curr.market_regime.market_state = state;
        curr.trend_cohesion.gate_passed = trend_gate_passed;
        curr
    }

    #[test]
    fn test_no_trade_persists() {
        let prev = mock_packet(MarketState::DEFENSIVE, false);
        let curr = mock_packet(MarketState::DEFENSIVE, false);
        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert!(log.no_trade_persists);
        assert!(!log.market_state.changed);
        assert!(!log.trend_cohesion_gate.to);
    }

    #[test]
    fn test_gate_transition() {
        let prev = mock_packet(MarketState::IGNITION, false);
        let curr = mock_packet(MarketState::IGNITION, true);

        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert!(!log.no_trade_persists);
        assert!(log.trend_cohesion_gate.to);
        assert!(!log.trend_cohesion_gate.from);
    }

    #[test]
    fn test_breakout_transition() {
        let mut prev = mock_packet(MarketState::ESTABLISHED, true);
        prev.assets.push(AssetActionDecision {
            symbol: "AAPL".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::NoBreakout,
                failed_breakout_risk: 10.0,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut curr = mock_packet(MarketState::ESTABLISHED, true);
        curr.assets.push(AssetActionDecision {
            symbol: "AAPL".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                failed_breakout_risk: 10.0,
                ..Default::default()
            },
            ..Default::default()
        });

        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert_eq!(log.breakout_changes.len(), 1);
        assert_eq!(log.breakout_changes[0].symbol, "AAPL");
        assert_eq!(
            log.breakout_changes[0].from_status,
            BreakoutStatus::NoBreakout
        );
        assert_eq!(
            log.breakout_changes[0].to_status,
            BreakoutStatus::EmergingBreakout
        );
        assert!(log.breakout_changes[0].status_changed);
        assert!(!log.breakout_changes[0].risk_changed);
    }

    #[test]
    fn test_first_packet() {
        let curr = mock_packet(MarketState::IGNITION, false);
        let log = StateTransitionLog::compare(None, &curr);
        assert!(log.no_trade_persists);
        assert!(log.market_state.changed); // from default IGNITION to IGNITION is technically same but compare treats None as default
        assert!(!log.trend_cohesion_gate.to);
    }

    #[test]
    fn test_reason_diffing() {
        use crate::core::trend_cohesion::{TrendCohesionGateCondition, TrendCohesionSnapshot};

        let prev = DecisionPacket {
            trend_cohesion: TrendCohesionSnapshot {
                unmet_conditions: vec![TrendCohesionGateCondition::StabilityThreshold],
                ..Default::default()
            },
            ..Default::default()
        };

        let curr = DecisionPacket {
            trend_cohesion: TrendCohesionSnapshot {
                unmet_conditions: vec![TrendCohesionGateCondition::ContinuityThreshold],
                ..Default::default()
            },
            ..Default::default()
        };

        let log = StateTransitionLog::compare(Some(&prev), &curr);

        // Trend Gate
        assert_eq!(
            log.trend_cohesion_gate.added,
            vec!["ContinuityThreshold".to_string()]
        );
        assert_eq!(
            log.trend_cohesion_gate.removed,
            vec!["StabilityThreshold".to_string()]
        );
        assert!(log.trend_cohesion_gate.persisting.is_empty());
    }

    #[test]
    fn test_transition_logger_writes_versioned_structured_jsonl() {
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

        logger.log_transition(&log).unwrap();

        let jsonl = std::fs::read_to_string(dir.path().join("state_transitions.jsonl")).unwrap();
        let line = jsonl.lines().next().unwrap();
        let record: TransitionAuditRecord = serde_json::from_str(line).unwrap();

        assert_eq!(record.schema_version, 2);
        assert_eq!(record.event_type, "state_transition");
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
    }

    #[test]
    fn test_transition_logger_rotates_incompatible_csv_header() {
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
        logger.log_transition(&log).unwrap();

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
