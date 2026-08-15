use super::*;
use crate::features::backtest::application::model::{
    BacktestAssetState, BacktestBreakoutStatus, BacktestDecisionClass, ValidationDecisionRecord,
};
use crate::features::backtest::application::model::ValidationStatus;
use chrono::NaiveDate;
use std::collections::HashMap;

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
        let population = &report.cohorts[0].population;
        assert_eq!(population.classified_record_count, 3);
        assert_eq!(population.gate_blocked_record_count, 2);
        assert_eq!(population.raw_candidate_record_count, 3);
        assert_eq!(population.raw_candidate_gate_blocked_record_count, 2);
        assert_eq!(population.raw_candidate_gate_blocked_no_trade_record_count, 2);
        assert_eq!(population.gate_blocked_non_candidate_record_count, 0);
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
        assert_eq!(cohort.population.classified_record_count, 3);
        assert_eq!(cohort.population.gate_blocked_record_count, 3);
        assert_eq!(cohort.population.raw_candidate_record_count, 2);
        assert_eq!(cohort.population.raw_candidate_gate_blocked_record_count, 2);
        assert_eq!(cohort.population.raw_candidate_gate_blocked_no_trade_record_count, 2);
        assert_eq!(cohort.population.gate_blocked_non_candidate_record_count, 1);
        assert_eq!(
            cohort.population.gate_blocked_non_candidate_reasons[0].reason,
            "TREND_GATE_BLOCKED"
        );
        assert_eq!(cohort.population.gate_blocked_non_candidate_reasons[0].count, 1);
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
