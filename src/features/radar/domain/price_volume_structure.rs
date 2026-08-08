#![allow(dead_code)]

use crate::features::shared::domain::market_data::DailyBar;
use crate::features::shared::domain::supply_event_context::{
    ObservationEffect, SupplyDirection, SupplyEventContext, SupplyEventContextAvailability,
};
use chrono::Datelike;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PriceVolumeStructure {
    Accumulation,
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
    None,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum StructurePersistence {
    Candidate,
    Developing,
    Confirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
}

pub(crate) struct PriceVolumeInput<'a> {
    pub bars: &'a [DailyBar],
    pub supply_context: Option<&'a SupplyEventContext>,
    pub overheated: bool,
    pub time_cost_rising: bool,
    pub persistence_days: u8,
    pub source_rate_limited: bool,
    pub volume_comparable: bool,
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
    let Some(metrics) = metrics(input.bars) else {
        return PriceVolumeAssessment {
            structure: PriceVolumeStructure::Unavailable,
            participation: ParticipationQuality::Unavailable,
            supply_absorption: SupplyAbsorption::Unavailable,
            quality,
            persistence,
            persistence_days,
            metrics: None,
            boundary,
        };
    };
    if quality == VolumeDataQuality::Unavailable || quality == VolumeDataQuality::Degraded {
        return PriceVolumeAssessment {
            structure: PriceVolumeStructure::Unavailable,
            participation: ParticipationQuality::Unavailable,
            supply_absorption: SupplyAbsorption::Unavailable,
            quality,
            persistence,
            persistence_days,
            metrics: Some(metrics),
            boundary,
        };
    }
    let supply_increase = input.supply_context.is_some_and(|context| {
        context.availability == SupplyEventContextAvailability::Available
            && context.supply_direction == SupplyDirection::Increase
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
    let accumulation =
        supply_increase && metrics.rvol_20 >= 1.3 && limited_downside && !metrics.new_low;
    let exhausted = metrics.return_5d > 2.0
        && high_price_position
        && metrics.rvol_20 < 1.0
        && metrics.rvol_5 < 1.0
        && stalled_candle
        && (input.overheated || input.time_cost_rising);
    let healthy = metrics.return_5d > 2.0
        && high_price_position
        && metrics.rvol_20 >= 1.0
        && metrics.up_day_average_volume > metrics.down_day_average_volume;
    let distribution = metrics.return_5d < -2.0
        && metrics.rvol_20 >= 1.3
        && metrics.down_day_average_volume > metrics.up_day_average_volume
        && downside_breakdown;
    let (structure, participation, supply_absorption) = if accumulation {
        (
            PriceVolumeStructure::Accumulation,
            ParticipationQuality::Improving,
            SupplyAbsorption::Active,
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
    if bars.len() < 20 {
        return VolumeDataQuality::Unavailable;
    }
    let present = bars
        .iter()
        .filter(|bar| bar.volume.is_some_and(|value| value > 0.0))
        .count();
    if present < 20 {
        VolumeDataQuality::Unavailable
    } else if present < bars.len() {
        VolumeDataQuality::Partial
    } else {
        VolumeDataQuality::Healthy
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

fn metrics(bars: &[DailyBar]) -> Option<PriceVolumeMetrics> {
    if bars.len() < 21 {
        return None;
    }
    let current = bars.last()?;
    let volume = current.volume?;
    if volume <= 0.0 {
        return None;
    }
    let prior = &bars[..bars.len() - 1];
    let recent20 = &prior[prior.len() - 20..];
    let recent5 = &prior[prior.len() - 5..];
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
    let change = |days: usize| (current.close / bars[bars.len() - 1 - days].close - 1.0) * 100.0;
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
    let atr_values = bars[bars.len() - 15..]
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
        (!atr_values.is_empty()).then(|| atr_values.iter().sum::<f64>() / atr_values.len() as f64);
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
        gap_percent: Some((open / bars[bars.len() - 2].close - 1.0) * 100.0),
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
    fn missing_volume_short_history_and_rate_limit_are_unavailable() {
        let short = bars(vec![100.0; 10], vec![Some(100.0); 10]);
        assert_eq!(
            assess_price_volume_structure(input(&short, None, false)).structure,
            PriceVolumeStructure::Unavailable
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
        assert_eq!(assessment.quality, VolumeDataQuality::Partial);
        assert_eq!(assessment.structure, PriceVolumeStructure::Neutral);
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
}
