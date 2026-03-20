use crate::config::AppConfig;
use crate::core::engine::Engine;
use crate::core::decision::DecisionPacket;
use crate::core::action_matrix::AssetAction;
use crate::data::yahoo_provider::{fetch_history, TickerHistory};
use anyhow::Result;
use chrono::NaiveDate;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use time::OffsetDateTime;

pub async fn run_backtest(
    config: &AppConfig,
    from_date_str: &str,
    to_date_str: &str,
) -> Result<()> {
    let from_date = NaiveDate::parse_from_str(from_date_str, "%Y-%m-%d")?;
    let to_date = NaiveDate::parse_from_str(to_date_str, "%Y-%m-%d")?;

    let from_dt = OffsetDateTime::from_unix_timestamp(
        from_date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
    ).ok();
    let to_dt = OffsetDateTime::from_unix_timestamp(
        to_date.and_hms_opt(23, 59, 59).unwrap().and_utc().timestamp(),
    ).ok();

    println!("📊 Fetching history for backtest from {} to {}...", from_date_str, to_date_str);

    let mut histories = HashMap::new();
    for entry in config.watchlist.iter().filter(|w| w.enable) {
        println!("   Fetching {}...", entry.symbol);
        if let Ok(hist) = fetch_history(&entry.symbol, from_dt, to_dt).await {
            histories.insert(entry.symbol.clone(), hist);
        }
    }

    if histories.is_empty() {
        return Err(anyhow::anyhow!("No history fetched."));
    }

    let index_symbol = if histories.contains_key("SPY") { "SPY".to_string() } else { histories.keys().next().unwrap().clone() };
    let index_history = histories.get(&index_symbol).unwrap();
    let mut index_prices = BTreeMap::new();
    let mut simulation_dates = Vec::new();
    for bar in index_history.bars.iter() {
        if bar.date >= from_date && bar.date <= to_date {
            simulation_dates.push(bar.date);
        }
        index_prices.insert(bar.date, bar.close);
    }
    simulation_dates.sort();

    println!("🧪 Simulating {} trading days using Modular Pipeline...", simulation_dates.len());

    let parsed_rules = config.get_parsed_rules();
    let mut transition_matrix: HashMap<(String, String), usize> = HashMap::new();
    let mut prev_packet: Option<DecisionPacket> = None;
    let mut reliability: HashMap<String, (usize, usize)> = HashMap::new();
    
    let mut regime_tracking: HashMap<String, RegimeStats> = HashMap::new();
    let mut potential_records: Vec<(NaiveDate, f64)> = Vec::new();

    #[derive(Default, Debug)]
    struct RegimeStats {
        total_signals: usize,
        correct_signals: usize,
        sum_20d_return: f64,
        sum_max_drawdown_20d: f64,
        count_drawdowns: usize,
    }

    // Optimization: Track indices for each asset to avoid O(N^2) slicing & cloning.
    let mut asset_indices: HashMap<String, usize> = HashMap::new();
    for sym in histories.keys() {
        asset_indices.insert(sym.clone(), 0);
    }

    for current_date in simulation_dates.iter() {
        let mut daily_histories = Vec::new();

        for entry in config.watchlist.iter().filter(|w| w.enable) {
            if let Some(hist) = histories.get(&entry.symbol) {
                let idx = asset_indices.get_mut(&entry.symbol).unwrap();
                while *idx < hist.bars.len() && hist.bars[*idx].date <= *current_date {
                    *idx += 1;
                }
                
                if *idx > 0 {
                    let segmented_history = TickerHistory {
                        symbol: entry.symbol.clone(),
                        bars: Cow::Borrowed(&hist.bars[0..*idx]),
                        total_trading_days: hist.total_trading_days,
                        latest_quote_timestamp: None,
                    };
                    daily_histories.push((segmented_history, entry));
                }
            }
        }

        if daily_histories.is_empty() { continue; }

        // Core Decision Pipeline (Phase 7 Refactored)
        let current_packet = Engine::run_daily_pipeline(&daily_histories, &parsed_rules, prev_packet.as_ref())?;



        // Track Transitions (Market Level)
        if let Some(ref prev) = prev_packet {
            if prev.market_regime.market_state != current_packet.market_regime.market_state {
                let key = (format!("{:?}", prev.market_regime.market_state), format!("{:?}", current_packet.market_regime.market_state));
                *transition_matrix.entry(key).or_insert(0) += 1;
            }
        }

        // Potential Energy
        potential_records.push((*current_date, current_packet.market_features.potential_energy));

        // Asset Level Metrics
        for asset in &current_packet.assets {
            let state_str = format!("{:?}", asset.state);
            let reg_entry = regime_tracking.entry(state_str.clone()).or_default();
            
            // Resolve 20-day forward return with direct indexing
            if let Some(full_hist) = histories.get(&asset.symbol) {
                let current_idx = asset_indices.get(&asset.symbol).cloned().unwrap_or(0).saturating_sub(1);
                let future_idx = current_idx + 20;

                if future_idx < full_hist.bars.len() {
                    let curr = &full_hist.bars[current_idx];
                    let fut = &full_hist.bars[future_idx];

                    let fwd_return = (fut.close - curr.close) / curr.close;
                    
                    reg_entry.total_signals += 1;
                    reg_entry.sum_20d_return += fwd_return;

                    // Max Drawdown in 20d (Window slice instead of find)
                    let search_end = future_idx.min(full_hist.bars.len() - 1);
                    let mut min_price = curr.close;
                    for b in &full_hist.bars[current_idx..=search_end] {
                        if b.close < min_price { min_price = b.close; }
                    }
                    reg_entry.sum_max_drawdown_20d += (min_price - curr.close) / curr.close;
                    reg_entry.count_drawdowns += 1;

                    // Correctness (Simplified: POSITIVE for Bull states, NEGATIVE for Bear states)
                    let is_bear = asset.action == AssetAction::REDUCE || asset.action == AssetAction::FREEZE || asset.action == AssetAction::AVOID;
                    let is_correct = if is_bear { fwd_return < 0.0 } else { fwd_return > 0.0 };
                    if is_correct {
                        reg_entry.correct_signals += 1;
                    }

                        // Reliability
                        let conf_bucket = match current_packet.market_features.system_confidence as usize {
                            90..=100 => "90-100",
                            80..=89 => "80-90",
                            70..=79 => "70-80",
                            60..=69 => "60-70",
                            50..=59 => "50-60",
                            _ => "<50",
                        };

                        let rel_entry = reliability.entry(conf_bucket.to_string()).or_insert((0, 0));
                        rel_entry.0 += 1;
                        if is_correct { rel_entry.1 += 1; }
                }
            }
        }

        prev_packet = Some(current_packet);
    }

    // Write Outputs
    fs::create_dir_all("backtest")?;
    
    // Summary Generation
    let mut summary = String::new();
    summary.push_str("# 🔭 Backtest Summary (Phase 7 Modular Pipeline)\n\n");
    
    summary.push_str("## 1. Reliability Calibration\n");
    summary.push_str("| Confidence Bucket | Total | Correct | Win Rate |\n|---|---|---|---|\n");
    let mut rel_vec: Vec<_> = reliability.into_iter().collect();
    rel_vec.sort_by(|a, b| b.0.cmp(&a.0));
    for (b, (t, c)) in rel_vec {
        summary.push_str(&format!("| {} | {} | {} | {:.1}% |\n", b, t, c, (c as f64 / t as f64) * 100.0));
    }

    summary.push_str("\n## 2. Regime Performance Audit\n");
    summary.push_str("| AssetState | Signals | Hit Rate | Avg 20d Return | Max Drawdown |\n|---|---|---|---|---|\n");
    let mut reg_vec: Vec<_> = regime_tracking.into_iter().collect();
    reg_vec.sort_by(|a, b| b.1.total_signals.cmp(&a.1.total_signals));
    for (state, stats) in reg_vec {
        if stats.total_signals == 0 { continue; }
        summary.push_str(&format!("| {} | {} | {:.1}% | {:+.2}% | {:+.2}% |\n", 
            state, stats.total_signals, 
            (stats.correct_signals as f64 / stats.total_signals as f64) * 100.0,
            (stats.sum_20d_return / stats.total_signals as f64) * 100.0,
            (stats.sum_max_drawdown_20d / stats.total_signals as f64) * 100.0
        ));
    }

    summary.push_str("\n## 3. Transition Matrix\n");
    summary.push_str("| From | To | Count |\n|---|---|---|\n");
    for ((from, to), count) in transition_matrix {
        summary.push_str(&format!("| {} | {} | {} |\n", from, to, count));
    }

    fs::write("backtest/summary.md", summary)?;
    println!("✅ Backtest complete! Output written to ./backtest/summary.md");

    Ok(())
}
