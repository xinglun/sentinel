use crate::features::backtest::application::decision_engine::BacktestDecisionEngine;
use crate::features::backtest::application::model::{
    BacktestComparisonReport, BacktestRules, BacktestSimulationReport, BacktestTickerHistory,
    BacktestWatchlistEntry,
};
use crate::features::backtest::application::simulation::run_core_simulation;
use anyhow::Result;
use chrono::NaiveDate;
use std::collections::HashMap;

/// baseline / enhanced の比較 simulation を実行する application use case。
pub fn run_comparative_backtest(
    baseline_engine: &dyn BacktestDecisionEngine,
    enhanced_engine: &dyn BacktestDecisionEngine,
    histories: &HashMap<String, BacktestTickerHistory>,
    watchlist: &[BacktestWatchlistEntry],
    simulation_dates: &[NaiveDate],
    parsed_rules: &BacktestRules,
) -> Result<BacktestComparisonReport> {
    let baseline = run_named_simulation(
        baseline_engine,
        histories,
        watchlist,
        simulation_dates,
        parsed_rules,
        false,
        "baseline",
    )?;
    let enhanced = run_named_simulation(
        enhanced_engine,
        histories,
        watchlist,
        simulation_dates,
        parsed_rules,
        true,
        "enhanced",
    )?;

    Ok(BacktestComparisonReport { baseline, enhanced })
}

fn run_named_simulation(
    decision_engine: &dyn BacktestDecisionEngine,
    histories: &HashMap<String, BacktestTickerHistory>,
    watchlist: &[BacktestWatchlistEntry],
    simulation_dates: &[NaiveDate],
    parsed_rules: &BacktestRules,
    use_memory: bool,
    name: &str,
) -> Result<BacktestSimulationReport> {
    run_core_simulation(
        decision_engine,
        histories,
        watchlist,
        simulation_dates,
        parsed_rules,
        use_memory,
        name,
    )
}
