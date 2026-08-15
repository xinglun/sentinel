use super::confirmation::build_confirmation_cost_from_episodes;
use super::net_value::{build_net_decision_value, is_trend_gate_eligible};
use super::utility::build_utility;
use crate::features::backtest::application::model::{
    BacktestDecisionClass, ValidationBaselineComparison, ValidationCohortReport,
    ValidationDecisionRecord, ValidationPopulationAudit, ValidationPopulationReasonCount,
    ValidationReport,
};
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};

pub(crate) fn build_validation_report(records: &[ValidationDecisionRecord]) -> ValidationReport {
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
        .map(|decision_class| super::runner::build_class_outcome(records, decision_class))
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
        population: build_population_audit(records),
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

fn build_population_audit(records: &[ValidationDecisionRecord]) -> ValidationPopulationAudit {
    let gate_blocked = records
        .iter()
        .filter(|record| record.gate_blocked)
        .collect::<Vec<_>>();
    let raw_candidates = records
        .iter()
        .filter(|record| record.raw_candidate)
        .collect::<Vec<_>>();
    let raw_and_blocked = records
        .iter()
        .filter(|record| record.raw_candidate && record.gate_blocked)
        .collect::<Vec<_>>();
    let mut reason_counts = HashMap::<String, usize>::new();
    for record in gate_blocked.iter().filter(|record| !record.raw_candidate) {
        let mut reasons = HashSet::new();
        for reason in &record.decision_reasons {
            if reasons.insert(reason) {
                *reason_counts.entry(reason.clone()).or_default() += 1;
            }
        }
    }
    let mut gate_blocked_non_candidate_reasons = reason_counts
        .into_iter()
        .map(|(reason, count)| ValidationPopulationReasonCount { reason, count })
        .collect::<Vec<_>>();
    gate_blocked_non_candidate_reasons.sort_by(|left, right| left.reason.cmp(&right.reason));

    ValidationPopulationAudit {
        classified_record_count: records.len(),
        gate_blocked_record_count: gate_blocked.len(),
        raw_candidate_record_count: raw_candidates.len(),
        raw_candidate_gate_blocked_record_count: raw_and_blocked.len(),
        raw_candidate_gate_blocked_no_trade_record_count: raw_and_blocked
            .iter()
            .filter(|record| record.decision_class == BacktestDecisionClass::NoTrade)
            .count(),
        gate_blocked_non_candidate_record_count: gate_blocked
            .iter()
            .filter(|record| !record.raw_candidate)
            .count(),
        gate_blocked_non_candidate_reasons,
    }
}

pub(crate) fn episode_records(
    records: &[ValidationDecisionRecord],
) -> Vec<ValidationDecisionRecord> {
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
