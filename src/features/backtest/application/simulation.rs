use crate::features::backtest::application::decision_engine::BacktestDecisionEngine;
use crate::features::backtest::application::model::{
    BacktestAssetAction, BacktestAssetSnapshot, BacktestAssetState, BacktestBreakoutStatus,
    BacktestDecisionClass, BacktestDecisionSnapshot, BacktestRegimeAudit,
    BacktestReliabilityBucket, BacktestRules, BacktestSimulationReport, BacktestTickerHistory,
    BacktestTrendStatus, BacktestTrendTopology, BacktestWatchlistEntry, ConfirmationCostSummary,
    NetDecisionHorizon, NetDecisionValue, ValidationBaselineComparison, ValidationClassOutcome,
    ValidationCohortReport, ValidationDecisionRecord, ValidationHorizonUtility,
    ValidationReasonUtility, ValidationReport, ValidationUtility,
};
use crate::features::backtest::application::validation::{
    empirical_quantile, forward_outcome, top_decile_mean, validation_status,
};
use crate::features::backtest::domain::metrics::{RegimeStats, StateMachineMetrics};
use anyhow::Result;
use chrono::NaiveDate;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

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

    Ok(BacktestSimulationReport {
        name: dir_name.to_string(),
        metrics: sm_metrics,
        reliability,
        regime_audit,
        validation,
    })
}

fn build_validation_report(records: &[ValidationDecisionRecord]) -> ValidationReport {
    let mut grouped: HashMap<(String, String), Vec<ValidationDecisionRecord>> = HashMap::new();
    for record in records
        .iter()
        .filter(|record| record.classification_available)
    {
        grouped
            .entry((
                record.decision_snapshot_version.clone(),
                record.universe_id.clone(),
            ))
            .or_default()
            .push(record.clone());
    }

    let mut keys = grouped.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let cohorts = keys
        .into_iter()
        .filter_map(|(version, universe)| {
            grouped
                .remove(&(version.clone(), universe.clone()))
                .map(|cohort_records| build_cohort_report(&version, &universe, &cohort_records))
        })
        .collect::<Vec<_>>();

    let (outcomes, baseline, sample_maturity) = match cohorts.as_slice() {
        [cohort] => (
            cohort.outcomes.clone(),
            cohort.baseline.clone(),
            cohort.sample_maturity.clone(),
        ),
        _ => (
            Vec::new(),
            ValidationBaselineComparison::default(),
            "COHORTED".to_string(),
        ),
    };
    ValidationReport {
        records: records.to_vec(),
        invalid_context_record_count: records
            .iter()
            .filter(|record| !record.classification_available)
            .count(),
        outcomes,
        baseline,
        sample_maturity,
        cohorts,
    }
}

fn build_cohort_report(
    decision_snapshot_version: &str,
    universe_id: &str,
    records: &[ValidationDecisionRecord],
) -> ValidationCohortReport {
    let classes = [
        BacktestDecisionClass::NoTrade,
        BacktestDecisionClass::Probe,
        BacktestDecisionClass::Ready,
    ];
    let outcomes = classes
        .into_iter()
        .map(|decision_class| build_class_outcome(records, decision_class))
        .collect();
    let episodes = episode_records(records);
    let eligible = episodes
        .iter()
        .filter(|record| is_trend_gate_eligible(record))
        .collect::<Vec<_>>();
    let raw = episodes
        .iter()
        .filter(|record| record.raw_candidate && record.forward_return_20d.is_some())
        .collect::<Vec<_>>();
    let ready = raw
        .iter()
        .filter(|record| {
            record.ready_date.is_some() || record.decision_class == BacktestDecisionClass::Ready
        })
        .copied()
        .collect::<Vec<_>>();
    let baseline = ValidationBaselineComparison {
        raw_top3_sample_count: raw.len(),
        ready_sample_count: ready.len(),
        raw_top3_average_20d_return: average_records(&raw, |record| record.forward_return_20d),
        ready_average_20d_return: average_records(&ready, |record| record.forward_return_20d),
        raw_top3_average_20d_mfe: average_records(&raw, |record| record.mfe_20d),
        ready_average_20d_mfe: average_records(&ready, |record| record.mfe_20d),
        raw_top3_average_mae_20d: average_records(&raw, |record| record.mae_20d),
        ready_average_mae_20d: average_records(&ready, |record| record.mae_20d),
        return_difference: difference(
            average_records(&ready, |record| record.forward_return_20d),
            average_records(&raw, |record| record.forward_return_20d),
        ),
        mae_difference: difference(
            average_records(&ready, |record| record.mae_20d),
            average_records(&raw, |record| record.mae_20d),
        ),
        mfe_difference: difference(
            average_records(&ready, |record| record.mfe_20d),
            average_records(&raw, |record| record.mfe_20d),
        ),
    };
    ValidationCohortReport {
        decision_snapshot_version: decision_snapshot_version.to_string(),
        universe_id: universe_id.to_string(),
        outcomes,
        baseline,
        utility: build_utility(&episodes),
        confirmation_cost: build_confirmation_cost_from_episodes(&eligible),
        net_decision_value: build_net_decision_value(&episodes),
        sample_maturity: sample_maturity(&episodes),
        protection_sample_maturity: maturity_for_count(
            eligible
                .iter()
                .filter(|record| record.forward_return_20d.is_some())
                .count(),
        ),
        confirmation_sample_maturity: maturity_for_count(
            eligible
                .iter()
                .filter(|record| record.strength_to_ready_sessions.is_some())
                .count(),
        ),
    }
}

fn episode_records(records: &[ValidationDecisionRecord]) -> Vec<ValidationDecisionRecord> {
    let mut episodes: HashMap<(String, String, String, NaiveDate), ValidationDecisionRecord> =
        HashMap::new();
    for record in records
        .iter()
        .filter(|record| record.classification_available && record.strength_date.is_some())
    {
        let key = (
            record.decision_snapshot_version.clone(),
            record.universe_id.clone(),
            record.symbol.clone(),
            record.strength_date.unwrap(),
        );
        episodes
            .entry(key)
            .and_modify(|existing| merge_episode_record(existing, record))
            .or_insert_with(|| record.clone());
    }
    let mut result = episodes.into_values().collect::<Vec<_>>();
    result.sort_by_key(|record| record.date);
    result
}

fn merge_episode_record(
    existing: &mut ValidationDecisionRecord,
    incoming: &ValidationDecisionRecord,
) {
    if incoming.date < existing.date {
        let mut earliest = incoming.clone();
        merge_lifecycle_facts(&mut earliest, existing);
        *existing = earliest;
    } else {
        merge_lifecycle_facts(existing, incoming);
    }
}

fn merge_lifecycle_facts(target: &mut ValidationDecisionRecord, source: &ValidationDecisionRecord) {
    target.breakout_date = target.breakout_date.or(source.breakout_date);
    target.ready_date = target.ready_date.or(source.ready_date);
    target.strength_to_breakout_sessions = target
        .strength_to_breakout_sessions
        .or(source.strength_to_breakout_sessions);
    target.breakout_to_ready_sessions = target
        .breakout_to_ready_sessions
        .or(source.breakout_to_ready_sessions);
    target.strength_to_ready_sessions = target
        .strength_to_ready_sessions
        .or(source.strength_to_ready_sessions);
    target.return_strength_to_ready = target
        .return_strength_to_ready
        .or(source.return_strength_to_ready);
    target.return_breakout_to_ready = target
        .return_breakout_to_ready
        .or(source.return_breakout_to_ready);
    target.max_move_strength_to_ready = target
        .max_move_strength_to_ready
        .or(source.max_move_strength_to_ready);
}

fn average_records(
    records: &[&ValidationDecisionRecord],
    select: fn(&ValidationDecisionRecord) -> Option<f64>,
) -> Option<f64> {
    let values = records
        .iter()
        .filter_map(|record| select(record))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn difference(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    left.zip(right).map(|(left, right)| left - right)
}

fn sample_maturity(records: &[ValidationDecisionRecord]) -> String {
    let complete_20d = records
        .iter()
        .filter(|record| record.forward_return_20d.is_some())
        .count();
    let lifecycle_complete = records
        .iter()
        .filter(|record| record.strength_to_ready_sessions.is_some())
        .count();
    maturity_for_count(complete_20d.min(lifecycle_complete))
}

fn maturity_for_count(count: usize) -> String {
    match count {
        0..=29 => "INSUFFICIENT",
        30..=99 => "DEVELOPING",
        _ => "USABLE",
    }
    .to_string()
}

fn build_utility(records: &[ValidationDecisionRecord]) -> ValidationUtility {
    let blocked_all = records
        .iter()
        .filter(|record| {
            record.raw_candidate
                && record.gate_blocked
                && record.decision_class == BacktestDecisionClass::NoTrade
        })
        .collect::<Vec<_>>();
    let blocked = blocked_all
        .iter()
        .filter(|record| record.forward_return_20d.is_some())
        .copied()
        .collect::<Vec<_>>();
    let mae = blocked
        .iter()
        .filter_map(|record| record.mae_20d)
        .collect::<Vec<_>>();
    let mfe = blocked
        .iter()
        .filter_map(|record| record.mfe_20d)
        .collect::<Vec<_>>();
    let positive_returns = blocked
        .iter()
        .filter_map(|record| record.forward_return_20d)
        .filter(|value| *value > 0.0)
        .collect::<Vec<_>>();
    let horizon_5d = build_horizon_utility(&blocked, |record| {
        (record.forward_return_5d, record.mfe_5d, record.mae_5d)
    });
    let horizon_10d = build_horizon_utility(&blocked, |record| {
        (record.forward_return_10d, record.mfe_10d, record.mae_10d)
    });
    let horizon_20d = build_horizon_utility(&blocked, |record| {
        (record.forward_return_20d, record.mfe_20d, record.mae_20d)
    });
    let reasons = [
        "TREND_GATE_BLOCKED",
        "NO_LEADER",
        "BREADTH_TOO_NARROW",
        "BREAKOUT_UNCONFIRMED",
        "CONFIDENCE_INSUFFICIENT",
        "RISK_OVERLAY_ACTIVE",
    ];
    let reason_breakdown = reasons
        .into_iter()
        .map(|reason| {
            let reason_records = blocked
                .iter()
                .filter(|record| record.decision_reasons.iter().any(|item| item == reason))
                .copied()
                .collect::<Vec<_>>();
            ValidationReasonUtility {
                reason: reason.to_string(),
                horizon_5d: build_horizon_utility(&reason_records, |record| {
                    (record.forward_return_5d, record.mfe_5d, record.mae_5d)
                }),
                horizon_10d: build_horizon_utility(&reason_records, |record| {
                    (record.forward_return_10d, record.mfe_10d, record.mae_10d)
                }),
                horizon_20d: build_horizon_utility(&reason_records, |record| {
                    (record.forward_return_20d, record.mfe_20d, record.mae_20d)
                }),
            }
        })
        .collect();
    ValidationUtility {
        blocked_candidate_count: blocked_all.len(),
        complete_20d_count: blocked.len(),
        downside_20d_count: blocked
            .iter()
            .filter(|record| record.forward_return_20d.is_some_and(|value| value < 0.0))
            .count(),
        missed_upside_count: positive_returns.len(),
        average_mae_20d: mean(&mae),
        median_mae_20d: empirical_quantile(&mae, 0.5),
        p90_mae_20d: empirical_quantile(&mae, 0.9),
        p95_mae_20d: empirical_quantile(&mae, 0.95),
        average_mfe_20d: mean(&mfe),
        average_positive_20d_return: mean(&positive_returns),
        top_decile_missed_upside: top_decile_mean(
            &mfe.into_iter()
                .filter(|value| *value > 0.0)
                .collect::<Vec<_>>(),
        ),
        horizon_5d,
        horizon_10d,
        horizon_20d,
        reason_breakdown,
    }
}

type HorizonSelector = fn(&ValidationDecisionRecord) -> (Option<f64>, Option<f64>, Option<f64>);

fn build_horizon_utility(
    records: &[&ValidationDecisionRecord],
    select: HorizonSelector,
) -> ValidationHorizonUtility {
    let complete = records
        .iter()
        .filter_map(|record| {
            let (forward_return, mfe, mae) = select(record);
            forward_return.map(|forward_return| (forward_return, mfe, mae))
        })
        .collect::<Vec<_>>();
    let mae = complete
        .iter()
        .filter_map(|(_, _, mae)| *mae)
        .collect::<Vec<_>>();
    let mfe = complete
        .iter()
        .filter_map(|(_, mfe, _)| *mfe)
        .collect::<Vec<_>>();
    let positive_returns = complete
        .iter()
        .map(|(forward_return, _, _)| *forward_return)
        .filter(|value| *value > 0.0)
        .collect::<Vec<_>>();
    ValidationHorizonUtility {
        complete_sample_count: complete.len(),
        downside_count: complete.iter().filter(|(value, _, _)| *value < 0.0).count(),
        missed_upside_count: positive_returns.len(),
        average_mae: mean(&mae),
        median_mae: empirical_quantile(&mae, 0.5),
        p90_mae: empirical_quantile(&mae, 0.9),
        p95_mae: empirical_quantile(&mae, 0.95),
        average_mfe: mean(&mfe),
        average_positive_return: mean(&positive_returns),
        top_decile_missed_upside: top_decile_mean(
            &mfe.into_iter()
                .filter(|value| *value > 0.0)
                .collect::<Vec<_>>(),
        ),
    }
}

#[cfg(test)]
fn build_confirmation_cost(records: &[ValidationDecisionRecord]) -> ConfirmationCostSummary {
    let records = episode_records(records);
    let records = records.iter().collect::<Vec<_>>();
    build_confirmation_cost_from_episodes(&records)
}

fn build_confirmation_cost_from_episodes(
    records: &[&ValidationDecisionRecord],
) -> ConfirmationCostSummary {
    let average_sessions = |select: fn(&ValidationDecisionRecord) -> Option<usize>| {
        let values = records
            .iter()
            .filter_map(|record| select(record))
            .map(|value| value as f64)
            .collect::<Vec<_>>();
        mean(&values)
    };
    ConfirmationCostSummary {
        episode_sample_count: records.len(),
        lifecycle_complete_episode_count: records
            .iter()
            .filter(|record| record.strength_to_ready_sessions.is_some())
            .count(),
        average_strength_to_breakout_sessions: average_sessions(|record| {
            record.strength_to_breakout_sessions
        }),
        average_breakout_to_ready_sessions: average_sessions(|record| {
            record.breakout_to_ready_sessions
        }),
        average_strength_to_ready_sessions: average_sessions(|record| {
            record.strength_to_ready_sessions
        }),
        average_return_strength_to_ready: mean(
            &records
                .iter()
                .filter_map(|record| record.return_strength_to_ready)
                .collect::<Vec<_>>(),
        ),
        average_return_lost_before_ready: mean(
            &records
                .iter()
                .filter_map(|record| record.return_strength_to_ready)
                .filter(|value| *value > 0.0)
                .collect::<Vec<_>>(),
        ),
        average_return_breakout_to_ready: mean(
            &records
                .iter()
                .filter_map(|record| record.return_breakout_to_ready)
                .collect::<Vec<_>>(),
        ),
        average_max_move_strength_to_ready: mean(
            &records
                .iter()
                .filter_map(|record| record.max_move_strength_to_ready)
                .collect::<Vec<_>>(),
        ),
    }
}

fn build_net_decision_value(records: &[ValidationDecisionRecord]) -> NetDecisionValue {
    let eligible = records
        .iter()
        .filter(|record| is_trend_gate_eligible(record))
        .collect::<Vec<_>>();
    let horizon_5d = build_net_decision_horizon(&eligible, 5, |record| record.forward_return_5d);
    let horizon_10d = build_net_decision_horizon(&eligible, 10, |record| record.forward_return_10d);
    let horizon_20d = build_net_decision_horizon(&eligible, 20, |record| record.forward_return_20d);
    NetDecisionValue {
        eligible_episode_count: eligible.len(),
        protection_episode_count: horizon_20d.paired_episode_count,
        confirmation_episode_count: horizon_20d.paired_episode_count,
        protection_benefit: horizon_20d.protection_benefit,
        confirmation_cost: horizon_20d.confirmation_cost,
        net_value: horizon_20d.net_value,
        horizon_5d,
        horizon_10d,
        horizon_20d,
    }
}

fn build_net_decision_horizon(
    records: &[&ValidationDecisionRecord],
    horizon: usize,
    select_forward_return: fn(&ValidationDecisionRecord) -> Option<f64>,
) -> NetDecisionHorizon {
    let paired_values = records
        .iter()
        .filter_map(|record| {
            let forward_return = select_forward_return(record)?;
            let strength_to_ready_sessions = record.strength_to_ready_sessions?;
            if strength_to_ready_sessions > horizon {
                return None;
            }
            let confirmation = record.return_strength_to_ready?.max(0.0);
            let adverse_waiting_return =
                record.return_strength_to_ready.filter(|value| *value < 0.0);
            Some((
                (-forward_return).max(0.0),
                confirmation,
                adverse_waiting_return,
            ))
        })
        .collect::<Vec<_>>();
    let protection_benefit = mean(
        &paired_values
            .iter()
            .map(|(protection, _, _)| *protection)
            .collect::<Vec<_>>(),
    );
    let confirmation_cost = mean(
        &paired_values
            .iter()
            .map(|(_, confirmation, _)| *confirmation)
            .collect::<Vec<_>>(),
    );
    let adverse_waiting_return = mean(
        &paired_values
            .iter()
            .filter_map(|(_, _, adverse)| *adverse)
            .collect::<Vec<_>>(),
    );
    let adverse_waiting_sample_count = paired_values
        .iter()
        .filter(|(_, _, adverse)| adverse.is_some())
        .count();
    NetDecisionHorizon {
        paired_episode_count: paired_values.len(),
        unpaired_episode_count: records.len().saturating_sub(paired_values.len()),
        protection_benefit,
        confirmation_cost,
        adverse_waiting_return,
        adverse_waiting_sample_count,
        net_value: protection_benefit
            .zip(confirmation_cost)
            .map(|(benefit, cost)| benefit - cost),
    }
}

fn is_trend_gate_eligible(record: &ValidationDecisionRecord) -> bool {
    record.raw_candidate
        && record.gate_blocked
        && record.decision_class == BacktestDecisionClass::NoTrade
}

fn raw_top_candidates(assets: &[BacktestAssetSnapshot]) -> Vec<String> {
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

fn retain_active_lifecycle_entries<T>(
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

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn build_class_outcome(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::backtest::application::model::ValidationStatus;
    use chrono::NaiveDate;

    fn record(
        decision_class: BacktestDecisionClass,
        raw_candidate: bool,
    ) -> ValidationDecisionRecord {
        ValidationDecisionRecord {
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            symbol: "AAPL".to_string(),
            decision_class,
            decision_reasons: vec!["TREND_GATE_BLOCKED".to_string()],
            gate_blocked: decision_class == BacktestDecisionClass::NoTrade,
            classification_available: true,
            decision_snapshot_version: "radar-v1.0.0".to_string(),
            universe_id: "watchlist:AAPL".to_string(),
            decision_session_index: 10,
            decision_close: 100.0,
            raw_candidate,
            strength_date: None,
            breakout_date: None,
            ready_date: None,
            strength_to_breakout_sessions: None,
            breakout_to_ready_sessions: None,
            strength_to_ready_sessions: None,
            return_strength_to_ready: None,
            return_breakout_to_ready: None,
            max_move_strength_to_ready: None,
            forward_return_5d: None,
            forward_return_10d: None,
            forward_return_20d: None,
            mfe_5d: None,
            mfe_10d: None,
            mfe_20d: None,
            mae_5d: None,
            mae_10d: None,
            mae_20d: None,
            validation_status: ValidationStatus::Pending,
        }
    }

    #[test]
    fn raw_top_candidates_exclude_assets_without_deviation() {
        let asset = |symbol: &str, deviation: Option<f64>| {
            crate::features::backtest::application::model::BacktestAssetSnapshot {
                symbol: symbol.to_string(),
                price: 100.0,
                action: crate::features::backtest::application::model::BacktestAssetAction::Other,
                deviation,
                asset_state: BacktestAssetState::Other,
                breakout_eligible: false,
                breakout_status: BacktestBreakoutStatus::NoBreakout,
                breakout_failed_risk: false,
                reasons: Vec::new(),
            }
        };

        let candidates = raw_top_candidates(&[
            asset("MISSING", None),
            asset("LOW", Some(0.2)),
            asset("HIGH", Some(0.8)),
        ]);

        assert_eq!(candidates, vec!["HIGH", "LOW"]);
    }

    #[test]
    fn inactive_raw_candidate_lifecycle_is_reset_before_reentry() {
        let key = (
            "radar-v1.0.0".to_string(),
            "watchlist:AAPL".to_string(),
            "AAPL".to_string(),
        );
        let mut strength_dates =
            HashMap::from([(key.clone(), NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())]);
        let mut breakout_dates =
            HashMap::from([(key.clone(), NaiveDate::from_ymd_opt(2026, 1, 2).unwrap())]);
        let mut ready_dates =
            HashMap::from([(key.clone(), NaiveDate::from_ymd_opt(2026, 1, 3).unwrap())]);
        let mut strength_indices = HashMap::from([(key.clone(), 1usize)]);
        let mut breakout_indices = HashMap::from([(key.clone(), 2usize)]);
        let mut ready_indices = HashMap::from([(key.clone(), 3usize)]);

        retain_active_lifecycle_entries("radar-v1.0.0", "watchlist:AAPL", &[], &mut strength_dates);
        retain_active_lifecycle_entries("radar-v1.0.0", "watchlist:AAPL", &[], &mut breakout_dates);
        retain_active_lifecycle_entries("radar-v1.0.0", "watchlist:AAPL", &[], &mut ready_dates);
        retain_active_lifecycle_entries(
            "radar-v1.0.0",
            "watchlist:AAPL",
            &[],
            &mut strength_indices,
        );
        retain_active_lifecycle_entries(
            "radar-v1.0.0",
            "watchlist:AAPL",
            &[],
            &mut breakout_indices,
        );
        retain_active_lifecycle_entries("radar-v1.0.0", "watchlist:AAPL", &[], &mut ready_indices);

        assert!(strength_dates.is_empty());
        assert!(breakout_dates.is_empty());
        assert!(ready_dates.is_empty());
        assert!(strength_indices.is_empty());
        assert!(breakout_indices.is_empty());
        assert!(ready_indices.is_empty());
    }

    #[test]
    fn net_decision_value_pairs_protection_and_confirmation_on_one_episode() {
        let mut paired = record(BacktestDecisionClass::NoTrade, true);
        paired.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        paired.forward_return_20d = Some(-0.10);
        paired.strength_to_ready_sessions = Some(2);
        paired.return_strength_to_ready = Some(0.08);

        let mut confirmation_only = record(BacktestDecisionClass::Probe, true);
        confirmation_only.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
        confirmation_only.strength_to_ready_sessions = Some(1);
        confirmation_only.return_strength_to_ready = Some(0.06);

        let net = build_net_decision_value(&[paired, confirmation_only]);

        assert_eq!(net.protection_episode_count, 1);
        assert_eq!(net.confirmation_episode_count, 1);
        assert_eq!(net.protection_benefit, Some(0.10));
        assert_eq!(net.confirmation_cost, Some(0.08));
        assert!((net.net_value.unwrap() - 0.02).abs() < 1e-12);
    }

    #[test]
    fn net_decision_value_is_unavailable_without_a_paired_episode() {
        let mut blocked = record(BacktestDecisionClass::NoTrade, true);
        blocked.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        blocked.forward_return_20d = Some(-0.10);

        let mut confirmed = record(BacktestDecisionClass::Probe, true);
        confirmed.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        confirmed.strength_to_ready_sessions = Some(1);
        confirmed.return_strength_to_ready = Some(0.06);

        let net = build_net_decision_value(&[blocked, confirmed]);

        assert_eq!(net.net_value, None);
    }

    #[test]
    fn validation_report_keeps_bidirectional_no_trade_outcomes_and_fixed_baseline() {
        let mut blocked_down = record(BacktestDecisionClass::NoTrade, true);
        blocked_down.forward_return_5d = Some(-0.05);
        blocked_down.forward_return_20d = Some(-0.10);
        blocked_down.mae_20d = Some(-0.12);
        blocked_down.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        blocked_down.validation_status = ValidationStatus::Complete;

        let mut blocked_up = record(BacktestDecisionClass::NoTrade, true);
        blocked_up.forward_return_5d = Some(0.04);
        blocked_up.forward_return_20d = Some(0.08);
        blocked_up.mae_20d = Some(-0.02);
        blocked_up.mfe_20d = Some(0.14);
        blocked_up.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
        blocked_up.validation_status = ValidationStatus::Complete;

        let mut ready = record(BacktestDecisionClass::Ready, true);
        ready.forward_return_20d = Some(0.06);
        ready.mae_20d = Some(-0.04);
        ready.mfe_20d = Some(0.10);
        ready.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 3).unwrap());
        ready.validation_status = ValidationStatus::Complete;

        let report = build_validation_report(&[blocked_down, blocked_up, ready]);
        let no_trade = report
            .outcomes
            .iter()
            .find(|outcome| outcome.decision_class == Some(BacktestDecisionClass::NoTrade))
            .unwrap();

        assert_eq!(no_trade.sample_count, 2);
        assert_eq!(no_trade.complete_10d, 0);
        assert_eq!(no_trade.positive_20d_count, 1);
        assert_eq!(report.baseline.raw_top3_sample_count, 3);
        assert_eq!(report.baseline.ready_sample_count, 1);
        assert_eq!(report.baseline.ready_average_20d_mfe, Some(0.10));
        assert!((report.baseline.return_difference.unwrap() - 0.04666666666666667).abs() < 1e-12);
        assert_eq!(report.sample_maturity, "INSUFFICIENT");
    }

    #[test]
    fn utility_excludes_non_candidates_censored_records_and_mixes_no_cohorts() {
        let mut blocked_complete = record(BacktestDecisionClass::NoTrade, true);
        blocked_complete.forward_return_20d = Some(-0.10);
        blocked_complete.mae_20d = Some(-0.12);
        blocked_complete.mfe_20d = Some(0.03);
        blocked_complete.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        blocked_complete.validation_status = ValidationStatus::Complete;

        let mut non_candidate = record(BacktestDecisionClass::NoTrade, false);
        non_candidate.forward_return_20d = Some(-0.50);
        non_candidate.mae_20d = Some(-0.60);
        non_candidate.validation_status = ValidationStatus::Complete;

        let mut censored = record(BacktestDecisionClass::NoTrade, true);
        censored.gate_blocked = true;
        censored.date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        censored.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
        censored.validation_status = ValidationStatus::Partial;

        let mut other_cohort = record(BacktestDecisionClass::NoTrade, true);
        other_cohort.universe_id = "watchlist:MSFT".to_string();
        other_cohort.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 3).unwrap());
        other_cohort.forward_return_20d = Some(-0.20);
        other_cohort.mae_20d = Some(-0.22);
        other_cohort.validation_status = ValidationStatus::Complete;

        let report =
            build_validation_report(&[blocked_complete, non_candidate, censored, other_cohort]);
        assert_eq!(report.cohorts.len(), 2);
        let cohort = report
            .cohorts
            .iter()
            .find(|cohort| cohort.universe_id == "watchlist:AAPL")
            .unwrap();
        assert_eq!(cohort.utility.blocked_candidate_count, 2);
        assert_eq!(cohort.utility.downside_20d_count, 1);
        assert_eq!(cohort.utility.complete_20d_count, 1);
        assert_eq!(cohort.utility.p95_mae_20d, Some(-0.12));
        assert_eq!(cohort.utility.top_decile_missed_upside, Some(0.03));
        assert_eq!(cohort.utility.horizon_5d.complete_sample_count, 0);
        assert_eq!(cohort.utility.horizon_20d.complete_sample_count, 1);
    }

    #[test]
    fn confirmation_cost_reports_all_lifecycle_latencies_and_price_costs() {
        let mut item = record(BacktestDecisionClass::Probe, true);
        item.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        item.strength_to_breakout_sessions = Some(2);
        item.breakout_to_ready_sessions = Some(3);
        item.strength_to_ready_sessions = Some(5);
        item.return_strength_to_ready = Some(0.08);
        item.return_breakout_to_ready = Some(0.03);
        item.max_move_strength_to_ready = Some(0.11);
        let mut duplicate = item.clone();
        duplicate.date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();

        let confirmation = build_confirmation_cost(&[item, duplicate]);
        assert_eq!(confirmation.episode_sample_count, 1);
        assert_eq!(
            confirmation.average_strength_to_breakout_sessions,
            Some(2.0)
        );
        assert_eq!(confirmation.average_breakout_to_ready_sessions, Some(3.0));
        assert_eq!(confirmation.average_strength_to_ready_sessions, Some(5.0));
        assert_eq!(confirmation.average_return_strength_to_ready, Some(0.08));
        assert_eq!(confirmation.average_return_lost_before_ready, Some(0.08));
        assert_eq!(confirmation.average_return_breakout_to_ready, Some(0.03));
        assert_eq!(confirmation.average_max_move_strength_to_ready, Some(0.11));
    }

    #[test]
    fn confirmation_cost_merges_lifecycle_completion_from_later_episode_records() {
        let mut strength_day = record(BacktestDecisionClass::Probe, true);
        strength_day.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        let mut ready_day = strength_day.clone();
        ready_day.date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        ready_day.decision_class = BacktestDecisionClass::Ready;
        ready_day.strength_to_ready_sessions = Some(1);
        ready_day.return_strength_to_ready = Some(0.05);
        ready_day.max_move_strength_to_ready = Some(0.07);

        let confirmation = build_confirmation_cost(&[strength_day, ready_day]);

        assert_eq!(confirmation.episode_sample_count, 1);
        assert_eq!(confirmation.lifecycle_complete_episode_count, 1);
        assert_eq!(confirmation.average_strength_to_ready_sessions, Some(1.0));
        assert_eq!(confirmation.average_return_strength_to_ready, Some(0.05));
        assert_eq!(confirmation.average_return_lost_before_ready, Some(0.05));
    }

    #[test]
    fn confirmation_cost_separates_negative_waiting_return_from_lost_upside() {
        let mut negative = record(BacktestDecisionClass::Probe, true);
        negative.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        negative.strength_to_ready_sessions = Some(2);
        negative.return_strength_to_ready = Some(-0.04);

        let confirmation = build_confirmation_cost(&[negative]);

        assert_eq!(confirmation.average_return_strength_to_ready, Some(-0.04));
        assert_eq!(confirmation.average_return_lost_before_ready, None);
    }

    #[test]
    fn ready_baseline_counts_episode_that_reaches_ready_after_strength() {
        let mut strength_day = record(BacktestDecisionClass::Probe, true);
        strength_day.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        strength_day.forward_return_20d = Some(0.10);
        let mut ready_day = strength_day.clone();
        ready_day.date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        ready_day.decision_class = BacktestDecisionClass::Ready;
        ready_day.ready_date = Some(ready_day.date);

        let report = build_validation_report(&[strength_day, ready_day]);

        assert_eq!(report.baseline.raw_top3_sample_count, 1);
        assert_eq!(report.baseline.ready_sample_count, 1);
    }

    #[test]
    fn sample_maturity_does_not_hide_protection_coverage_without_ready_lifecycle() {
        let records = (0..30)
            .map(|offset| {
                let mut item = record(BacktestDecisionClass::NoTrade, true);
                let date =
                    NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::Duration::days(offset);
                item.date = date;
                item.strength_date = Some(date);
                item.forward_return_20d = Some(-0.01);
                item
            })
            .collect::<Vec<_>>();

        let report = build_validation_report(&records);

        let cohort = &report.cohorts[0];
        assert_eq!(cohort.protection_sample_maturity, "DEVELOPING");
        assert_eq!(cohort.confirmation_sample_maturity, "INSUFFICIENT");
    }

    #[test]
    fn protection_maturity_uses_the_eligible_protection_cohort() {
        let mut eligible = record(BacktestDecisionClass::NoTrade, true);
        eligible.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        eligible.forward_return_20d = Some(-0.01);

        let unrelated = (0..30).map(|offset| {
            let mut item = record(BacktestDecisionClass::NoTrade, false);
            let date =
                NaiveDate::from_ymd_opt(2026, 2, 1).unwrap() + chrono::Duration::days(offset);
            item.date = date;
            item.strength_date = Some(date);
            item.forward_return_20d = Some(-0.01);
            item
        });
        let records = std::iter::once(eligible)
            .chain(unrelated)
            .collect::<Vec<_>>();

        let cohort = &build_validation_report(&records).cohorts[0];

        assert_eq!(cohort.protection_sample_maturity, "INSUFFICIENT");
    }

    #[test]
    fn reason_breakdown_includes_breadth_too_narrow() {
        let mut blocked = record(BacktestDecisionClass::NoTrade, true);
        blocked.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        blocked.decision_reasons = vec!["BREADTH_TOO_NARROW".to_string()];
        blocked.forward_return_20d = Some(-0.10);

        let report = build_validation_report(&[blocked]);
        let reason = report.cohorts[0]
            .utility
            .reason_breakdown
            .iter()
            .find(|item| item.reason == "BREADTH_TOO_NARROW")
            .unwrap();

        assert_eq!(reason.horizon_20d.complete_sample_count, 1);
        assert_eq!(reason.horizon_20d.downside_count, 1);
    }

    #[test]
    fn lifecycle_state_isolated_by_cohort_and_net_value_is_unavailable_without_cost() {
        let mut first = record(BacktestDecisionClass::NoTrade, true);
        first.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        first.decision_snapshot_version = "radar-v1.0.0".to_string();
        first.forward_return_20d = Some(-0.10);
        first.validation_status = ValidationStatus::Complete;

        let mut second = first.clone();
        second.decision_snapshot_version = "radar-v2.0.0".to_string();
        second.strength_to_ready_sessions = None;

        let report = build_validation_report(&[first, second]);
        assert_eq!(report.cohorts.len(), 2);
        for cohort in &report.cohorts {
            assert_eq!(cohort.confirmation_cost.episode_sample_count, 1);
            assert_eq!(cohort.net_decision_value.net_value, None);
        }
    }

    #[test]
    fn utility_reports_all_horizons_reason_breakdown_and_net_value() {
        let mut blocked = record(BacktestDecisionClass::NoTrade, true);
        blocked.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        blocked.decision_reasons = vec!["TREND_GATE_BLOCKED".to_string(), "NO_LEADER".to_string()];
        blocked.forward_return_5d = Some(-0.04);
        blocked.forward_return_10d = Some(-0.07);
        blocked.forward_return_20d = Some(-0.10);
        blocked.mae_5d = Some(-0.05);
        blocked.mae_10d = Some(-0.08);
        blocked.mae_20d = Some(-0.12);
        blocked.mfe_5d = Some(0.02);
        blocked.mfe_10d = Some(0.03);
        blocked.mfe_20d = Some(0.04);
        blocked.validation_status = ValidationStatus::Complete;

        let mut confirmed = record(BacktestDecisionClass::Probe, true);
        confirmed.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        confirmed.return_strength_to_ready = Some(0.06);
        confirmed.strength_to_ready_sessions = Some(3);
        confirmed.validation_status = ValidationStatus::Complete;

        let report = build_validation_report(&[blocked, confirmed]);
        let cohort = &report.cohorts[0];
        assert_eq!(cohort.utility.horizon_5d.complete_sample_count, 1);
        assert_eq!(cohort.utility.horizon_10d.complete_sample_count, 1);
        assert_eq!(cohort.utility.horizon_20d.complete_sample_count, 1);
        assert_eq!(
            cohort.utility.reason_breakdown[0]
                .horizon_20d
                .downside_count,
            1
        );
        assert_eq!(
            cohort.utility.reason_breakdown[1]
                .horizon_20d
                .downside_count,
            1
        );
        assert_eq!(cohort.net_decision_value.protection_benefit, Some(0.10));
        assert_eq!(cohort.net_decision_value.confirmation_cost, Some(0.06));
        assert!((cohort.net_decision_value.net_value.unwrap() - 0.04).abs() < 1e-12);
    }

    #[test]
    fn net_decision_value_excludes_non_raw_candidates_from_the_episode_denominator() {
        let mut raw = record(BacktestDecisionClass::NoTrade, true);
        raw.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        raw.forward_return_20d = Some(-0.10);
        raw.strength_to_ready_sessions = Some(2);
        raw.return_strength_to_ready = Some(0.04);

        let mut non_raw = raw.clone();
        non_raw.raw_candidate = false;
        non_raw.forward_return_20d = Some(-0.50);

        let net = build_net_decision_value(&[raw, non_raw]);

        assert_eq!(net.eligible_episode_count, 1);
        assert_eq!(net.horizon_20d.paired_episode_count, 1);
        assert_eq!(net.horizon_20d.unpaired_episode_count, 0);
        assert_eq!(net.horizon_20d.protection_benefit, Some(0.10));
    }

    #[test]
    fn net_decision_value_keeps_positive_and_incomplete_episodes_in_horizon_denominators() {
        let mut positive = record(BacktestDecisionClass::NoTrade, true);
        positive.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        positive.forward_return_5d = Some(0.08);
        positive.forward_return_10d = Some(0.12);
        positive.forward_return_20d = Some(0.15);
        positive.strength_to_ready_sessions = Some(3);
        positive.return_strength_to_ready = Some(0.03);

        let mut incomplete = positive.clone();
        incomplete.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
        incomplete.forward_return_10d = None;
        incomplete.return_strength_to_ready = None;

        let net = build_net_decision_value(&[positive, incomplete]);

        assert_eq!(net.eligible_episode_count, 2);
        assert_eq!(net.horizon_5d.paired_episode_count, 1);
        assert_eq!(net.horizon_5d.unpaired_episode_count, 1);
        assert_eq!(net.horizon_5d.protection_benefit, Some(0.0));
        assert_eq!(net.horizon_10d.paired_episode_count, 1);
        assert_eq!(net.horizon_10d.unpaired_episode_count, 1);
        assert_eq!(net.horizon_20d.paired_episode_count, 1);
        assert_eq!(net.horizon_20d.unpaired_episode_count, 1);
    }

    #[test]
    fn net_decision_value_does_not_pair_confirmation_after_horizon() {
        let mut delayed = record(BacktestDecisionClass::NoTrade, true);
        delayed.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        delayed.forward_return_5d = Some(-0.10);
        delayed.forward_return_10d = Some(-0.12);
        delayed.forward_return_20d = Some(-0.15);
        delayed.strength_to_ready_sessions = Some(10);
        delayed.return_strength_to_ready = Some(0.08);

        let net = build_net_decision_value(&[delayed]);

        assert_eq!(net.horizon_5d.paired_episode_count, 0);
        assert_eq!(net.horizon_5d.unpaired_episode_count, 1);
        assert_eq!(net.horizon_10d.paired_episode_count, 1);
        assert_eq!(net.horizon_20d.paired_episode_count, 1);
    }

    #[test]
    fn net_decision_value_preserves_negative_waiting_return_separately() {
        let mut blocked = record(BacktestDecisionClass::NoTrade, true);
        blocked.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        blocked.forward_return_20d = Some(-0.10);
        blocked.strength_to_ready_sessions = Some(2);
        blocked.return_strength_to_ready = Some(-0.04);

        let net = build_net_decision_value(&[blocked]);

        assert_eq!(net.horizon_20d.confirmation_cost, Some(0.0));
        assert_eq!(net.horizon_20d.adverse_waiting_return, Some(-0.04));
        assert_eq!(net.horizon_20d.adverse_waiting_sample_count, 1);
    }

    #[test]
    fn confirmation_cost_uses_the_same_trend_gate_eligible_cohort_as_net_value() {
        let mut blocked = record(BacktestDecisionClass::NoTrade, true);
        blocked.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        blocked.strength_to_ready_sessions = Some(2);
        blocked.return_strength_to_ready = Some(0.04);

        let mut unrelated = record(BacktestDecisionClass::Probe, true);
        unrelated.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
        unrelated.strength_to_ready_sessions = Some(1);
        unrelated.return_strength_to_ready = Some(0.50);

        let report = build_validation_report(&[blocked, unrelated]);
        let cohort = &report.cohorts[0];

        assert_eq!(cohort.confirmation_cost.episode_sample_count, 1);
        assert_eq!(
            cohort.confirmation_cost.average_return_strength_to_ready,
            Some(0.04)
        );
    }

    #[test]
    fn confirmation_summary_exposes_episode_and_lifecycle_denominators_in_markdown() {
        let mut incomplete = record(BacktestDecisionClass::Probe, true);
        incomplete.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        let mut complete = incomplete.clone();
        complete.strength_date = Some(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
        complete.strength_to_ready_sessions = Some(3);

        let confirmation = build_confirmation_cost(&[incomplete, complete]);

        assert_eq!(confirmation.episode_sample_count, 2);
        assert_eq!(confirmation.lifecycle_complete_episode_count, 1);
    }
}
