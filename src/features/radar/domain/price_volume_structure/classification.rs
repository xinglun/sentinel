use super::baseline::secondary_baseline_start;
use super::baseline::{baseline_selection, is_valid_ohlcv};
use super::eligibility::{
    next_eligibility_condition, observation_confidence, observation_is_unavailable,
    unavailable_reason, volume_quality,
};
use super::lifecycle::lifecycle;
use super::*;

pub(crate) fn assess_price_volume_structure(input: PriceVolumeInput<'_>) -> PriceVolumeAssessment {
    let quality = volume_quality(
        input.bars,
        input.source_rate_limited,
        input.volume_comparable,
        input.market_holidays,
    );
    let boundary = PriceVolumeObservationBoundary {
        decision_weight_percent: 0,
        trade_signal: false,
        gate_effect: ObservationEffect::None,
        execution_effect: ObservationEffect::None,
        position_sizing_effect: ObservationEffect::None,
    };
    let persistence_days = input.persistence_days.max(1);
    let mut persistence = match persistence_days {
        1 => StructurePersistence::Candidate,
        2 => StructurePersistence::Developing,
        _ => StructurePersistence::Confirmed,
    };
    let (eligibility, primary_baseline, secondary_baseline, baseline_start) =
        baseline_selection(&input);
    if matches!(
        eligibility,
        EligibilityStatus::Unavailable | EligibilityStatus::Insufficient
    ) {
        persistence = StructurePersistence::Unavailable;
    }
    let unavailable_reason = unavailable_reason(&input, eligibility);
    if matches!(
        eligibility,
        EligibilityStatus::Unavailable | EligibilityStatus::Insufficient
    ) {
        return PriceVolumeAssessment {
            structure: PriceVolumeStructure::Unavailable,
            participation: ParticipationQuality::Unavailable,
            supply_absorption: SupplyAbsorption::Unavailable,
            quality,
            persistence,
            persistence_days,
            metrics: None,
            secondary_metrics: None,
            observation_confidence: observation_confidence(eligibility),
            boundary,
            eligibility,
            primary_baseline,
            secondary_baseline,
            lifecycle: CandidateLifecycle::Unavailable,
            unavailable_reason,
            next_eligibility_condition: next_eligibility_condition(eligibility, unavailable_reason),
        };
    }
    let Some(primary_metrics) = metrics(input.bars, primary_baseline, baseline_start) else {
        return PriceVolumeAssessment {
            structure: PriceVolumeStructure::Unavailable,
            participation: ParticipationQuality::Unavailable,
            supply_absorption: SupplyAbsorption::Unavailable,
            quality,
            persistence,
            persistence_days,
            metrics: None,
            secondary_metrics: None,
            observation_confidence: observation_confidence(eligibility),
            boundary,
            eligibility,
            primary_baseline,
            secondary_baseline,
            lifecycle: CandidateLifecycle::Unavailable,
            unavailable_reason: Some(
                unavailable_reason.unwrap_or(UnavailableReason::MissingVolume),
            ),
            next_eligibility_condition: Some(
                "Need one valid OHLCV session before calculating a comparison.".to_string(),
            ),
        };
    };
    if observation_is_unavailable(&input, quality) {
        return PriceVolumeAssessment {
            structure: PriceVolumeStructure::Unavailable,
            participation: ParticipationQuality::Unavailable,
            supply_absorption: SupplyAbsorption::Unavailable,
            quality,
            persistence,
            persistence_days,
            metrics: Some(primary_metrics),
            secondary_metrics: secondary_baseline.and_then(|baseline| {
                metrics(
                    input.bars,
                    baseline,
                    secondary_baseline_start(&input, baseline),
                )
            }),
            observation_confidence: observation_confidence(eligibility),
            boundary,
            eligibility,
            primary_baseline,
            secondary_baseline,
            lifecycle: CandidateLifecycle::Unavailable,
            unavailable_reason,
            next_eligibility_condition: next_eligibility_condition(eligibility, unavailable_reason),
        };
    }
    let supply_increase = input.supply_context.is_some_and(|context| {
        context.availability == SupplyEventContextAvailability::Available
            && context.supply_direction == SupplyDirection::Increase
            && context.confidence == SupplyEventConfidence::High
    });
    let limited_downside = primary_metrics.return_1d >= -1.5
        && primary_metrics.return_5d >= -3.0
        && primary_metrics
            .atr_normalized_move
            .is_some_and(|move_size| move_size <= 1.0)
        && primary_metrics
            .lower_wick_ratio
            .is_some_and(|ratio| ratio >= 0.20);
    let high_price_position =
        primary_metrics.new_high || primary_metrics.distance_from_20d_high >= -2.0;
    let stalled_candle = primary_metrics
        .body_ratio
        .is_some_and(|ratio| ratio <= 0.45)
        || primary_metrics
            .upper_wick_ratio
            .is_some_and(|ratio| ratio >= 0.30);
    let downside_breakdown =
        primary_metrics.new_low || primary_metrics.gap_percent.is_some_and(|gap| gap <= -1.0);
    let potential_accumulation =
        primary_metrics.relative_volume >= 1.3 && limited_downside && !primary_metrics.new_low;
    let accumulation = supply_increase && potential_accumulation;
    let supply_context_missing = input
        .supply_context
        .is_none_or(|context| context.availability == SupplyEventContextAvailability::Unavailable);
    let accumulation_candidate = supply_context_missing
        && input.persistence_days >= 2
        && potential_accumulation
        && primary_metrics.return_5d < 0.0
        && !primary_metrics.new_high;
    let exhausted = primary_metrics.return_5d > 2.0
        && high_price_position
        && primary_metrics.relative_volume < 1.0
        && primary_metrics.rvol_5 < 1.0
        && stalled_candle
        && (input.overheated || input.time_cost_rising);
    let healthy = primary_metrics.return_5d > 2.0
        && high_price_position
        && primary_metrics.relative_volume >= 1.0
        && primary_metrics.up_day_average_volume > primary_metrics.down_day_average_volume;
    let distribution = primary_metrics.return_5d < -2.0
        && primary_metrics.relative_volume >= 1.3
        && primary_metrics.down_day_average_volume > primary_metrics.up_day_average_volume
        && downside_breakdown;
    let (structure, participation, supply_absorption) = if accumulation {
        (
            PriceVolumeStructure::Accumulation,
            ParticipationQuality::Improving,
            if eligibility == EligibilityStatus::Full {
                SupplyAbsorption::Active
            } else {
                SupplyAbsorption::Candidate
            },
        )
    } else if accumulation_candidate {
        (
            PriceVolumeStructure::AccumulationCandidate,
            ParticipationQuality::Improving,
            SupplyAbsorption::None,
        )
    } else if exhausted {
        (
            PriceVolumeStructure::ExhaustedAdvance,
            ParticipationQuality::Weakening,
            SupplyAbsorption::None,
        )
    } else if healthy {
        (
            PriceVolumeStructure::HealthyAdvance,
            ParticipationQuality::Healthy,
            SupplyAbsorption::None,
        )
    } else if distribution {
        (
            PriceVolumeStructure::Distribution,
            ParticipationQuality::Deteriorating,
            SupplyAbsorption::None,
        )
    } else {
        (
            PriceVolumeStructure::Neutral,
            ParticipationQuality::Neutral,
            SupplyAbsorption::None,
        )
    };
    PriceVolumeAssessment {
        structure,
        participation,
        supply_absorption,
        quality,
        persistence,
        persistence_days,
        metrics: Some(primary_metrics),
        secondary_metrics: secondary_baseline.and_then(|baseline| {
            metrics(
                input.bars,
                baseline,
                secondary_baseline_start(&input, baseline),
            )
        }),
        observation_confidence: observation_confidence(eligibility),
        boundary,
        eligibility,
        primary_baseline,
        secondary_baseline,
        lifecycle: lifecycle(
            eligibility,
            persistence_days,
            structure,
            baseline_start.is_some()
                && !matches!(
                    structure,
                    PriceVolumeStructure::Neutral | PriceVolumeStructure::AccumulationCandidate
                ),
        ),
        unavailable_reason: accumulation_candidate
            .then_some(UnavailableReason::MissingSupplyContext),
        next_eligibility_condition: next_eligibility_condition(
            eligibility,
            accumulation_candidate.then_some(UnavailableReason::MissingSupplyContext),
        ),
    }
}

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

pub(super) fn metrics(
    bars: &[DailyBar],
    baseline_type: BaselineType,
    baseline_start: Option<NaiveDate>,
) -> Option<PriceVolumeMetrics> {
    let current = bars.last()?;
    let volume = current.volume?;
    if volume <= 0.0 {
        return None;
    }
    let baseline_bars = baseline_start
        .map(|start| {
            let event_bars = bars
                .iter()
                .filter(|bar| bar.date >= start)
                .cloned()
                .collect::<Vec<_>>();
            if event_bars.iter().filter(|bar| is_valid_ohlcv(bar)).count() >= 2 {
                event_bars
            } else {
                bars.to_vec()
            }
        })
        .unwrap_or_else(|| bars.to_vec());
    let prior = baseline_bars[..baseline_bars.len().saturating_sub(1)]
        .iter()
        .filter(|bar| is_valid_ohlcv(bar))
        .cloned()
        .collect::<Vec<_>>();
    if prior.is_empty() {
        return None;
    }
    let recent20 = &prior[prior.len().saturating_sub(20)..];
    let recent5 = &prior[prior.len().saturating_sub(5)..];
    let avg = |items: &[DailyBar]| -> Option<f64> {
        let values: Vec<f64> = items
            .iter()
            .filter_map(|bar| bar.volume)
            .filter(|value| *value > 0.0)
            .collect();
        (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
    };
    let avg5 = avg(recent5)?;
    let avg20 = avg(recent20)?;
    let change = |days: usize| {
        if bars.len() > days {
            (current.close / bars[bars.len() - 1 - days].close - 1.0) * 100.0
        } else {
            0.0
        }
    };
    let up: Vec<DailyBar> = recent20
        .iter()
        .filter(|bar| bar.close > bar.open.unwrap_or(bar.close))
        .cloned()
        .collect();
    let down: Vec<DailyBar> = recent20
        .iter()
        .filter(|bar| bar.close < bar.open.unwrap_or(bar.close))
        .cloned()
        .collect();
    let high = recent20
        .iter()
        .map(|bar| bar.high.unwrap_or(bar.close))
        .fold(current.close, f64::max);
    let low = recent20
        .iter()
        .map(|bar| bar.low.unwrap_or(bar.close))
        .fold(current.close, f64::min);
    let range = current
        .high
        .zip(current.low)
        .map(|(high, low)| high - low)
        .filter(|range| *range > 0.0);
    let open = current.open?;
    let atr_values = bars[bars.len().saturating_sub(15)..]
        .windows(2)
        .filter_map(|pair| {
            let previous_close = pair[0].close;
            let bar = &pair[1];
            let high = bar.high?;
            let low = bar.low?;
            Some(
                (high - low)
                    .max((high - previous_close).abs())
                    .max((low - previous_close).abs()),
            )
        })
        .collect::<Vec<_>>();
    let atr =
        (atr_values.len() == 14).then(|| atr_values.iter().sum::<f64>() / atr_values.len() as f64);
    Some(PriceVolumeMetrics {
        return_1d: change(1),
        return_5d: change(5),
        return_10d: change(10),
        return_20d: change(20),
        rvol_5: volume / avg5,
        rvol_20: volume / avg20,
        average_volume_5: avg5,
        average_volume_20: avg20,
        up_day_average_volume: avg(&up).unwrap_or(0.0),
        down_day_average_volume: avg(&down).unwrap_or(0.0),
        distance_from_20d_high: (current.close / high - 1.0) * 100.0,
        distance_from_20d_low: (current.close / low - 1.0) * 100.0,
        new_high: current.close >= high,
        new_low: current.close <= low,
        atr_normalized_move: atr.map(|value| (current.close - open).abs() / value),
        body_ratio: range.map(|value| (current.close - open).abs() / value),
        upper_wick_ratio: range
            .map(|value| (current.high.unwrap_or(current.close) - current.close.max(open)) / value),
        lower_wick_ratio: range
            .map(|value| (current.close.min(open) - current.low.unwrap_or(current.close)) / value),
        gap_percent: (bars.len() >= 2).then(|| (open / bars[bars.len() - 2].close - 1.0) * 100.0),
        baseline_days: prior.len(),
        baseline_type,
        relative_volume: volume / avg20,
        relative_volume_label: match baseline_type {
            BaselineType::Standard20d => "RVOL_20".to_string(),
            BaselineType::AvailableHistory => "RVOL_AVAILABLE".to_string(),
            BaselineType::PostIpo => "RVOL_POST_IPO".to_string(),
            BaselineType::PostEvent => "RVOL_POST_EVENT".to_string(),
            BaselineType::PostLockup => "RVOL_POST_LOCKUP".to_string(),
            BaselineType::PostEarnings => "RVOL_POST_EARNINGS".to_string(),
            BaselineType::Unavailable => "RVOL_UNAVAILABLE".to_string(),
        },
    })
}
