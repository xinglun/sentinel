use crate::features::backtest::application::model::{
    BacktestDecisionSnapshot, BacktestTickerHistory,
};
use anyhow::Result;
use std::collections::HashMap;

/// Backtest が利用する decision pipeline port。
pub trait BacktestDecisionEngine {
    fn run_daily_pipeline<'a>(
        &self,
        ticker_histories: &[BacktestTickerHistory<'a>],
        history: &[BacktestDecisionSnapshot],
        positions: &HashMap<String, (f64, f64)>,
    ) -> Result<BacktestDecisionSnapshot>;
}
