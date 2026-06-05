use crate::features::backtest::application::decision_engine::BacktestDecisionEngine;
use crate::features::backtest::application::model::{
    BacktestAssetAction, BacktestAssetState, BacktestBreakoutStatus, BacktestDecisionSnapshot,
    BacktestRegimeAudit, BacktestReliabilityBucket, BacktestRules, BacktestSimulationReport,
    BacktestTickerHistory, BacktestTrendStatus, BacktestTrendTopology, BacktestWatchlistEntry,
};
use crate::features::backtest::domain::metrics::{RegimeStats, StateMachineMetrics};
use anyhow::Result;
use chrono::NaiveDate;
use std::borrow::Cow;
use std::collections::HashMap;

pub fn run_core_simulation(
    decision_engine: &dyn BacktestDecisionEngine,
    histories: &HashMap<String, BacktestTickerHistory>,
    watchlist: &[BacktestWatchlistEntry],
    simulation_dates: &[NaiveDate],
    parsed_rules: &BacktestRules,
    use_memory: bool,
    dir_name: &str,
) -> Result<BacktestSimulationReport> {
    let mut transition_matrix: HashMap<(String, String), usize> = HashMap::new();
    let mut prev_packet: Option<BacktestDecisionSnapshot> = None;
    let mut history_window: Vec<BacktestDecisionSnapshot> = Vec::with_capacity(20);
    let mut reliability: HashMap<String, (usize, usize)> = HashMap::new();

    let mut regime_tracking: HashMap<String, RegimeStats> = HashMap::new();
    let mut potential_records: Vec<(NaiveDate, f64)> = Vec::new();
    let mut sm_metrics = StateMachineMetrics::default();
    let mut state_history: Vec<String> = Vec::new();

    let mut asset_indices: HashMap<String, usize> = HashMap::new();
    let mut raw_top3_first_seen: HashMap<String, NaiveDate> = HashMap::new();
    let mut mem_top3_first_seen: HashMap<String, NaiveDate> = HashMap::new();

    let optimal_threshold = parsed_rules.optimal_threshold;

    for sym in histories.keys() {
        asset_indices.insert(sym.clone(), 0);
    }

    for current_date in simulation_dates.iter() {
        let mut daily_histories = Vec::new();

        for entry in watchlist.iter().filter(|w| w.enable) {
            if let Some(hist) = histories.get(&entry.symbol) {
                let idx = asset_indices.get_mut(&entry.symbol).unwrap();
                while *idx < hist.bars.len() && hist.bars[*idx].date <= *current_date {
                    *idx += 1;
                }

                if *idx > 0 {
                    let segmented_history = BacktestTickerHistory {
                        symbol: entry.symbol.clone(),
                        bars: Cow::Borrowed(&hist.bars[0..*idx]),
                        total_trading_days: hist.total_trading_days,
                    };
                    daily_histories.push(segmented_history);
                }
            }
        }

        // core decision pipeline。
        let effective_window: &[BacktestDecisionSnapshot] = if use_memory {
            history_window.as_slice()
        } else {
            &[]
        };
        let current_packet = decision_engine.run_daily_pipeline(
            &daily_histories,
            effective_window,
            &std::collections::HashMap::new(),
        )?;

        // history window を更新する。
        history_window.push(current_packet.clone());
        if history_window.len() > 20 {
            history_window.remove(0);
        }

        // metrics を集計する。
        sm_metrics.total_days += 1;
        state_history.push(current_packet.market_state.clone());
        if !current_packet.trend_gate_passed {
            sm_metrics.trend_gate_blocked_days += 1;
        }
        match current_packet.trend_status {
            BacktestTrendStatus::Dispersed => {
                sm_metrics.trend_status_dispersed_days += 1;
            }
            BacktestTrendStatus::Forming => {
                sm_metrics.trend_status_forming_days += 1;
            }
            BacktestTrendStatus::Formed => {
                sm_metrics.trend_status_formed_days += 1;
            }
        }
        match current_packet.trend_topology {
            BacktestTrendTopology::NoLeader => {
                sm_metrics.topology_no_leader_days += 1;
            }
            BacktestTrendTopology::SingleLeader => {
                sm_metrics.topology_single_leader_days += 1;
            }
            BacktestTrendTopology::FragmentedLeaders => {
                sm_metrics.topology_fragmented_leaders_days += 1;
            }
        }
        for asset in &current_packet.assets {
            sm_metrics.evaluated_asset_days += 1;
            if asset.breakout_eligible {
                sm_metrics.breakout_eligible_asset_days += 1;
            }
            match asset.breakout_status {
                BacktestBreakoutStatus::NoBreakout => {
                    sm_metrics.breakout_no_breakout_count += 1;
                }
                BacktestBreakoutStatus::EmergingBreakout => {
                    sm_metrics.breakout_emerging_count += 1;
                }
                BacktestBreakoutStatus::ConfirmedBreakout => {
                    sm_metrics.breakout_confirmed_count += 1;
                }
            }
            if asset.breakout_failed_risk {
                sm_metrics.breakout_failed_risk_count += 1;
            }
        }

        if let Some(ref prev) = prev_packet {
            if prev.market_state != current_packet.market_state {
                let key = (
                    prev.market_state.clone(),
                    current_packet.market_state.clone(),
                );
                *transition_matrix.entry(key).or_insert(0) += 1;
            }
        }

        if state_history.len() >= 5 {
            let window = &state_history[state_history.len() - 5..];
            let mut flips = 0;
            for i in 0..4 {
                if window[i] != window[i + 1] {
                    flips += 1;
                }
            }
            if flips >= 2 {
                sm_metrics.state_flip_count_5d += 1;
            }
        }

        if let Some(audit) = &current_packet.transition_audit {
            if audit.to == "IGNITION" && audit.from != "NEWBORN" {
                sm_metrics.reset_count += 1;
            }
            if audit.is_reset_blocked {
                sm_metrics.blocked_reset_count += 1;
            }
            if audit.is_downgrade_clamped {
                sm_metrics.multi_step_downgrade_attempt_count += 1;
            }
            if audit.duration_locked {
                sm_metrics.duration_lock_count += 1;
            }
            if audit.soft_reset_applied {
                sm_metrics.soft_reset_count += 1;
            }
            if audit.defensive_override {
                sm_metrics.defensive_override_count += 1;
            }
        }

        potential_records.push((*current_date, current_packet.potential_energy));

        // Asset Level Stability & Behavior Calibration (V1.4+)
        let current_top_actions: Vec<String> = current_packet
            .assets
            .iter()
            .take(3)
            .map(|a| a.symbol.clone())
            .collect();

        let mut raw_sorted_assets = current_packet.assets.clone();
        raw_sorted_assets.sort_by(|a, b| {
            b.deviation
                .unwrap_or(0.0)
                .partial_cmp(&a.deviation.unwrap_or(0.0))
                .unwrap()
        });
        let raw_top3: Vec<String> = raw_sorted_assets
            .iter()
            .take(3)
            .map(|a| a.symbol.clone())
            .collect();

        for asset in &current_packet.assets {
            let raw_score = asset.deviation.unwrap_or(0.0);
            let is_raw_optimal = raw_score >= optimal_threshold;
            let is_actual_optimal = asset.asset_state == BacktestAssetState::Optimal;

            if is_actual_optimal && !is_raw_optimal {
                sm_metrics.total_raw_vs_actual_divergence_days += 1;
            }
            if !is_actual_optimal && is_raw_optimal {
                sm_metrics.total_raw_optimal_suppression_days += 1;
            }
        }

        // top actions latency を追跡する。
        for sym in &raw_top3 {
            raw_top3_first_seen
                .entry(sym.to_string())
                .or_insert(*current_date);
            if current_top_actions.contains(sym) {
                if let Some(raw_date) = raw_top3_first_seen.get(sym) {
                    let delay = (*current_date - *raw_date).num_days() as usize;
                    if delay > 0 && !mem_top3_first_seen.contains_key(sym) {
                        sm_metrics.total_initial_top_actions_latency_days += delay;
                        mem_top3_first_seen.insert(sym.clone(), *current_date);
                    }
                }
            }
        }

        if let Some(ref prev) = prev_packet {
            let prev_top_actions: Vec<String> = prev
                .assets
                .iter()
                .take(3)
                .map(|a| a.symbol.clone())
                .collect();
            if !prev_top_actions.is_empty() {
                let mut matches = 0;
                for sym in &current_top_actions {
                    if prev_top_actions.contains(sym) {
                        matches += 1;
                    }
                }
                let turnover = 1.0 - (matches as f64 / 3.0);
                sm_metrics.top_actions_turnover_sum += turnover;
            }

            for asset in &current_packet.assets {
                let reasons_str = asset.reasons.join(" | ");
                if reasons_str.contains("Friction:Hold") || reasons_str.contains("Top Tier Lock") {
                    sm_metrics.core_asset_protection_hits += 1;
                }
                if reasons_str.contains("Friction:Block") || reasons_str.contains("Promotion Cap") {
                    sm_metrics.weak_asset_promotion_cap_hits += 1;
                }
            }
        }

        // reliability を確認する。
        for asset in &current_packet.assets {
            let state_str = format!("{:?}", asset.asset_state);
            let reg_entry = regime_tracking.entry(state_str.clone()).or_default();
            if let Some(full_hist) = histories.get(&asset.symbol) {
                let current_idx = asset_indices
                    .get(&asset.symbol)
                    .cloned()
                    .unwrap_or(0)
                    .saturating_sub(1);
                let future_idx = current_idx + 20;
                if future_idx < full_hist.bars.len() {
                    let curr = &full_hist.bars[current_idx];
                    let fut = &full_hist.bars[future_idx];
                    let fwd_return = (fut.close - curr.close) / curr.close;
                    reg_entry.total_signals += 1;
                    reg_entry.sum_20d_return += fwd_return;
                    let search_end = future_idx.min(full_hist.bars.len() - 1);
                    let mut min_price = curr.close;
                    for b in &full_hist.bars[current_idx..=search_end] {
                        if b.close < min_price {
                            min_price = b.close;
                        }
                    }
                    reg_entry.sum_max_drawdown_20d += (min_price - curr.close) / curr.close;
                    reg_entry.count_drawdowns += 1;
                    let is_bear = asset.action == BacktestAssetAction::Reduce
                        || asset.action == BacktestAssetAction::Freeze
                        || asset.action == BacktestAssetAction::Avoid;
                    let is_correct = if is_bear {
                        fwd_return < 0.0
                    } else {
                        fwd_return > 0.0
                    };
                    if is_correct {
                        reg_entry.correct_signals += 1;
                    }
                    let conf_bucket = match current_packet.system_confidence as usize {
                        90..=100 => "90-100",
                        80..=89 => "80-90",
                        70..=79 => "70-80",
                        60..=69 => "60-70",
                        50..=59 => "50-60",
                        _ => "<50",
                    };
                    let rel_entry = reliability.entry(conf_bucket.to_string()).or_insert((0, 0));
                    rel_entry.0 += 1;
                    if is_correct {
                        rel_entry.1 += 1;
                    }
                }
            }
        }

        prev_packet = Some(current_packet);
    }

    let mut rel_vec: Vec<_> = reliability.into_iter().collect();
    rel_vec.sort_by(|a, b| b.0.cmp(&a.0));
    let reliability = rel_vec
        .into_iter()
        .map(|(bucket, (total, correct))| BacktestReliabilityBucket {
            bucket,
            total,
            correct,
        })
        .collect();

    let mut reg_vec: Vec<_> = regime_tracking.into_iter().collect();
    reg_vec.sort_by_key(|b| std::cmp::Reverse(b.1.total_signals));
    let regime_audit = reg_vec
        .into_iter()
        .filter(|(_, stats)| stats.total_signals > 0)
        .map(|(state, stats)| BacktestRegimeAudit {
            state,
            total_signals: stats.total_signals,
            correct_signals: stats.correct_signals,
            average_20d_return: stats.sum_20d_return / stats.total_signals as f64,
            max_drawdown_20d: stats.sum_max_drawdown_20d / stats.total_signals as f64,
        })
        .collect();

    Ok(BacktestSimulationReport {
        name: dir_name.to_string(),
        metrics: sm_metrics,
        reliability,
        regime_audit,
    })
}
