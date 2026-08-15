use super::utility::mean;
use crate::features::backtest::application::model::{
    BacktestDecisionClass, NetDecisionHorizon, NetDecisionValue, ValidationDecisionRecord,
};

pub(crate) fn build_net_decision_value(records: &[ValidationDecisionRecord]) -> NetDecisionValue {
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

pub(super) fn is_trend_gate_eligible(record: &ValidationDecisionRecord) -> bool {
    record.raw_candidate
        && record.gate_blocked
        && record.decision_class == BacktestDecisionClass::NoTrade
}
