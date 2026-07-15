// 資本吸収警告センサーのレポート生成検証テスト
use super::cognitive_reports::build_capital_absorption_report;
use crate::config;
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoConfidence, CapitalAbsorptionAutoEvent,
    CapitalAbsorptionAutoEventCategory, CapitalAbsorptionAutoRatio,
    CapitalAbsorptionAutoRatioState, CapitalAbsorptionAutoSnapshot, CapitalAbsorptionAutoStatus,
    CapitalAbsorptionAutoTrend, CapitalAbsorptionIpoLifecycleStatus,
    CapitalAbsorptionIpoQueueHistoryPoint, CapitalAbsorptionIpoQueueItem,
    CapitalAbsorptionIpoQueueStatus, CapitalAbsorptionNearTermSupplyWeight,
    CapitalAbsorptionObservationEventType, CapitalAbsorptionObservationWatchlistItem,
    CapitalAbsorptionPotentialSupplyPressure, CapitalAbsorptionPotentialSupplyPressureLevel,
    CapitalAbsorptionPotentialSupplyTrend, CapitalAbsorptionPressureDriverStrength,
    CapitalAbsorptionSourceHealth, CapitalAbsorptionSourceStatus,
    CapitalAbsorptionSupplyEventCounts, CapitalAbsorptionSupplyKind,
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
    assert!(empty_report.contains("Future Queue details unavailable."));

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

fn auto_snapshot_with_potential_ipo() -> CapitalAbsorptionAutoSnapshot {
    CapitalAbsorptionAutoSnapshot {
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
