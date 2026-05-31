use crate::features::backtest::acl::radar_backtest_mapper::decision_packet_to_snapshot;
use crate::features::backtest::application::decision_engine::BacktestDecisionEngine;
use crate::features::backtest::application::model::{
    BacktestDecisionSnapshot, BacktestTickerHistory,
};
use crate::features::radar::application::engine::Engine;
use crate::features::radar::application::provider::TickerHistory;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::rules::{ParsedRules, WatchlistEntry};
use crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord;
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
        Ok(decision_packet_to_snapshot(&packet))
    }
}
