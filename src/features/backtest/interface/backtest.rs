use crate::config::AppConfig;
use crate::features::backtest::acl::radar_decision_engine::RadarBacktestDecisionEngine;
use crate::features::backtest::application::model::{
    BacktestRules, BacktestTickerHistory, BacktestWatchlistEntry,
};
use crate::features::backtest::application::simulation::run_core_simulation;
use crate::features::backtest::infrastructure::output::{
    generate_comparison_report, publish_primary_backtest_outputs, write_run_artifacts,
};
use crate::features::radar::application::provider::MarketDataProvider;
use crate::features::radar::domain::rules::{ParsedRules, WatchlistEntry};
use anyhow::Result;
use chrono::NaiveDate;
use std::collections::HashMap;
use time::OffsetDateTime;

pub async fn run_backtest(
    config: &AppConfig,
    provider: &(dyn MarketDataProvider + Send + Sync),
    from_date_str: &str,
    to_date_str: &str,
) -> Result<()> {
    let from_date = NaiveDate::parse_from_str(from_date_str, "%Y-%m-%d")?;
    let to_date = NaiveDate::parse_from_str(to_date_str, "%Y-%m-%d")?;

    let from_dt = OffsetDateTime::from_unix_timestamp(
        from_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp(),
    )
    .ok();
    let to_dt = OffsetDateTime::from_unix_timestamp(
        to_date
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .timestamp(),
    )
    .ok();

    println!(
        "📊 Fetching history for backtest from {} to {}...",
        from_date_str, to_date_str
    );

    let mut histories = HashMap::new();
    for entry in config.watchlist.iter().filter(|w| w.enable) {
        println!("   Fetching {}...", entry.symbol);
        if let Ok(hist) = provider.fetch_history(&entry.symbol, from_dt, to_dt).await {
            histories.insert(entry.symbol.clone(), hist);
        }
    }

    if histories.is_empty() {
        return Err(anyhow::anyhow!("No history fetched."));
    }

    let index_symbol = if histories.contains_key("SPY") {
        "SPY".to_string()
    } else {
        histories.keys().next().unwrap().clone()
    };
    let index_history = histories.get(&index_symbol).unwrap();
    let mut simulation_dates = Vec::new();
    for bar in index_history.bars.iter() {
        if bar.date >= from_date && bar.date <= to_date {
            simulation_dates.push(bar.date);
        }
    }
    simulation_dates.sort();

    println!(
        "🧪 Running Comparative Backtest (Baseline vs Enhanced) over {} days",
        simulation_dates.len()
    );

    let parsed_rules = ParsedRules::from(&config.get_parsed_rules());
    let watchlist: Vec<WatchlistEntry> =
        config.watchlist.iter().map(WatchlistEntry::from).collect();
    let backtest_histories = histories
        .iter()
        .map(|(symbol, history)| {
            (
                symbol.clone(),
                BacktestTickerHistory {
                    symbol: history.symbol.clone(),
                    bars: history.bars.clone(),
                    total_trading_days: history.total_trading_days,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let backtest_watchlist = watchlist
        .iter()
        .map(|entry| BacktestWatchlistEntry {
            symbol: entry.symbol.clone(),
            enable: entry.enable,
        })
        .collect::<Vec<_>>();
    let optimal_threshold = parsed_rules
        .sorted_bands
        .iter()
        .find(|(name, _)| name.to_lowercase().contains("optimal"))
        .map(|(_, threshold)| *threshold)
        .unwrap_or(f64::MAX);
    let backtest_rules = BacktestRules { optimal_threshold };

    // baseline（memory / friction なし）を実行する。
    println!("   [1/2] Running Baseline...");
    let baseline_artifacts = run_core_simulation(
        &RadarBacktestDecisionEngine::new(parsed_rules.clone(), watchlist.clone()),
        &backtest_histories,
        &backtest_watchlist,
        &simulation_dates,
        &backtest_rules,
        false,
        "baseline",
    )?;

    // enhanced（memory / friction あり）を実行する。
    println!("   [2/2] Running Enhanced (V1.4)...");
    let enhanced_artifacts = run_core_simulation(
        &RadarBacktestDecisionEngine::new(parsed_rules.clone(), watchlist.clone()),
        &backtest_histories,
        &backtest_watchlist,
        &simulation_dates,
        &backtest_rules,
        true,
        "enhanced",
    )?;

    write_run_artifacts("baseline", &baseline_artifacts)?;
    write_run_artifacts("enhanced", &enhanced_artifacts)?;

    // 比較 report を生成する。
    generate_comparison_report(&baseline_artifacts.metrics, &enhanced_artifacts.metrics)?;
    publish_primary_backtest_outputs()?;

    Ok(())
}
