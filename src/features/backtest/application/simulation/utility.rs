use crate::features::backtest::application::model::{
    BacktestDecisionClass, ValidationDecisionRecord, ValidationHorizonUtility,
    ValidationReasonUtility, ValidationUtility,
};
use crate::features::backtest::application::validation::{empirical_quantile, top_decile_mean};

pub(crate) fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

pub(super) fn build_utility(records: &[ValidationDecisionRecord]) -> ValidationUtility {
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

pub(super) fn build_horizon_utility(
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
