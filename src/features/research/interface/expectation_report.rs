use std::collections::BTreeMap;

use crate::config;
use crate::features::research::domain::expectation::{
    ExpectationEventType, ExpectationLifecycleState, ExpectationObservation,
};
use crate::features::shared::interface::i18n::Language;
use serde::Serialize;
use serde_json::json;

#[cfg(test)]
use super::expectation_report_builder::build_expectation_layer_fixture_snapshot;
use super::expectation_report_builder::{
    build_expectation_layer_snapshot_for_market_date, ExpectationLayerSnapshot,
};

const SILENT_PENDING_MIN_AGE_DAYS: i64 = 14;

/// Expectation Layer の fixture 用 read-only report を組み立てる。
#[cfg(test)]
pub(crate) fn build_expectation_layer_report(language: Language) -> String {
    let snapshot = build_expectation_layer_fixture_snapshot();
    build_expectation_layer_report_from_snapshot(&snapshot, language)
}

/// Expectation Layer の fixture 用 snapshot を weekly metrics / latest_context 用の JSON に変換する。
#[cfg(test)]
pub(crate) fn build_expectation_layer_weekly_summary() -> serde_json::Value {
    let snapshot = build_expectation_layer_fixture_snapshot();
    expectation_layer_summary(&snapshot)
}

/// Expectation Layer の snapshot を weekly metrics / latest_context 用の JSON に変換する。
#[allow(dead_code)]
pub(crate) fn build_expectation_layer_weekly_summary_with_config(
    app_config: &config::AppConfig,
) -> serde_json::Value {
    build_expectation_layer_weekly_summary_with_config_for_market_date(
        app_config,
        chrono::Local::now().date_naive(),
    )
}

/// 指定した取引日で weekly metrics 用の Expectation snapshot を組み立てる。
pub(crate) fn build_expectation_layer_weekly_summary_with_config_for_market_date(
    app_config: &config::AppConfig,
    market_date: chrono::NaiveDate,
) -> serde_json::Value {
    let snapshot = build_expectation_layer_snapshot_for_market_date(app_config, market_date);
    expectation_layer_summary(&snapshot)
}

pub(crate) fn build_expectation_layer_report_from_snapshot(
    snapshot: &ExpectationLayerSnapshot,
    language: Language,
) -> String {
    let mut out = String::new();
    out.push_str(expectation_title(language));
    out.push_str("\n\n");
    out.push_str(expectation_intro(language));
    out.push_str("\n\n");
    out.push_str(&format!(
        "- {}: {}\n",
        expectation_as_of_label(language),
        snapshot.as_of_date
    ));
    out.push_str(&format!(
        "- {}: {}%\n",
        expectation_decision_weight_label(language),
        snapshot.decision_weight_percent
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        expectation_trade_signal_label(language),
        snapshot.trade_signal
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        expectation_gate_effect_label(language),
        snapshot.gate_effect
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        expectation_execution_effect_label(language),
        snapshot.execution_effect
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        expectation_position_sizing_effect_label(language),
        snapshot.position_sizing_effect
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        expectation_observation_count_label(language),
        snapshot.observations.len()
    ));

    let subjects = snapshot
        .observations
        .iter()
        .map(|observation| observation.subject.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if !subjects.is_empty() {
        out.push_str(&format!(
            "- {}: {}\n",
            expectation_subjects_label(language),
            subjects.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    if is_silent_expectation_snapshot(snapshot) {
        out.push_str(&format!(
            "\n{}\n\n",
            expectation_no_updates_today_label(language)
        ));
    } else {
        for observation in &snapshot.observations {
            out.push_str("\n### ");
            out.push_str(&format!(
                "{} / {} / {}",
                observation.subject,
                observation.period,
                enum_code(&observation.event_type)
            ));
            out.push('\n');
            push_observation_block(&mut out, observation, language);
        }
    }

    if is_silent_expectation_snapshot(snapshot) {
        out.push_str("\n## Appendix\n\n");
        for observation in &snapshot.observations {
            out.push_str("\n### ");
            out.push_str(&format!(
                "{} / {} / {}",
                observation.subject,
                observation.period,
                enum_code(&observation.event_type)
            ));
            out.push('\n');
            push_observation_block(&mut out, observation, language);
        }
    }

    out.push('\n');
    out.push_str(expectation_boundary(language));
    out
}

fn is_silent_expectation_snapshot(snapshot: &ExpectationLayerSnapshot) -> bool {
    if snapshot.observations.is_empty() {
        return false;
    }

    if !snapshot.observations.iter().all(|observation| {
        observation.lifecycle_state == ExpectationLifecycleState::Pending
            && observation.result.is_none()
            && observation.released_at.is_none()
            && observation.archived_at.is_none()
    }) {
        return false;
    }

    let Some(latest_observed_at) = snapshot
        .observations
        .iter()
        .map(|observation| observation.observed_at)
        .max()
    else {
        return false;
    };

    snapshot
        .as_of_date
        .signed_duration_since(latest_observed_at)
        .num_days()
        >= SILENT_PENDING_MIN_AGE_DAYS
}

fn push_observation_block(
    out: &mut String,
    observation: &ExpectationObservation,
    language: Language,
) {
    out.push_str(&format!(
        "- Lifecycle Stage: {}\n",
        enum_code(&observation.lifecycle_state)
    ));
    out.push_str(&format!("- Period: {}\n", observation.period));
    out.push_str(&format!("- As of: {}\n", observation.as_of_date));
    out.push_str(&format!(
        "- Expected: {}\n",
        localized_observation_value(&observation.expected_value, language)
    ));
    out.push_str(&format!(
        "- Actual: {}\n",
        localized_observation_value(&observation.actual_value, language)
    ));
    out.push_str(&format!(
        "- Result: {}\n",
        observation
            .result
            .map(|value| enum_code(&value))
            .unwrap_or_else(|| "UNAVAILABLE".to_string())
    ));
    out.push_str(&format!(
        "- Surprise Percent: {}\n",
        observation
            .surprise_percent
            .map(|value| format!("{:.2}%", value))
            .unwrap_or_else(|| "UNAVAILABLE".to_string())
    ));
    out.push_str(&format!(
        "- Market Reaction: {}\n",
        observation
            .market_reaction
            .as_deref()
            .unwrap_or("UNAVAILABLE")
    ));
    out.push_str(&format!(
        "- Released At: {}\n",
        observation
            .released_at
            .map(|date| date.to_string())
            .unwrap_or_else(|| "UNAVAILABLE".to_string())
    ));
    out.push_str(&format!(
        "- Archived At: {}\n",
        observation
            .archived_at
            .map(|date| date.to_string())
            .unwrap_or_else(|| "UNAVAILABLE".to_string())
    ));
    out.push_str(&format!("- Unit: {}\n", observation.unit));
    out.push_str(&format!(
        "- Consensus Source: {}\n",
        localized_consensus_source(observation, language)
    ));
    out.push_str(&format!(
        "- Estimate Count: {}\n",
        observation.estimate_count
    ));
    out.push_str(&format!(
        "- Estimate High / Low / Median / Average: {} / {} / {} / {}\n",
        option_or_na(&observation.estimate_high),
        option_or_na(&observation.estimate_low),
        option_or_na(&observation.estimate_median),
        option_or_na(&observation.estimate_average)
    ));
    out.push_str(&format!(
        "- Revision: {}\n",
        enum_code(&observation.revision_direction)
    ));
    out.push_str(&format!(
        "- Surprise: {}\n",
        enum_code(&observation.surprise_state)
    ));
    out.push_str(&format!(
        "- Expectation Pressure: {}\n",
        enum_code(&observation.expectation_pressure)
    ));
    out.push_str(&format!(
        "- Confidence: {}\n",
        observation
            .confidence
            .map(|value| format!("{:.0}%", value * 100.0))
            .unwrap_or_else(|| "N/A".to_string())
    ));
    out.push_str(&format!(
        "- Source Health: {}\n",
        enum_code(&observation.source_health)
    ));
    out.push_str(&format!(
        "- Interpretation: {}\n",
        localized_observation_interpretation(observation, language)
    ));
}

fn localized_consensus_source(observation: &ExpectationObservation, language: Language) -> String {
    let source = &observation.consensus_source;
    if !source.starts_with("unavailable:") || matches!(language, Language::JaJp) {
        return source.clone();
    }

    let provider_unavailable = source.contains("Finnhub");
    match language {
        Language::JaJp => source.clone(),
        Language::EnUs if provider_unavailable => {
            "unavailable: The source is unavailable because the Finnhub consensus provider cannot provide expectation data.".to_string()
        }
        Language::EnUs => {
            "unavailable: No direct consensus endpoint is available for this observation.".to_string()
        }
        Language::ZhCn if provider_unavailable => {
            "来源不可用：Finnhub 共识数据提供方不可用，当前无法取得预期数据。".to_string()
        }
        Language::ZhCn => {
            "来源不可用：该观测没有可用的直接共识端点。".to_string()
        }
    }
}

fn localized_observation_value(value: &str, language: Language) -> String {
    match (value, language) {
        ("未対応", Language::ZhCn) => "未提供".to_string(),
        ("未対応", Language::EnUs) => "Unavailable".to_string(),
        ("未発表", Language::ZhCn) => "尚未发布".to_string(),
        ("未発表", Language::EnUs) => "Not released".to_string(),
        _ => value.to_string(),
    }
}

fn localized_observation_interpretation(
    observation: &ExpectationObservation,
    language: Language,
) -> String {
    if matches!(
        observation.source_health,
        crate::features::research::domain::expectation::SourceHealth::Unavailable
    ) {
        return match language {
            Language::ZhCn => "来源不可用，当前无法确认市场预期；数据发布后再比较实际结果。".to_string(),
            Language::EnUs => {
                "The source is unavailable, so the market expectation cannot be confirmed; compare the actual result when released.".to_string()
            }
            Language::JaJp => observation.interpretation.clone(),
        };
    }

    match language {
        Language::JaJp => observation.interpretation.clone(),
        Language::EnUs if observation.interpretation.is_ascii() => {
            observation.interpretation.clone()
        }
        Language::EnUs => format!(
            "{}; compare the actual result with the market expectation when it is released.",
            english_expectation_subject(observation.event_type)
        ),
        Language::ZhCn => format!(
            "{}；数据发布后与实际结果比较。",
            chinese_expectation_subject(observation.event_type)
        ),
    }
}

fn english_expectation_subject(event_type: ExpectationEventType) -> &'static str {
    match event_type {
        ExpectationEventType::DeliveryConsensus => "The market has a delivery baseline",
        ExpectationEventType::EarningsConsensus => "The market has an earnings expectation",
        ExpectationEventType::RevenueConsensus => "The market has a revenue expectation",
        ExpectationEventType::MarginConsensus => "The market has a margin expectation",
        ExpectationEventType::CloudGrowthConsensus => "The market has a cloud-growth expectation",
        ExpectationEventType::CapexConsensus => "The market has a capital-expenditure expectation",
        ExpectationEventType::ProductEventExpectation => {
            "The market has a product-event expectation"
        }
        ExpectationEventType::UserGrowthConsensus => "The market has a user-growth expectation",
        ExpectationEventType::ProcedureGrowthConsensus => {
            "The market has a procedure-growth expectation"
        }
    }
}

fn chinese_expectation_subject(event_type: ExpectationEventType) -> &'static str {
    match event_type {
        ExpectationEventType::DeliveryConsensus => "市场已形成交付量基准",
        ExpectationEventType::EarningsConsensus => "市场已形成盈利预期",
        ExpectationEventType::RevenueConsensus => "市场已形成收入预期",
        ExpectationEventType::MarginConsensus => "市场已形成利润率预期",
        ExpectationEventType::CloudGrowthConsensus => "市场已形成云业务增长预期",
        ExpectationEventType::CapexConsensus => "市场已形成资本开支预期",
        ExpectationEventType::ProductEventExpectation => "市场已形成产品事件预期",
        ExpectationEventType::UserGrowthConsensus => "市场已形成用户增长预期",
        ExpectationEventType::ProcedureGrowthConsensus => "市场已形成业务流程增长预期",
    }
}

fn expectation_layer_summary(snapshot: &ExpectationLayerSnapshot) -> serde_json::Value {
    let mut revision_direction_counts = BTreeMap::<String, usize>::new();
    let mut surprise_state_counts = BTreeMap::<String, usize>::new();
    let mut expectation_pressure_counts = BTreeMap::<String, usize>::new();
    let mut source_health_counts = BTreeMap::<String, usize>::new();
    let mut event_type_counts = BTreeMap::<String, usize>::new();
    let mut lifecycle_state_counts = BTreeMap::<String, usize>::new();

    lifecycle_state_counts.insert(enum_code(&ExpectationLifecycleState::Upcoming), 0);
    lifecycle_state_counts.insert(enum_code(&ExpectationLifecycleState::Pending), 0);
    lifecycle_state_counts.insert(enum_code(&ExpectationLifecycleState::Released), 0);
    lifecycle_state_counts.insert(enum_code(&ExpectationLifecycleState::Compared), 0);
    lifecycle_state_counts.insert(enum_code(&ExpectationLifecycleState::Archived), 0);

    for observation in &snapshot.observations {
        *revision_direction_counts
            .entry(enum_code(&observation.revision_direction))
            .or_insert(0) += 1;
        *surprise_state_counts
            .entry(enum_code(&observation.surprise_state))
            .or_insert(0) += 1;
        *expectation_pressure_counts
            .entry(enum_code(&observation.expectation_pressure))
            .or_insert(0) += 1;
        *source_health_counts
            .entry(enum_code(&observation.source_health))
            .or_insert(0) += 1;
        *event_type_counts
            .entry(enum_code(&observation.event_type))
            .or_insert(0) += 1;
        *lifecycle_state_counts
            .entry(enum_code(&observation.lifecycle_state))
            .or_insert(0) += 1;
    }

    let mut subjects = snapshot
        .observations
        .iter()
        .map(|observation| observation.subject.clone())
        .collect::<Vec<_>>();
    subjects.sort();
    subjects.dedup();

    json!({
        "configured": true,
        "as_of_date": snapshot.as_of_date,
        "decision_weight_percent": snapshot.decision_weight_percent,
        "trade_signal": snapshot.trade_signal,
        "gate_effect": snapshot.gate_effect,
        "execution_effect": snapshot.execution_effect,
        "position_sizing_effect": snapshot.position_sizing_effect,
        "observation_count": snapshot.observations.len(),
        "subject_count": subjects.len(),
        "subjects": subjects,
        "revision_direction_counts": revision_direction_counts,
        "surprise_state_counts": surprise_state_counts,
        "expectation_pressure_counts": expectation_pressure_counts,
        "source_health_counts": source_health_counts,
        "event_type_counts": event_type_counts,
        "lifecycle_state_counts": lifecycle_state_counts,
        "snapshot": serde_json::to_value(snapshot).unwrap_or(serde_json::Value::Null)
    })
}

fn expectation_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 9. Expectation Layer（市场预期观测）",
        Language::EnUs => "## 9. Expectation Layer (Market Expectation Observation)",
        Language::JaJp => "## 9. Expectation Layer（市場期待観測）",
    }
}

fn expectation_intro(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "Expectation Layer 只观察市场已经相信了什么，以及现实结果与预期之间的差值；它不预测未来，也不下交易命令。"
        }
        Language::EnUs => {
            "Expectation Layer only observes what the market already believes and the delta between expectation and reality; it does not predict the future or issue trade commands."
        }
        Language::JaJp => {
            "Expectation Layer は売買判断ではなく、市場がすでに織り込んでいる期待値を観察する。短期反応は事実そのものではなく、Reality と Expectation の差分によって決まりやすい。"
        }
    }
}

fn expectation_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "Boundary: Expectation Layer 仅用于观测市场预期，不进入 Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing，也不生成交易信号。"
        }
        Language::EnUs => {
            "Boundary: Expectation Layer is for observing market expectations only. It does not enter Gate, Execution, Trader, Action Matrix, READY / EXECUTE, or Position Sizing, and it does not generate trade signals."
        }
        Language::JaJp => {
            "境界: Expectation Layer は市場期待の観測専用であり、Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing に入らず、売買シグナルも生成しない。"
        }
    }
}

fn expectation_as_of_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "As of",
        Language::EnUs => "As of",
        Language::JaJp => "As of",
    }
}

fn expectation_decision_weight_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "decision_weight",
        Language::EnUs => "decision_weight",
        Language::JaJp => "decision_weight",
    }
}

fn expectation_trade_signal_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "trade_signal",
        Language::EnUs => "trade_signal",
        Language::JaJp => "trade_signal",
    }
}

fn expectation_gate_effect_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "gate_effect",
        Language::EnUs => "gate_effect",
        Language::JaJp => "gate_effect",
    }
}

fn expectation_execution_effect_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "execution_effect",
        Language::EnUs => "execution_effect",
        Language::JaJp => "execution_effect",
    }
}

fn expectation_position_sizing_effect_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "position_sizing_effect",
        Language::EnUs => "position_sizing_effect",
        Language::JaJp => "position_sizing_effect",
    }
}

fn expectation_observation_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "observation_count",
        Language::EnUs => "observation_count",
        Language::JaJp => "observation_count",
    }
}

fn expectation_subjects_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "subjects",
        Language::EnUs => "subjects",
        Language::JaJp => "subjects",
    }
}

fn expectation_no_updates_today_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "No expectation updates today.",
        Language::EnUs => "No expectation updates today.",
        Language::JaJp => "No expectation updates today.",
    }
}

fn option_or_na(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("N/A")
}

fn enum_code<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

#[cfg(test)]
mod tests {
    use super::build_expectation_layer_weekly_summary_with_config_for_market_date;
    use crate::config::AppConfig;
    use chrono::NaiveDate;

    #[test]
    fn weekly_expectation_summary_uses_explicit_market_date() {
        let mut config = AppConfig::load("config.toml").expect("config.toml should load");
        config.finnhub = None;

        let summary = build_expectation_layer_weekly_summary_with_config_for_market_date(
            &config,
            NaiveDate::from_ymd_opt(2026, 8, 12).unwrap(),
        );

        assert_eq!(summary["as_of_date"], "2026-08-12");
        assert_eq!(summary["snapshot"]["as_of_date"], "2026-08-12");
    }
}
