use crate::features::backtest::application::decision_engine::BacktestDecisionEngine;
use crate::features::radar::application::engine::Engine;
use crate::features::radar::application::provider::TickerHistory;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::rules::{ParsedRules, WatchlistEntry};
use crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord;
use anyhow::Result;
use std::collections::HashMap;

/// Backtest から radar decision pipeline を呼び出す防腐層。
pub(crate) struct RadarBacktestDecisionEngine;

impl BacktestDecisionEngine for RadarBacktestDecisionEngine {
    fn run_daily_pipeline<'a>(
        &self,
        ticker_histories: &[(TickerHistory<'a>, &WatchlistEntry)],
        rules: &ParsedRules,
        history: &[DecisionPacket],
        evidence_history: &[AutomatedEvidenceRecord],
        positions: &HashMap<String, (f64, f64)>,
    ) -> Result<DecisionPacket> {
        Engine::run_daily_pipeline(
            ticker_histories,
            rules,
            history,
            evidence_history,
            positions,
        )
    }
}
