use chrono::NaiveDate;
use std::collections::HashMap;
use stock_sentinel::features::radar::domain::leader_persistence::{
    build_leader_persistence, LeaderObservation,
};
use stock_sentinel::features::radar::infrastructure::persistence::{
    PersistenceLayer, TradingDaySnapshot, TradingDaySnapshotWriteDisposition,
};
use stock_sentinel::features::radar::interface::presentation::{
    CurrentRelativeStrengthItemViewModel, CurrentRelativeStrengthViewModel, PresentationPacket,
};
use stock_sentinel::features::radar::interface::report::{
    generate_refined_report, ReportRenderContext,
};
use stock_sentinel::features::shared::application::run_status::{
    DataProvenanceBundle, ReportLifecycle, ReportLifecycleMode, ReportRuntimeIdentity,
    RuntimeIntegrity, RuntimeIntegrityStatus,
};
use stock_sentinel::features::shared::interface::i18n::Language;

fn runtime_identity() -> ReportRuntimeIdentity {
    ReportRuntimeIdentity::new(
        "run-2026-09-02",
        "2026-09-02T23:30:00+09:00",
        "abc123",
        "develop",
        "0.1.0",
        "decision-v1",
        "data-2026-09-02",
        "2026-09-02",
        "abc123",
        "abc123",
    )
}

#[test]
fn runtime_integrity_is_read_only_and_detects_revision_mismatch() {
    let mut identity = runtime_identity();
    identity.execution_git_commit_sha = "def456".to_string();
    let integrity = RuntimeIntegrity::from_checks(&identity, true, true, true, true, true);

    assert_eq!(integrity.status, RuntimeIntegrityStatus::Degraded);
    assert_eq!(integrity.decision_weight, 0);
    assert!(integrity
        .diagnostics
        .iter()
        .any(|value| value == "RUNTIME_MISMATCH"));
}

#[test]
fn provenance_bundle_has_a_named_fail_closed_record_for_each_input() {
    let bundle = DataProvenanceBundle::unavailable("runtime-integrity-test", "fixture missing");
    let encoded = serde_json::to_value(bundle).unwrap();

    for key in [
        "price_history",
        "benchmark_history",
        "relative_strength_input",
        "leadership_history",
        "market_change_baseline",
        "corporate_event_evidence",
        "expectation_data",
        "price_volume_history",
    ] {
        assert_eq!(encoded[key]["status"], "UNAVAILABLE");
        assert!(encoded[key]["diagnostic"].is_string());
    }
}

#[test]
fn partial_leadership_history_is_marked_as_recomputed() {
    let observations = (0..3)
        .map(|offset| LeaderObservation {
            date: NaiveDate::from_ymd_opt(2026, 9, 1 + offset).unwrap(),
            leader: "NVDA".to_string(),
            confidence: Some(80.0),
            breadth: Some(55.0),
            relative_strength: Some(70.0),
            rotation_stability: Some(60.0),
            sector_or_index_rotation: None,
            supply_state: None,
        })
        .collect::<Vec<_>>();
    let result = build_leader_persistence(&observations).unwrap();

    assert_eq!(result.history_coverage, "PARTIAL");
    assert_eq!(result.calculation_mode, "RECOMPUTED_FROM_PARTIAL_HISTORY");
}

#[test]
fn generated_report_keeps_runtime_details_in_archive_and_uses_short_degraded_alert() {
    let presentation = PresentationPacket {
        language: Language::EnUs,
        runtime_identity: Some(runtime_identity()),
        runtime_integrity: Some(RuntimeIntegrity::from_checks(
            &runtime_identity(),
            true,
            true,
            true,
            false,
            true,
        )),
        data_provenance: Some(DataProvenanceBundle::unavailable(
            "runtime-integrity-test",
            "fixture missing",
        )),
        report_lifecycle: Some(ReportLifecycle {
            mode: ReportLifecycleMode::Generated,
            ..Default::default()
        }),
        current_relative_strength: Some(CurrentRelativeStrengthViewModel {
            title: "Current Relative Strength".to_string(),
            confirmed_leader: "none".to_string(),
            benchmark_symbol: "SPY".to_string(),
            items: vec![CurrentRelativeStrengthItemViewModel {
                symbol: "NVDA".to_string(),
                status: "UNAVAILABLE".to_string(),
                health: "UNAVAILABLE".to_string(),
                diagnostic: Some("benchmark_history_missing".to_string()),
                ..Default::default()
            }],
            boundary: "Observation only".to_string(),
        }),
        ..Default::default()
    };
    let result = generate_refined_report(
        &ReportRenderContext {
            compact_transition_in_no_trade: false,
            compact_stability_threshold: "0".to_string(),
            compact_continuity_threshold: "0".to_string(),
            observation_timeline: None,
        },
        &presentation,
        0.0,
        &HashMap::new(),
        &HashMap::new(),
    )
    .unwrap();

    assert!(result
        .markdown_body
        .contains("⚠️ Observation Integrity: Partially degraded"));
    assert!(result
        .markdown_body
        .contains("Observation only; excluded from decisions"));
    assert!(result.markdown_body.contains("run-2026-09-02"));
    assert!(!result.markdown_body.contains("report_runtime_identity"));
    assert!(!result.markdown_body.contains("data_provenance"));
    assert!(!result
        .markdown_body
        .contains("LEADERSHIP_SNAPSHOT_DEGRADED"));
    assert!(!result.markdown_body.contains("decision_weight=0"));
    assert!(result.markdown_body.contains("RS Observation Health"));
    assert!(result.markdown_body.contains("benchmark_history_missing"));
    assert!(result
        .telegram_html_body
        .contains("Observation Integrity: Partially degraded"));
    assert!(result
        .telegram_html_body
        .contains("Observation only; excluded from decisions"));
    assert!(result.telegram_html_body.contains("run-2026-09-02"));
    assert!(!result
        .telegram_html_body
        .contains("report_runtime_identity"));
    assert!(!result
        .telegram_html_body
        .contains("LEADERSHIP_SNAPSHOT_DEGRADED"));
    assert!(!result.telegram_html_body.contains("decision_weight=0"));
    assert!(result.archival_markdown.contains("report_runtime_identity"));
    assert!(result.archival_markdown.contains("Runtime Integrity"));
    assert!(result.archival_markdown.contains("data_provenance"));
    assert!(result
        .archival_markdown
        .contains("LEADERSHIP_SNAPSHOT_DEGRADED"));
    assert!(result.archival_markdown.contains("\"decision_weight\": 0"));
    assert!(result.archival_markdown.contains("UNAVAILABLE"));
    assert_eq!(presentation.runtime_integrity.unwrap().decision_weight, 0);
}

#[test]
fn healthy_report_hides_runtime_integrity_technical_lines_in_user_surfaces() {
    let presentation = PresentationPacket {
        language: Language::ZhCn,
        runtime_identity: Some(runtime_identity()),
        runtime_integrity: Some(RuntimeIntegrity::from_checks(
            &runtime_identity(),
            true,
            true,
            true,
            true,
            true,
        )),
        data_provenance: Some(DataProvenanceBundle::unavailable(
            "healthy-runtime-integrity-test",
            "provenance fixture",
        )),
        ..Default::default()
    };

    let result = generate_refined_report(
        &ReportRenderContext {
            compact_transition_in_no_trade: false,
            compact_stability_threshold: "0".to_string(),
            compact_continuity_threshold: "0".to_string(),
            observation_timeline: None,
        },
        &presentation,
        0.0,
        &HashMap::new(),
        &HashMap::new(),
    )
    .unwrap();

    for body in [&result.markdown_body, &result.telegram_html_body] {
        assert!(body.contains("run-2026-09-02"));
        assert!(!body.contains("Runtime Integrity"));
        assert!(!body.contains("report_runtime_identity"));
        assert!(!body.contains("data_provenance"));
        assert!(!body.contains("观测完整性"));
    }
    assert!(result.archival_markdown.contains("report_runtime_identity"));
    assert!(result.archival_markdown.contains("Runtime Integrity"));
    assert!(result.archival_markdown.contains("\"status\": \"HEALTHY\""));
    assert!(result
        .archival_markdown
        .contains("\"report_run_id\": \"run-2026-09-02\""));
    assert!(result
        .archival_markdown
        .contains("\"report_run_at\": \"2026-09-02T23:30:00+09:00\""));
    assert!(result
        .archival_markdown
        .contains("\"git_commit_sha\": \"abc123\""));
    assert!(result
        .archival_markdown
        .contains("\"git_branch\": \"develop\""));
    assert!(result
        .archival_markdown
        .contains("\"binary_version\": \"0.1.0\""));
    assert!(result
        .archival_markdown
        .contains("\"decision_snapshot_version\": \"decision-v1\""));
    assert!(result
        .archival_markdown
        .contains("\"data_snapshot_id\": \"data-2026-09-02\""));
    assert!(result
        .archival_markdown
        .contains("\"data_snapshot_date\": \"2026-09-02\""));
    assert!(result
        .archival_markdown
        .contains("\"build_git_commit_sha\": \"abc123\""));
    assert!(result
        .archival_markdown
        .contains("\"execution_git_commit_sha\": \"abc123\""));
    assert!(result.archival_markdown.contains("data_provenance"));
    assert!(result.archival_markdown.contains("provenance fixture"));
    assert!(result.archival_markdown.contains("\"decision_weight\": 0"));
}

#[test]
fn unavailable_report_shows_non_silent_short_alert_without_machine_diagnostics() {
    let presentation = PresentationPacket {
        language: Language::ZhCn,
        runtime_identity: Some(runtime_identity()),
        runtime_integrity: Some(RuntimeIntegrity::from_checks(
            &runtime_identity(),
            false,
            true,
            true,
            true,
            true,
        )),
        ..Default::default()
    };

    let result = generate_refined_report(
        &ReportRenderContext {
            compact_transition_in_no_trade: false,
            compact_stability_threshold: "0".to_string(),
            compact_continuity_threshold: "0".to_string(),
            observation_timeline: None,
        },
        &presentation,
        0.0,
        &HashMap::new(),
        &HashMap::new(),
    )
    .unwrap();

    for body in [&result.markdown_body, &result.telegram_html_body] {
        assert!(body.contains("run-2026-09-02"));
        assert!(body.contains("观测完整性：不可用"));
        assert!(body.contains("仅观测，不参与决策"));
        assert!(!body.contains("DATA_SNAPSHOT_UNAVAILABLE"));
        assert!(!body.contains("decision_weight=0"));
        assert!(!body.contains("report_runtime_identity"));
    }
    assert!(result
        .archival_markdown
        .contains("DATA_SNAPSHOT_UNAVAILABLE"));
    assert!(result.archival_markdown.contains("\"decision_weight\": 0"));
}

#[test]
fn runtime_integrity_alerts_are_specific_and_localized() {
    let mut mismatched_identity = runtime_identity();
    mismatched_identity.execution_git_commit_sha = "def456".to_string();
    let revision_integrity =
        RuntimeIntegrity::from_checks(&mismatched_identity, true, true, true, true, true);
    let artifact_integrity =
        RuntimeIntegrity::from_checks(&runtime_identity(), true, true, true, true, false);

    for (language, expected_detail, excluded_detail, old_generic_detail) in [
        (
            Language::ZhCn,
            "运行版本不一致",
            "仅观测，不参与决策",
            "部分观测输入不完整",
        ),
        (
            Language::EnUs,
            "Runtime revision mismatch",
            "Observation only; excluded from decisions",
            "Partial observation input incomplete",
        ),
        (
            Language::JaJp,
            "実行リビジョン不一致",
            "観測のみ・意思決定には不参加",
            "一部の観測入力が不完全です",
        ),
    ] {
        for (identity, integrity, expected_artifact_detail) in [
            (
                mismatched_identity.clone(),
                revision_integrity.clone(),
                expected_detail,
            ),
            (
                runtime_identity(),
                artifact_integrity.clone(),
                match language {
                    Language::ZhCn => "报告产物完整性需要复核",
                    Language::EnUs => "Report artifact traceability incomplete",
                    Language::JaJp => "レポート成果物の追跡性が不完全です",
                },
            ),
        ] {
            let presentation = PresentationPacket {
                language,
                runtime_identity: Some(identity),
                runtime_integrity: Some(integrity),
                ..Default::default()
            };
            let result = generate_refined_report(
                &ReportRenderContext {
                    compact_transition_in_no_trade: false,
                    compact_stability_threshold: "0".to_string(),
                    compact_continuity_threshold: "0".to_string(),
                    observation_timeline: None,
                },
                &presentation,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            for body in [&result.markdown_body, &result.telegram_html_body] {
                assert!(body.contains(expected_artifact_detail));
                assert!(body.contains(excluded_detail));
                assert!(!body.contains(old_generic_detail));
                assert!(!body.contains("RUNTIME_MISMATCH"));
                assert!(!body.contains("REPORT_ARTIFACT_MISMATCH"));
            }
            assert!(result.archival_markdown.contains(expected_artifact_detail));
        }
    }
}

#[test]
fn runtime_integrity_alert_prioritizes_traceability_diagnostics() {
    let mut mismatched_identity = runtime_identity();
    mismatched_identity.execution_git_commit_sha = "def456".to_string();
    let mut runtime_integrity =
        RuntimeIntegrity::from_checks(&mismatched_identity, true, true, true, true, true);
    runtime_integrity
        .diagnostics
        .push("LEADERSHIP_SNAPSHOT_DEGRADED".to_string());
    runtime_integrity.diagnostics.sort();

    let mut artifact_integrity =
        RuntimeIntegrity::from_checks(&runtime_identity(), true, true, true, true, false);
    artifact_integrity
        .diagnostics
        .push("RS_INPUT_DEGRADED".to_string());
    artifact_integrity.diagnostics.sort();

    let mut combined_integrity = runtime_integrity.clone();
    combined_integrity
        .diagnostics
        .push("REPORT_ARTIFACT_MISMATCH".to_string());
    combined_integrity.diagnostics.sort();

    for (identity, integrity, expected) in [
        (
            mismatched_identity,
            runtime_integrity,
            "Runtime revision mismatch",
        ),
        (
            runtime_identity(),
            artifact_integrity,
            "Report artifact traceability incomplete",
        ),
        (
            runtime_identity(),
            combined_integrity.clone(),
            "Runtime and report artifact traceability require review",
        ),
    ] {
        let presentation = PresentationPacket {
            language: Language::EnUs,
            runtime_identity: Some(identity),
            runtime_integrity: Some(integrity),
            ..Default::default()
        };
        let result = generate_refined_report(
            &ReportRenderContext {
                compact_transition_in_no_trade: false,
                compact_stability_threshold: "0".to_string(),
                compact_continuity_threshold: "0".to_string(),
                observation_timeline: None,
            },
            &presentation,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        for body in [&result.markdown_body, &result.telegram_html_body] {
            assert!(body.contains(expected));
            assert!(!body.contains("Partial observation input incomplete"));
        }
    }

    for (language, expected) in [
        (Language::ZhCn, "运行版本与报告产物追踪均需复核"),
        (
            Language::JaJp,
            "実行リビジョンとレポート成果物の追跡性を確認してください",
        ),
    ] {
        let presentation = PresentationPacket {
            language,
            runtime_identity: Some(runtime_identity()),
            runtime_integrity: Some(combined_integrity.clone()),
            ..Default::default()
        };
        let result = generate_refined_report(
            &ReportRenderContext {
                compact_transition_in_no_trade: false,
                compact_stability_threshold: "0".to_string(),
                compact_continuity_threshold: "0".to_string(),
                observation_timeline: None,
            },
            &presentation,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        for body in [&result.markdown_body, &result.telegram_html_body] {
            assert!(body.contains(expected));
        }
    }
}

#[test]
fn legacy_run_status_without_runtime_fields_remains_readable() {
    let legacy = r#"{
        "date": "2026-09-02",
        "timestamp": "2026-09-02T23:30:00+09:00",
        "preflight": null,
        "decisioning": "succeeded",
        "archival": "succeeded",
        "notification": "skipped",
        "execution": "skipped",
        "reconciliation": "skipped",
        "data_quality": "OK",
        "execution_details": null
    }"#;

    let outcome: stock_sentinel::features::shared::application::run_status::RunOutcome =
        serde_json::from_str(legacy).unwrap();
    assert!(outcome.runtime_identity.is_none());
    assert!(outcome.runtime_integrity.is_none());
}

#[test]
fn snapshot_digest_conflict_is_rejected_without_overwriting_existing_fact() {
    let temp_dir = tempfile::tempdir().unwrap();
    let layer = PersistenceLayer::new(temp_dir.path());
    let date = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
    let snapshot = TradingDaySnapshot {
        schema_version: "1".to_string(),
        market_date: date,
        report_date: date,
        as_of_date: date,
        generated_at: "2026-09-02T23:30:00+09:00".to_string(),
        run_id: "run-1".to_string(),
        cycle_id: "cycle-1".to_string(),
        snapshot_id: "cycle-1-2026-09-02".to_string(),
        is_valid_trading_day: true,
        source_status: "complete".to_string(),
        market_state: "RANGE".to_string(),
        decision_state: "NO_TRADE".to_string(),
        new_position_limit: 0.0,
        breadth: Some(50.0),
        breadth_classification: Some("Narrow".to_string()),
        confidence: 50.0,
        supply_phase: "IDLE".to_string(),
        risk_state: "NORMAL".to_string(),
        primary_leader: None,
        secondary_leaders: Vec::new(),
        breakouts: serde_json::json!({}),
        stability: 1.0,
        continuity: 1,
        cycle_length_days: 1,
        reset_event: None,
        data_quality: serde_json::json!({"history": "HEALTHY"}),
        report_run_id: Some("run-1".to_string()),
        git_commit_sha: Some("abc123".to_string()),
        data_digest: Some("sha256:data-1".to_string()),
        decision_packet_digest: Some("sha256:decision-1".to_string()),
        observation_digest: Some("sha256:observation-1".to_string()),
        runtime_integrity: Some(RuntimeIntegrity::from_checks(
            &runtime_identity(),
            true,
            true,
            true,
            true,
            true,
        )),
    };

    assert_eq!(
        layer.save_trading_day_snapshot(&snapshot).unwrap(),
        TradingDaySnapshotWriteDisposition::Created
    );
    let mut rerun = snapshot.clone();
    rerun.run_id = "run-2".to_string();
    rerun.report_run_id = Some("run-2".to_string());
    rerun.generated_at = "2026-09-02T23:31:00+09:00".to_string();
    assert_eq!(
        layer.save_trading_day_snapshot(&rerun).unwrap(),
        TradingDaySnapshotWriteDisposition::SameDayRerun
    );

    let mut conflict = snapshot.clone();
    conflict.data_digest = Some("sha256:data-2".to_string());
    let error = layer.save_trading_day_snapshot(&conflict).unwrap_err();
    assert_eq!(error.to_string(), "SNAPSHOT_CONFLICT");
    assert_eq!(
        layer
            .load_trading_day_snapshots()
            .unwrap()
            .first()
            .and_then(|value| value.data_digest.as_deref()),
        Some("sha256:data-1")
    );
}
