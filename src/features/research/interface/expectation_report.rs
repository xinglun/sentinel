use std::collections::BTreeMap;

use crate::config;
use crate::features::research::domain::expectation::ExpectationObservation;
use crate::features::shared::interface::i18n::Language;
use serde::Serialize;
use serde_json::json;

use super::expectation_report_builder::{
    build_expectation_layer_fixture_snapshot, build_expectation_layer_snapshot_from_config,
    ExpectationLayerSnapshot,
};

/// Expectation Layer の read-only report を組み立てる。
#[allow(dead_code)]
pub(crate) fn build_expectation_layer_report(language: Language) -> String {
    let snapshot = build_expectation_layer_fixture_snapshot();
    build_expectation_layer_report_from_snapshot(&snapshot, language)
}

/// Expectation Layer の read-only report を live source を優先して組み立てる。
pub(crate) fn build_expectation_layer_report_with_config(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    let snapshot = build_expectation_layer_snapshot_from_config(app_config);
    build_expectation_layer_report_from_snapshot(&snapshot, language)
}

/// Expectation Layer の snapshot を weekly metrics / latest_context 用の JSON に変換する。
#[allow(dead_code)]
pub(crate) fn build_expectation_layer_weekly_summary() -> serde_json::Value {
    let snapshot = build_expectation_layer_fixture_snapshot();
    expectation_layer_summary(&snapshot)
}

/// Expectation Layer の snapshot を weekly metrics / latest_context 用の JSON に変換する。
pub(crate) fn build_expectation_layer_weekly_summary_with_config(
    app_config: &config::AppConfig,
) -> serde_json::Value {
    let snapshot = build_expectation_layer_snapshot_from_config(app_config);
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

    out.push('\n');
    out.push_str(expectation_boundary(language));
    out
}

fn push_observation_block(
    out: &mut String,
    observation: &ExpectationObservation,
    _language: Language,
) {
    out.push_str(&format!("- Period: {}\n", observation.period));
    out.push_str(&format!("- As of: {}\n", observation.as_of_date));
    out.push_str(&format!("- Expected: {}\n", observation.expected_value));
    out.push_str(&format!("- Actual: {}\n", observation.actual_value));
    out.push_str(&format!("- Unit: {}\n", observation.unit));
    out.push_str(&format!(
        "- Consensus Source: {}\n",
        observation.consensus_source
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
        observation.interpretation
    ));
}

fn expectation_layer_summary(snapshot: &ExpectationLayerSnapshot) -> serde_json::Value {
    let mut revision_direction_counts = BTreeMap::<String, usize>::new();
    let mut surprise_state_counts = BTreeMap::<String, usize>::new();
    let mut expectation_pressure_counts = BTreeMap::<String, usize>::new();
    let mut source_health_counts = BTreeMap::<String, usize>::new();
    let mut event_type_counts = BTreeMap::<String, usize>::new();

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

fn option_or_na(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("N/A")
}

fn enum_code<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "UNKNOWN".to_string())
}
