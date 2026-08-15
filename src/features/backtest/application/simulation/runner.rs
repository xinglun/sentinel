use crate::features::backtest::application::decision_engine::BacktestDecisionEngine;
use crate::features::backtest::application::model::{
    BacktestAssetAction, BacktestAssetSnapshot, BacktestAssetState, BacktestBreakoutStatus,
    BacktestDecisionClass, BacktestDecisionSnapshot, BacktestRegimeAudit,
    BacktestReliabilityBucket, BacktestRules, BacktestSimulationReport, BacktestTickerHistory,
    BacktestTrendStatus, BacktestTrendTopology, BacktestWatchlistEntry, ValidationClassOutcome,
    ValidationDecisionRecord, ValidationWindow,
};
use crate::features::backtest::application::validation::{
    empirical_quantile, forward_outcome, top_decile_mean, validation_status,
};
use crate::features::backtest::domain::metrics::{RegimeStats, StateMachineMetrics};
use anyhow::Result;
use chrono::NaiveDate;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::episodes::build_validation_report;

type LifecycleKey = (String, String, String);

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
    let mut validation_strength_first_seen: HashMap<(String, String, String), NaiveDate> =
        HashMap::new();
    let mut mem_top3_first_seen: HashMap<String, NaiveDate> = HashMap::new();
    let mut breakout_first_seen: HashMap<(String, String, String), NaiveDate> = HashMap::new();
    let mut ready_first_seen: HashMap<(String, String, String), NaiveDate> = HashMap::new();
    let mut strength_first_seen_idx: HashMap<(String, String, String), usize> = HashMap::new();
    let mut breakout_first_seen_idx: HashMap<(String, String, String), usize> = HashMap::new();
    let mut ready_first_seen_idx: HashMap<(String, String, String), usize> = HashMap::new();
    let mut validation_records = Vec::new();

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

        let raw_top3 = raw_top_candidates(&current_packet.assets);

        let lifecycle_prefix = (
            current_packet.decision_snapshot_version.clone(),
            current_packet.universe_id.clone(),
        );

        retain_active_lifecycle_entries(
            &lifecycle_prefix.0,
            &lifecycle_prefix.1,
            &raw_top3,
            &mut validation_strength_first_seen,
        );
        retain_active_lifecycle_entries(
            &lifecycle_prefix.0,
            &lifecycle_prefix.1,
            &raw_top3,
            &mut breakout_first_seen,
        );
        retain_active_lifecycle_entries(
            &lifecycle_prefix.0,
            &lifecycle_prefix.1,
            &raw_top3,
            &mut ready_first_seen,
        );
        retain_active_lifecycle_entries(
            &lifecycle_prefix.0,
            &lifecycle_prefix.1,
            &raw_top3,
            &mut strength_first_seen_idx,
        );
        retain_active_lifecycle_entries(
            &lifecycle_prefix.0,
            &lifecycle_prefix.1,
            &raw_top3,
            &mut breakout_first_seen_idx,
        );
        retain_active_lifecycle_entries(
            &lifecycle_prefix.0,
            &lifecycle_prefix.1,
            &raw_top3,
            &mut ready_first_seen_idx,
        );

        for sym in &raw_top3 {
            validation_strength_first_seen
                .entry((
                    lifecycle_prefix.0.clone(),
                    lifecycle_prefix.1.clone(),
                    sym.clone(),
                ))
                .or_insert(*current_date);
        }
        for asset in &current_packet.assets {
            if raw_top3.contains(&asset.symbol) {
                let key = (
                    lifecycle_prefix.0.clone(),
                    lifecycle_prefix.1.clone(),
                    asset.symbol.clone(),
                );
                let current_idx = asset_indices
                    .get(&asset.symbol)
                    .cloned()
                    .unwrap_or(0)
                    .saturating_sub(1);
                strength_first_seen_idx.entry(key).or_insert(current_idx);
            }
        }
        if current_packet.decision_class == BacktestDecisionClass::Ready {
            for asset in &current_packet.assets {
                let key = (
                    lifecycle_prefix.0.clone(),
                    lifecycle_prefix.1.clone(),
                    asset.symbol.clone(),
                );
                ready_first_seen.entry(key.clone()).or_insert(*current_date);
                let current_idx = asset_indices
                    .get(&asset.symbol)
                    .cloned()
                    .unwrap_or(0)
                    .saturating_sub(1);
                ready_first_seen_idx.entry(key).or_insert(current_idx);
            }
        }
        for asset in &current_packet.assets {
            if asset.breakout_status != BacktestBreakoutStatus::NoBreakout {
                let key = (
                    lifecycle_prefix.0.clone(),
                    lifecycle_prefix.1.clone(),
                    asset.symbol.clone(),
                );
                breakout_first_seen
                    .entry(key.clone())
                    .or_insert(*current_date);
                let current_idx = asset_indices
                    .get(&asset.symbol)
                    .cloned()
                    .unwrap_or(0)
                    .saturating_sub(1);
                breakout_first_seen_idx.entry(key).or_insert(current_idx);
            }
        }

        for asset in &current_packet.assets {
            let is_raw_optimal = asset
                .deviation
                .is_some_and(|raw_score| raw_score >= optimal_threshold);
            let is_actual_optimal = asset.asset_state == BacktestAssetState::Optimal;

            if is_actual_optimal && !is_raw_optimal {
                sm_metrics.total_raw_vs_actual_divergence_days += 1;
            }
            if !is_actual_optimal && is_raw_optimal {
                sm_metrics.total_raw_optimal_suppression_days += 1;
            }
        }

        for asset in &current_packet.assets {
            if let Some(full_hist) = histories.get(&asset.symbol) {
                let current_idx = asset_indices
                    .get(&asset.symbol)
                    .cloned()
                    .unwrap_or(0)
                    .saturating_sub(1);
                let outcome_5d = forward_outcome(full_hist.bars.as_ref(), current_idx, 5);
                let outcome_10d = forward_outcome(full_hist.bars.as_ref(), current_idx, 10);
                let outcome_20d = forward_outcome(full_hist.bars.as_ref(), current_idx, 20);
                let status = validation_status(
                    outcome_5d.is_some(),
                    outcome_10d.is_some(),
                    outcome_20d.is_some(),
                );
                let lifecycle_key = (
                    lifecycle_prefix.0.clone(),
                    lifecycle_prefix.1.clone(),
                    asset.symbol.clone(),
                );
                let strength_idx = strength_first_seen_idx.get(&lifecycle_key).copied();
                let breakout_idx = breakout_first_seen_idx.get(&lifecycle_key).copied();
                let ready_idx = ready_first_seen_idx.get(&lifecycle_key).copied();
                let return_strength_to_ready = match (strength_idx, ready_idx) {
                    (Some(strength), Some(ready)) if ready >= strength => {
                        let strength_close = full_hist.bars[strength].close;
                        let ready_close = full_hist.bars[ready].close;
                        (strength_close > 0.0)
                            .then_some((ready_close - strength_close) / strength_close)
                    }
                    _ => None,
                };
                let return_breakout_to_ready = match (breakout_idx, ready_idx) {
                    (Some(breakout), Some(ready)) if ready >= breakout => {
                        let breakout_close = full_hist.bars[breakout].close;
                        let ready_close = full_hist.bars[ready].close;
                        (breakout_close > 0.0)
                            .then_some((ready_close - breakout_close) / breakout_close)
                    }
                    _ => None,
                };
                let max_move_strength_to_ready = match (strength_idx, ready_idx) {
                    (Some(strength), Some(ready)) if ready >= strength => {
                        let strength_close = full_hist.bars[strength].close;
                        (strength_close > 0.0).then(|| {
                            full_hist.bars[strength..=ready]
                                .iter()
                                .filter_map(|bar| bar.high.or(Some(bar.close)))
                                .map(|price| (price - strength_close) / strength_close)
                                .fold(0.0, f64::max)
                        })
                    }
                    _ => None,
                };
                validation_records.push(ValidationDecisionRecord {
                    date: *current_date,
                    symbol: asset.symbol.clone(),
                    decision_class: current_packet.decision_class,
                    decision_reasons: current_packet.decision_reasons.clone(),
                    gate_blocked: current_packet.gate_blocked,
                    classification_available: current_packet.classification_available,
                    decision_snapshot_version: current_packet.decision_snapshot_version.clone(),
                    universe_id: current_packet.universe_id.clone(),
                    decision_session_index: current_idx,
                    decision_close: asset.price,
                    raw_candidate: raw_top3.contains(&asset.symbol),
                    strength_date: validation_strength_first_seen.get(&lifecycle_key).copied(),
                    breakout_date: breakout_first_seen.get(&lifecycle_key).copied(),
                    ready_date: ready_first_seen.get(&lifecycle_key).copied(),
                    strength_to_breakout_sessions: strength_idx
                        .zip(breakout_idx)
                        .and_then(|(strength, breakout)| breakout.checked_sub(strength)),
                    breakout_to_ready_sessions: breakout_idx
                        .zip(ready_idx)
                        .and_then(|(breakout, ready)| ready.checked_sub(breakout)),
                    strength_to_ready_sessions: strength_idx
                        .zip(ready_idx)
                        .and_then(|(strength, ready)| ready.checked_sub(strength)),
                    return_strength_to_ready,
                    return_breakout_to_ready,
                    max_move_strength_to_ready,
                    forward_return_5d: outcome_5d.map(|outcome| outcome.forward_return),
                    forward_return_10d: outcome_10d.map(|outcome| outcome.forward_return),
                    forward_return_20d: outcome_20d.map(|outcome| outcome.forward_return),
                    mfe_5d: outcome_5d.map(|outcome| outcome.mfe),
                    mfe_10d: outcome_10d.map(|outcome| outcome.mfe),
                    mfe_20d: outcome_20d.map(|outcome| outcome.mfe),
                    mae_5d: outcome_5d.map(|outcome| outcome.mae),
                    mae_10d: outcome_10d.map(|outcome| outcome.mae),
                    mae_20d: outcome_20d.map(|outcome| outcome.mae),
                    validation_status: status,
                });
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

    let validation = build_validation_report(&validation_records);
    let decision_start = simulation_dates.first().copied();
    let decision_end = simulation_dates.last().copied();
    let outcome_end = histories
        .values()
        .filter_map(|history| history.bars.last().map(|bar| bar.date))
        .max();

    Ok(BacktestSimulationReport {
        name: dir_name.to_string(),
        metrics: sm_metrics,
        reliability,
        regime_audit,
        validation,
        window: ValidationWindow {
            decision_start,
            decision_end,
            outcome_end,
        },
    })
}

pub(crate) fn raw_top_candidates(assets: &[BacktestAssetSnapshot]) -> Vec<String> {
    let mut sorted_assets = assets
        .iter()
        .filter(|asset| asset.deviation.is_some())
        .collect::<Vec<_>>();
    sorted_assets
        .sort_by(|left, right| right.deviation.unwrap().total_cmp(&left.deviation.unwrap()));
    sorted_assets
        .into_iter()
        .take(3)
        .map(|asset| asset.symbol.clone())
        .collect()
}

pub(crate) fn retain_active_lifecycle_entries<T>(
    snapshot_version: &str,
    universe_id: &str,
    active_raw_candidates: &[String],
    entries: &mut HashMap<LifecycleKey, T>,
) {
    let active_symbols = active_raw_candidates.iter().collect::<HashSet<_>>();
    entries.retain(|(version, universe, symbol), _| {
        version != snapshot_version || universe != universe_id || active_symbols.contains(symbol)
    });
}

pub(crate) fn build_class_outcome(
    records: &[ValidationDecisionRecord],
    decision_class: BacktestDecisionClass,
) -> ValidationClassOutcome {
    let matching = records
        .iter()
        .filter(|record| record.decision_class == decision_class)
        .collect::<Vec<_>>();
    let average = |select: fn(&ValidationDecisionRecord) -> Option<f64>| {
        let values = matching
            .iter()
            .filter_map(|record| select(record))
            .collect::<Vec<_>>();
        (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
    };
    let mut mae_values = matching
        .iter()
        .filter_map(|record| record.mae_20d)
        .collect::<Vec<_>>();
    mae_values.sort_by(f64::total_cmp);
    let median_mae_20d = (!mae_values.is_empty()).then(|| mae_values[mae_values.len() / 2]);
    ValidationClassOutcome {
        decision_class: Some(decision_class),
        sample_count: matching.len(),
        complete_5d: matching
            .iter()
            .filter(|record| record.forward_return_5d.is_some())
            .count(),
        complete_10d: matching
            .iter()
            .filter(|record| record.forward_return_10d.is_some())
            .count(),
        complete_20d: matching
            .iter()
            .filter(|record| record.forward_return_20d.is_some())
            .count(),
        average_5d_return: average(|record| record.forward_return_5d),
        average_10d_return: average(|record| record.forward_return_10d),
        average_20d_return: average(|record| record.forward_return_20d),
        average_mfe_20d: average(|record| record.mfe_20d),
        average_mae_20d: average(|record| record.mae_20d),
        median_mae_20d,
        p90_mae_20d: empirical_quantile(
            &matching
                .iter()
                .filter_map(|record| record.mae_20d)
                .collect::<Vec<_>>(),
            0.90,
        ),
        p95_mae_20d: empirical_quantile(
            &matching
                .iter()
                .filter_map(|record| record.mae_20d)
                .collect::<Vec<_>>(),
            0.95,
        ),
        average_positive_20d_return: average(|record| {
            record.forward_return_20d.filter(|value| *value > 0.0)
        }),
        top_decile_missed_upside: top_decile_mean(
            &matching
                .iter()
                .filter_map(|record| record.mfe_20d)
                .filter(|value| *value > 0.0)
                .collect::<Vec<_>>(),
        ),
        downside_20d_count: matching
            .iter()
            .filter(|record| record.forward_return_20d.is_some_and(|value| value < 0.0))
            .count(),
        positive_20d_count: matching
            .iter()
            .filter(|record| record.forward_return_20d.is_some_and(|value| value > 0.0))
            .count(),
    }
}
