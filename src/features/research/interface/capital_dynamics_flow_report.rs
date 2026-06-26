use crate::config;
use crate::features::research::domain::capital_dynamics::{
    FlowDivergence, FlowDivergenceType, FlowLayerSnapshot, FlowObservation,
};
use crate::features::shared::interface::i18n::Language;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeSet;

/// Flow Layer の手動設定を report 表示へ変換する。
pub(crate) fn build_flow_layer_report_from_config(
    capital_dynamics: Option<&config::CapitalDynamicsConfig>,
    language: Language,
) -> String {
    let Some(snapshot) =
        capital_dynamics.and_then(config::CapitalDynamicsConfig::flow_layer_snapshot)
    else {
        return flow_layer_empty(language).to_string();
    };

    let providers = snapshot
        .observations
        .iter()
        .map(|observation| observation.provider.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = String::new();
    out.push_str(flow_layer_title(language));
    out.push_str("\n\n");
    out.push_str(&format!(
        "{} {}\n",
        flow_layer_as_of_label(language),
        snapshot.as_of_date
    ));
    out.push_str(&format!(
        "{} {}\n",
        flow_layer_provider_label(language),
        if providers.is_empty() {
            flow_layer_none(language).to_string()
        } else {
            providers
        }
    ));
    out.push_str(&format!(
        "{} {} / {}\n",
        flow_layer_count_label(language),
        snapshot.observations.len(),
        snapshot.divergences.len()
    ));
    out.push('\n');

    push_flow_breadth(&mut out, &snapshot, language);
    push_flow_observations(&mut out, &snapshot.observations, language);
    push_flow_divergences(&mut out, &snapshot.divergences, language);
    out.push_str(flow_layer_boundary(language));
    out
}

/// weekly metrics / review 用に Flow Layer snapshot を構造化して返す。
pub(crate) fn build_flow_layer_weekly_summary(
    capital_dynamics: Option<&config::CapitalDynamicsConfig>,
) -> serde_json::Value {
    let Some(snapshot) =
        capital_dynamics.and_then(config::CapitalDynamicsConfig::flow_layer_snapshot)
    else {
        return json!({
            "configured": false,
            "balance_layer": {
                "configured": false,
                "status": "UNAVAILABLE"
            }
        });
    };

    let positive_divergence_count = snapshot
        .divergences
        .iter()
        .filter(|divergence| divergence.divergence_type == FlowDivergenceType::Positive)
        .count();
    let negative_divergence_count = snapshot
        .divergences
        .iter()
        .filter(|divergence| divergence.divergence_type == FlowDivergenceType::Negative)
        .count();

    json!({
        "configured": true,
        "as_of_date": snapshot.as_of_date,
        "observation_count": snapshot.observations.len(),
        "divergence_count": snapshot.divergences.len(),
        "positive_divergence_count": positive_divergence_count,
        "negative_divergence_count": negative_divergence_count,
        "breadth": {
            "market_breadth": enum_code(&snapshot.breadth.market_breadth),
            "sector_breadth": enum_code(&snapshot.breadth.sector_breadth),
            "watchlist_breadth": enum_code(&snapshot.breadth.watchlist_breadth),
            "core_holding_breadth": enum_code(&snapshot.breadth.core_holding_breadth)
        },
        "boundary": {
            "observation_only": snapshot.observation_only,
            "decision_weight_percent": snapshot.decision_weight_percent,
            "trend_override_allowed": snapshot.trend_override_allowed
        },
        "balance_layer": {
            "configured": false,
            "status": "UNAVAILABLE"
        },
        "snapshot": serde_json::to_value(snapshot).unwrap_or(serde_json::Value::Null)
    })
}

fn push_flow_breadth(out: &mut String, snapshot: &FlowLayerSnapshot, language: Language) {
    out.push_str(flow_layer_breadth_label(language));
    out.push('\n');
    out.push_str(&format!(
        "- {}: {}\n",
        flow_layer_market_breadth_label(language),
        enum_code(&snapshot.breadth.market_breadth)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        flow_layer_sector_breadth_label(language),
        enum_code(&snapshot.breadth.sector_breadth)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        flow_layer_watchlist_breadth_label(language),
        enum_code(&snapshot.breadth.watchlist_breadth)
    ));
    out.push_str(&format!(
        "- {}: {}\n\n",
        flow_layer_core_holding_breadth_label(language),
        enum_code(&snapshot.breadth.core_holding_breadth)
    ));
}

fn push_flow_observations(out: &mut String, observations: &[FlowObservation], language: Language) {
    out.push_str(flow_layer_observations_label(language));
    out.push('\n');
    if observations.is_empty() {
        out.push_str(&format!("- {}\n\n", flow_layer_not_configured(language)));
        return;
    }

    for observation in observations {
        out.push_str(&format!(
            "- {} [{}] · {}/{} · {} · {} · {} · {} {} · {} {} · {} {}\n",
            observation.subject,
            enum_code(&observation.scope),
            observation.provider,
            observation.source_kind,
            enum_code(&observation.direction),
            enum_code(&observation.strength),
            enum_code(&observation.quality),
            flow_layer_continuity_label(language),
            observation.continuity_days,
            flow_layer_net_flow_label(language),
            format_optional_flow(observation.net_flow),
            flow_layer_source_health_label(language),
            enum_code(&observation.source_health)
        ));
    }
    out.push('\n');
}

fn push_flow_divergences(out: &mut String, divergences: &[FlowDivergence], language: Language) {
    out.push_str(flow_layer_divergence_label(language));
    out.push('\n');
    if divergences.is_empty() {
        out.push_str(&format!("- {}\n\n", flow_layer_none(language)));
        return;
    }

    for divergence in divergences {
        out.push_str(&format!(
            "- {} · {} {} · {} {} · {} · {} · key {}\n",
            divergence.subject,
            flow_layer_price_label(language),
            enum_code(&divergence.price_direction),
            flow_layer_flow_label(language),
            enum_code(&divergence.flow_direction),
            enum_code(&divergence.divergence_type),
            enum_code(&divergence.severity),
            divergence.explanation_key
        ));
    }
    out.push('\n');
}

fn format_optional_flow(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn enum_code<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn flow_layer_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🌊 Flow Layer（需求侧观察）",
        Language::EnUs => "🌊 Flow Layer (Demand Observation)",
        Language::JaJp => "🌊 Flow Layer（需要側観測）",
    }
}

fn flow_layer_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "🌊 Flow Layer（需求侧观察）\n\nFlow Layer 未配置。\n\n边界: 仅作 Observation Only 观察层。当前 decision weight 为 0%，不接 Gate、Execution、Trader、Action Matrix、Position Sizing，也不覆盖 Trend Layer。"
        }
        Language::EnUs => {
            "🌊 Flow Layer (Demand Observation)\n\nFlow Layer is not configured.\n\nBoundary: observation only. Current decision weight is 0%; it does not connect to Gate, Execution, Trader, Action Matrix, or Position Sizing, and it must not override Trend Layer."
        }
        Language::JaJp => {
            "🌊 Flow Layer（需要側観測）\n\nFlow Layer は未設定です。\n\n境界: Observation Only の観測レイヤーです。現在の decision weight は 0% であり、Gate、Execution、Trader、Action Matrix、Position Sizing へ接続せず、Trend Layer を override しません。"
        }
    }
}

fn flow_layer_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: Flow Layer is Observation Only。Current decision weight is 0%。它只解释 trend quality，不覆盖 Trend Layer，也不连接 Gate、Execution、Trader、Action Matrix 或 Position Sizing。"
        }
        Language::EnUs => {
            "Boundary: Flow Layer is Observation Only. Current decision weight is 0%. It may explain trend quality, but it must not override Trend Layer or connect to Gate, Execution, Trader, Action Matrix, or Position Sizing."
        }
        Language::JaJp => {
            "境界: Flow Layer is Observation Only。Current decision weight is 0% です。trend quality の説明には使えるが、Trend Layer を override せず、Gate、Execution、Trader、Action Matrix、Position Sizing に接続しない。"
        }
    }
}

fn flow_layer_as_of_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察日:",
        Language::EnUs => "As of:",
        Language::JaJp => "観測日:",
    }
}

fn flow_layer_provider_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Provider:",
        Language::EnUs => "Provider:",
        Language::JaJp => "Provider:",
    }
}

fn flow_layer_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Observation / Divergence:",
        Language::EnUs => "Observations / Divergences:",
        Language::JaJp => "Observation / Divergence:",
    }
}

fn flow_layer_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Breadth",
        Language::EnUs => "Breadth",
        Language::JaJp => "Breadth",
    }
}

fn flow_layer_market_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Market Breadth",
        Language::EnUs => "Market Breadth",
        Language::JaJp => "Market Breadth",
    }
}

fn flow_layer_sector_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Sector Breadth",
        Language::EnUs => "Sector Breadth",
        Language::JaJp => "Sector Breadth",
    }
}

fn flow_layer_watchlist_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Watchlist Breadth",
        Language::EnUs => "Watchlist Breadth",
        Language::JaJp => "Watchlist Breadth",
    }
}

fn flow_layer_core_holding_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Core Holding Breadth",
        Language::EnUs => "Core Holding Breadth",
        Language::JaJp => "Core Holding Breadth",
    }
}

fn flow_layer_observations_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Observations",
        Language::EnUs => "Observations",
        Language::JaJp => "Observations",
    }
}

fn flow_layer_divergence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Divergences",
        Language::EnUs => "Divergences",
        Language::JaJp => "Divergences",
    }
}

fn flow_layer_continuity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "continuity",
        Language::EnUs => "continuity",
        Language::JaJp => "continuity",
    }
}

fn flow_layer_net_flow_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "net",
        Language::EnUs => "net",
        Language::JaJp => "net",
    }
}

fn flow_layer_source_health_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "source",
        Language::EnUs => "source",
        Language::JaJp => "source",
    }
}

fn flow_layer_price_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "price",
        Language::EnUs => "price",
        Language::JaJp => "price",
    }
}

fn flow_layer_flow_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "flow",
        Language::EnUs => "flow",
        Language::JaJp => "flow",
    }
}

fn flow_layer_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无",
        Language::EnUs => "none",
        Language::JaJp => "なし",
    }
}

fn flow_layer_not_configured(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未配置",
        Language::EnUs => "not configured",
        Language::JaJp => "未設定",
    }
}

#[cfg(test)]
mod tests {
    use super::{build_flow_layer_report_from_config, build_flow_layer_weekly_summary};
    use crate::config::{CapitalDynamicsConfig, FlowLayerConfig};
    use crate::features::research::domain::capital_dynamics::{
        FlowBreadth, FlowBreadthState, FlowDirection, FlowDivergence, FlowDivergenceSeverity,
        FlowDivergenceType, FlowObservation, FlowObservationScope, FlowQuality, FlowSourceHealth,
        FlowStrength, PriceDirection,
    };
    use crate::features::shared::interface::i18n::Language;
    use chrono::NaiveDate;

    fn sample_config() -> CapitalDynamicsConfig {
        CapitalDynamicsConfig {
            enable: Some(true),
            flow_layer: Some(FlowLayerConfig {
                as_of_date: NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
                observations: vec![FlowObservation {
                    as_of_date: NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
                    observed_at: NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
                    scope: FlowObservationScope::Asset,
                    subject: "NVDA".to_string(),
                    provider: "Manual".to_string(),
                    source_kind: "CapitalFlow".to_string(),
                    direction: FlowDirection::Inflow,
                    strength: FlowStrength::Strong,
                    quality: FlowQuality::Healthy,
                    continuity_days: 5,
                    net_flow: Some(12.5),
                    main_net_flow: Some(8.2),
                    source_health: FlowSourceHealth::Succeeded,
                }],
                divergences: vec![FlowDivergence {
                    subject: "GOOG".to_string(),
                    price_direction: PriceDirection::Up,
                    flow_direction: FlowDirection::Outflow,
                    divergence_type: FlowDivergenceType::Negative,
                    severity: FlowDivergenceSeverity::High,
                    explanation_key: "negative_divergence".to_string(),
                }],
                breadth: FlowBreadth {
                    market_breadth: FlowBreadthState::Unavailable,
                    sector_breadth: FlowBreadthState::Divergent,
                    watchlist_breadth: FlowBreadthState::Supportive,
                    core_holding_breadth: FlowBreadthState::Neutral,
                },
                observation_only: true,
                decision_weight_percent: 0,
                trend_override_allowed: false,
                enable: Some(true),
            }),
        }
    }

    #[test]
    fn flow_layer_report_keeps_observation_only_boundary() {
        let report = build_flow_layer_report_from_config(Some(&sample_config()), Language::ZhCn);

        assert!(report.contains("Flow Layer（需求侧观察）"));
        assert!(report.contains("NVDA [ASSET]"));
        assert!(report.contains("INFLOW"));
        assert!(report.contains("HEALTHY"));
        assert!(report.contains("GOOG"));
        assert!(report.contains("NEGATIVE"));
        assert!(report.contains("Current decision weight is 0%"));
        assert!(report.contains("不连接 Gate"));
    }

    #[test]
    fn flow_layer_weekly_summary_persists_boundary_and_snapshot() {
        let summary = build_flow_layer_weekly_summary(Some(&sample_config()));

        assert_eq!(summary["configured"], serde_json::Value::Bool(true));
        assert_eq!(summary["observation_count"], serde_json::Value::from(1));
        assert_eq!(
            summary["negative_divergence_count"],
            serde_json::Value::from(1)
        );
        assert_eq!(
            summary["boundary"]["decision_weight_percent"],
            serde_json::Value::from(0)
        );
        assert_eq!(
            summary["balance_layer"]["status"],
            serde_json::Value::String("UNAVAILABLE".to_string())
        );
        assert_eq!(
            summary["snapshot"]["observations"][0]["subject"],
            serde_json::Value::String("NVDA".to_string())
        );
    }

    #[test]
    fn flow_layer_weekly_summary_keeps_balance_placeholder_when_unconfigured() {
        let summary = build_flow_layer_weekly_summary(None);

        assert_eq!(summary["configured"], serde_json::Value::Bool(false));
        assert_eq!(
            summary["balance_layer"]["status"],
            serde_json::Value::String("UNAVAILABLE".to_string())
        );
    }
}
