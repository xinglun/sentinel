use crate::config::{ParsedRules, WatchlistEntry};
use crate::features::radar::application::provider::TickerHistory;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord;
use anyhow::Result;
use std::collections::HashMap;

/// Backtest が利用する decision pipeline port。
pub trait BacktestDecisionEngine {
    fn run_daily_pipeline<'a>(
        &self,
        ticker_histories: &[(TickerHistory<'a>, &WatchlistEntry)],
        rules: &ParsedRules,
        history: &[DecisionPacket],
        evidence_history: &[AutomatedEvidenceRecord],
        positions: &HashMap<String, (f64, f64)>,
    ) -> Result<DecisionPacket>;
}
