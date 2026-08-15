#[cfg(test)]
use super::episodes::episode_records;
use super::utility::mean;
use crate::features::backtest::application::model::{
    ConfirmationCostSummary, ValidationDecisionRecord,
};

#[cfg(test)]
pub(crate) fn build_confirmation_cost(
    records: &[ValidationDecisionRecord],
) -> ConfirmationCostSummary {
    let records = episode_records(records);
    let records = records.iter().collect::<Vec<_>>();
    build_confirmation_cost_from_episodes(&records)
}

pub(crate) fn build_confirmation_cost_from_episodes(
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
