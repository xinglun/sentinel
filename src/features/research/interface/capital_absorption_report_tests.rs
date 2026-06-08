use super::cognitive_reports::build_capital_absorption_report;
use crate::config;
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoConfidence, CapitalAbsorptionAutoEvent,
    CapitalAbsorptionAutoEventCategory, CapitalAbsorptionAutoRatio,
    CapitalAbsorptionAutoRatioState, CapitalAbsorptionAutoSnapshot, CapitalAbsorptionAutoStatus,
    CapitalAbsorptionAutoTrend, CapitalAbsorptionIpoQueueHistoryPoint,
    CapitalAbsorptionIpoQueueItem, CapitalAbsorptionIpoQueueStatus,
    CapitalAbsorptionObservationEventType, CapitalAbsorptionPotentialSupplyTrend,
    CapitalAbsorptionSourceHealth, CapitalAbsorptionSourceStatus,
    CapitalAbsorptionSupplyEventCounts, CapitalAbsorptionSupplyKind,
};
use crate::features::shared::interface::i18n::Language;
use chrono::NaiveDate;

#[test]
fn auto_report_locks_new_sections_in_en_and_ja() {
    for (
        language,
        title,
        actual_supply,
        potential_trend,
        queue_history,
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
            "IPO 队列历史",
            "队列规模 = 1",
            "SpaceX: IPO 阶段 传闻（Rumor） · 事件类型 传闻（Rumor） · 来源 2",
            "- SpaceX x2",
            "不影响 READY / EXECUTE / Position Sizing / Gate / Trend Layer",
            Some("结构影响: Observation Only"),
        ),
        (
            Language::EnUs,
            "Capital Absorption Early Warning Sensor",
            "Actual Capital Supply",
            "Potential Supply Trend",
            "IPO Queue History",
            "Queue Size = 1",
            "SpaceX: IPO Stage Rumor · Event Type Rumor · Sources 2",
            "- SpaceX x2",
            "does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer",
            None,
        ),
        (
            Language::JaJp,
            "資本吸収早期警戒センサー",
            "実際の資本供給",
            "潜在供給トレンド",
            "IPO キュー履歴",
            "キュー規模 = 1",
            "SpaceX: IPO 段階 噂（Rumor） · イベント種別 噂（Rumor） · ソース数 2",
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
        assert!(report.contains(queue_history));
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
            "Anthropic: IPO 阶段 准备中（Preparing） · 事件类型 传闻（Rumor）",
            "- Anthropic x1",
            "实际供给 · 事件类型 确认（Confirmed） · IPO 供给 · Anthropic",
            "不影响 READY / EXECUTE / Position Sizing / Gate / Trend Layer",
        ),
        (
            Language::EnUs,
            "Actual Capital Supply",
            "No completed large equity or convertible supply observed.",
            "Anthropic: IPO Stage Preparing · Event Type Rumor",
            "- Anthropic x1",
            "Actual Supply · Event Type Confirmed · IPO Supply · Anthropic",
            "does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer",
        ),
        (
            Language::JaJp,
            "実際の資本供給",
            "発生済みの大型株式・転換社債供給は未観測です。",
            "Anthropic: IPO 段階 準備中（Preparing） · イベント種別 噂（Rumor）",
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
            event_type: CapitalAbsorptionObservationEventType::Rumor,
            subject: "SpaceX".to_string(),
            description: "SpaceX IPO rumor".to_string(),
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
        ai_ipo_queue: vec![CapitalAbsorptionIpoQueueItem {
            issuer: "SpaceX".to_string(),
            status: CapitalAbsorptionIpoQueueStatus::Rumor,
            source_count: 2,
            event_type: CapitalAbsorptionObservationEventType::Rumor,
        }],
        ipo_queue_history: vec![CapitalAbsorptionIpoQueueHistoryPoint {
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            queue_size: 1,
        }],
        potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend::Rising,
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
        ai_ipo_queue: vec![CapitalAbsorptionIpoQueueItem {
            issuer: "Anthropic".to_string(),
            status: CapitalAbsorptionIpoQueueStatus::Preparing,
            source_count: 1,
            event_type: CapitalAbsorptionObservationEventType::Rumor,
        }],
        ipo_queue_history: vec![CapitalAbsorptionIpoQueueHistoryPoint {
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            queue_size: 1,
        }],
        potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend::Rising,
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
