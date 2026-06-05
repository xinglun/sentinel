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
    for (language, title, queue_history, potential_event, boundary) in [
        (
            Language::EnUs,
            "Capital Absorption Early Warning Sensor",
            "IPO Queue History",
            "Potential Queue · Event Type Rumor · IPO Supply · SpaceX",
            "does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer",
        ),
        (
            Language::JaJp,
            "資本吸収早期警戒センサー",
            "IPO キュー履歴",
            "潜在キュー · イベント種別 噂（Rumor） · IPO 供給 · SpaceX",
            "READY / EXECUTE / Position Sizing / Gate / Trend Layer に影響しない",
        ),
    ] {
        let report = build_capital_absorption_report(
            &minimal_app_config(language),
            Some(&auto_snapshot_with_potential_ipo()),
            language,
        );

        assert!(report.contains(title));
        assert!(report.contains("Actual Capital Supply") || report.contains("実際の資本供給"));
        assert!(report.contains("Potential Supply Trend") || report.contains("潜在供給トレンド"));
        assert!(report.contains(queue_history));
        assert!(report.contains("Queue Size = 1"));
        assert!(report.contains(potential_event));
        assert!(report.contains(boundary));
        assert!(!report.contains("Capital Demand"));
        assert!(!report.contains("ACCELERATING"));
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
