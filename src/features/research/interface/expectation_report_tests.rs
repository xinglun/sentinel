use super::expectation_report::build_expectation_layer_report;
use super::expectation_report_builder::build_expectation_layer_snapshot;
use crate::features::research::domain::expectation::{
    ExpectationLifecycleState, ExpectationObservation, ExpectationResult,
};
use crate::features::shared::interface::i18n::Language;
use chrono::NaiveDate;

#[test]
fn report_renders_tsla_fixture_without_trade_signal_keywords() {
    let report = build_expectation_layer_report(Language::JaJp);

    assert!(report.contains("TSLA / 2026Q2 / DELIVERY_CONSENSUS"));
    assert!(report.contains("Lifecycle Stage: PENDING"));
    assert!(report.contains("Expected: ~401k deliveries"));
    assert!(report.contains("Actual: 未発表"));
    assert!(report.contains("Surprise: NOT_RELEASED"));
    assert!(report.contains("Revision: UP"));
    assert!(report.contains("Expectation Pressure: HIGH"));
    assert!(report.contains("Confidence: 82%"));
    assert!(report.contains("Source Health: SUCCEEDED"));
    assert!(report.contains("Expectation Layer は売買判断ではなく"));
    assert!(report.contains("境界: Expectation Layer"));
    assert!(!report.contains("BUY"));
    assert!(!report.contains("SELL"));
    assert!(!report.contains("decision packet action"));
}

#[test]
fn report_renders_expectation_lifecycle_progression() {
    let as_of_date = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let released_at = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
    let archived_at = NaiveDate::from_ymd_opt(2026, 6, 25).unwrap();
    let snapshot = super::expectation_report_builder::ExpectationLayerSnapshot {
        as_of_date,
        decision_weight_percent: 0,
        trade_signal: false,
        gate_effect: "none".to_string(),
        execution_effect: "none".to_string(),
        position_sizing_effect: "none".to_string(),
        observations: vec![
            ExpectationObservation {
                subject: "TSLA".to_string(),
                period: "2026Q3".to_string(),
                as_of_date,
                event_type: crate::features::research::domain::expectation::ExpectationEventType::DeliveryConsensus,
                lifecycle_state: ExpectationLifecycleState::Upcoming,
                expected_value: "~405k deliveries".to_string(),
                actual_value: "未発表".to_string(),
                result: None,
                surprise_percent: None,
                market_reaction: None,
                released_at: None,
                archived_at: None,
                unit: "deliveries".to_string(),
                consensus_source: "fixture".to_string(),
                estimate_count: 0,
                estimate_high: None,
                estimate_low: None,
                estimate_median: None,
                estimate_average: None,
                revision_direction: crate::features::research::domain::expectation::RevisionDirection::Unknown,
                surprise_state: crate::features::research::domain::expectation::SurpriseState::NotReleased,
                expectation_pressure: crate::features::research::domain::expectation::ExpectationPressure::Low,
                confidence: None,
                source_health: crate::features::research::domain::expectation::SourceHealth::Unavailable,
                interpretation: "Upcoming".to_string(),
                observed_at: as_of_date,
            },
            ExpectationObservation {
                subject: "TSLA".to_string(),
                period: "2026Q2".to_string(),
                as_of_date,
                event_type: crate::features::research::domain::expectation::ExpectationEventType::DeliveryConsensus,
                lifecycle_state: ExpectationLifecycleState::Pending,
                expected_value: "~401k deliveries".to_string(),
                actual_value: "未発表".to_string(),
                result: None,
                surprise_percent: None,
                market_reaction: None,
                released_at: None,
                archived_at: None,
                unit: "deliveries".to_string(),
                consensus_source: "fixture".to_string(),
                estimate_count: 0,
                estimate_high: None,
                estimate_low: None,
                estimate_median: None,
                estimate_average: None,
                revision_direction: crate::features::research::domain::expectation::RevisionDirection::Unknown,
                surprise_state: crate::features::research::domain::expectation::SurpriseState::NotReleased,
                expectation_pressure: crate::features::research::domain::expectation::ExpectationPressure::Low,
                confidence: None,
                source_health: crate::features::research::domain::expectation::SourceHealth::Unavailable,
                interpretation: "Pending".to_string(),
                observed_at: as_of_date,
            },
            ExpectationObservation {
                subject: "TSLA".to_string(),
                period: "2026Q1".to_string(),
                as_of_date: released_at,
                event_type: crate::features::research::domain::expectation::ExpectationEventType::DeliveryConsensus,
                lifecycle_state: ExpectationLifecycleState::Released,
                expected_value: "~390k deliveries".to_string(),
                actual_value: "392k deliveries".to_string(),
                result: Some(ExpectationResult::Beat),
                surprise_percent: Some(0.5),
                market_reaction: Some("Market rallied on the surprise".to_string()),
                released_at: Some(released_at),
                archived_at: None,
                unit: "deliveries".to_string(),
                consensus_source: "fixture".to_string(),
                estimate_count: 11,
                estimate_high: Some("398k".to_string()),
                estimate_low: Some("384k".to_string()),
                estimate_median: Some("390k".to_string()),
                estimate_average: Some("390k".to_string()),
                revision_direction: crate::features::research::domain::expectation::RevisionDirection::Stable,
                surprise_state: crate::features::research::domain::expectation::SurpriseState::Above,
                expectation_pressure: crate::features::research::domain::expectation::ExpectationPressure::High,
                confidence: Some(0.82),
                source_health: crate::features::research::domain::expectation::SourceHealth::Succeeded,
                interpretation: "Released".to_string(),
                observed_at: released_at,
            },
            ExpectationObservation {
                subject: "TSLA".to_string(),
                period: "2026Q1".to_string(),
                as_of_date: archived_at,
                event_type: crate::features::research::domain::expectation::ExpectationEventType::DeliveryConsensus,
                lifecycle_state: ExpectationLifecycleState::Archived,
                expected_value: "~390k deliveries".to_string(),
                actual_value: "392k deliveries".to_string(),
                result: Some(ExpectationResult::Beat),
                surprise_percent: Some(0.5),
                market_reaction: Some("Market rallied on the surprise".to_string()),
                released_at: Some(released_at),
                archived_at: Some(archived_at),
                unit: "deliveries".to_string(),
                consensus_source: "fixture".to_string(),
                estimate_count: 11,
                estimate_high: Some("398k".to_string()),
                estimate_low: Some("384k".to_string()),
                estimate_median: Some("390k".to_string()),
                estimate_average: Some("390k".to_string()),
                revision_direction: crate::features::research::domain::expectation::RevisionDirection::Stable,
                surprise_state: crate::features::research::domain::expectation::SurpriseState::Above,
                expectation_pressure: crate::features::research::domain::expectation::ExpectationPressure::High,
                confidence: Some(0.82),
                source_health: crate::features::research::domain::expectation::SourceHealth::Succeeded,
                interpretation: "Archived".to_string(),
                observed_at: archived_at,
            },
        ],
    };

    let report = super::expectation_report::build_expectation_layer_report_from_snapshot(
        &snapshot,
        Language::EnUs,
    );

    assert!(report.contains("Lifecycle Stage: UPCOMING"));
    assert!(report.contains("Lifecycle Stage: PENDING"));
    assert!(report.contains("Lifecycle Stage: RELEASED"));
    assert!(report.contains("Lifecycle Stage: ARCHIVED"));
    assert!(report.contains("Result: BEAT"));
    assert!(report.contains("Surprise Percent: 0.50%"));
    assert!(report.contains("Market Reaction: Market rallied on the surprise"));
    assert!(report.contains("Released At: 2026-06-18"));
    assert!(report.contains("Archived At: 2026-06-25"));
}

#[test]
fn snapshot_contains_fixture_sample_for_tsla_delivery_consensus() {
    let snapshot = build_expectation_layer_snapshot();
    let fixture: ExpectationObservation = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/expectation/tsla_q2_delivery_consensus.json"
    ))
    .expect("fixture should deserialize");

    assert_eq!(snapshot.decision_weight_percent, 0);
    assert!(!snapshot.trade_signal);
    assert_eq!(snapshot.gate_effect, "none");
    assert_eq!(snapshot.execution_effect, "none");
    assert_eq!(snapshot.position_sizing_effect, "none");
    assert_eq!(snapshot.observations.first(), Some(&fixture));
}

#[test]
fn weekly_summary_exposes_expectation_snapshot_for_latest_context() {
    let summary = super::expectation_report::build_expectation_layer_weekly_summary();

    assert_eq!(summary["configured"], true);
    assert_eq!(summary["decision_weight_percent"], 0);
    assert_eq!(summary["trade_signal"], false);
    assert_eq!(summary["gate_effect"], "none");
    assert_eq!(summary["execution_effect"], "none");
    assert_eq!(summary["position_sizing_effect"], "none");
    assert_eq!(summary["observation_count"], 16);
    assert_eq!(summary["subjects"].as_array().map(Vec::len), Some(6));
    assert_eq!(summary["lifecycle_state_counts"]["PENDING"], 16);
    assert_eq!(summary["lifecycle_state_counts"]["UPCOMING"], 0);
    assert!(summary["snapshot"]["observations"].is_array());
}
