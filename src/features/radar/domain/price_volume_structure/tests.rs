use super::*;
use crate::features::shared::domain::supply_event_context::{
    SupplyEventConfidence, SupplyEventFact, SupplyEventType,
};
use chrono::Datelike;
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
        market_holidays: None,
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
    let short_assessment = assess_price_volume_structure(input(&short, None, false));
    assert_eq!(short_assessment.eligibility, EligibilityStatus::Partial);
    assert!(short_assessment.metrics.is_some());
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
fn price_volume_refactor_preserves_raw_metrics() {
    let assessment = assess_price_volume_structure(input(&rising_data(300.0), None, false));
    let metrics = assessment
        .metrics
        .expect("valid OHLCV must preserve metrics");

    assert!(metrics.return_1d > 0.0);
    assert_eq!(metrics.average_volume_5, 100.0);
    assert!(metrics.relative_volume > 1.0);
}

#[test]
fn unavailable_structure_does_not_claim_confirmed_persistence() {
    let data = bars(vec![100.0; 10], vec![Some(100.0); 10]);
    let assessment = assess_price_volume_structure(PriceVolumeInput {
        source_rate_limited: true,
        persistence_days: 3,
        ..input(&data, None, false)
    });

    assert_eq!(assessment.structure, PriceVolumeStructure::Unavailable);
    assert_eq!(assessment.lifecycle, CandidateLifecycle::Unavailable);
    assert_eq!(assessment.persistence, StructurePersistence::Unavailable);
    assert_eq!(assessment.persistence_days, 3);
}

#[test]
fn missing_ohlcv_fields_have_an_explicit_unavailable_reason() {
    let mut data = bars(vec![100.0; 10], vec![Some(100.0); 10]);
    for bar in &mut data {
        bar.open = None;
        bar.high = None;
        bar.low = None;
    }

    let assessment = assess_price_volume_structure(input(&data, None, false));

    assert_eq!(assessment.structure, PriceVolumeStructure::Unavailable);
    assert_eq!(
        assessment.unavailable_reason,
        Some(UnavailableReason::MissingOhlcv)
    );
}

#[test]
fn partial_ohlcv_gap_has_an_explicit_unavailable_reason() {
    let mut data = bars(vec![100.0; 10], vec![Some(100.0); 10]);
    data[3].open = None;

    let assessment = assess_price_volume_structure(input(&data, None, false));

    assert_eq!(assessment.structure, PriceVolumeStructure::Unavailable);
    assert_eq!(
        assessment.unavailable_reason,
        Some(UnavailableReason::MissingOhlcv)
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
fn observation_confidence_tracks_eligibility_strength() {
    assert_eq!(
        observation_confidence(EligibilityStatus::Full),
        ObservationConfidence::High
    );
    assert_eq!(
        observation_confidence(EligibilityStatus::Partial),
        ObservationConfidence::Partial
    );
    assert_eq!(
        observation_confidence(EligibilityStatus::Insufficient),
        ObservationConfidence::Low
    );
    assert_eq!(
        observation_confidence(EligibilityStatus::Unavailable),
        ObservationConfidence::Unavailable
    );
}

#[test]
fn twenty_sessions_are_enough_for_partial_observation() {
    assert_eq!(
        assess_price_volume_structure(input(&rising_data(150.0)[..20], None, false)).eligibility,
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
    assert_eq!(assessment.lifecycle, CandidateLifecycle::Developing);
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
fn earnings_event_baseline_is_explicit_and_can_confirm_with_evidence() {
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
    assert_eq!(assessment.lifecycle, CandidateLifecycle::Confirmed);
    assert_eq!(
        assessment.metrics.unwrap().relative_volume_label,
        "RVOL_POST_EARNINGS"
    );
}

#[test]
fn partial_event_baseline_confirms_after_three_observations() {
    let data = bars(
        vec![100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0],
        vec![
            Some(100.0),
            Some(100.0),
            Some(100.0),
            Some(100.0),
            Some(100.0),
            Some(100.0),
            Some(180.0),
        ],
    );
    let assessment = assess_price_volume_structure(PriceVolumeInput {
        persistence_days: 3,
        event_baseline: Some((
            BaselineType::PostEarnings,
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        )),
        ..input(&data, None, false)
    });

    assert_eq!(assessment.eligibility, EligibilityStatus::Partial);
    assert_eq!(
        assessment.observation_confidence,
        ObservationConfidence::Partial
    );
    assert_eq!(assessment.primary_baseline, BaselineType::PostEarnings);
    assert_eq!(assessment.lifecycle, CandidateLifecycle::Confirmed);
    assert_eq!(assessment.boundary.decision_weight_percent, 0);
    assert!(!assessment.boundary.trade_signal);
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
    assert_eq!(assessment.lifecycle, CandidateLifecycle::Developing);
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
    assert_eq!(assessment.lifecycle, CandidateLifecycle::Developing);
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
fn mature_event_baseline_exposes_independent_secondary_metrics() {
    let data = rising_data(200.0);
    let assessment = assess_price_volume_structure(PriceVolumeInput {
        event_baseline: Some((
            BaselineType::PostEarnings,
            NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
        )),
        ..input(&data, None, false)
    });

    let primary = assessment.metrics.as_ref().unwrap();
    let secondary = assessment.secondary_metrics.as_ref().unwrap();
    assert_eq!(assessment.primary_baseline, BaselineType::Standard20d);
    assert_eq!(
        assessment.secondary_baseline,
        Some(BaselineType::PostEarnings)
    );
    assert_eq!(primary.baseline_type, BaselineType::Standard20d);
    assert_eq!(secondary.baseline_type, BaselineType::PostEarnings);
    assert_eq!(secondary.relative_volume_label, "RVOL_POST_EARNINGS");
    assert_eq!(secondary.baseline_days, 5);
}

#[test]
fn secondary_supply_context_is_used_when_primary_context_type_differs() {
    let data = bars_start(
        vec![100.0; 7],
        vec![Some(100.0); 7],
        NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
    );
    let ipo = SupplyEventContext::from_fact(SupplyEventFact {
        symbol: "NEWCO".to_string(),
        event_type: SupplyEventType::Ipo,
        event_date: Some(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
        supply_direction: SupplyDirection::Increase,
        confidence: SupplyEventConfidence::High,
    });
    let lockup = lockup_context(SupplyEventType::LockupExpiry);
    let assessment = assess_price_volume_structure(PriceVolumeInput {
        secondary_supply_context: Some(&lockup),
        ..input(&data, Some(&ipo), false)
    });

    assert_eq!(
        assessment.secondary_baseline,
        Some(BaselineType::PostLockup)
    );
    assert_eq!(
        assessment.secondary_metrics.as_ref().unwrap().baseline_type,
        BaselineType::PostLockup
    );
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

#[test]
fn declared_exchange_holiday_does_not_break_continuous_trading_sessions() {
    let holiday = NaiveDate::from_ymd_opt(2026, 6, 19).unwrap();
    let mut data = bars(vec![100.0; 20], vec![Some(100.0); 20]);
    let mut date = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    for bar in &mut data {
        while matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun)
            || date == holiday
        {
            date += Duration::days(1);
        }
        bar.date = date;
        date += Duration::days(1);
    }

    let assessment = assess_price_volume_structure(PriceVolumeInput {
        market_holidays: Some(&[holiday]),
        ..input(&data, None, false)
    });

    assert_eq!(assessment.eligibility, EligibilityStatus::Full);
    assert_ne!(
        assessment.unavailable_reason,
        Some(UnavailableReason::DataGap)
    );
}

#[test]
fn baseline_days_count_only_valid_comparison_samples() {
    let mut data = bars(vec![100.0; 8], vec![Some(100.0); 8]);
    data[2].open = None;

    let assessment = assess_price_volume_structure(input(&data, None, false));

    assert_eq!(assessment.metrics.unwrap().baseline_days, 6);
}

#[test]
fn unavailable_supply_context_remains_candidate_without_absorption_confirmation() {
    let mut data = bars(vec![100.0; 26], vec![Some(100.0); 26]);
    for (index, close) in [100.3, 99.8, 99.9, 99.8, 99.9, 100.0]
        .into_iter()
        .enumerate()
    {
        data[20 + index].close = close;
    }
    data[25].volume = Some(250.0);
    let context = SupplyEventContext::unavailable("SPCX".to_string());

    let assessment = assess_price_volume_structure(PriceVolumeInput {
        persistence_days: 2,
        ..input(&data, Some(&context), false)
    });

    assert_eq!(
        assessment.structure,
        PriceVolumeStructure::AccumulationCandidate
    );
    assert_eq!(assessment.supply_absorption, SupplyAbsorption::None);
    assert_eq!(
        assessment.next_eligibility_condition.as_deref(),
        Some("Need Supply Event Context to evaluate absorption.")
    );
}

#[test]
fn short_squeeze_is_observation_only() {
    let assessment = assess_price_volume_structure(input(&rising_data(500.0), None, false));
    assert!(!assessment.boundary.trade_signal);
}

#[test]
fn meme_spike_is_observation_only() {
    let mut data = rising_data(400.0);
    data.last_mut().unwrap().close += 15.0;
    let assessment = assess_price_volume_structure(input(&data, None, true));
    assert!(!assessment.boundary.trade_signal);
}

#[test]
fn options_expiry_volume_spike_is_observation_only() {
    let mut data = bars(vec![100.0; 26], vec![Some(100.0); 26]);
    data.last_mut().unwrap().volume = Some(500.0);
    let assessment = assess_price_volume_structure(input(&data, None, false));
    assert!(!assessment.boundary.trade_signal);
}

#[test]
fn index_rebalance_volume_expansion_is_observation_only() {
    let assessment = assess_price_volume_structure(input(&rising_data(300.0), None, false));
    assert!(!assessment.boundary.trade_signal);
}

#[test]
fn two_day_repair_failure_is_observation_only() {
    let assessment = assess_price_volume_structure(PriceVolumeInput {
        persistence_days: 2,
        ..input(&falling_data(300.0), None, false)
    });
    assert!(!assessment.boundary.trade_signal);
}
