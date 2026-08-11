#![allow(dead_code)]

use crate::features::shared::domain::market_data::DailyBar;
use crate::features::shared::domain::supply_event_context::{
    ObservationEffect, SupplyDirection, SupplyEventConfidence, SupplyEventContext,
    SupplyEventContextAvailability,
};
use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum EligibilityStatus {
    Full,
    Partial,
    Insufficient,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum BaselineType {
    Standard20d,
    AvailableHistory,
    PostIpo,
    PostEvent,
    PostLockup,
    PostEarnings,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum CandidateLifecycle {
    Candidate,
    Developing,
    Confirmed,
    #[default]
    Invalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UnavailableReason {
    InsufficientValidHistory,
    MissingVolume,
    DataGap,
    CorporateActionConflict,
    ApiFailure,
    MissingSupplyContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PriceVolumeStructure {
    Accumulation,
    AccumulationCandidate,
    HealthyAdvance,
    ExhaustedAdvance,
    Distribution,
    Neutral,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum VolumeDataQuality {
    Healthy,
    Partial,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ParticipationQuality {
    Improving,
    Healthy,
    Weakening,
    Deteriorating,
    Neutral,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SupplyAbsorption {
    Active,
    Candidate,
    None,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum StructurePersistence {
    Candidate,
    Developing,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PriceVolumeMetrics {
    pub return_1d: f64,
    pub return_5d: f64,
    pub return_10d: f64,
    pub return_20d: f64,
    pub rvol_5: f64,
    pub rvol_20: f64,
    pub average_volume_5: f64,
    pub average_volume_20: f64,
    pub up_day_average_volume: f64,
    pub down_day_average_volume: f64,
    pub distance_from_20d_high: f64,
    pub distance_from_20d_low: f64,
    pub new_high: bool,
    pub new_low: bool,
    pub atr_normalized_move: Option<f64>,
    pub body_ratio: Option<f64>,
    pub upper_wick_ratio: Option<f64>,
    pub lower_wick_ratio: Option<f64>,
    pub gap_percent: Option<f64>,
    pub baseline_days: usize,
    pub baseline_type: BaselineType,
    pub relative_volume: f64,
    pub relative_volume_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PriceVolumeObservationBoundary {
    pub decision_weight_percent: u8,
    pub trade_signal: bool,
    pub gate_effect: ObservationEffect,
    pub execution_effect: ObservationEffect,
    pub position_sizing_effect: ObservationEffect,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PriceVolumeAssessment {
    pub structure: PriceVolumeStructure,
    pub participation: ParticipationQuality,
    pub supply_absorption: SupplyAbsorption,
    pub quality: VolumeDataQuality,
    pub persistence: StructurePersistence,
    pub persistence_days: u8,
    pub metrics: Option<PriceVolumeMetrics>,
    pub boundary: PriceVolumeObservationBoundary,
    #[serde(default)]
    pub eligibility: EligibilityStatus,
    #[serde(default)]
    pub primary_baseline: BaselineType,
    #[serde(default)]
    pub secondary_baseline: Option<BaselineType>,
    #[serde(default)]
    pub lifecycle: CandidateLifecycle,
    #[serde(default)]
    pub unavailable_reason: Option<UnavailableReason>,
    #[serde(default)]
    pub next_eligibility_condition: Option<String>,
}

pub(crate) struct PriceVolumeInput<'a> {
    pub bars: &'a [DailyBar],
    pub supply_context: Option<&'a SupplyEventContext>,
    pub overheated: bool,
    pub time_cost_rising: bool,
    pub persistence_days: u8,
    pub source_rate_limited: bool,
    pub volume_comparable: bool,
    pub event_baseline: Option<(BaselineType, NaiveDate)>,
    pub secondary_supply_context: Option<&'a SupplyEventContext>,
}

pub(crate) fn assess_price_volume_structure(input: PriceVolumeInput<'_>) -> PriceVolumeAssessment {
    let quality = volume_quality(
        input.bars,
        input.source_rate_limited,
        input.volume_comparable,
    );
    let boundary = PriceVolumeObservationBoundary {
        decision_weight_percent: 0,
        trade_signal: false,
        gate_effect: ObservationEffect::None,
        execution_effect: ObservationEffect::None,
        position_sizing_effect: ObservationEffect::None,
    };
    let persistence_days = input.persistence_days.max(1);
    let persistence = match persistence_days {
        1 => StructurePersistence::Candidate,
        2 => StructurePersistence::Developing,
        _ => StructurePersistence::Confirmed,
    };
    let (eligibility, primary_baseline, secondary_baseline, baseline_start) =
        baseline_selection(&input);
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
            boundary,
            eligibility,
            primary_baseline,
            secondary_baseline,
            lifecycle: CandidateLifecycle::Invalidated,
            unavailable_reason,
            next_eligibility_condition: next_eligibility_condition(eligibility, unavailable_reason),
        };
    }
    let Some(metrics) = metrics(input.bars, primary_baseline, baseline_start) else {
        return PriceVolumeAssessment {
            structure: PriceVolumeStructure::Unavailable,
            participation: ParticipationQuality::Unavailable,
            supply_absorption: SupplyAbsorption::Unavailable,
            quality,
            persistence,
            persistence_days,
            metrics: None,
            boundary,
            eligibility,
            primary_baseline,
            secondary_baseline,
            lifecycle: CandidateLifecycle::Invalidated,
            unavailable_reason: Some(UnavailableReason::MissingVolume),
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
            metrics: Some(metrics),
            boundary,
            eligibility,
            primary_baseline,
            secondary_baseline,
            lifecycle: CandidateLifecycle::Invalidated,
            unavailable_reason,
            next_eligibility_condition: next_eligibility_condition(eligibility, unavailable_reason),
        };
    }
    let supply_increase = input.supply_context.is_some_and(|context| {
        context.availability == SupplyEventContextAvailability::Available
            && context.supply_direction == SupplyDirection::Increase
            && context.confidence == SupplyEventConfidence::High
    });
    let limited_downside = metrics.return_1d >= -1.5
        && metrics.return_5d >= -3.0
        && metrics
            .atr_normalized_move
            .is_some_and(|move_size| move_size <= 1.0)
        && metrics.lower_wick_ratio.is_some_and(|ratio| ratio >= 0.20);
    let high_price_position = metrics.new_high || metrics.distance_from_20d_high >= -2.0;
    let stalled_candle = metrics.body_ratio.is_some_and(|ratio| ratio <= 0.45)
        || metrics.upper_wick_ratio.is_some_and(|ratio| ratio >= 0.30);
    let downside_breakdown = metrics.new_low || metrics.gap_percent.is_some_and(|gap| gap <= -1.0);
    let potential_accumulation =
        metrics.relative_volume >= 1.3 && limited_downside && !metrics.new_low;
    let accumulation = supply_increase && potential_accumulation;
    let accumulation_candidate = input.supply_context.is_none()
        && input.persistence_days >= 2
        && potential_accumulation
        && metrics.return_5d < 0.0
        && !metrics.new_high;
    let exhausted = metrics.return_5d > 2.0
        && high_price_position
        && metrics.relative_volume < 1.0
        && metrics.rvol_5 < 1.0
        && stalled_candle
        && (input.overheated || input.time_cost_rising);
    let healthy = metrics.return_5d > 2.0
        && high_price_position
        && metrics.relative_volume >= 1.0
        && metrics.up_day_average_volume > metrics.down_day_average_volume;
    let distribution = metrics.return_5d < -2.0
        && metrics.relative_volume >= 1.3
        && metrics.down_day_average_volume > metrics.up_day_average_volume
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
        metrics: Some(metrics),
        boundary,
        eligibility,
        primary_baseline,
        secondary_baseline,
        lifecycle: lifecycle(eligibility, persistence_days, structure),
        unavailable_reason: accumulation_candidate
            .then_some(UnavailableReason::MissingSupplyContext),
        next_eligibility_condition: next_eligibility_condition(
            eligibility,
            accumulation_candidate.then_some(UnavailableReason::MissingSupplyContext),
        ),
    }
}

fn volume_quality(
    bars: &[DailyBar],
    rate_limited: bool,
    volume_comparable: bool,
) -> VolumeDataQuality {
    if rate_limited || !volume_comparable || !continuous_dates(bars) {
        return VolumeDataQuality::Degraded;
    }
    if bars.len() < 5 {
        return VolumeDataQuality::Unavailable;
    }
    let present = bars
        .iter()
        .filter(|bar| bar.volume.is_some_and(|value| value > 0.0))
        .count();
    if present < bars.len() {
        if present >= 5 {
            VolumeDataQuality::Degraded
        } else {
            VolumeDataQuality::Unavailable
        }
    } else {
        let complete_ohlc = bars
            .iter()
            .all(|bar| bar.open.is_some() && bar.high.is_some() && bar.low.is_some());
        if complete_ohlc {
            if bars.len() >= 20 {
                VolumeDataQuality::Healthy
            } else {
                VolumeDataQuality::Partial
            }
        } else {
            VolumeDataQuality::Unavailable
        }
    }
}

fn observation_is_unavailable(input: &PriceVolumeInput<'_>, quality: VolumeDataQuality) -> bool {
    input.source_rate_limited
        || !input.volume_comparable
        || !continuous_dates(input.bars)
        || quality == VolumeDataQuality::Unavailable
}

fn baseline_selection(
    input: &PriceVolumeInput<'_>,
) -> (
    EligibilityStatus,
    BaselineType,
    Option<BaselineType>,
    Option<NaiveDate>,
) {
    let valid_days = input.bars.iter().filter(|bar| is_valid_ohlcv(bar)).count();
    if input.source_rate_limited {
        return (
            EligibilityStatus::Unavailable,
            BaselineType::Unavailable,
            None,
            None,
        );
    }
    if !input.volume_comparable || !continuous_dates(input.bars) {
        return (
            EligibilityStatus::Unavailable,
            BaselineType::Unavailable,
            None,
            None,
        );
    }
    let eligibility = if valid_days >= 20
        && volume_quality(
            input.bars,
            input.source_rate_limited,
            input.volume_comparable,
        ) == VolumeDataQuality::Healthy
    {
        EligibilityStatus::Full
    } else {
        EligibilityStatus::Partial
    };
    let supply_baseline = input.supply_context.and_then(|context| {
        let baseline = match context.event_type {
            crate::features::shared::domain::supply_event_context::SupplyEventType::Ipo => {
                BaselineType::PostIpo
            }
            crate::features::shared::domain::supply_event_context::SupplyEventType::LockupExpiry
            | crate::features::shared::domain::supply_event_context::SupplyEventType::ShareUnlock => {
                BaselineType::PostLockup
            }
            _ => BaselineType::PostEvent,
        };
        context.event_date.map(|date| (baseline, date))
    });
    let secondary_supply_baseline = input.secondary_supply_context.and_then(|context| {
        let baseline = match context.event_type {
            crate::features::shared::domain::supply_event_context::SupplyEventType::Ipo => {
                BaselineType::PostIpo
            }
            crate::features::shared::domain::supply_event_context::SupplyEventType::LockupExpiry
            | crate::features::shared::domain::supply_event_context::SupplyEventType::ShareUnlock => {
                BaselineType::PostLockup
            }
            _ => BaselineType::PostEvent,
        };
        context.event_date.map(|date| (baseline, date))
    });
    let event_baseline = input.event_baseline.or(supply_baseline);
    if valid_days < 5 {
        let (baseline, date) = event_baseline
            .map(|(baseline, date)| (baseline, Some(date)))
            .unwrap_or((BaselineType::Unavailable, None));
        return (EligibilityStatus::Insufficient, baseline, None, date);
    }
    if let Some((baseline, date)) = event_baseline {
        let event_valid_days = input
            .bars
            .iter()
            .filter(|bar| bar.date >= date && is_valid_ohlcv(bar))
            .count();
        if event_valid_days < 2 && valid_days >= 5 {
            return (eligibility, BaselineType::AvailableHistory, None, None);
        }
        if eligibility == EligibilityStatus::Full {
            return (eligibility, BaselineType::Standard20d, Some(baseline), None);
        }
        let secondary = secondary_supply_baseline
            .map(|(secondary, _)| secondary)
            .or((baseline != BaselineType::AvailableHistory)
                .then_some(BaselineType::AvailableHistory));
        return (eligibility, baseline, secondary, Some(date));
    }
    (
        eligibility,
        if eligibility == EligibilityStatus::Full {
            BaselineType::Standard20d
        } else {
            BaselineType::AvailableHistory
        },
        None,
        None,
    )
}

fn is_valid_ohlcv(bar: &DailyBar) -> bool {
    bar.volume.is_some_and(|volume| volume > 0.0)
        && bar.open.is_some()
        && bar.high.is_some()
        && bar.low.is_some()
}

fn unavailable_reason(
    input: &PriceVolumeInput<'_>,
    eligibility: EligibilityStatus,
) -> Option<UnavailableReason> {
    if input.source_rate_limited {
        Some(UnavailableReason::ApiFailure)
    } else if !input.volume_comparable {
        Some(UnavailableReason::CorporateActionConflict)
    } else if !continuous_dates(input.bars) {
        Some(UnavailableReason::DataGap)
    } else if input.bars.iter().all(|bar| bar.volume.is_none()) {
        Some(UnavailableReason::MissingVolume)
    } else if eligibility == EligibilityStatus::Insufficient {
        Some(UnavailableReason::InsufficientValidHistory)
    } else {
        None
    }
}

fn next_eligibility_condition(
    eligibility: EligibilityStatus,
    reason: Option<UnavailableReason>,
) -> Option<String> {
    if reason == Some(UnavailableReason::MissingSupplyContext) {
        return Some("Need Supply Event Context to evaluate absorption.".to_string());
    }
    match (eligibility, reason) {
        (EligibilityStatus::Insufficient, _) => {
            Some("Need 2 additional valid OHLCV sessions for PARTIAL observation.".to_string())
        }
        (EligibilityStatus::Unavailable, Some(UnavailableReason::ApiFailure)) => {
            Some("Need a successful provider response or a valid cached history.".to_string())
        }
        (EligibilityStatus::Unavailable, Some(UnavailableReason::MissingVolume)) => {
            Some("Need one valid volume observation.".to_string())
        }
        (EligibilityStatus::Unavailable, Some(UnavailableReason::DataGap)) => {
            Some("Need a continuous comparable session sequence.".to_string())
        }
        (EligibilityStatus::Partial, _) => Some(
            "Need FULL history or event-specific minimum evidence before confirmation.".to_string(),
        ),
        _ => None,
    }
}

fn lifecycle(
    eligibility: EligibilityStatus,
    persistence_days: u8,
    structure: PriceVolumeStructure,
) -> CandidateLifecycle {
    if structure == PriceVolumeStructure::Unavailable {
        return CandidateLifecycle::Invalidated;
    }
    match (eligibility, persistence_days) {
        (EligibilityStatus::Full, 0..=1) => CandidateLifecycle::Candidate,
        (EligibilityStatus::Full, 2) => CandidateLifecycle::Developing,
        (EligibilityStatus::Full, _) => CandidateLifecycle::Confirmed,
        (EligibilityStatus::Partial, 0..=1) => CandidateLifecycle::Candidate,
        (EligibilityStatus::Partial, _) => CandidateLifecycle::Developing,
        _ => CandidateLifecycle::Invalidated,
    }
}

fn continuous_dates(bars: &[DailyBar]) -> bool {
    bars.windows(2).all(|pair| {
        let previous = pair[0].date;
        let current = pair[1].date;
        let elapsed_days = (current - previous).num_days();
        elapsed_days == 1
            || (previous.weekday() == chrono::Weekday::Fri
                && current.weekday() == chrono::Weekday::Mon
                && elapsed_days == 3)
    })
}

fn metrics(
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
    let prior = &baseline_bars[..baseline_bars.len().saturating_sub(1)];
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::shared::domain::supply_event_context::{
        SupplyEventConfidence, SupplyEventFact, SupplyEventType,
    };
    use chrono::{Duration, NaiveDate};

    fn bars(closes: Vec<f64>, volumes: Vec<Option<f64>>) -> Vec<DailyBar> {
        closes
            .into_iter()
            .zip(volumes)
            .enumerate()
            .map(|(index, (close, volume))| DailyBar {
                date: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap() + Duration::days(index as i64),
                open: Some(close - 0.5),
                high: Some(close + 1.0),
                low: Some(close - 1.0),
                close,
                volume,
            })
            .collect()
    }

    fn bars_start(closes: Vec<f64>, volumes: Vec<Option<f64>>, start: NaiveDate) -> Vec<DailyBar> {
        let mut data = bars(closes, volumes);
        for (index, bar) in data.iter_mut().enumerate() {
            bar.date = start + Duration::days(index as i64);
        }
        data
    }

    fn input<'a>(
        bars: &'a [DailyBar],
        supply_context: Option<&'a SupplyEventContext>,
        overheated: bool,
    ) -> PriceVolumeInput<'a> {
        PriceVolumeInput {
            bars,
            supply_context,
            overheated,
            time_cost_rising: overheated,
            persistence_days: 3,
            source_rate_limited: false,
            volume_comparable: true,
            event_baseline: None,
            secondary_supply_context: None,
        }
    }

    fn lockup_context(event_type: SupplyEventType) -> SupplyEventContext {
        SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "SPCX".to_string(),
            event_type,
            event_date: Some(NaiveDate::from_ymd_opt(2026, 8, 6).unwrap()),
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::High,
        })
    }

    fn rising_data(current_volume: f64) -> Vec<DailyBar> {
        bars(
            (0..26).map(|index| 100.0 + index as f64).collect(),
            (0..25)
                .map(|_| Some(100.0))
                .chain(std::iter::once(Some(current_volume)))
                .collect(),
        )
    }

    fn falling_data(current_volume: f64) -> Vec<DailyBar> {
        let mut data = bars(
            (0..26).map(|index| 130.0 - index as f64).collect(),
            (0..25)
                .map(|_| Some(100.0))
                .chain(std::iter::once(Some(current_volume)))
                .collect(),
        );
        for bar in &mut data {
            bar.open = Some(bar.close + 0.5);
        }
        data
    }

    #[test]
    fn spacex_style_supply_absorption_is_accumulation_without_trade_signal() {
        let mut closes = vec![100.0; 20];
        closes.extend([99.0, 99.2, 99.5, 99.7, 100.0, 100.5]);
        let mut volumes = vec![Some(100.0); 25];
        volumes.push(Some(190.0));
        let data = bars(closes, volumes);
        let context = SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "SPCX".to_string(),
            event_type: SupplyEventType::LockupExpiry,
            event_date: Some(NaiveDate::from_ymd_opt(2026, 8, 6).unwrap()),
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::High,
        });
        let assessment = assess_price_volume_structure(input(&data, Some(&context), false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Accumulation);
        assert_eq!(assessment.supply_absorption, SupplyAbsorption::Active);
        assert_eq!(assessment.boundary.decision_weight_percent, 0);
        assert!(!assessment.boundary.trade_signal);
        assert_eq!(assessment.boundary.gate_effect, ObservationEffect::None);
        assert_eq!(
            assessment.boundary.execution_effect,
            ObservationEffect::None
        );
        assert_eq!(
            assessment.boundary.position_sizing_effect,
            ObservationEffect::None
        );
    }

    #[test]
    fn microsoft_style_high_price_low_participation_is_exhausted_advance_not_sell() {
        let mut closes = (0..21)
            .map(|index| 100.0 + index as f64)
            .collect::<Vec<_>>();
        closes.extend([122.0, 123.0, 124.0, 125.0, 126.0]);
        let mut volumes = vec![Some(150.0); 21];
        volumes.extend([Some(90.0), Some(80.0), Some(70.0), Some(60.0), Some(50.0)]);
        let data = bars(closes, volumes);
        let assessment = assess_price_volume_structure(input(&data, None, true));
        assert_eq!(assessment.structure, PriceVolumeStructure::ExhaustedAdvance);
        assert_eq!(assessment.participation, ParticipationQuality::Weakening);
        assert!(!assessment.boundary.trade_signal);
    }

    #[test]
    fn low_volume_advance_with_time_cost_needs_a_stalling_candle() {
        let mut data = rising_data(60.0);
        let current = data.last_mut().unwrap();
        current.open = Some(current.close - 0.9);
        current.high = Some(current.close + 0.05);
        current.low = Some(current.open.unwrap() - 0.05);

        let assessment = assess_price_volume_structure(PriceVolumeInput {
            overheated: false,
            time_cost_rising: true,
            ..input(&data, None, false)
        });

        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn atr_normalized_move_uses_recent_true_range_average_not_current_body_range() {
        let mut data = rising_data(150.0);
        let prior = data.len() - 2;
        data[prior].high = Some(data[prior].close + 8.0);
        data[prior].low = Some(data[prior].close - 8.0);

        let metrics = assess_price_volume_structure(input(&data, None, false))
            .metrics
            .unwrap();

        assert!(metrics.atr_normalized_move.unwrap() < metrics.body_ratio.unwrap());
    }

    #[test]
    fn one_day_volume_noise_without_context_is_neutral() {
        let mut closes = vec![100.0; 25];
        closes.push(100.1);
        let mut volumes = vec![Some(100.0); 25];
        volumes.push(Some(300.0));
        let assessment = assess_price_volume_structure(input(&bars(closes, volumes), None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn short_history_is_partial_but_missing_volume_and_rate_limit_are_unavailable() {
        let short = bars(vec![100.0; 10], vec![Some(100.0); 10]);
        assert_eq!(
            assess_price_volume_structure(input(&short, None, false)).eligibility,
            EligibilityStatus::Partial
        );
        let missing = bars(vec![100.0; 26], vec![None; 26]);
        assert_eq!(
            assess_price_volume_structure(input(&missing, None, false)).quality,
            VolumeDataQuality::Unavailable
        );
        let complete = bars(vec![100.0; 26], vec![Some(100.0); 26]);
        let rate_limited = PriceVolumeInput {
            source_rate_limited: true,
            ..input(&complete, None, false)
        };
        assert_eq!(
            assess_price_volume_structure(rate_limited).quality,
            VolumeDataQuality::Degraded
        );
        let rate_limited = PriceVolumeInput {
            source_rate_limited: true,
            ..input(&complete, None, false)
        };
        let assessment = assess_price_volume_structure(rate_limited);
        assert_eq!(assessment.eligibility, EligibilityStatus::Unavailable);
        assert_eq!(
            assessment.unavailable_reason,
            Some(UnavailableReason::ApiFailure)
        );
    }

    #[test]
    fn volume_expansion_with_rising_price_is_healthy_advance() {
        let assessment = assess_price_volume_structure(input(&rising_data(150.0), None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::HealthyAdvance);
        assert_eq!(assessment.participation, ParticipationQuality::Healthy);
    }

    #[test]
    fn volume_expansion_with_falling_price_is_distribution() {
        let assessment = assess_price_volume_structure(input(&falling_data(180.0), None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Distribution);
        assert_eq!(
            assessment.participation,
            ParticipationQuality::Deteriorating
        );
    }

    #[test]
    fn high_volume_sideways_price_is_neutral() {
        let assessment = assess_price_volume_structure(input(
            &bars(vec![100.0; 26], vec![Some(180.0); 26]),
            None,
            false,
        ));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn low_volume_rise_without_overheated_context_is_neutral() {
        let assessment = assess_price_volume_structure(input(&rising_data(60.0), None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn low_volume_decline_is_neutral_without_persistent_selling_pressure() {
        let assessment = assess_price_volume_structure(input(&falling_data(60.0), None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn high_price_high_volume_stall_does_not_force_distribution() {
        let mut data = bars(
            (0..26)
                .map(|index| {
                    if index < 21 {
                        100.0 + index as f64
                    } else {
                        120.0
                    }
                })
                .collect(),
            vec![Some(160.0); 26],
        );
        data.last_mut().unwrap().open = Some(120.0);
        let assessment = assess_price_volume_structure(input(&data, None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn earnings_day_volume_noise_without_supply_context_is_neutral() {
        let mut data = bars(vec![100.0; 26], vec![Some(100.0); 26]);
        data.last_mut().unwrap().volume = Some(400.0);
        let assessment = assess_price_volume_structure(input(&data, None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn secondary_offering_with_limited_downside_can_be_accumulation() {
        let mut closes = vec![100.0; 20];
        closes.extend([99.0, 99.2, 99.5, 99.7, 100.0, 100.5]);
        let mut volumes = vec![Some(100.0); 25];
        volumes.push(Some(190.0));
        let context = lockup_context(SupplyEventType::SecondaryOffering);
        let assessment =
            assess_price_volume_structure(input(&bars(closes, volumes), Some(&context), false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Accumulation);
        assert_eq!(assessment.supply_absorption, SupplyAbsorption::Active);
    }

    #[test]
    fn gap_up_with_low_volume_is_not_automatically_healthy_advance() {
        let mut data = rising_data(60.0);
        let previous_close = data[data.len() - 2].close;
        let current = data.last_mut().unwrap();
        current.open = Some(previous_close * 1.05);
        current.high = Some(current.close + 1.0);
        current.low = Some(current.open.unwrap() - 1.0);
        let assessment = assess_price_volume_structure(input(&data, None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
        assert!(assessment.metrics.unwrap().gap_percent.unwrap() > 0.0);
    }

    #[test]
    fn gap_down_with_high_volume_and_new_low_is_distribution() {
        let mut data = falling_data(180.0);
        let previous_close = data[data.len() - 2].close;
        let current = data.last_mut().unwrap();
        current.open = Some(previous_close * 0.95);
        current.high = Some(current.open.unwrap() + 1.0);
        current.low = Some(current.close - 1.0);
        let assessment = assess_price_volume_structure(input(&data, None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Distribution);
        assert!(assessment.metrics.unwrap().gap_percent.unwrap() < 0.0);
    }

    #[test]
    fn one_day_anomaly_followed_by_normal_volume_is_neutral() {
        let mut data = bars(vec![100.0; 26], vec![Some(100.0); 26]);
        let prior_index = data.len() - 2;
        data[prior_index].volume = Some(400.0);
        let assessment = assess_price_volume_structure(input(&data, None, false));
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
    }

    #[test]
    fn partial_volume_history_is_reported_without_forced_classification() {
        let mut volumes = vec![Some(100.0); 26];
        volumes[3] = None;
        let assessment =
            assess_price_volume_structure(input(&bars(vec![100.0; 26], volumes), None, false));
        assert_eq!(assessment.quality, VolumeDataQuality::Degraded);
        assert_ne!(assessment.structure, PriceVolumeStructure::Unavailable);
    }

    #[test]
    fn explicit_corporate_action_makes_volume_structure_unavailable() {
        let data = rising_data(150.0);
        let assessment = assess_price_volume_structure(PriceVolumeInput {
            volume_comparable: false,
            ..input(&data, None, false)
        });

        assert_eq!(assessment.quality, VolumeDataQuality::Degraded);
        assert_eq!(assessment.structure, PriceVolumeStructure::Unavailable);
        assert_eq!(assessment.participation, ParticipationQuality::Unavailable);
    }

    #[test]
    fn missing_weekday_in_history_degrades_volume_quality() {
        let mut data = rising_data(150.0);
        let missing_index = data.len() - 4;
        data.remove(missing_index);

        let assessment = assess_price_volume_structure(input(&data, None, false));

        assert_eq!(assessment.quality, VolumeDataQuality::Degraded);
        assert_eq!(assessment.structure, PriceVolumeStructure::Unavailable);
    }

    #[test]
    fn persistence_transitions_from_candidate_to_developing_to_confirmed() {
        let data = rising_data(150.0);
        for (days, expected) in [
            (1, StructurePersistence::Candidate),
            (2, StructurePersistence::Developing),
            (3, StructurePersistence::Confirmed),
        ] {
            let assessment = assess_price_volume_structure(PriceVolumeInput {
                persistence_days: days,
                ..input(&data, None, false)
            });
            assert_eq!(assessment.persistence, expected);
        }
    }

    #[test]
    fn twenty_sessions_are_enough_for_partial_observation() {
        assert_eq!(
            assess_price_volume_structure(input(&rising_data(150.0)[..20], None, false))
                .eligibility,
            EligibilityStatus::Full
        );
    }

    #[test]
    fn seven_post_ipo_sessions_are_partial_and_use_post_ipo_baseline() {
        let data = bars(
            vec![100.0, 100.5, 101.0, 100.8, 101.2, 101.5, 101.8],
            vec![Some(100.0); 7],
        );
        let context = SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "NEWCO".to_string(),
            event_type: SupplyEventType::Ipo,
            event_date: Some(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::High,
        });

        let assessment = assess_price_volume_structure(input(&data, Some(&context), false));

        assert_eq!(assessment.eligibility, EligibilityStatus::Partial);
        assert_eq!(assessment.primary_baseline, BaselineType::PostIpo);
        let metrics = assessment.metrics.as_ref().unwrap();
        assert_eq!(metrics.baseline_days, 6);
        assert_ne!(metrics.relative_volume_label, "RVOL_20");
        assert_ne!(assessment.lifecycle, CandidateLifecycle::Confirmed);
    }

    #[test]
    fn three_sessions_are_insufficient_with_explicit_next_condition() {
        let data = bars(vec![100.0, 101.0, 100.5], vec![Some(100.0); 3]);
        let assessment = assess_price_volume_structure(input(&data, None, false));

        assert_eq!(assessment.eligibility, EligibilityStatus::Insufficient);
        assert_eq!(assessment.primary_baseline, BaselineType::Unavailable);
        assert_eq!(
            assessment.unavailable_reason,
            Some(UnavailableReason::InsufficientValidHistory)
        );
        assert!(assessment.next_eligibility_condition.is_some());
    }

    #[test]
    fn lockup_context_provides_secondary_post_lockup_baseline_without_ticker_logic() {
        let data = bars_start(
            vec![100.0, 100.2, 100.1, 100.4, 100.6, 100.5, 100.8],
            vec![Some(100.0); 7],
            NaiveDate::from_ymd_opt(2026, 8, 6).unwrap(),
        );
        let context = lockup_context(SupplyEventType::LockupExpiry);
        let assessment = assess_price_volume_structure(input(&data, Some(&context), false));

        assert_eq!(assessment.primary_baseline, BaselineType::PostLockup);
        assert_eq!(
            assessment.secondary_baseline,
            Some(BaselineType::AvailableHistory)
        );
    }

    #[test]
    fn lockup_day_one_three_and_five_keep_event_baseline_while_eligibility_grows() {
        let context = lockup_context(SupplyEventType::LockupExpiry);
        for (sessions, expected) in [
            (1, EligibilityStatus::Insufficient),
            (3, EligibilityStatus::Insufficient),
            (5, EligibilityStatus::Partial),
        ] {
            let data = bars_start(
                vec![100.0; sessions],
                vec![Some(100.0); sessions],
                NaiveDate::from_ymd_opt(2026, 8, 6).unwrap(),
            );
            let assessment = assess_price_volume_structure(input(&data, Some(&context), false));
            assert_eq!(assessment.primary_baseline, BaselineType::PostLockup);
            assert_eq!(assessment.eligibility, expected);
        }
    }

    #[test]
    fn earnings_event_baseline_is_explicit_and_partial_cannot_confirm() {
        let data = bars(
            vec![100.0, 101.0, 102.0, 101.5, 102.5, 103.0],
            vec![Some(100.0); 6],
        );
        let assessment = assess_price_volume_structure(PriceVolumeInput {
            event_baseline: Some((
                BaselineType::PostEarnings,
                NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            )),
            ..input(&data, None, false)
        });

        assert_eq!(assessment.primary_baseline, BaselineType::PostEarnings);
        assert_eq!(assessment.eligibility, EligibilityStatus::Partial);
        assert_ne!(assessment.lifecycle, CandidateLifecycle::Confirmed);
        assert_eq!(
            assessment.metrics.unwrap().relative_volume_label,
            "RVOL_POST_EARNINGS"
        );
    }

    #[test]
    fn earnings_day_one_and_three_keep_post_earnings_baseline_without_confirmation() {
        for sessions in [1, 3] {
            let data = bars_start(
                vec![100.0; sessions],
                vec![Some(100.0); sessions],
                NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            );
            let assessment = assess_price_volume_structure(PriceVolumeInput {
                event_baseline: Some((
                    BaselineType::PostEarnings,
                    NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                )),
                ..input(&data, None, false)
            });

            assert_eq!(assessment.primary_baseline, BaselineType::PostEarnings);
            assert_eq!(assessment.eligibility, EligibilityStatus::Insufficient);
            assert_ne!(assessment.lifecycle, CandidateLifecycle::Confirmed);
        }
    }

    #[test]
    fn partial_eligibility_never_confirms_single_day_volume_noise() {
        let data = bars(
            vec![100.0, 100.1, 100.0, 100.2, 100.1, 100.3, 100.2],
            vec![
                Some(100.0),
                Some(100.0),
                Some(100.0),
                Some(100.0),
                Some(100.0),
                Some(100.0),
                Some(400.0),
            ],
        );
        let assessment = assess_price_volume_structure(PriceVolumeInput {
            persistence_days: 1,
            ..input(&data, None, false)
        });

        assert_eq!(assessment.lifecycle, CandidateLifecycle::Candidate);
        assert_ne!(assessment.lifecycle, CandidateLifecycle::Confirmed);
        assert_eq!(assessment.boundary.decision_weight_percent, 0);
        assert!(!assessment.boundary.trade_signal);
    }

    #[test]
    fn recoverable_volume_gap_keeps_partial_observation_available() {
        let mut volumes = vec![Some(100.0); 7];
        volumes[2] = None;
        let assessment = assess_price_volume_structure(input(
            &bars(
                vec![100.0, 100.2, 100.1, 100.3, 100.4, 100.5, 100.6],
                volumes,
            ),
            None,
            false,
        ));

        assert_eq!(assessment.eligibility, EligibilityStatus::Partial);
        assert_ne!(assessment.structure, PriceVolumeStructure::Unavailable);
        assert_eq!(assessment.lifecycle, CandidateLifecycle::Developing);
    }

    #[test]
    fn missing_supply_context_keeps_price_volume_candidate_without_absorption_confirmation() {
        let mut data = bars(vec![100.0; 26], vec![Some(100.0); 26]);
        for (index, close) in [100.3, 99.8, 99.9, 99.8, 99.9, 100.0]
            .into_iter()
            .enumerate()
        {
            data[20 + index].close = close;
        }
        data[25].volume = Some(250.0);
        let assessment = assess_price_volume_structure(PriceVolumeInput {
            persistence_days: 2,
            ..input(&data, None, false)
        });

        assert_eq!(
            assessment.structure,
            PriceVolumeStructure::AccumulationCandidate
        );
        assert_eq!(assessment.supply_absorption, SupplyAbsorption::None);
        assert_eq!(
            assessment.unavailable_reason,
            Some(UnavailableReason::MissingSupplyContext)
        );
        assert_ne!(assessment.lifecycle, CandidateLifecycle::Confirmed);
    }

    #[test]
    fn event_baseline_requires_real_post_event_history() {
        let data = bars(vec![100.0; 7], vec![Some(100.0); 7]);
        let assessment = assess_price_volume_structure(PriceVolumeInput {
            event_baseline: Some((
                BaselineType::PostEarnings,
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
            )),
            ..input(&data, None, false)
        });

        assert_eq!(assessment.primary_baseline, BaselineType::AvailableHistory);
        assert_ne!(
            assessment.metrics.unwrap().relative_volume_label,
            "RVOL_POST_EARNINGS"
        );
    }

    #[test]
    fn fifteen_post_ipo_sessions_remain_partial_with_event_baseline() {
        let data = bars(vec![100.0; 15], vec![Some(100.0); 15]);
        let context = SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "GENERIC_NEWCO".to_string(),
            event_type: SupplyEventType::Ipo,
            event_date: Some(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::High,
        });
        let assessment = assess_price_volume_structure(input(&data, Some(&context), false));

        assert_eq!(assessment.eligibility, EligibilityStatus::Partial);
        assert_eq!(assessment.primary_baseline, BaselineType::PostIpo);
        assert_ne!(assessment.lifecycle, CandidateLifecycle::Confirmed);
    }

    #[test]
    fn full_history_can_confirm_after_three_observations_but_partial_cannot() {
        let data = rising_data(150.0);
        let full = assess_price_volume_structure(PriceVolumeInput {
            persistence_days: 3,
            ..input(&data, None, false)
        });
        let partial_data = bars(vec![100.0; 7], vec![Some(100.0); 7]);
        let partial = assess_price_volume_structure(PriceVolumeInput {
            persistence_days: 3,
            ..input(&partial_data, None, false)
        });

        assert_eq!(full.lifecycle, CandidateLifecycle::Confirmed);
        assert_eq!(partial.lifecycle, CandidateLifecycle::Developing);
    }

    #[test]
    fn mature_history_keeps_standard_baseline_primary_when_event_baseline_exists() {
        let data = rising_data(150.0);
        let assessment = assess_price_volume_structure(PriceVolumeInput {
            event_baseline: Some((
                BaselineType::PostEarnings,
                NaiveDate::from_ymd_opt(2026, 7, 20).unwrap(),
            )),
            ..input(&data, None, false)
        });

        assert_eq!(assessment.eligibility, EligibilityStatus::Full);
        assert_eq!(assessment.primary_baseline, BaselineType::Standard20d);
        assert_eq!(
            assessment.secondary_baseline,
            Some(BaselineType::PostEarnings)
        );
        assert_eq!(assessment.metrics.unwrap().relative_volume_label, "RVOL_20");
    }

    #[test]
    fn missing_volume_prevents_full_history_and_standard_rvol_label() {
        let mut volumes = vec![Some(100.0); 26];
        volumes[3] = None;
        let assessment = assess_price_volume_structure(input(
            &bars((0..26).map(|index| 100.0 + index as f64).collect(), volumes),
            None,
            false,
        ));

        assert_ne!(assessment.eligibility, EligibilityStatus::Full);
        assert_ne!(assessment.primary_baseline, BaselineType::Standard20d);
        assert_ne!(assessment.metrics.unwrap().relative_volume_label, "RVOL_20");
    }

    #[test]
    fn accumulation_candidate_requires_supply_event_context_next_condition() {
        let mut data = bars(vec![100.0; 26], vec![Some(100.0); 26]);
        for (index, close) in [100.3, 99.8, 99.9, 99.8, 99.9, 100.0]
            .into_iter()
            .enumerate()
        {
            data[20 + index].close = close;
        }
        data[25].volume = Some(250.0);
        let assessment = assess_price_volume_structure(PriceVolumeInput {
            persistence_days: 2,
            ..input(&data, None, false)
        });

        assert_eq!(
            assessment.next_eligibility_condition.as_deref(),
            Some("Need Supply Event Context to evaluate absorption.")
        );
    }
}
