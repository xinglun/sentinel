// 資本吸収警告センサーのレポート生成検証テスト
use super::cognitive_reports::build_capital_absorption_report;
use crate::config;
use crate::features::research::application::capital_absorption::{
    unavailable_capital_absorption_snapshot, CapitalAbsorptionAutoConfidence,
    CapitalAbsorptionAutoEvent, CapitalAbsorptionAutoEventCategory, CapitalAbsorptionAutoRatio,
    CapitalAbsorptionAutoRatioState, CapitalAbsorptionAutoSnapshot, CapitalAbsorptionAutoStatus,
    CapitalAbsorptionAutoTrend, CapitalAbsorptionIpoLifecycleStatus,
    CapitalAbsorptionIpoQueueHistoryPoint, CapitalAbsorptionIpoQueueItem,
    CapitalAbsorptionIpoQueueStatus, CapitalAbsorptionNearTermSupplyWeight,
    CapitalAbsorptionObservationCoverageState, CapitalAbsorptionObservationEventType,
    CapitalAbsorptionObservationWatchlistItem, CapitalAbsorptionPotentialSupplyPressure,
    CapitalAbsorptionPotentialSupplyPressureLevel, CapitalAbsorptionPotentialSupplyTrend,
    CapitalAbsorptionPressureDriverStrength, CapitalAbsorptionSourceCoverage,
    CapitalAbsorptionSourceCoverageStatus, CapitalAbsorptionSourceHealth,
    CapitalAbsorptionSourceStatus, CapitalAbsorptionSupplyEventCounts, CapitalAbsorptionSupplyKind,
    CapitalAbsorptionSupplyTimelineBucket, CapitalAbsorptionSupplyTimelineItem,
};
use crate::features::shared::interface::i18n::Language;
use chrono::{Duration, NaiveDate};

#[test]
fn auto_report_locks_new_sections_in_en_and_ja() {
    for (
        language,
        title,
        actual_supply,
        potential_trend,
        potential_pressure,
        timeline,
        queue_size,
        queue_stage,
        summary,
        boundary,
        forbidden_structural_impact,
    ) in [
        (
            Language::ZhCn,
            "资本吸收早期预警传感器",
            "实际资本供给",
            "潜在供给趋势",
            "潜在供给压力",
            "Upcoming Supply Timeline",
            "队列规模 = 0",
            "Subject: SpaceX · Event Type: 确认（Confirmed） · Expected Window:",
            "- SpaceX x2",
            "不影响 READY / EXECUTE / Position Sizing / Gate / Trend Layer",
            Some("结构影响: Observation Only"),
        ),
        (
            Language::EnUs,
            "Capital Absorption Early Warning Sensor",
            "Actual Capital Supply",
            "Potential Supply Trend",
            "Potential Supply Pressure",
            "Upcoming Supply Timeline",
            "Queue Size = 0",
            "Subject: SpaceX · Event Type: Confirmed · Expected Window:",
            "- SpaceX x2",
            "does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer",
            None,
        ),
        (
            Language::JaJp,
            "資本吸収早期警戒センサー",
            "実際の資本供給",
            "潜在供給トレンド",
            "潜在供給圧力",
            "Upcoming Supply Timeline",
            "キュー規模 = 0",
            "Subject: SpaceX · Event Type: 確認（Confirmed） · Expected Window:",
            "- SpaceX x2",
            "READY / EXECUTE / Position Sizing / Gate / Trend Layer に影響しない",
            Some("構造的影響: Observation Only"),
        ),
    ] {
        let report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&auto_snapshot_with_potential_ipo()),
            language,
        );

        assert!(report.contains(title));
        assert!(report.contains(actual_supply));
        assert!(report.contains(potential_trend));
        assert!(report.contains(potential_pressure));
        assert!(report.contains("ABSORBING"));
        assert!(report.contains("Near-Term Supply Count: 1"));
        assert!(report.contains("Future Queue Count: 0"));
        assert!(report.contains("Drivers"));
        assert!(report.contains("SpaceX IPO (High)"));
        assert!(report.contains("Reported Count: 0"));
        assert!(report.contains("Confirmed Count: 1"));
        assert!(report.contains(timeline));
        assert!(report.contains("0-30 Days"));
        assert!(report.contains("SpaceX ("));
        assert!(report.contains("2026-05-07"));
        assert!(report.contains(queue_size));
        assert!(report.contains(queue_stage));
        assert!(report.contains(summary));
        assert!(report.contains(boundary));
        assert!(!report.contains("Capital Demand"));
        assert!(!report.contains("ACCELERATING"));
        if let Some(forbidden) = forbidden_structural_impact {
            assert!(!report.contains(forbidden));
        }
    }
}

#[test]
fn auto_report_keeps_anthropic_potential_out_of_actual_supply() {
    for (language, actual_label, no_actual, queue, summary, actual_event, boundary) in [
        (
            Language::ZhCn,
            "实际资本供给",
            "未观察到已发生的大型股权/可转债供给。",
            "Subject: Anthropic · Event Type: 传闻（Rumor） · Expected Window: within 1 days",
            "- Anthropic x1",
            "实际供给 · 事件类型 确认（Confirmed） · IPO 供给 · Anthropic",
            "不影响 READY / EXECUTE / Position Sizing / Gate / Trend Layer",
        ),
        (
            Language::EnUs,
            "Actual Capital Supply",
            "No completed large equity or convertible supply observed.",
            "Subject: Anthropic · Event Type: Rumor · Expected Window: within 1 days",
            "- Anthropic x1",
            "Actual Supply · Event Type Confirmed · IPO Supply · Anthropic",
            "does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer",
        ),
        (
            Language::JaJp,
            "実際の資本供給",
            "発生済みの大型株式・転換社債供給は未観測です。",
            "Subject: Anthropic · Event Type: 噂（Rumor） · Expected Window: within 1 days",
            "- Anthropic x1",
            "実供給 · イベント種別 確認（Confirmed） · IPO 供給 · Anthropic",
            "READY / EXECUTE / Position Sizing / Gate / Trend Layer に影響しない",
        ),
    ] {
        let report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&auto_snapshot_with_anthropic_potential_ipo()),
            language,
        );

        assert!(report.contains(actual_label));
        assert!(report.contains(no_actual));
        assert!(report.contains(queue));
        assert!(report.contains(summary));
        assert!(!report.contains(actual_event));
        assert!(!report.contains("$60.0B"));
        assert!(!report.contains("Anthropic IPO discussion after private valuation"));
        assert!(report.contains(boundary));
    }
}

#[test]
fn auto_report_shows_post_ipo_observation_watchlist_without_near_term_pressure() {
    let report = build_capital_absorption_report(
        &minimal_app_config(Language::EnUs),
        Some(&auto_snapshot_with_listed_spacex_observation()),
        Language::EnUs,
    );

    assert!(report.contains("Observation Watchlist"));
    assert!(report.contains("SpaceX: Status Listed · Observation Day: 1 · Review Window: 90 Days"));
    assert!(report.contains("Near-Term Supply Count: 0"));
    assert!(!report.contains("SpaceX IPO (High)"));
    assert!(!report.contains("Future IPO Queue:\n- SpaceX"));
}

#[test]
fn auto_report_limits_future_queue_to_three_items_and_explains_empty_queue() {
    let mut snapshot = auto_snapshot_with_anthropic_potential_ipo();
    snapshot.ai_ipo_queue.clear();
    let empty_report = build_capital_absorption_report(
        &minimal_app_config(Language::EnUs),
        Some(&snapshot),
        Language::EnUs,
    );
    assert!(empty_report.contains("Future IPO Queue: None"));
    assert!(!empty_report.contains("Future Queue details unavailable."));

    snapshot.ai_ipo_queue = (0..4)
        .map(|index| CapitalAbsorptionIpoQueueItem {
            issuer: format!("Issuer-{index}"),
            status: CapitalAbsorptionIpoQueueStatus::Rumor,
            source_count: 1,
            event_type: CapitalAbsorptionObservationEventType::Rumor,
            lifecycle_status: CapitalAbsorptionIpoLifecycleStatus::Rumor,
            observed_at: None,
            observation_day: None,
            near_term_weight: None,
        })
        .collect();
    let report = build_capital_absorption_report(
        &minimal_app_config(Language::EnUs),
        Some(&snapshot),
        Language::EnUs,
    );
    assert!(report.contains("Issuer-0"));
    assert!(report.contains("Issuer-2"));
    assert_eq!(report.matches("Subject: Issuer-").count(), 3);
}

#[test]
fn auto_report_preserves_partial_positive_events_and_discloses_source_gaps_in_all_languages() {
    for (language, coverage_label, succeeded_count, failed, not_attempted) in [
        (Language::ZhCn, "来源覆盖: 部分", "成功 1", "失败", "未尝试"),
        (
            Language::EnUs,
            "Source Coverage: PARTIAL",
            "1 succeeded",
            "FAILED",
            "NOT ATTEMPTED",
        ),
        (
            Language::JaJp,
            "ソース網羅性: 一部",
            "成功 1",
            "失敗",
            "未試行",
        ),
    ] {
        let mut snapshot = auto_snapshot_with_potential_ipo();
        snapshot.observation_coverage = CapitalAbsorptionObservationCoverageState::Partial;
        snapshot.source_status.status = CapitalAbsorptionSourceHealth::Partial;
        snapshot.source_coverage = vec![
            CapitalAbsorptionSourceCoverage {
                source: "company-news:GOOG".to_string(),
                status: CapitalAbsorptionSourceCoverageStatus::Succeeded,
                message: "response processed".to_string(),
            },
            CapitalAbsorptionSourceCoverage {
                source: "company-news:MSFT".to_string(),
                status: CapitalAbsorptionSourceCoverageStatus::Failed,
                message: "HTTP 429 rate limited".to_string(),
            },
            CapitalAbsorptionSourceCoverage {
                source: "market-news:general".to_string(),
                status: CapitalAbsorptionSourceCoverageStatus::NotAttempted,
                message: "not attempted after HTTP 429 rate limit".to_string(),
            },
        ];

        let report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&snapshot),
            language,
        );

        assert!(report.contains(coverage_label));
        assert!(report.contains(succeeded_count));
        assert!(report.contains(failed));
        assert!(report.contains(not_attempted));
        assert!(report.contains("company-news:MSFT"));
        assert!(report.contains("HTTP 429 rate limited"));
        assert!(report.contains("SpaceX x2"));
        assert!(report.contains("WATCH"));
        assert!(!report.contains("risk-free"));
    }
}

#[test]
fn incomplete_supply_sections_do_not_turn_missing_values_into_zero_or_stable() {
    for (
        language,
        missing_actual,
        observed_note,
        uncovered,
        no_actual_supply,
        supply_label,
        future_queue_label,
    ) in [
        (
            Language::ZhCn,
            "无法判断是否还有其他实际供给",
            "成功来源中观察到",
            "来源未覆盖，无法判断",
            "未观察到已发生的大型股权/可转债供给。",
            "资本供给趋势",
            "Future IPO 队列",
        ),
        (
            Language::EnUs,
            "additional actual supply cannot be determined",
            "Observed in successful sources",
            "Unknown; source coverage is incomplete",
            "No completed large equity or convertible supply observed.",
            "Capital Supply",
            "Future IPO Queue",
        ),
        (
            Language::JaJp,
            "実際の供給総額は判定できません",
            "成功したソースで観測",
            "ソース未網羅のため判定不能",
            "発生済みの大型株式・転換社債供給は未観測です。",
            "資本供給トレンド",
            "Future IPO キュー",
        ),
    ] {
        let mut partial = auto_snapshot_with_potential_ipo();
        partial.observation_coverage = CapitalAbsorptionObservationCoverageState::Partial;
        partial.source_status.status = CapitalAbsorptionSourceHealth::Partial;
        partial.source_coverage = vec![
            CapitalAbsorptionSourceCoverage {
                source: "company-news:GOOG".to_string(),
                status: CapitalAbsorptionSourceCoverageStatus::Succeeded,
                message: "response processed".to_string(),
            },
            CapitalAbsorptionSourceCoverage {
                source: "company-news:MSFT".to_string(),
                status: CapitalAbsorptionSourceCoverageStatus::Failed,
                message: "network request failed".to_string(),
            },
        ];
        partial.capital_demand.rolling_12m_usd_b = Some(2.5);
        partial.capital_demand.ipo_financing_usd_b = Some(2.5);

        let report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&partial),
            language,
        );

        assert!(report.contains(missing_actual), "{report}");
        assert!(report.contains(observed_note), "{report}");
        assert!(report.contains("SpaceX"), "{report}");
        assert!(report.contains(uncovered), "{report}");
        assert!(!report.contains(no_actual_supply), "{report}");
        let timeline = report
            .split("Upcoming Supply Timeline:\n")
            .nth(1)
            .expect("upcoming supply timeline")
            .split("\n\n")
            .next()
            .unwrap_or_default();
        assert!(timeline.contains("SpaceX"), "{timeline}");
        assert!(timeline.contains(uncovered), "{timeline}");
        assert!(timeline.matches(uncovered).count() >= 2, "{timeline}");
        assert!(!timeline.contains("- None"), "{timeline}");
        assert!(!timeline.contains("- 无"), "{timeline}");
        assert!(!timeline.contains("- 無"), "{timeline}");

        let supply = report
            .split(&format!("\n{supply_label}:\n"))
            .nth(1)
            .expect("capital supply section")
            .split("\n\n")
            .next()
            .unwrap_or_default();
        assert!(!supply.contains("STABLE"), "{supply}");
        assert!(supply.contains(uncovered), "{supply}");
        assert!(supply.matches(uncovered).count() >= 8, "{supply}");

        let mut partial_empty = auto_snapshot_with_potential_ipo();
        partial_empty.observation_coverage = CapitalAbsorptionObservationCoverageState::Partial;
        partial_empty.source_status.status = CapitalAbsorptionSourceHealth::Partial;
        partial_empty.source_coverage = partial.source_coverage.clone();
        partial_empty.observed_events.clear();
        partial_empty.near_term_supply.clear();
        partial_empty.ai_ipo_queue.clear();
        partial_empty.upcoming_supply_timeline.clear();
        let partial_empty_report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&partial_empty),
            language,
        );
        assert!(
            partial_empty_report.contains(missing_actual),
            "{partial_empty_report}"
        );
        assert!(
            !partial_empty_report.contains(no_actual_supply),
            "{partial_empty_report}"
        );
        let partial_empty_near_term = partial_empty_report
            .split("Near-Term Supply: ")
            .nth(1)
            .expect("partial empty near-term queue")
            .split("\n\n")
            .next()
            .unwrap_or_default();
        assert!(
            partial_empty_near_term.contains(uncovered),
            "{partial_empty_near_term}"
        );
        let partial_empty_future_queue = partial_empty_report
            .split(&format!("{future_queue_label}: "))
            .nth(1)
            .expect("partial empty future IPO queue")
            .split("\n\n")
            .next()
            .unwrap_or_default();
        assert!(
            partial_empty_future_queue.contains(uncovered),
            "{partial_empty_future_queue}"
        );
        let partial_empty_timeline = partial_empty_report
            .split("Upcoming Supply Timeline:\n")
            .nth(1)
            .expect("partial empty upcoming supply timeline")
            .split("\n\n")
            .next()
            .unwrap_or_default();
        assert_eq!(
            partial_empty_timeline.matches(uncovered).count(),
            3,
            "{partial_empty_timeline}"
        );

        let mut complete_empty_snapshot = auto_snapshot_with_potential_ipo();
        complete_empty_snapshot.observed_events.clear();
        complete_empty_snapshot.near_term_supply.clear();
        complete_empty_snapshot.ai_ipo_queue.clear();
        complete_empty_snapshot.upcoming_supply_timeline.clear();
        let complete_empty = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&complete_empty_snapshot),
            language,
        );
        assert!(
            complete_empty.contains(no_actual_supply),
            "{complete_empty}"
        );
    }
}

#[test]
fn auto_report_labels_complete_and_unavailable_coverage_in_all_languages() {
    for (
        language,
        complete,
        unavailable,
        unknown_status,
        complete_empty_events,
        complete_zero,
        no_actual_supply,
        observed_empty_near_term,
        observed_empty_future_queue,
        no_events,
    ) in [
        (
            Language::ZhCn,
            "来源覆盖: 完整",
            "来源覆盖: 不可用",
            "状态未知（来源不可用）",
            "未观察到大型资本吸收事件。",
            "Mega Cap 融资: 0",
            "未观察到已发生的大型股权/可转债供给。",
            "Near-Term Supply: 无",
            "Future IPO 队列: 无",
            "无",
        ),
        (
            Language::EnUs,
            "Source Coverage: COMPLETE",
            "Source Coverage: UNAVAILABLE",
            "UNKNOWN (SOURCES UNAVAILABLE)",
            "No large capital absorption events observed.",
            "Mega Cap Financing: 0",
            "No completed large equity or convertible supply observed.",
            "Near-Term Supply: None",
            "Future IPO Queue: None",
            "None",
        ),
        (
            Language::JaJp,
            "ソース網羅性: 完全",
            "ソース網羅性: 利用不可",
            "状態不明（ソース利用不可）",
            "大型の資本吸収イベントは未観測です。",
            "Mega Cap 調達: 0",
            "発生済みの大型株式・転換社債供給は未観測です。",
            "Near-Term Supply: なし",
            "Future IPO キュー: なし",
            "なし",
        ),
    ] {
        let complete_snapshot = auto_snapshot_with_potential_ipo();
        let unavailable_snapshot =
            unavailable_capital_absorption_snapshot("HTTP 429 rate limited".to_string());

        let complete_report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&complete_snapshot),
            language,
        );
        let unavailable_report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&unavailable_snapshot),
            language,
        );

        assert!(complete_report.contains(complete));
        assert!(unavailable_report.contains(unavailable));
        assert!(unavailable_report.contains(unknown_status));
        assert!(unavailable_report.contains("HTTP 429 rate limited"));

        let mut contradictory_snapshot = auto_snapshot_with_potential_ipo();
        contradictory_snapshot.observed_events.clear();
        contradictory_snapshot.near_term_supply.clear();
        contradictory_snapshot.ai_ipo_queue.clear();
        contradictory_snapshot.upcoming_supply_timeline.clear();
        contradictory_snapshot.source_status.status = CapitalAbsorptionSourceHealth::Unavailable;
        contradictory_snapshot.source_status.message = "HTTP 429 rate limited".to_string();
        contradictory_snapshot.source_coverage = vec![CapitalAbsorptionSourceCoverage {
            source: "company-news:GOOG".to_string(),
            status: CapitalAbsorptionSourceCoverageStatus::Failed,
            message: "HTTP 429 rate limited".to_string(),
        }];
        let contradictory_report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&contradictory_snapshot),
            language,
        );
        assert!(
            !contradictory_report.contains(complete),
            "{contradictory_report}"
        );
        assert!(
            contradictory_report.contains(unavailable),
            "{contradictory_report}"
        );
        assert!(
            contradictory_report.contains(unknown_status),
            "{contradictory_report}"
        );
        assert!(
            !contradictory_report.contains(no_actual_supply),
            "{contradictory_report}"
        );

        let mut empty_source_snapshot = auto_snapshot_with_potential_ipo();
        empty_source_snapshot.observed_events.clear();
        empty_source_snapshot.near_term_supply.clear();
        empty_source_snapshot.ai_ipo_queue.clear();
        empty_source_snapshot.upcoming_supply_timeline.clear();
        empty_source_snapshot.source_coverage.clear();
        let empty_source_report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&empty_source_snapshot),
            language,
        );
        assert!(
            !empty_source_report.contains(complete),
            "{empty_source_report}"
        );
        assert!(
            empty_source_report.contains(unavailable),
            "{empty_source_report}"
        );
        assert!(
            empty_source_report.contains(unknown_status),
            "{empty_source_report}"
        );
        assert!(
            !empty_source_report.contains(no_actual_supply),
            "{empty_source_report}"
        );
        assert!(
            !empty_source_report.contains("STABLE"),
            "{empty_source_report}"
        );

        let mut complete_empty_snapshot = auto_snapshot_with_potential_ipo();
        complete_empty_snapshot.observed_events.clear();
        complete_empty_snapshot.near_term_supply.clear();
        complete_empty_snapshot.ai_ipo_queue.clear();
        complete_empty_snapshot.upcoming_supply_timeline.clear();
        complete_empty_snapshot.potential_supply_pressure.level =
            CapitalAbsorptionPotentialSupplyPressureLevel::Low;
        complete_empty_snapshot
            .potential_supply_pressure
            .near_term_supply_count = 0;
        complete_empty_snapshot
            .potential_supply_pressure
            .future_queue_count = 0;
        complete_empty_snapshot
            .potential_supply_pressure
            .queue_count = 0;
        complete_empty_snapshot
            .potential_supply_pressure
            .reported_count = 0;
        complete_empty_snapshot
            .potential_supply_pressure
            .confirmed_count = 0;
        let complete_empty_report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&complete_empty_snapshot),
            language,
        );
        assert!(complete_empty_report.contains(complete_empty_events));
        assert!(complete_empty_report.contains(complete_zero));
        let timeline = complete_empty_report
            .split("Upcoming Supply Timeline:\n")
            .nth(1)
            .expect("complete empty coverage should define the upcoming timeline")
            .split("\n\n")
            .next()
            .unwrap_or_default();
        assert!(timeline.contains("0-30 Days:"), "{timeline}");
        assert!(timeline.contains("1-12 Months:"), "{timeline}");
        assert!(timeline.contains("Unknown:"), "{timeline}");
        assert!(
            timeline.matches(&format!("- {no_events}")).count() >= 3,
            "{timeline}"
        );
        assert!(
            !unavailable_report.contains(no_actual_supply),
            "{unavailable_report}"
        );
        assert!(
            complete_empty_report.contains(observed_empty_near_term),
            "{complete_empty_report}"
        );
        assert!(
            complete_empty_report.contains(observed_empty_future_queue),
            "{complete_empty_report}"
        );
    }
}

#[test]
fn incomplete_empty_coverage_never_renders_zero_or_absence_as_a_fact() {
    for (language, status, no_events, trend, pressure, phase, count) in [
        (
            Language::ZhCn,
            "状态未知（来源不可用）",
            "来源覆盖不完整，无法判断是否存在相关事件。",
            "- 趋势: 未知",
            "- 压力: 未知",
            "供给阶段 未知",
            "- Mega Cap 融资: 未知",
        ),
        (
            Language::EnUs,
            "UNKNOWN (SOURCES UNAVAILABLE)",
            "Incomplete source coverage; event absence cannot be determined.",
            "- Trend: UNKNOWN",
            "- Pressure: UNKNOWN",
            "Supply Phase UNKNOWN",
            "- Mega Cap Financing: UNKNOWN",
        ),
        (
            Language::JaJp,
            "状態不明（ソース利用不可）",
            "ソース網羅性が不完全のため、関連イベントの有無は判断できません。",
            "- トレンド: 不明",
            "- 圧力: 不明",
            "供給段階 不明",
            "- Mega Cap 調達: 不明",
        ),
    ] {
        let report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&unavailable_capital_absorption_snapshot(
                "HTTP 429 rate limited".to_string(),
            )),
            language,
        );

        assert!(report.contains(status));
        assert!(report.contains(no_events));
        assert!(report.contains(trend));
        assert!(report.contains(pressure));
        assert!(report.contains(phase));
        assert!(report.contains(count));
        assert!(!report.contains("IDLE"));
        assert!(!report.contains("No relevant observations detected."));
        assert!(!report.contains("no abnormal dilution pressure"));
    }
}

fn auto_snapshot_with_potential_ipo() -> CapitalAbsorptionAutoSnapshot {
    CapitalAbsorptionAutoSnapshot {
        collection_snapshot_id: "fixture-1".to_string(),
        observation_coverage: CapitalAbsorptionObservationCoverageState::Complete,
        source_coverage: vec![CapitalAbsorptionSourceCoverage {
            source: "fixture".to_string(),
            status: CapitalAbsorptionSourceCoverageStatus::Succeeded,
            message: "fixture".to_string(),
        }],
        source_status: CapitalAbsorptionSourceStatus {
            provider: "fixture".to_string(),
            status: CapitalAbsorptionSourceHealth::Succeeded,
            message: "fixture".to_string(),
        },
        status: CapitalAbsorptionAutoStatus::Watch,
        observed_events: vec![CapitalAbsorptionAutoEvent {
            category: CapitalAbsorptionAutoEventCategory::IpoSupply,
            supply_kind: CapitalAbsorptionSupplyKind::Potential,
            event_type: CapitalAbsorptionObservationEventType::Confirmed,
            subject: "SpaceX".to_string(),
            description: "SpaceX IPO confirmed for near-term listing window".to_string(),
            amount_usd_b: None,
            ai_capex_related: false,
            source_url: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            source_count: 2,
            confidence: CapitalAbsorptionAutoConfidence::Medium,
        }],
        supply_event_counts: CapitalAbsorptionSupplyEventCounts {
            mega_cap_financing: 0,
            ai_ipo_candidate: 0,
            secondary_offering: 0,
            convertible_debt: 0,
            secondary_liquidity: 0,
        },
        near_term_supply: vec![CapitalAbsorptionIpoQueueItem {
            issuer: "SpaceX".to_string(),
            status: CapitalAbsorptionIpoQueueStatus::NearTerm,
            source_count: 2,
            event_type: CapitalAbsorptionObservationEventType::Confirmed,
            lifecycle_status: CapitalAbsorptionIpoLifecycleStatus::Confirmed,
            observed_at: Some(NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()),
            observation_day: Some(1),
            near_term_weight: Some(CapitalAbsorptionNearTermSupplyWeight::High),
        }],
        ai_ipo_queue: Vec::new(),
        upcoming_supply_timeline: vec![CapitalAbsorptionSupplyTimelineItem {
            issuer: "SpaceX".to_string(),
            bucket: CapitalAbsorptionSupplyTimelineBucket::Next30Days,
            lifecycle_status: CapitalAbsorptionIpoLifecycleStatus::Confirmed,
        }],
        observation_watchlist: Vec::new(),
        ipo_queue_history: ipo_queue_history_ending_with_size(
            NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            0,
        ),
        potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend::Rising,
        potential_supply_pressure: CapitalAbsorptionPotentialSupplyPressure {
            level: CapitalAbsorptionPotentialSupplyPressureLevel::Normal,
            near_term_supply_count: 1,
            future_queue_count: 0,
            queue_count: 0,
            reported_count: 0,
            confirmed_count: 1,
            drivers: vec![
                crate::features::research::application::capital_absorption::CapitalAbsorptionPotentialSupplyPressureDriver {
                    label: "SpaceX IPO".to_string(),
                    strength: CapitalAbsorptionPressureDriverStrength::High,
                },
            ],
        },
        capital_demand:
            crate::features::research::application::capital_absorption::CapitalDemandAutoSnapshot {
                rolling_12m_usd_b: None,
                score: None,
                trend: CapitalAbsorptionAutoTrend::Stable,
                ipo_financing_usd_b: None,
                secondary_offering_usd_b: None,
                convertible_debt_usd_b: None,
                ai_related_financing_usd_b: None,
            },
        capital_supply:
            crate::features::research::application::capital_absorption::CapitalSupplyAutoSnapshot {
                rolling_12m_usd_b: None,
                score: None,
                trend: CapitalAbsorptionAutoTrend::Stable,
                etf_net_inflow_usd_b: None,
                mutual_fund_net_inflow_usd_b: None,
                pension_allocation_flow_usd_b: None,
                foreign_capital_inflow_usd_b: None,
                corporate_buyback_usd_b: None,
            },
        absorption_ratio: CapitalAbsorptionAutoRatio {
            value: None,
            state: CapitalAbsorptionAutoRatioState::Neutral,
        },
        structural_impact: "Observation Only".to_string(),
        upgrade_to_active: Vec::new(),
        upgrade_to_stressed: Vec::new(),
    }
}

fn auto_snapshot_with_anthropic_potential_ipo() -> CapitalAbsorptionAutoSnapshot {
    CapitalAbsorptionAutoSnapshot {
        collection_snapshot_id: "fixture-2".to_string(),
        observation_coverage: CapitalAbsorptionObservationCoverageState::Complete,
        source_coverage: vec![CapitalAbsorptionSourceCoverage {
            source: "fixture".to_string(),
            status: CapitalAbsorptionSourceCoverageStatus::Succeeded,
            message: "fixture".to_string(),
        }],
        source_status: CapitalAbsorptionSourceStatus {
            provider: "fixture".to_string(),
            status: CapitalAbsorptionSourceHealth::Succeeded,
            message: "fixture".to_string(),
        },
        status: CapitalAbsorptionAutoStatus::Watch,
        observed_events: vec![CapitalAbsorptionAutoEvent {
            category: CapitalAbsorptionAutoEventCategory::IpoSupply,
            supply_kind: CapitalAbsorptionSupplyKind::Potential,
            event_type: CapitalAbsorptionObservationEventType::Rumor,
            subject: "Anthropic".to_string(),
            description: "Anthropic IPO discussion after private valuation".to_string(),
            amount_usd_b: None,
            ai_capex_related: true,
            source_url: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            source_count: 1,
            confidence: CapitalAbsorptionAutoConfidence::Low,
        }],
        supply_event_counts: CapitalAbsorptionSupplyEventCounts {
            mega_cap_financing: 0,
            ai_ipo_candidate: 0,
            secondary_offering: 0,
            convertible_debt: 0,
            secondary_liquidity: 0,
        },
        near_term_supply: Vec::new(),
        ai_ipo_queue: vec![CapitalAbsorptionIpoQueueItem {
            issuer: "Anthropic".to_string(),
            status: CapitalAbsorptionIpoQueueStatus::Reported,
            source_count: 1,
            event_type: CapitalAbsorptionObservationEventType::Rumor,
            lifecycle_status: CapitalAbsorptionIpoLifecycleStatus::Reported,
            observed_at: Some(NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()),
            observation_day: Some(1),
            near_term_weight: None,
        }],
        upcoming_supply_timeline: vec![CapitalAbsorptionSupplyTimelineItem {
            issuer: "Anthropic".to_string(),
            bucket: CapitalAbsorptionSupplyTimelineBucket::Unknown,
            lifecycle_status: CapitalAbsorptionIpoLifecycleStatus::Reported,
        }],
        observation_watchlist: Vec::new(),
        ipo_queue_history: ipo_queue_history_ending(NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()),
        potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend::Rising,
        potential_supply_pressure: CapitalAbsorptionPotentialSupplyPressure {
            level: CapitalAbsorptionPotentialSupplyPressureLevel::Normal,
            near_term_supply_count: 0,
            future_queue_count: 1,
            queue_count: 1,
            reported_count: 1,
            confirmed_count: 0,
            drivers: vec![
                crate::features::research::application::capital_absorption::CapitalAbsorptionPotentialSupplyPressureDriver {
                    label: "Anthropic IPO Discussion".to_string(),
                    strength: CapitalAbsorptionPressureDriverStrength::Medium,
                },
            ],
        },
        capital_demand:
            crate::features::research::application::capital_absorption::CapitalDemandAutoSnapshot {
                rolling_12m_usd_b: None,
                score: None,
                trend: CapitalAbsorptionAutoTrend::Stable,
                ipo_financing_usd_b: None,
                secondary_offering_usd_b: None,
                convertible_debt_usd_b: None,
                ai_related_financing_usd_b: None,
            },
        capital_supply:
            crate::features::research::application::capital_absorption::CapitalSupplyAutoSnapshot {
                rolling_12m_usd_b: None,
                score: None,
                trend: CapitalAbsorptionAutoTrend::Stable,
                etf_net_inflow_usd_b: None,
                mutual_fund_net_inflow_usd_b: None,
                pension_allocation_flow_usd_b: None,
                foreign_capital_inflow_usd_b: None,
                corporate_buyback_usd_b: None,
            },
        absorption_ratio: CapitalAbsorptionAutoRatio {
            value: None,
            state: CapitalAbsorptionAutoRatioState::Neutral,
        },
        structural_impact: "Observation Only".to_string(),
        upgrade_to_active: Vec::new(),
        upgrade_to_stressed: Vec::new(),
    }
}

fn auto_snapshot_with_listed_spacex_observation() -> CapitalAbsorptionAutoSnapshot {
    let mut snapshot = auto_snapshot_with_potential_ipo();
    snapshot.near_term_supply = Vec::new();
    snapshot.upcoming_supply_timeline = Vec::new();
    snapshot.observation_watchlist = vec![CapitalAbsorptionObservationWatchlistItem {
        issuer: "SpaceX".to_string(),
        lifecycle_status: CapitalAbsorptionIpoLifecycleStatus::Listed,
        observation_day: Some(1),
        review_window_days: Some(90),
        review_candidate: false,
    }];
    snapshot.potential_supply_pressure.near_term_supply_count = 0;
    snapshot.potential_supply_pressure.confirmed_count = 0;
    snapshot.potential_supply_pressure.drivers = Vec::new();
    snapshot
}

fn ipo_queue_history_ending(latest: NaiveDate) -> Vec<CapitalAbsorptionIpoQueueHistoryPoint> {
    ipo_queue_history_ending_with_size(latest, 1)
}

fn ipo_queue_history_ending_with_size(
    latest: NaiveDate,
    queue_size: usize,
) -> Vec<CapitalAbsorptionIpoQueueHistoryPoint> {
    (0..30)
        .map(|offset| CapitalAbsorptionIpoQueueHistoryPoint {
            observed_at: latest - Duration::days(29 - offset),
            queue_size,
        })
        .collect()
}

fn minimal_app_config(language: Language) -> config::AppConfig {
    let language_value = match language {
        Language::ZhCn => "zh-cn",
        Language::EnUs => "en-us",
        Language::JaJp => "ja-jp",
    };
    toml::from_str(&format!(
        r#"
version = 1
provider = "fixture"

[output]
timezone = "Asia/Tokyo"
format = "markdown"
save_to = "./reports"
language = "{language_value}"

[rules.trend]
lookback_days = 20
flat_threshold_pct = 0.5

[rules.deviation_bands]
overheat_2 = 30.0
optimal = -5.0

[rules.actions]
overheat_2 = "停止买入"
optimal = "买入"
fear = "恐慌加仓"

[[watchlist]]
symbol = "TSLA"
weight = 1.0
market = "US"
owner_ma_days = 120
leash_ma_days = 20
deviation_basis = "owner"
enable = true
"#
    ))
    .expect("minimal config should parse")
}
