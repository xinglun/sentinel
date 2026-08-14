use crate::features::backtest::application::model::ValidationStatus;
use crate::features::shared::domain::market_data::DailyBar;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ForwardOutcome {
    pub forward_return: f64,
    pub mfe: f64,
    pub mae: f64,
}

pub(crate) fn forward_outcome(
    bars: &[DailyBar],
    decision_session_index: usize,
    horizon_sessions: usize,
) -> Option<ForwardOutcome> {
    let end = decision_session_index.checked_add(horizon_sessions)?;
    let decision = bars.get(decision_session_index)?.close;
    let future_close = bars.get(end)?.close;
    if decision <= 0.0 {
        return None;
    }

    let path = bars.get(decision_session_index + 1..=end)?;
    let mfe = path
        .iter()
        .filter_map(|bar| bar.high.or(Some(bar.close)))
        .map(|price| (price - decision) / decision)
        .fold(0.0, f64::max);
    let mae = path
        .iter()
        .filter_map(|bar| bar.low.or(Some(bar.close)))
        .map(|price| (price - decision) / decision)
        .fold(0.0, f64::min);

    Some(ForwardOutcome {
        forward_return: (future_close - decision) / decision,
        mfe,
        mae,
    })
}

pub(crate) fn validation_status(has_5d: bool, _has_10d: bool, has_20d: bool) -> ValidationStatus {
    if has_20d {
        ValidationStatus::Complete
    } else if has_5d {
        ValidationStatus::Partial
    } else {
        ValidationStatus::Pending
    }
}

pub(crate) fn empirical_quantile(values: &[f64], quantile: f64) -> Option<f64> {
    if values.is_empty() || !(0.0..=1.0).contains(&quantile) {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = ((sorted.len() as f64 * quantile).ceil() as usize).max(1);
    sorted
        .get(rank.saturating_sub(1).min(sorted.len() - 1))
        .copied()
}

pub(crate) fn top_decile_mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let count = ((sorted.len() as f64 * 0.1).ceil() as usize).max(1);
    let tail = &sorted[sorted.len() - count..];
    Some(tail.iter().sum::<f64>() / tail.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn bar(day: u32, close: f64, high: f64, low: f64) -> DailyBar {
        DailyBar {
            date: NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
            open: None,
            high: Some(high),
            low: Some(low),
            close,
            volume: None,
        }
    }

    #[test]
    fn horizon_uses_trading_session_index_and_keeps_mfe_mae_from_path() {
        let bars = vec![
            bar(2, 100.0, 101.0, 99.0),
            bar(3, 103.0, 105.0, 98.0),
            bar(5, 102.0, 106.0, 97.0),
            bar(6, 108.0, 110.0, 101.0),
        ];
        let outcome = forward_outcome(&bars, 0, 3).unwrap();
        assert_eq!(outcome.forward_return, 0.08);
        assert_eq!(outcome.mfe, 0.10);
        assert_eq!(outcome.mae, -0.03);
        assert!(forward_outcome(&bars, 0, 4).is_none());
    }

    #[test]
    fn status_marks_partial_without_zero_filling_uncensored_horizons() {
        assert_eq!(
            validation_status(true, false, false),
            ValidationStatus::Partial
        );
        assert_eq!(
            validation_status(false, false, false),
            ValidationStatus::Pending
        );
        assert_eq!(
            validation_status(true, true, true),
            ValidationStatus::Complete
        );
    }

    #[test]
    fn empirical_tail_quantiles_and_top_decile_mean_are_stable() {
        let values = [
            -0.30, -0.20, -0.10, -0.05, 0.01, 0.04, 0.08, 0.12, 0.20, 0.40,
        ];
        assert_eq!(empirical_quantile(&values, 0.90), Some(0.20));
        assert_eq!(empirical_quantile(&values, 0.95), Some(0.40));
        assert_eq!(top_decile_mean(&[0.01, 0.04, 0.08, 0.12, 0.20]), Some(0.20));
    }

    #[test]
    fn invalid_tail_inputs_do_not_create_utility_facts() {
        assert_eq!(empirical_quantile(&[], 0.90), None);
        assert_eq!(top_decile_mean(&[],), None);
    }
}
