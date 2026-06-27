use super::expectation_report::build_expectation_layer_report;
use super::expectation_report_builder::build_expectation_layer_snapshot;
use crate::features::research::domain::expectation::ExpectationObservation;
use crate::features::shared::interface::i18n::Language;

#[test]
fn report_renders_tsla_fixture_without_trade_signal_keywords() {
    let report = build_expectation_layer_report(Language::JaJp);

    assert!(report.contains("TSLA / 2026Q2 / DELIVERY_CONSENSUS"));
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
    assert!(summary["snapshot"]["observations"].is_array());
}
