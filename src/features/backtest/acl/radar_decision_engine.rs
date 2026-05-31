use crate::features::backtest::application::decision_engine::BacktestDecisionEngine;
use crate::features::backtest::application::model::{
    BacktestAssetAction, BacktestAssetSnapshot, BacktestAssetState, BacktestBreakoutStatus,
    BacktestDecisionSnapshot, BacktestTickerHistory, BacktestTransitionAudit, BacktestTrendStatus,
    BacktestTrendTopology,
};
use crate::features::radar::application::engine::Engine;
use crate::features::radar::application::provider::TickerHistory;
use crate::features::radar::domain::action_matrix::AssetAction;
use crate::features::radar::domain::asset_state::AssetState;
use crate::features::radar::domain::breakout_detection::{BreakoutReason, BreakoutStatus};
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::market_regime::LifecycleState;
use crate::features::radar::domain::rules::{ParsedRules, WatchlistEntry};
use crate::features::radar::domain::trend_cohesion::{
    AutomatedEvidenceRecord, TrendCohesionStatus, TrendCohesionTopology,
};
use anyhow::Result;
use std::cell::RefCell;
use std::collections::HashMap;

/// Backtest から radar decision pipeline を呼び出す防腐層。
pub(crate) struct RadarBacktestDecisionEngine {
    rules: ParsedRules,
    watchlist: HashMap<String, WatchlistEntry>,
    evidence_history: Vec<AutomatedEvidenceRecord>,
    radar_history: RefCell<Vec<DecisionPacket>>,
}

impl RadarBacktestDecisionEngine {
    pub(crate) fn new(rules: ParsedRules, watchlist: Vec<WatchlistEntry>) -> Self {
        Self {
            rules,
            watchlist: watchlist
                .into_iter()
                .map(|entry| (entry.symbol.clone(), entry))
                .collect(),
            evidence_history: Vec::new(),
            radar_history: RefCell::new(Vec::with_capacity(20)),
        }
    }

    fn to_snapshot(packet: &DecisionPacket) -> BacktestDecisionSnapshot {
        BacktestDecisionSnapshot {
            date: packet.date,
            market_state: format!("{:?}", packet.market_regime.market_state),
            trend_gate_passed: packet.trend_cohesion.gate_passed,
            trend_status: match packet.trend_cohesion.status {
                TrendCohesionStatus::Dispersed => BacktestTrendStatus::Dispersed,
                TrendCohesionStatus::Forming => BacktestTrendStatus::Forming,
                TrendCohesionStatus::Formed => BacktestTrendStatus::Formed,
            },
            trend_topology: match packet.trend_cohesion.topology {
                TrendCohesionTopology::NoLeader => BacktestTrendTopology::NoLeader,
                TrendCohesionTopology::SingleLeader => BacktestTrendTopology::SingleLeader,
                TrendCohesionTopology::FragmentedLeaders => {
                    BacktestTrendTopology::FragmentedLeaders
                }
            },
            transition_audit: packet.market_regime.transition_audit.as_ref().map(|audit| {
                BacktestTransitionAudit {
                    from: lifecycle_state_code(audit.from),
                    to: lifecycle_state_code(audit.to),
                    is_reset_blocked: audit.is_reset_blocked,
                    is_downgrade_clamped: audit.is_downgrade_clamped,
                    duration_locked: audit.duration_locked,
                    soft_reset_applied: audit.soft_reset_applied,
                    defensive_override: audit.defensive_override,
                }
            }),
            potential_energy: packet.market_features.potential_energy,
            system_confidence: packet.market_features.system_confidence,
            assets: packet
                .assets
                .iter()
                .map(|asset| BacktestAssetSnapshot {
                    symbol: asset.symbol.clone(),
                    action: match asset.action {
                        AssetAction::REDUCE => BacktestAssetAction::Reduce,
                        AssetAction::FREEZE => BacktestAssetAction::Freeze,
                        AssetAction::AVOID => BacktestAssetAction::Avoid,
                        _ => BacktestAssetAction::Other,
                    },
                    deviation: asset.deviation,
                    asset_state: match asset.asset_state.state {
                        AssetState::OPTIMAL => BacktestAssetState::Optimal,
                        _ => BacktestAssetState::Other,
                    },
                    breakout_eligible: asset.breakout.breakout_eligible,
                    breakout_status: match asset.breakout.status {
                        BreakoutStatus::NoBreakout => BacktestBreakoutStatus::NoBreakout,
                        BreakoutStatus::EmergingBreakout => {
                            BacktestBreakoutStatus::EmergingBreakout
                        }
                        BreakoutStatus::ConfirmedBreakout => {
                            BacktestBreakoutStatus::ConfirmedBreakout
                        }
                    },
                    breakout_failed_risk: asset
                        .breakout
                        .reasons
                        .contains(&BreakoutReason::FailedBreakoutRisk),
                    reasons: asset.reasons.clone(),
                })
                .collect(),
        }
    }
}

impl BacktestDecisionEngine for RadarBacktestDecisionEngine {
    fn run_daily_pipeline<'a>(
        &self,
        ticker_histories: &[BacktestTickerHistory<'a>],
        history: &[BacktestDecisionSnapshot],
        positions: &HashMap<String, (f64, f64)>,
    ) -> Result<BacktestDecisionSnapshot> {
        let radar_histories = ticker_histories
            .iter()
            .filter_map(|history| {
                self.watchlist.get(&history.symbol).map(|entry| {
                    (
                        TickerHistory {
                            symbol: history.symbol.clone(),
                            bars: history.bars.clone(),
                            total_trading_days: history.total_trading_days,
                            latest_quote_timestamp: None,
                        },
                        entry,
                    )
                })
            })
            .collect::<Vec<_>>();
        let stored_history = self.radar_history.borrow();
        let effective_history = if history.is_empty() {
            &[][..]
        } else {
            stored_history.as_slice()
        };
        let packet = Engine::run_daily_pipeline(
            &radar_histories,
            &self.rules,
            effective_history,
            &self.evidence_history,
            positions,
        )?;
        drop(stored_history);
        let mut mutable_history = self.radar_history.borrow_mut();
        mutable_history.push(packet.clone());
        if mutable_history.len() > 20 {
            mutable_history.remove(0);
        }
        Ok(Self::to_snapshot(&packet))
    }
}

fn lifecycle_state_code(state: LifecycleState) -> String {
    format!("{:?}", state)
}
