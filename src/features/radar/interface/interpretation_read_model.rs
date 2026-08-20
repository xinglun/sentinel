use crate::features::radar::interface::presentation::{
    InterpretationExpectationQuality, InterpretationExpectationQualityReason,
    InterpretationGravityDataQuality, InterpretationGravityDataQualityReason,
    InterpretationLayerViewModel, InterpretationQuality, InterpretationTrendState,
    StateTransitionViewModel,
};
use crate::features::radar::interface::signal_context_read_model::{
    build_signal_context_assessment, signal_context_boundary,
    signal_context_information_content_label, signal_context_lifecycle_label,
    signal_context_primary_context_label, signal_context_quality_label,
    SignalContextReadModelInput,
};
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoSnapshot, CapitalAbsorptionPotentialSupplyPressureLevel,
};
use crate::features::research::application::valuation_gravity::{
    ValuationGravityObservation, ValuationPersistenceHealth, ValuationPersistenceReason,
};
use crate::features::research::domain::valuation_gravity::GravityStatus;
use crate::features::research::interface::expectation_report_builder::ExpectationLayerSnapshot;
use crate::features::shared::interface::i18n::{DisplayDictionary, Language};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(test)]
use crate::features::radar::interface::presentation::InterpretationPattern;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct InterpretationNarrativeSignal {
    pub trend_state: InterpretationTrendState,
    pub trend_available: bool,
    pub expectation_quality: InterpretationExpectationQuality,
    pub expectation_quality_reason: InterpretationExpectationQualityReason,
    pub gravity_data_quality: InterpretationGravityDataQuality,
    pub gravity_data_quality_reason: InterpretationGravityDataQualityReason,
    pub gravity_status: Option<GravityStatus>,
    pub supply_pressure: bool,
    pub supply_available: bool,
    pub flow_acceleration: Option<f64>,
    pub gray_rhino_escalated: bool,
}

impl Default for InterpretationNarrativeSignal {
    fn default() -> Self {
        Self {
            trend_state: InterpretationTrendState::Weak,
            trend_available: false,
            expectation_quality: InterpretationExpectationQuality::Unavailable,
            expectation_quality_reason: InterpretationExpectationQualityReason::SystemUnavailable,
            gravity_data_quality: InterpretationGravityDataQuality::Unavailable,
            gravity_data_quality_reason:
                InterpretationGravityDataQualityReason::ProviderUnavailable,
            gravity_status: None,
            supply_pressure: false,
            supply_available: false,
            flow_acceleration: None,
            gray_rhino_escalated: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct InterpretationLayerReadModelInput<'a> {
    pub as_of_date: NaiveDate,
    pub subjects: &'a [String],
    pub signal: InterpretationNarrativeSignal,
    pub future_context: crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel,
    pub decision_summary:
        Option<&'a crate::features::radar::interface::presentation::DecisionSummaryViewModel>,
    pub language: Language,
    pub dict: &'a DisplayDictionary,
}

pub(crate) fn build_interpretation_layer_view_model(
    input: InterpretationLayerReadModelInput<'_>,
) -> InterpretationLayerViewModel {
    let interpretation = &input.dict.interpretation;
    let signal_context = build_signal_context_assessment(SignalContextReadModelInput {
        as_of_date: input.as_of_date,
        signal: input.signal,
        future_context: input.future_context,
        language: input.language,
    });
    let subjects_value = render_subjects(input.subjects);
    let trend_value = trend_component_value(&input.signal, input.language);
    let expectation_value = expectation_component_value(&input.signal, input.language);
    let supply_value = supply_component_value(&input.signal, input.language);
    let gravity_value = gravity_component_value(&input.signal, input.language);
    let flow_value = flow_component_value(&input.signal, input.language);
    let trend_confidence_value = trend_confidence_value(&input.signal);
    let expectation_confidence_value = expectation_confidence_value(&input.signal);
    let supply_confidence_value = supply_confidence_value(&input.signal);
    let gravity_confidence_value = gravity_confidence_value(&input.signal);
    let flow_confidence_value = flow_confidence_value(&input.signal);
    let observation_health_value = observation_health_value(&input.signal, &signal_context);
    let interpretation_quality_value = interpretation_quality_value(&input.signal, &signal_context);
    let expectation_lifecycle_value =
        expectation_lifecycle_value(&input.signal, &signal_context, input.language);
    let expectation_next_observation_value =
        expectation_next_observation_value(&input.signal, &signal_context, input.language);
    let interpretation_value =
        compose_interpretation_value(&signal_context, &input.signal, input.language);
    let todays_explanation_navigation_value = todays_explanation_navigation_text(input.language);
    let (decision_explanation_intro, decision_explanation_reasons, decision_explanation_conclusion) =
        decision_explanation_values(input.decision_summary, input.language, interpretation);
    let todays_explanation =
        build_todays_explanation(&input.signal, &signal_context, input.language);
    let primary_event = signal_context.v1.primary_context.as_ref();

    InterpretationLayerViewModel {
        title: interpretation.title.clone(),
        notice: interpretation.notice.clone(),
        current_decision_weight_label: interpretation.current_decision_weight_label.clone(),
        current_decision_weight_value: "0%".to_string(),
        signal_context_label: interpretation.signal_context_label.clone(),
        signal_context_information_content_label: interpretation
            .signal_context_information_content_label
            .clone(),
        signal_context_information_content_value: signal_context_information_content_label(
            signal_context.information_content,
        )
        .to_string(),
        signal_context_primary_context_label: interpretation
            .signal_context_primary_context_label
            .clone(),
        signal_context_primary_context_value: signal_context
            .v1
            .primary_context
            .as_ref()
            .map(|item| {
                format!(
                    "{}: {}",
                    signal_context_primary_context_label(signal_context.primary_context),
                    item.title
                )
            })
            .unwrap_or_else(|| {
                signal_context_primary_context_label(signal_context.primary_context).to_string()
            }),
        signal_context_quality_label: interpretation.signal_context_quality_label.clone(),
        signal_context_quality_value: signal_context_quality_label(signal_context.context_quality)
            .to_string(),
        signal_context_event_fact_label: interpretation.signal_context_event_fact_label.clone(),
        signal_context_event_fact_value: signal_context.event_fact,
        signal_context_source_diagnostics_label: interpretation
            .signal_context_source_diagnostics_label
            .clone(),
        signal_context_source_diagnostics_value: signal_context.source_diagnostics_summary,
        signal_context_source_diagnostics_appendix_label: interpretation
            .signal_context_source_diagnostics_appendix_label
            .clone(),
        signal_context_source_diagnostics_appendix_value: signal_context
            .source_diagnostics_appendix,
        signal_context_interpretation_label: interpretation
            .signal_context_interpretation_label
            .clone(),
        signal_context_interpretation_value: signal_context.interpretation,
        signal_context_next_observation_label: interpretation
            .signal_context_next_observation_label
            .clone(),
        signal_context_next_observation_value: signal_context.next_observation,
        signal_context_boundary: signal_context_boundary(input.language).to_string(),
        signal_context_coverage: signal_context.v1.coverage,
        signal_context_lifecycle_label: "Lifecycle".to_string(),
        signal_context_lifecycle_value: primary_event
            .map(|item| signal_context_lifecycle_label(item.lifecycle))
            .unwrap_or("UNAVAILABLE")
            .to_string(),
        signal_context_expected_label: "Expected".to_string(),
        signal_context_expected_value: primary_event
            .and_then(|item| item.expected_value.clone())
            .unwrap_or_else(|| "UNAVAILABLE".to_string()),
        signal_context_actual_label: "Actual".to_string(),
        signal_context_actual_value: primary_event
            .and_then(|item| item.actual_value.clone())
            .unwrap_or_else(|| "UNAVAILABLE".to_string()),
        signal_context_surprise_label: "Surprise".to_string(),
        signal_context_surprise_value: primary_event
            .and_then(|item| item.surprise.clone())
            .unwrap_or_else(|| "UNAVAILABLE".to_string()),
        signal_context_reason_label: "Reason".to_string(),
        signal_context_reason_value: primary_event
            .and_then(|item| item.reason.clone())
            .unwrap_or_else(|| "UNAVAILABLE".to_string()),
        expectation_quality_label: interpretation.expectation_quality_label.clone(),
        expectation_quality_value: expectation_quality_label(input.signal.expectation_quality),
        expectation_quality_reason_label: interpretation.expectation_quality_reason_label.clone(),
        expectation_quality_reason_value: expectation_quality_reason_label(
            input.signal.expectation_quality_reason,
            input.language,
        ),
        expectation_lifecycle_label: interpretation.expectation_lifecycle_label.clone(),
        expectation_lifecycle_value,
        expectation_next_observation_label: interpretation
            .expectation_next_observation_label
            .clone(),
        expectation_next_observation_value,
        gravity_data_quality_label: interpretation.gravity_data_quality_label.clone(),
        gravity_data_quality_value: gravity_data_quality_label(input.signal.gravity_data_quality),
        gravity_data_quality_reason_label: interpretation.gravity_data_quality_reason_label.clone(),
        gravity_data_quality_reason_value: gravity_data_quality_reason_label(
            input.signal.gravity_data_quality_reason,
        ),
        observation_health_label: interpretation.observation_health_label.clone(),
        observation_health_value,
        interpretation_quality_label: interpretation.interpretation_quality_label.clone(),
        interpretation_quality_value,
        narrative_components_label: interpretation.narrative_components_label.clone(),
        trend_label: interpretation.trend_label.clone(),
        trend_value,
        trend_confidence_label: interpretation.trend_confidence_label.clone(),
        trend_confidence_value,
        expectation_label: interpretation.expectation_label.clone(),
        expectation_value,
        expectation_confidence_label: interpretation.expectation_confidence_label.clone(),
        expectation_confidence_value,
        supply_label: interpretation.supply_label.clone(),
        supply_value,
        supply_confidence_label: interpretation.supply_confidence_label.clone(),
        supply_confidence_value,
        gravity_label: interpretation.gravity_label.clone(),
        gravity_value,
        gravity_confidence_label: interpretation.gravity_confidence_label.clone(),
        gravity_confidence_value,
        flow_label: interpretation.flow_label.clone(),
        flow_value,
        flow_confidence_label: interpretation.flow_confidence_label.clone(),
        flow_confidence_value,
        interpretation_label: interpretation.interpretation_label.clone(),
        interpretation_value,
        decision_explanation_label: interpretation.decision_explanation_label.clone(),
        decision_explanation_intro,
        decision_explanation_reasons,
        decision_explanation_conclusion,
        subjects_label: interpretation.subjects_label.clone(),
        subjects_value,
        boundary: interpretation.boundary.clone(),
        todays_explanation_label: interpretation.todays_explanation_label.clone(),
        primary_driver_label: interpretation.primary_driver_label.clone(),
        primary_driver_value: todays_explanation.primary_driver_value,
        primary_driver_confidence: todays_explanation.primary_driver_confidence,
        todays_explanation_navigation_value,
        secondary_drivers_label: interpretation.secondary_drivers_label.clone(),
        secondary_drivers_values: todays_explanation.secondary_drivers,
        ignored_today_label: interpretation.ignored_today_label.clone(),
        ignored_today_values: todays_explanation.ignored_today,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExplanationRole {
    Primary,
    Secondary,
    Ignored,
}

pub(crate) struct TodaysExplanation {
    pub primary_driver_value: String,
    pub primary_driver_confidence: String,
    pub secondary_drivers: Vec<String>,
    pub ignored_today: Vec<String>,
}

fn build_todays_explanation(
    signal: &InterpretationNarrativeSignal,
    signal_context: &crate::features::radar::interface::signal_context_read_model::SignalContextAssessment,
    language: Language,
) -> TodaysExplanation {
    let (macro_role, macro_text) = match signal_context.information_content {
        crate::features::radar::interface::presentation::SignalContextInformationContent::High => (
            ExplanationRole::Primary,
            macro_explanation_text(ExplanationRole::Primary, language),
        ),
        crate::features::radar::interface::presentation::SignalContextInformationContent::Medium => (
            ExplanationRole::Secondary,
            macro_explanation_text(ExplanationRole::Secondary, language),
        ),
        crate::features::radar::interface::presentation::SignalContextInformationContent::Low => (
            ExplanationRole::Ignored,
            macro_explanation_text(ExplanationRole::Ignored, language),
        ),
        crate::features::radar::interface::presentation::SignalContextInformationContent::Unknown => (
            ExplanationRole::Secondary,
            macro_unknown_explanation_text(language),
        ),
    };

    let is_trend_stable = signal.trend_available
        && (signal.trend_state == InterpretationTrendState::Stable
            || signal.trend_state == InterpretationTrendState::PostRallyConsolidation);
    let trend_role = if is_trend_stable {
        if macro_role != ExplanationRole::Primary {
            ExplanationRole::Primary
        } else {
            ExplanationRole::Secondary
        }
    } else {
        ExplanationRole::Ignored
    };

    let is_supply_pressure_present = signal.supply_available && signal.supply_pressure;
    let supply_role = if is_supply_pressure_present {
        if macro_role != ExplanationRole::Primary && trend_role != ExplanationRole::Primary {
            ExplanationRole::Primary
        } else {
            ExplanationRole::Secondary
        }
    } else {
        ExplanationRole::Ignored
    };

    let expectation_state = derive_expectation_state(signal, signal_context);
    let expectation_role = if expectation_state == ExpectationState::Pending {
        ExplanationRole::Secondary
    } else {
        ExplanationRole::Ignored
    };

    let has_clear_flow_direction =
        signal.flow_acceleration.is_some() && signal.flow_acceleration.unwrap().abs() > 0.05;
    let flow_role = if has_clear_flow_direction {
        ExplanationRole::Secondary
    } else {
        ExplanationRole::Ignored
    };

    let gravity_role = match signal.gravity_data_quality {
        InterpretationGravityDataQuality::Ready => ExplanationRole::Secondary,
        InterpretationGravityDataQuality::Partial => ExplanationRole::Secondary,
        InterpretationGravityDataQuality::Unavailable => ExplanationRole::Ignored,
    };

    let gray_rhino_role = if signal.gray_rhino_escalated {
        ExplanationRole::Secondary
    } else {
        ExplanationRole::Ignored
    };

    let (primary_name, primary_conf) = if macro_role == ExplanationRole::Primary {
        (
            macro_explanation_text(ExplanationRole::Primary, language),
            match signal_context.context_quality {
            crate::features::radar::interface::presentation::SignalContextQuality::High => {
                "HIGH".to_string()
            }
            crate::features::radar::interface::presentation::SignalContextQuality::Medium => {
                "MEDIUM".to_string()
            }
            crate::features::radar::interface::presentation::SignalContextQuality::Low => {
                "LOW".to_string()
            }
            crate::features::radar::interface::presentation::SignalContextQuality::Unavailable => {
                "UNAVAILABLE".to_string()
            }
            },
        )
    } else if trend_role == ExplanationRole::Primary {
        (
            trend_explanation_text(ExplanationRole::Primary, language),
            trend_confidence_value(signal),
        )
    } else if supply_role == ExplanationRole::Primary {
        (
            supply_explanation_text(ExplanationRole::Primary, language),
            supply_confidence_value(signal),
        )
    } else {
        (no_primary_driver_text(language), "LOW".to_string())
    };

    let mut secondary_drivers = Vec::new();
    if macro_role == ExplanationRole::Secondary {
        secondary_drivers.push(macro_text.clone());
    }
    if trend_role == ExplanationRole::Secondary {
        secondary_drivers.push(trend_explanation_text(ExplanationRole::Secondary, language));
    }
    if supply_role == ExplanationRole::Secondary {
        secondary_drivers.push(supply_explanation_text(
            ExplanationRole::Secondary,
            language,
        ));
    }
    if expectation_role == ExplanationRole::Secondary {
        secondary_drivers.push(expectation_explanation_text(
            ExplanationRole::Secondary,
            language,
        ));
    }
    if flow_role == ExplanationRole::Secondary {
        secondary_drivers.push(flow_explanation_text(ExplanationRole::Secondary, language));
    }
    if gravity_role == ExplanationRole::Secondary {
        secondary_drivers.push(gravity_explanation_text(
            gravity_role,
            signal.gravity_data_quality,
            language,
        ));
    }
    if gray_rhino_role == ExplanationRole::Secondary {
        secondary_drivers.push(gray_rhino_explanation_text(
            ExplanationRole::Secondary,
            language,
        ));
    }

    let mut ignored_today = Vec::new();
    if macro_role == ExplanationRole::Ignored {
        ignored_today.push(macro_text);
    }
    if trend_role == ExplanationRole::Ignored {
        ignored_today.push(trend_explanation_text(ExplanationRole::Ignored, language));
    }
    if supply_role == ExplanationRole::Ignored {
        ignored_today.push(supply_explanation_text(ExplanationRole::Ignored, language));
    }
    if expectation_role == ExplanationRole::Ignored {
        ignored_today.push(expectation_explanation_text(
            ExplanationRole::Ignored,
            language,
        ));
    }
    if flow_role == ExplanationRole::Ignored {
        ignored_today.push(flow_explanation_text(ExplanationRole::Ignored, language));
    }
    if gravity_role == ExplanationRole::Ignored {
        ignored_today.push(gravity_explanation_text(
            gravity_role,
            signal.gravity_data_quality,
            language,
        ));
    }
    if gray_rhino_role == ExplanationRole::Ignored {
        ignored_today.push(gray_rhino_explanation_text(
            ExplanationRole::Ignored,
            language,
        ));
    }

    TodaysExplanation {
        primary_driver_value: primary_name,
        primary_driver_confidence: primary_conf,
        secondary_drivers,
        ignored_today,
    }
}

fn no_primary_driver_text(lang: Language) -> String {
    match lang {
        Language::ZhCn => "今日未识别到主导性主要驱动因子。".to_string(),
        Language::EnUs => "No dominant primary driver today.".to_string(),
        Language::JaJp => "本日は支配的な主要ドライバーは検出されていません。".to_string(),
    }
}

fn macro_explanation_text(role: ExplanationRole, lang: Language) -> String {
    match role {
        ExplanationRole::Primary => match lang {
            Language::ZhCn => "高信息量的宏观事件（Macro Event）。".to_string(),
            Language::EnUs => "High-information macro event today.".to_string(),
            Language::JaJp => "本日の主要なマクロイベント。".to_string(),
        },
        ExplanationRole::Secondary => match lang {
            Language::ZhCn => "宏观事件信息量中等，起到辅助解释作用。".to_string(),
            Language::EnUs => {
                "Macro event information content is medium, serving as a secondary driver."
                    .to_string()
            }
            Language::JaJp => {
                "マクロイベントの情報量は中程度であり、本日は補助的な説明要因です。".to_string()
            }
        },
        ExplanationRole::Ignored => match lang {
            Language::ZhCn => "宏观事件信息量较低或未知，今日忽略。".to_string(),
            Language::EnUs => {
                "Macro event has low or unknown information content, and is ignored today."
                    .to_string()
            }
            Language::JaJp => {
                "マクロイベントの情報量は低いか未知数であり、本日は無視されます。".to_string()
            }
        },
    }
}

fn macro_unknown_explanation_text(lang: Language) -> String {
    match lang {
        Language::ZhCn => "宏观信息暂无法确认，解释置信度下降，但仍保留为补充上下文。".to_string(),
        Language::EnUs => "Macro information is not confirmed yet, so explanation confidence is lower, but it is still kept as supporting context.".to_string(),
        Language::JaJp => "マクロ情報はまだ確認できず、説明の信頼度は下がるが、補助コンテキストとして保持される。".to_string(),
    }
}

fn trend_explanation_text(role: ExplanationRole, lang: Language) -> String {
    match role {
        ExplanationRole::Primary => match lang {
            Language::ZhCn => "趋势延续".to_string(),
            Language::EnUs => "Trend continuation.".to_string(),
            Language::JaJp => "トレンド継続".to_string(),
        },
        ExplanationRole::Secondary => match lang {
            Language::ZhCn => "趋势结构虽稳定，但今天仅起到辅助解释作用。".to_string(),
            Language::EnUs => "Trend stable, secondary.".to_string(),
            Language::JaJp => "トレンド安定、補助".to_string(),
        },
        ExplanationRole::Ignored => match lang {
            Language::ZhCn => "趋势结构偏弱或不可用，今日忽略。".to_string(),
            Language::EnUs => {
                "Trend is weak or unavailable, and is not a primary driver today.".to_string()
            }
            Language::JaJp => {
                "トレンド構造は弱含みまたは利用不可であり、本日は無視されます。".to_string()
            }
        },
    }
}

fn supply_explanation_text(role: ExplanationRole, lang: Language) -> String {
    match role {
        ExplanationRole::Primary => match lang {
            Language::ZhCn => "供给压力".to_string(),
            Language::EnUs => "Supply pressure.".to_string(),
            Language::JaJp => "供給圧力".to_string(),
        },
        ExplanationRole::Secondary => match lang {
            Language::ZhCn => "供给压力，次要".to_string(),
            Language::EnUs => "Supply pressure, secondary.".to_string(),
            Language::JaJp => "供給圧力、補助".to_string(),
        },
        ExplanationRole::Ignored => match lang {
            Language::ZhCn => "供给忽略".to_string(),
            Language::EnUs => "Supply ignored.".to_string(),
            Language::JaJp => "供給無視".to_string(),
        },
    }
}

fn expectation_explanation_text(role: ExplanationRole, lang: Language) -> String {
    match role {
        ExplanationRole::Primary => match lang {
            Language::ZhCn => "市场一致预期变化为主驱动。".to_string(),
            Language::EnUs => {
                "Market consensus expectation acts as the primary driver.".to_string()
            }
            Language::JaJp => "市場コンセンサス期待が主要なドライバーです。".to_string(),
        },
        ExplanationRole::Secondary => match lang {
            Language::ZhCn => "处于事件等待或业绩公布前夕的预期悬停状态。".to_string(),
            Language::EnUs => {
                "Expectation is in pending state awaiting event outcomes.".to_string()
            }
            Language::JaJp => "イベント待ちまたは決算発表前夕の期待待機状態です。".to_string(),
        },
        ExplanationRole::Ignored => match lang {
            Language::ZhCn => "预期质量偏低或已定价，今日忽略。".to_string(),
            Language::EnUs => {
                "Expectation quality is low or priced in, and is ignored today.".to_string()
            }
            Language::JaJp => {
                "期待の質が低いか織り込み済みであり、本日は説明要因になりません。".to_string()
            }
        },
    }
}

fn flow_explanation_text(role: ExplanationRole, lang: Language) -> String {
    match role {
        ExplanationRole::Primary => match lang {
            Language::ZhCn => "资金流向".to_string(),
            Language::EnUs => "Flow direction.".to_string(),
            Language::JaJp => "資金フロー".to_string(),
        },
        ExplanationRole::Secondary => match lang {
            Language::ZhCn => "资金流向，次要".to_string(),
            Language::EnUs => "Flow secondary.".to_string(),
            Language::JaJp => "資金フロー、補助".to_string(),
        },
        ExplanationRole::Ignored => match lang {
            Language::ZhCn => "资金流向忽略".to_string(),
            Language::EnUs => "Flow ignored.".to_string(),
            Language::JaJp => "資金フロー無視".to_string(),
        },
    }
}

fn gravity_explanation_text(
    role: ExplanationRole,
    dq: InterpretationGravityDataQuality,
    lang: Language,
) -> String {
    match role {
        ExplanationRole::Primary => match lang {
            Language::ZhCn => "估值引力".to_string(),
            Language::EnUs => "Gravity.".to_string(),
            Language::JaJp => "重力".to_string(),
        },
        ExplanationRole::Secondary => match lang {
            Language::ZhCn => "估值引力作为价值边界起到辅助解释作用。".to_string(),
            Language::EnUs => "Gravity is relevant as a valuation boundary.".to_string(),
            Language::JaJp => {
                "バリュエーション重力は価値の境界として補助的な説明になります。".to_string()
            }
        },
        ExplanationRole::Ignored => {
            if dq == InterpretationGravityDataQuality::Unavailable {
                match lang {
                    Language::ZhCn => "估值引力不可用".to_string(),
                    Language::EnUs => "Gravity unavailable.".to_string(),
                    Language::JaJp => "重力利用不可".to_string(),
                }
            } else {
                match lang {
                    Language::ZhCn => "估值引力忽略".to_string(),
                    Language::EnUs => "Gravity ignored.".to_string(),
                    Language::JaJp => "重力無視".to_string(),
                }
            }
        }
    }
}

fn gray_rhino_explanation_text(role: ExplanationRole, lang: Language) -> String {
    match role {
        ExplanationRole::Primary => match lang {
            Language::ZhCn => "灰犀牛升级".to_string(),
            Language::EnUs => "Gray Rhino.".to_string(),
            Language::JaJp => "灰犀牛".to_string(),
        },
        ExplanationRole::Secondary => match lang {
            Language::ZhCn => "灰犀牛次要".to_string(),
            Language::EnUs => "Gray Rhino secondary.".to_string(),
            Language::JaJp => "灰犀牛補助".to_string(),
        },
        ExplanationRole::Ignored => match lang {
            Language::ZhCn => "灰犀牛忽略".to_string(),
            Language::EnUs => "Gray Rhino ignored.".to_string(),
            Language::JaJp => "灰犀牛無視".to_string(),
        },
    }
}

fn trend_confidence_value(signal: &InterpretationNarrativeSignal) -> String {
    match signal.trend_state {
        InterpretationTrendState::Weak => "MEDIUM".to_string(),
        InterpretationTrendState::Stable => "HIGH".to_string(),
        InterpretationTrendState::PostRallyConsolidation => "MEDIUM".to_string(),
    }
}

fn expectation_confidence_value(signal: &InterpretationNarrativeSignal) -> String {
    match signal.expectation_quality {
        InterpretationExpectationQuality::High => "HIGH".to_string(),
        InterpretationExpectationQuality::Medium => "MEDIUM".to_string(),
        InterpretationExpectationQuality::Low => "LOW".to_string(),
        InterpretationExpectationQuality::Unavailable => "UNAVAILABLE".to_string(),
    }
}

fn supply_confidence_value(signal: &InterpretationNarrativeSignal) -> String {
    if signal.supply_available {
        "HIGH".to_string()
    } else {
        "UNAVAILABLE".to_string()
    }
}

fn gravity_confidence_value(signal: &InterpretationNarrativeSignal) -> String {
    match signal.gravity_data_quality {
        InterpretationGravityDataQuality::Ready => "HIGH".to_string(),
        InterpretationGravityDataQuality::Partial => "MEDIUM".to_string(),
        InterpretationGravityDataQuality::Unavailable => "UNAVAILABLE".to_string(),
    }
}

fn flow_confidence_value(signal: &InterpretationNarrativeSignal) -> String {
    match signal.flow_acceleration {
        None => "UNAVAILABLE".to_string(),
        Some(flow) if flow.abs() > 0.05 => "HIGH".to_string(),
        Some(_) => "MEDIUM".to_string(),
    }
}

fn observation_health_value(
    signal: &InterpretationNarrativeSignal,
    signal_context: &crate::features::radar::interface::signal_context_read_model::SignalContextAssessment,
) -> String {
    let mut rows = Vec::new();
    rows.push(format!(
        "Trend: {}",
        if signal.trend_available {
            "Healthy"
        } else {
            "Unavailable"
        }
    ));
    rows.push(format!(
        "Expectation: {}",
        if signal.expectation_quality == InterpretationExpectationQuality::Unavailable {
            "Unavailable"
        } else {
            "Healthy"
        }
    ));
    rows.push(format!(
        "Gravity: {}",
        match signal.gravity_data_quality {
            InterpretationGravityDataQuality::Ready => "Healthy",
            InterpretationGravityDataQuality::Partial => "Partial",
            InterpretationGravityDataQuality::Unavailable => "Unavailable",
        }
    ));
    rows.push(format!(
        "Supply: {}",
        if signal.supply_available {
            "Healthy"
        } else {
            "Unavailable"
        }
    ));
    rows.push(format!(
        "Flow: {}",
        if signal.flow_acceleration.is_some() {
            "Neutral"
        } else {
            "Unavailable"
        }
    ));
    rows.push(format!(
        "Macro Context: {}",
        match signal_context.source_health {
            crate::features::research::interface::macro_event_observation::MacroEventSourceHealth::Succeeded => "Healthy",
            crate::features::research::interface::macro_event_observation::MacroEventSourceHealth::Partial => "Partial",
            crate::features::research::interface::macro_event_observation::MacroEventSourceHealth::Unavailable => "Unavailable",
        }
    ));
    rows.join("\n")
}

fn interpretation_quality_value(
    signal: &InterpretationNarrativeSignal,
    signal_context: &crate::features::radar::interface::signal_context_read_model::SignalContextAssessment,
) -> String {
    let mut score = 0;
    if signal.trend_available {
        score += 1;
    }
    if signal.expectation_quality != InterpretationExpectationQuality::Unavailable {
        score += 1;
    }
    if signal.gravity_data_quality != InterpretationGravityDataQuality::Unavailable {
        score += 1;
    }
    if signal.supply_available {
        score += 1;
    }
    if signal.flow_acceleration.is_some() {
        score += 1;
    }
    if signal_context.source_health != crate::features::research::interface::macro_event_observation::MacroEventSourceHealth::Unavailable {
        score += 1;
    }
    if signal_context.information_content
        != crate::features::radar::interface::presentation::SignalContextInformationContent::Unknown
    {
        score += 1;
    }

    let mut quality = if score >= 6
        && signal_context.source_health
            == crate::features::research::interface::macro_event_observation::MacroEventSourceHealth::Succeeded
    {
        InterpretationQuality::High
    } else if score >= 4 {
        InterpretationQuality::Medium
    } else {
        InterpretationQuality::Low
    };

    if signal.gravity_data_quality == InterpretationGravityDataQuality::Unavailable
        && quality == InterpretationQuality::High
    {
        quality = InterpretationQuality::Medium;
    }

    match quality {
        InterpretationQuality::High => "HIGH".to_string(),
        InterpretationQuality::Medium => "MEDIUM".to_string(),
        InterpretationQuality::Low => "LOW".to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectationState {
    Unavailable,
    Pending,
    Observing,
    Low,
    Unknown,
}

fn derive_expectation_state(
    _signal: &InterpretationNarrativeSignal,
    signal_context: &crate::features::radar::interface::signal_context_read_model::SignalContextAssessment,
) -> ExpectationState {
    if signal_context.information_content
        == crate::features::radar::interface::presentation::SignalContextInformationContent::Low
    {
        return ExpectationState::Low;
    }

    if signal_context.source_health == crate::features::research::interface::macro_event_observation::MacroEventSourceHealth::Unavailable {
        return ExpectationState::Unavailable;
    }

    if signal_context.primary_context == crate::features::radar::interface::presentation::SignalContextPrimaryContext::PreEarningsWaiting
        || signal_context.primary_context == crate::features::radar::interface::presentation::SignalContextPrimaryContext::MajorEventWaiting
    {
        return ExpectationState::Pending;
    }

    if signal_context.information_content
        == crate::features::radar::interface::presentation::SignalContextInformationContent::Unknown
    {
        return ExpectationState::Unknown;
    }

    ExpectationState::Observing
}

fn expectation_lifecycle_value(
    signal: &InterpretationNarrativeSignal,
    signal_context: &crate::features::radar::interface::signal_context_read_model::SignalContextAssessment,
    language: Language,
) -> String {
    let _ = language;
    let state = derive_expectation_state(signal, signal_context);
    match state {
        ExpectationState::Unavailable => "UNAVAILABLE".to_string(),
        ExpectationState::Pending => "PENDING".to_string(),
        ExpectationState::Observing => "OBSERVING".to_string(),
        ExpectationState::Low => "LOW".to_string(),
        ExpectationState::Unknown => "UNKNOWN".to_string(),
    }
}

fn expectation_next_observation_value(
    signal: &InterpretationNarrativeSignal,
    signal_context: &crate::features::radar::interface::signal_context_read_model::SignalContextAssessment,
    language: Language,
) -> String {
    let state = derive_expectation_state(signal, signal_context);
    match state {
        ExpectationState::Unavailable => match language {
            Language::ZhCn => "官方来源当前不可用；待来源恢复后再确认是否存在事件".to_string(),
            Language::EnUs => "Official source is currently unavailable; verify if events exist once the source is restored.".to_string(),
            Language::JaJp => "公式ソースは現在利用できません。ソース復旧後にイベントが存在するかどうかを再確認します。".to_string(),
        },
        ExpectationState::Pending => match language {
            Language::ZhCn => "等待官方公布；公布后对比 Expected / Actual、计算 Surprise、更新 Narrative".to_string(),
            Language::EnUs => "Waiting for the official release; comparison between Expected / Actual, calculation of Surprise, and Narrative updates will follow publication.".to_string(),
            Language::JaJp => "公式発表を待機中；公表後は Expected / Actual を比較し、Surprise を計算して Narrative を更新する。".to_string(),
        },
        ExpectationState::Observing => match language {
            Language::ZhCn => "继续观察后续价格/预期修正/资金反应".to_string(),
            Language::EnUs => "Continue observing subsequent price actions, expectation adjustments, and capital flow responses.".to_string(),
            Language::JaJp => "その後の価格動向、予想の修正、資金の反応を引き続き観察する。".to_string(),
        },
        ExpectationState::Low => match language {
            Language::ZhCn => "当前无高信息量事件，继续等待官方日历更新".to_string(),
            Language::EnUs => "No high-information events currently; continue waiting for the official calendar update.".to_string(),
            Language::JaJp => "現在、高情報量のイベントはありません。公式カレンダーの更新を引き続き待機します。".to_string(),
        },
        ExpectationState::Unknown => match language {
            Language::ZhCn => "无法确认是否存在事件；待来源更新或数据完整后再确认".to_string(),
            Language::EnUs => "Unable to confirm if events exist; pending source updates or data completion.".to_string(),
            Language::JaJp => "イベントの有無を確認できません。ソース更新またはデータ完了を待ちます。".to_string(),
        },
    }
}

pub(crate) fn derive_expectation_quality(
    snapshot: &ExpectationLayerSnapshot,
) -> (
    InterpretationExpectationQuality,
    InterpretationExpectationQualityReason,
) {
    if snapshot.observations.is_empty() {
        return (
            InterpretationExpectationQuality::Unavailable,
            InterpretationExpectationQualityReason::SystemUnavailable,
        );
    }

    let available_confidences = snapshot
        .observations
        .iter()
        .filter(|observation| {
            observation.source_health
                == crate::features::research::domain::expectation::SourceHealth::Succeeded
        })
        .filter_map(|observation| observation.confidence)
        .collect::<Vec<_>>();
    if available_confidences.is_empty() {
        let reason = if snapshot.observations.iter().any(|observation| {
            observation.source_health
                == crate::features::research::domain::expectation::SourceHealth::Succeeded
        }) {
            InterpretationExpectationQualityReason::MarketConsensusUnavailable
        } else {
            InterpretationExpectationQualityReason::SystemUnavailable
        };
        return (InterpretationExpectationQuality::Unavailable, reason);
    }

    let average_confidence =
        available_confidences.iter().sum::<f64>() / available_confidences.len() as f64;
    let all_succeeded = snapshot.observations.iter().all(|observation| {
        observation.source_health
            == crate::features::research::domain::expectation::SourceHealth::Succeeded
    });

    if all_succeeded && average_confidence >= 0.80 {
        (
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
        )
    } else if average_confidence >= 0.60 {
        (
            InterpretationExpectationQuality::Medium,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
        )
    } else {
        (
            InterpretationExpectationQuality::Low,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
        )
    }
}

pub(crate) fn derive_gravity_data_quality(
    observation: &ValuationGravityObservation,
) -> InterpretationGravityDataQuality {
    if observation.snapshot.assets.is_empty() {
        return InterpretationGravityDataQuality::Unavailable;
    }

    let available_assets = observation
        .snapshot
        .assets
        .iter()
        .filter(|asset| asset.source_health != crate::features::research::domain::valuation_gravity::ValuationSourceHealth::Unavailable)
        .count();
    if available_assets == 0 {
        return InterpretationGravityDataQuality::Unavailable;
    }

    if available_assets != observation.snapshot.assets.len()
        || observation.persistence_health == ValuationPersistenceHealth::Missing
        || observation.persistence_reason == ValuationPersistenceReason::HistoricalSnapshotMissing
    {
        InterpretationGravityDataQuality::Partial
    } else {
        InterpretationGravityDataQuality::Ready
    }
}

pub(crate) fn derive_gravity_data_quality_reason(
    observation: &ValuationGravityObservation,
) -> InterpretationGravityDataQualityReason {
    if observation.persistence_health == ValuationPersistenceHealth::Missing
        || observation.persistence_reason == ValuationPersistenceReason::HistoricalSnapshotMissing
    {
        return InterpretationGravityDataQualityReason::HistoricalSnapshotMissing;
    }

    if observation
        .snapshot
        .assets
        .iter()
        .any(|asset| matches!(
            asset.quality_reason,
            crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::HistoricalSnapshotMissing
                | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::HistoricalSnapshotReadFailure
        ))
    {
        return InterpretationGravityDataQualityReason::HistoricalSnapshotMissing;
    }

    if observation.snapshot.assets.iter().any(|asset| matches!(
        asset.quality_reason,
        crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::InsufficientEvidence
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::PriceTargetConsensus
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::MarketMultipleFallback
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::RecommendationFallback
    )) {
        return InterpretationGravityDataQualityReason::ConsensusUnavailable;
    }

    if observation.snapshot.assets.iter().any(|asset| matches!(
        asset.quality_reason,
        crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::MissingCredential
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::EntitlementDenied
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::ProviderFailure
    )) {
        return InterpretationGravityDataQualityReason::ProviderUnavailable;
    }

    InterpretationGravityDataQualityReason::SourceTemporarilyUnavailable
}

fn trend_component_value(signal: &InterpretationNarrativeSignal, language: Language) -> String {
    if !signal.trend_available {
        return unavailable_component("Trend", trend_unavailable_reason(language), language);
    }

    match signal.trend_state {
        InterpretationTrendState::Weak => trend_weak_text(language),
        InterpretationTrendState::Stable => trend_stable_text(language),
        InterpretationTrendState::PostRallyConsolidation => trend_post_rally_text(language),
    }
}

fn expectation_component_value(
    signal: &InterpretationNarrativeSignal,
    language: Language,
) -> String {
    match signal.expectation_quality {
        InterpretationExpectationQuality::High => expectation_high_text(language),
        InterpretationExpectationQuality::Medium => expectation_medium_text(language),
        InterpretationExpectationQuality::Low => expectation_low_text(language),
        InterpretationExpectationQuality::Unavailable => unavailable_component(
            "Expectation",
            expectation_unavailable_reason(signal.expectation_quality_reason, language),
            language,
        ),
    }
}

fn supply_component_value(signal: &InterpretationNarrativeSignal, language: Language) -> String {
    if !signal.supply_available {
        return unavailable_component("Supply", supply_unavailable_reason(language), language);
    }

    if signal.supply_pressure {
        supply_pressure_text(language)
    } else {
        supply_clear_text(language)
    }
}

fn gravity_component_value(signal: &InterpretationNarrativeSignal, language: Language) -> String {
    match signal.gravity_data_quality {
        InterpretationGravityDataQuality::Unavailable => unavailable_component(
            "Gravity",
            gravity_unavailable_reason(signal.gravity_data_quality_reason, language),
            language,
        ),
        InterpretationGravityDataQuality::Partial => {
            if let Some(status) = signal.gravity_status {
                gravity_status_text(status, language, true)
            } else {
                gravity_partial_without_status_text(language)
            }
        }
        InterpretationGravityDataQuality::Ready => {
            if let Some(status) = signal.gravity_status {
                gravity_status_text(status, language, false)
            } else {
                gravity_ready_without_status_text(language)
            }
        }
    }
}

fn flow_component_value(signal: &InterpretationNarrativeSignal, language: Language) -> String {
    let Some(flow) = signal.flow_acceleration else {
        return unavailable_component("Flow", flow_unavailable_reason(language), language);
    };

    if flow > 0.05 {
        flow_supporting_text(language)
    } else if flow < -0.05 {
        flow_deteriorating_text(language)
    } else {
        flow_neutral_text(language)
    }
}

fn compose_interpretation_value(
    signal_context: &crate::features::radar::interface::signal_context_read_model::SignalContextAssessment,
    _signal: &InterpretationNarrativeSignal,
    language: Language,
) -> String {
    let content_summary = match signal_context.information_content {
        crate::features::radar::interface::presentation::SignalContextInformationContent::High => {
            match language {
                Language::ZhCn => "今天为高信息量。".to_string(),
                Language::EnUs => "High information today.".to_string(),
                Language::JaJp => "本日は高情報量。".to_string(),
            }
        }
        crate::features::radar::interface::presentation::SignalContextInformationContent::Medium => {
            match language {
                Language::ZhCn => "今天为中等信息量。".to_string(),
                Language::EnUs => "Medium information today.".to_string(),
                Language::JaJp => "本日は中情報量。".to_string(),
            }
        }
        crate::features::radar::interface::presentation::SignalContextInformationContent::Low => {
            match language {
                Language::ZhCn => "基于可用来源，今天未识别到高信息量事件。".to_string(),
                Language::EnUs => "No high-information event identified from available sources.".to_string(),
                Language::JaJp => "利用可能なソースから高情報量イベントは識別されていない。".to_string(),
            }
        }
        crate::features::radar::interface::presentation::SignalContextInformationContent::Unknown => {
            match language {
                Language::ZhCn => "信号上下文暂时不可用。".to_string(),
                Language::EnUs => "Signal context unavailable today.".to_string(),
                Language::JaJp => "シグナルコンテキストは一時的に利用不可。".to_string(),
            }
        }
    };

    format!(
        "{} {}",
        content_summary,
        todays_explanation_navigation_text(language)
    )
}

fn todays_explanation_navigation_text(language: Language) -> String {
    match language {
        Language::ZhCn => "主叙事见 Market Interpretation。".to_string(),
        Language::EnUs => "See Market Interpretation for the main narrative.".to_string(),
        Language::JaJp => "主叙事は Market Interpretation を参照。".to_string(),
    }
}

fn decision_explanation_values(
    decision_summary: Option<
        &crate::features::radar::interface::presentation::DecisionSummaryViewModel,
    >,
    language: Language,
    interpretation: &crate::features::shared::interface::i18n::InterpretationDictionary,
) -> (String, Vec<String>, String) {
    let Some(summary) = decision_summary else {
        return (
            interpretation.decision_explanation_intro.clone(),
            vec![unavailable_component(
                "Decision",
                decision_explanation_unavailable_reason(language),
                language,
            )],
            interpretation
                .decision_explanation_fact_safe_conclusion
                .clone(),
        );
    };

    if summary.is_no_trade {
        let reasons = if summary.readiness_reasons.is_empty() {
            vec![summary.summary.clone()]
        } else {
            summary.readiness_reasons.clone()
        };
        (
            interpretation.decision_explanation_intro.clone(),
            reasons,
            interpretation
                .decision_explanation_fact_safe_conclusion
                .clone(),
        )
    } else {
        (summary.summary.clone(), Vec::new(), String::new())
    }
}

fn unavailable_component(layer: &str, reason: String, language: Language) -> String {
    match language {
        Language::ZhCn => format!("{layer}: UNAVAILABLE - {reason}"),
        Language::EnUs => format!("{layer}: UNAVAILABLE - {reason}"),
        Language::JaJp => format!("{layer}: UNAVAILABLE - {reason}"),
    }
}

fn trend_unavailable_reason(language: Language) -> String {
    match language {
        Language::ZhCn => "缺少状态转移证据".to_string(),
        Language::EnUs => "transition evidence unavailable".to_string(),
        Language::JaJp => "状態遷移証拠がない".to_string(),
    }
}

fn expectation_unavailable_reason(
    reason: InterpretationExpectationQualityReason,
    language: Language,
) -> String {
    match (reason, language) {
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::ZhCn) => {
            "市场共识已取得，但当前质量不满足输出".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::EnUs) => {
            "market consensus available, but current quality is not sufficient for output"
                .to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::JaJp) => {
            "市場コンセンサスはあるが、現状の品質では出力に足りない".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::ZhCn) => {
            "市场没有一致预期".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::EnUs) => {
            "market consensus unavailable".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::JaJp) => {
            "市場コンセンサスなし".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::ZhCn) => {
            "系统未取得预期数据".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::EnUs) => {
            "system unavailable".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::JaJp) => {
            "システム未取得".to_string()
        }
    }
}

fn supply_unavailable_reason(language: Language) -> String {
    match language {
        Language::ZhCn => "缺少供给观测".to_string(),
        Language::EnUs => "supply observation unavailable".to_string(),
        Language::JaJp => "供給観測がない".to_string(),
    }
}

fn gravity_unavailable_reason(
    reason: InterpretationGravityDataQualityReason,
    language: Language,
) -> String {
    match (reason, language) {
        (InterpretationGravityDataQualityReason::ProviderUnavailable, Language::ZhCn) => {
            "估值数据提供方不可用".to_string()
        }
        (InterpretationGravityDataQualityReason::ProviderUnavailable, Language::EnUs) => {
            "provider unavailable".to_string()
        }
        (InterpretationGravityDataQualityReason::ProviderUnavailable, Language::JaJp) => {
            "プロバイダ利用不可".to_string()
        }
        (InterpretationGravityDataQualityReason::HistoricalSnapshotMissing, Language::ZhCn) => {
            "历史快照缺失".to_string()
        }
        (InterpretationGravityDataQualityReason::HistoricalSnapshotMissing, Language::EnUs) => {
            "historical snapshot missing".to_string()
        }
        (InterpretationGravityDataQualityReason::HistoricalSnapshotMissing, Language::JaJp) => {
            "履歴スナップショットがない".to_string()
        }
        (InterpretationGravityDataQualityReason::ConsensusUnavailable, Language::ZhCn) => {
            "共识数据不可用".to_string()
        }
        (InterpretationGravityDataQualityReason::ConsensusUnavailable, Language::EnUs) => {
            "consensus unavailable".to_string()
        }
        (InterpretationGravityDataQualityReason::ConsensusUnavailable, Language::JaJp) => {
            "コンセンサスがない".to_string()
        }
        (InterpretationGravityDataQualityReason::SourceTemporarilyUnavailable, Language::ZhCn) => {
            "数据源临时不可用".to_string()
        }
        (InterpretationGravityDataQualityReason::SourceTemporarilyUnavailable, Language::EnUs) => {
            "source temporarily unavailable".to_string()
        }
        (InterpretationGravityDataQualityReason::SourceTemporarilyUnavailable, Language::JaJp) => {
            "データソースが一時的に利用不可".to_string()
        }
    }
}

fn flow_unavailable_reason(language: Language) -> String {
    match language {
        Language::ZhCn => "资金流数据不可用".to_string(),
        Language::EnUs => "flow data unavailable".to_string(),
        Language::JaJp => "資金フローが未取得".to_string(),
    }
}

fn decision_explanation_unavailable_reason(language: Language) -> String {
    match language {
        Language::ZhCn => "没有可用的状态机结果".to_string(),
        Language::EnUs => "state machine result unavailable".to_string(),
        Language::JaJp => "状態機械結果がない".to_string(),
    }
}

fn trend_weak_text(language: Language) -> String {
    match language {
        Language::ZhCn => "价格结构尚未形成一致扩散。".to_string(),
        Language::EnUs => "Price structure has not yet formed a consistent diffusion.".to_string(),
        Language::JaJp => "価格構造はまだ一貫した拡散に入っていない。".to_string(),
    }
}

fn trend_stable_text(language: Language) -> String {
    match language {
        Language::ZhCn => "价格结构保持稳定。".to_string(),
        Language::EnUs => "Price structure remains stable.".to_string(),
        Language::JaJp => "価格構造は安定している。".to_string(),
    }
}

fn trend_post_rally_text(language: Language) -> String {
    match language {
        Language::ZhCn => "价格正在经历上升后的整理。".to_string(),
        Language::EnUs => "Price is in post-rally consolidation.".to_string(),
        Language::JaJp => "価格は上昇後の整理局面にある。".to_string(),
    }
}

fn expectation_high_text(language: Language) -> String {
    match language {
        Language::ZhCn => "市场预期清晰，且系统已取得一致预期数据。".to_string(),
        Language::EnUs => {
            "Market expectation is clear and the system has consensus data.".to_string()
        }
        Language::JaJp => {
            "市場期待は明確で、システムもコンセンサスデータを取得している。".to_string()
        }
    }
}

fn expectation_medium_text(language: Language) -> String {
    match language {
        Language::ZhCn => "市场有预期，但粒度较粗或部分信息缺失。".to_string(),
        Language::EnUs => {
            "The market has an expectation, but the granularity is coarse or partially missing."
                .to_string()
        }
        Language::JaJp => "市場期待はあるが、粒度が粗いか一部情報が欠けている。".to_string(),
    }
}

fn expectation_low_text(language: Language) -> String {
    match language {
        Language::ZhCn => "市场预期较弱或碎片化，结论仍不稳定。".to_string(),
        Language::EnUs => {
            "Market expectation is weak or fragmented, so the conclusion remains unstable."
                .to_string()
        }
        Language::JaJp => "市場期待は弱いか断片的で、結論はまだ不安定である。".to_string(),
    }
}

fn supply_pressure_text(language: Language) -> String {
    match language {
        Language::ZhCn => "新增供给压力仍需市场吸收。".to_string(),
        Language::EnUs => {
            "New supply pressure still needs to be absorbed by the market.".to_string()
        }
        Language::JaJp => "新しい供給圧力はまだ市場に吸収される必要がある。".to_string(),
    }
}

fn supply_clear_text(language: Language) -> String {
    match language {
        Language::ZhCn => "暂无新增供给风险。".to_string(),
        Language::EnUs => "No new supply risk is visible yet.".to_string(),
        Language::JaJp => "新しい供給リスクはまだ見えていない。".to_string(),
    }
}

fn gravity_status_text(status: GravityStatus, language: Language, partial: bool) -> String {
    let prefix = if partial {
        match language {
            Language::ZhCn => "估值数据部分可用，",
            Language::EnUs => "Valuation data is partial, ",
            Language::JaJp => "バリュエーションデータは一部のみ有効で、",
        }
    } else {
        ""
    };

    let body = match status {
        GravityStatus::DeepUndervalued => match language {
            Language::ZhCn => "价格明显低于价值锚。".to_string(),
            Language::EnUs => "Price is materially below the value anchor.".to_string(),
            Language::JaJp => "価格は価値アンカーよりかなり低い。".to_string(),
        },
        GravityStatus::Undervalued => match language {
            Language::ZhCn => "价格低于价值锚。".to_string(),
            Language::EnUs => "Price is below the value anchor.".to_string(),
            Language::JaJp => "価格は価値アンカーを下回っている。".to_string(),
        },
        GravityStatus::Fair => match language {
            Language::ZhCn => "价格大致处于价值锚附近。".to_string(),
            Language::EnUs => "Price is broadly near the value anchor.".to_string(),
            Language::JaJp => "価格は概ね価値アンカー付近にある。".to_string(),
        },
        GravityStatus::SlightlyExpensive => match language {
            Language::ZhCn => "价格略高于价值锚。".to_string(),
            Language::EnUs => "Price is slightly above the value anchor.".to_string(),
            Language::JaJp => "価格は価値アンカーよりやや高い。".to_string(),
        },
        GravityStatus::Expensive => match language {
            Language::ZhCn => "价格高于价值锚。".to_string(),
            Language::EnUs => "Price is above the value anchor.".to_string(),
            Language::JaJp => "価格は価値アンカーを上回っている。".to_string(),
        },
        GravityStatus::VeryExpensive => match language {
            Language::ZhCn => "价格显著高于价值锚。".to_string(),
            Language::EnUs => "Price is materially above the value anchor.".to_string(),
            Language::JaJp => "価格は価値アンカーを大きく上回っている。".to_string(),
        },
    };

    format!("{prefix}{body}")
}

fn gravity_partial_without_status_text(language: Language) -> String {
    match language {
        Language::ZhCn => "估值数据部分可用，但价格相对价值的方向仍未落定。".to_string(),
        Language::EnUs => {
            "Valuation data is partial, but the direction relative to value is still undecided."
                .to_string()
        }
        Language::JaJp => {
            "バリュエーションデータは一部のみ有効だが、価値に対する方向はまだ未確定である。"
                .to_string()
        }
    }
}

fn gravity_ready_without_status_text(language: Language) -> String {
    match language {
        Language::ZhCn => "估值数据可用，但尚未形成明确的价值锚判断。".to_string(),
        Language::EnUs => {
            "Valuation data is ready, but no clear value-anchor judgment has been formed yet."
                .to_string()
        }
        Language::JaJp => {
            "バリュエーションデータは有効だが、明確な価値アンカー判断はまだない。".to_string()
        }
    }
}

fn flow_supporting_text(language: Language) -> String {
    match language {
        Language::ZhCn => "资金流正在支持当前趋势。".to_string(),
        Language::EnUs => "Flow is supporting the current trend.".to_string(),
        Language::JaJp => "資金フローは現在のトレンドを支えている。".to_string(),
    }
}

fn flow_deteriorating_text(language: Language) -> String {
    match language {
        Language::ZhCn => "资金流正在削弱当前趋势。".to_string(),
        Language::EnUs => "Flow is weakening the current trend.".to_string(),
        Language::JaJp => "資金フローは現在のトレンドを弱めている。".to_string(),
    }
}

fn flow_neutral_text(language: Language) -> String {
    match language {
        Language::ZhCn => "资金流处于中性。".to_string(),
        Language::EnUs => "Flow is neutral.".to_string(),
        Language::JaJp => "資金フローは中立である。".to_string(),
    }
}

pub(crate) fn derive_trend_state(
    transition_evidence: Option<&StateTransitionViewModel>,
) -> InterpretationTrendState {
    let Some(evidence) = transition_evidence else {
        return InterpretationTrendState::Weak;
    };

    if evidence.no_trade_persists
        || !evidence.breakout_changes.is_empty()
        || evidence.trend_cohesion_status_change.is_some()
        || evidence.trend_cohesion_topology_change.is_some()
    {
        return InterpretationTrendState::PostRallyConsolidation;
    }

    if evidence.trend_recognition_state.is_some() || evidence.has_significant_change {
        InterpretationTrendState::Stable
    } else {
        InterpretationTrendState::Weak
    }
}

pub(crate) fn has_supply_pressure(snapshot: &CapitalAbsorptionAutoSnapshot) -> bool {
    matches!(
        snapshot.potential_supply_pressure.level,
        CapitalAbsorptionPotentialSupplyPressureLevel::Elevated
    ) || snapshot.potential_supply_pressure.future_queue_count > 0
        || snapshot.potential_supply_pressure.near_term_supply_count > 0
}

pub(crate) fn collect_subjects(
    expectation_snapshot: &ExpectationLayerSnapshot,
    gravity_observation: &ValuationGravityObservation,
) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut subjects = Vec::new();
    for subject in expectation_snapshot
        .observations
        .iter()
        .map(|observation| observation.subject.clone())
        .chain(
            gravity_observation
                .snapshot
                .assets
                .iter()
                .map(|asset| asset.symbol.clone()),
        )
    {
        let subject: String = subject;
        if seen.insert(subject.clone()) {
            subjects.push(subject);
        }
    }
    subjects
}

#[cfg(test)]
pub(crate) fn classify_interpretation_pattern(
    signal: &InterpretationNarrativeSignal,
) -> InterpretationPattern {
    if signal.supply_pressure {
        return InterpretationPattern::SupplyPressure;
    }

    if matches!(
        signal.trend_state,
        InterpretationTrendState::PostRallyConsolidation
    ) {
        return InterpretationPattern::PostRallyConsolidation;
    }

    if matches!(signal.trend_state, InterpretationTrendState::Stable)
        && matches!(
            signal.gravity_data_quality,
            InterpretationGravityDataQuality::Ready
        )
        && matches!(
            signal.gravity_status,
            Some(GravityStatus::Fair)
                | Some(GravityStatus::Undervalued)
                | Some(GravityStatus::DeepUndervalued)
        )
        && matches!(
            signal.expectation_quality,
            InterpretationExpectationQuality::High | InterpretationExpectationQuality::Medium
        )
    {
        return InterpretationPattern::FundamentalPricing;
    }

    InterpretationPattern::EventWaiting
}

fn render_subjects(subjects: &[String]) -> String {
    let mut seen = HashSet::<String>::new();
    let mut unique = Vec::new();
    for subject in subjects.iter().map(|subject| subject.trim()) {
        if !subject.is_empty() && seen.insert(subject.to_string()) {
            unique.push(subject.to_string());
        }
    }
    if unique.is_empty() {
        return "N/A".to_string();
    }
    unique.join(", ")
}

fn expectation_quality_label(value: InterpretationExpectationQuality) -> String {
    match value {
        InterpretationExpectationQuality::High => "HIGH".to_string(),
        InterpretationExpectationQuality::Medium => "MEDIUM".to_string(),
        InterpretationExpectationQuality::Low => "LOW".to_string(),
        InterpretationExpectationQuality::Unavailable => "UNAVAILABLE".to_string(),
    }
}

fn expectation_quality_reason_label(
    value: InterpretationExpectationQualityReason,
    language: Language,
) -> String {
    match (value, language) {
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::ZhCn) => {
            "市场预期已形成".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::EnUs) => {
            "Market consensus available".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::JaJp) => {
            "市場コンセンサスあり".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::ZhCn) => {
            "市场没有一致预期".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::EnUs) => {
            "Market consensus unavailable".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::JaJp) => {
            "市場コンセンサスなし".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::ZhCn) => {
            "系统未取得预期数据".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::EnUs) => {
            "System unavailable".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::JaJp) => {
            "システム未取得".to_string()
        }
    }
}

fn gravity_data_quality_label(value: InterpretationGravityDataQuality) -> String {
    match value {
        InterpretationGravityDataQuality::Ready => "READY".to_string(),
        InterpretationGravityDataQuality::Partial => "PARTIAL".to_string(),
        InterpretationGravityDataQuality::Unavailable => "UNAVAILABLE".to_string(),
    }
}

fn gravity_data_quality_reason_label(value: InterpretationGravityDataQualityReason) -> String {
    match value {
        InterpretationGravityDataQualityReason::ProviderUnavailable => {
            "Provider unavailable".to_string()
        }
        InterpretationGravityDataQualityReason::HistoricalSnapshotMissing => {
            "Historical snapshot missing".to_string()
        }
        InterpretationGravityDataQualityReason::ConsensusUnavailable => {
            "Consensus unavailable".to_string()
        }
        InterpretationGravityDataQualityReason::SourceTemporarilyUnavailable => {
            "Source temporarily unavailable".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::interface::presentation::SignalContextQuality;
    use crate::features::radar::interface::signal_context_event_read_model::{
        SignalContextEventReadModel, SignalContextEventSlot, SignalContextEvidence,
        SignalContextEvidenceSource,
    };
    use crate::features::shared::interface::i18n::get_dictionary;

    fn signal(
        trend_state: InterpretationTrendState,
        expectation_quality: InterpretationExpectationQuality,
        expectation_quality_reason: InterpretationExpectationQualityReason,
        gravity_data_quality: InterpretationGravityDataQuality,
        gravity_status: Option<GravityStatus>,
        supply_pressure: bool,
    ) -> InterpretationNarrativeSignal {
        InterpretationNarrativeSignal {
            trend_state,
            trend_available: true,
            expectation_quality,
            expectation_quality_reason,
            gravity_data_quality,
            gravity_data_quality_reason:
                InterpretationGravityDataQualityReason::ProviderUnavailable,
            gravity_status,
            supply_pressure,
            supply_available: true,
            flow_acceleration: Some(0.0),
            gray_rhino_escalated: false,
        }
    }

    #[test]
    fn tsla_style_event_waiting_selects_event_waiting_pattern() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::Weak,
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Fair),
            false,
        ));

        assert_eq!(pattern, InterpretationPattern::EventWaiting);
    }

    #[test]
    fn goog_style_fundamental_pricing_selects_fundamental_pricing_pattern() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::Stable,
            InterpretationExpectationQuality::Medium,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Fair),
            false,
        ));

        assert_eq!(pattern, InterpretationPattern::FundamentalPricing);
    }

    #[test]
    fn nvda_style_post_rally_consolidation_selects_post_rally_pattern() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::PostRallyConsolidation,
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Expensive),
            false,
        ));

        assert_eq!(pattern, InterpretationPattern::PostRallyConsolidation);
    }

    #[test]
    fn supply_pressure_wins_over_other_signals() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::Stable,
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Fair),
            true,
        ));

        assert_eq!(pattern, InterpretationPattern::SupplyPressure);
    }

    #[test]
    fn view_model_renders_boundary_and_zero_weight() {
        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string(), "GOOG".to_string(), "NVDA".to_string()];
        let view_model = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
            subjects: &subjects,
            signal: signal(
                InterpretationTrendState::Stable,
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                Some(GravityStatus::Fair),
                false,
            ),
            future_context: Default::default(),
            decision_summary: Some(
                &crate::features::radar::interface::presentation::DecisionSummaryViewModel {
                    is_no_trade: true,
                    summary: "NO TRADE".to_string(),
                    readiness_reasons_label: "Readiness Reasons".to_string(),
                    readiness_reasons: vec![
                        "突破连续性不足".to_string(),
                        "扩散范围有限".to_string(),
                    ],
                    ..Default::default()
                },
            ),
            language: Language::EnUs,
            dict: &dict,
        });

        assert_eq!(view_model.current_decision_weight_value, "0%");
        assert_eq!(
            view_model.signal_context_information_content_value,
            "UNAVAILABLE"
        );
        assert_eq!(
            view_model.signal_context_primary_context_value,
            "Month-end Rebalancing"
        );
        assert_eq!(view_model.signal_context_quality_value, "UNAVAILABLE");
        assert!(view_model
            .signal_context_interpretation_value
            .contains("Available source coverage is incomplete"));
        assert_eq!(
            view_model.expectation_quality_reason_value,
            "Market consensus available"
        );
        assert!(view_model.boundary.contains("Observation Layer"));
        assert!(view_model.subjects_value.contains("TSLA"));
        assert!(view_model
            .narrative_components_label
            .contains("Narrative Components"));
        assert!(view_model.trend_value.contains("Price structure"));
        assert!(view_model.expectation_value.contains("Market expectation"));
        assert!(view_model.supply_value.contains("No new supply risk"));
        assert!(view_model
            .gravity_value
            .contains("Price is broadly near the value anchor"));
        assert!(view_model.flow_value.contains("Flow is neutral"));
        assert!(view_model
            .interpretation_value
            .contains("See Market Interpretation for the main narrative."));
        assert!(view_model
            .decision_explanation_reasons
            .contains(&"突破连续性不足".to_string()));
    }

    #[test]
    fn expectation_quality_reason_distinguishes_system_and_market_unavailability() {
        use crate::features::research::domain::expectation::{
            ExpectationEventType, ExpectationObservation, ExpectationPressure, RevisionDirection,
            SourceHealth, SurpriseState,
        };

        let as_of_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        let system_unavailable_snapshot = ExpectationLayerSnapshot {
            as_of_date,
            decision_weight_percent: 0,
            trade_signal: false,
            gate_effect: "none".to_string(),
            execution_effect: "none".to_string(),
            position_sizing_effect: "none".to_string(),
            observations: vec![],
        };
        let (quality, reason) = derive_expectation_quality(&system_unavailable_snapshot);
        assert_eq!(quality, InterpretationExpectationQuality::Unavailable);
        assert_eq!(
            reason,
            InterpretationExpectationQualityReason::SystemUnavailable
        );

        let market_unavailable_snapshot = ExpectationLayerSnapshot {
            as_of_date,
            decision_weight_percent: 0,
            trade_signal: false,
            gate_effect: "none".to_string(),
            execution_effect: "none".to_string(),
            position_sizing_effect: "none".to_string(),
            observations: vec![ExpectationObservation {
                subject: "TSLA".to_string(),
                period: "2026Q2".to_string(),
                event_type: ExpectationEventType::DeliveryConsensus,
                lifecycle_state: crate::features::research::domain::expectation::ExpectationLifecycleState::Pending,
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
                revision_direction: RevisionDirection::Stable,
                surprise_state: SurpriseState::NotReleased,
                expectation_pressure: ExpectationPressure::Normal,
                confidence: None,
                source_health: SourceHealth::Succeeded,
                interpretation: "市場没有一致预期".to_string(),
                as_of_date,
                observed_at: as_of_date,
            }],
        };
        let (quality, reason) = derive_expectation_quality(&market_unavailable_snapshot);
        assert_eq!(quality, InterpretationExpectationQuality::Unavailable);
        assert_eq!(
            reason,
            InterpretationExpectationQualityReason::MarketConsensusUnavailable
        );
    }

    #[test]
    fn trend_state_uses_transition_read_model_only() {
        let weak = derive_trend_state(None);
        assert_eq!(weak, InterpretationTrendState::Weak);

        let stable = derive_trend_state(Some(&StateTransitionViewModel {
            trend_recognition_state: Some("STRUCTURAL_PERSISTENCE".to_string()),
            ..Default::default()
        }));
        assert_eq!(stable, InterpretationTrendState::Stable);

        let post_rally = derive_trend_state(Some(&StateTransitionViewModel {
            no_trade_persists: true,
            ..Default::default()
        }));
        assert_eq!(post_rally, InterpretationTrendState::PostRallyConsolidation);
    }

    #[test]
    fn test_todays_explanation_rules() {
        let _dict = get_dictionary(Language::EnUs);

        // 1. trend primary when signal context is low and gravity unavailable
        // シグナルコンテキストが Low で重力が Unavailable の時、トレンドが Primary になる
        let signal = InterpretationNarrativeSignal {
            trend_state: InterpretationTrendState::Stable,
            trend_available: true,
            gravity_data_quality: InterpretationGravityDataQuality::Unavailable,
            ..Default::default()
        };
        let future_context = SignalContextEventReadModel::default();
        let signal_context = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            signal,
            future_context,
            language: Language::EnUs,
        });

        let explanation = build_todays_explanation(&signal, &signal_context, Language::EnUs);
        assert!(explanation
            .primary_driver_value
            .contains("Trend continuation"));
        // Gravity は unavailable なので Ignored Today に入る
        assert!(explanation
            .ignored_today
            .iter()
            .any(|s| s.contains("Gravity unavailable")));

        // 2. macro primary when information content is high
        // マクロ情報が High の時にマクロが Primary になる
        let signal_macro_high = InterpretationNarrativeSignal {
            trend_state: InterpretationTrendState::Stable,
            trend_available: true,
            ..Default::default()
        };
        let future_context_high = SignalContextEventReadModel {
            macro_event: SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                detected: true,
                quality: SignalContextQuality::High,
                source: SignalContextEvidenceSource::Calendar,
                summary: "Major Macro Event".to_string(),
            })),
            ..Default::default()
        };
        let signal_context_high = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            signal: signal_macro_high,
            future_context: future_context_high,
            language: Language::EnUs,
        });
        let explanation_high =
            build_todays_explanation(&signal_macro_high, &signal_context_high, Language::EnUs);
        assert!(explanation_high
            .primary_driver_value
            .contains("High-information macro event"));
        // トレンドは Primary をマクロに譲り、Secondary に回る
        assert!(explanation_high
            .secondary_drivers
            .iter()
            .any(|s| s.contains("Trend stable, secondary.")));

        // 3. gravity unavailable moves to ignored_today
        // 重力が不可用の時に Ignored Today に入る
        assert!(explanation
            .ignored_today
            .iter()
            .any(|s| s.contains("Gravity unavailable.")));

        // 4. gray_rhino remains ignored_by_default
        // 灰色サイはデフォルトで Ignored Today
        let signal_default = InterpretationNarrativeSignal::default();
        let signal_context_default = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            signal: signal_default,
            future_context: SignalContextEventReadModel::default(),
            language: Language::EnUs,
        });
        let explanation_default =
            build_todays_explanation(&signal_default, &signal_context_default, Language::EnUs);
        assert!(explanation_default
            .secondary_drivers
            .iter()
            .any(|s| s.contains("Macro information is not confirmed yet")));
        assert!(!explanation_default
            .ignored_today
            .iter()
            .any(|s| s.contains("Macro event has low or unknown information content")));

        // 5. gray_rhino_moves_to_secondary_when_escalated
        // escalated の場合は 灰色サイが Secondary に昇格
        let signal_escalated = InterpretationNarrativeSignal {
            gray_rhino_escalated: true,
            ..Default::default()
        };
        let explanation_escalated =
            build_todays_explanation(&signal_escalated, &signal_context_default, Language::EnUs);
        assert!(explanation_escalated
            .secondary_drivers
            .iter()
            .any(|s| s.contains("Gray Rhino secondary.")));

        // 6. hypothesis excluded from todays_explanation
        // 推測 (Hypothesis) は本日の説明に含まれない
        assert!(!explanation_default
            .primary_driver_value
            .contains("Hypothesis"));
        assert!(!explanation_default
            .secondary_drivers
            .iter()
            .any(|s| s.contains("Hypothesis")));
        assert!(!explanation_default
            .ignored_today
            .iter()
            .any(|s| s.contains("Hypothesis")));

        // 7. supply secondary when elevated but not dominant
        // 供給圧力が存在するが他が Primary の時に Secondary になる
        let signal_supply = InterpretationNarrativeSignal {
            trend_state: InterpretationTrendState::Stable,
            trend_available: true,
            supply_pressure: true,
            supply_available: true,
            ..Default::default()
        };
        let signal_context_supply = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            signal: signal_supply,
            future_context: SignalContextEventReadModel::default(),
            language: Language::EnUs,
        });
        let explanation_supply =
            build_todays_explanation(&signal_supply, &signal_context_supply, Language::EnUs);
        assert!(explanation_supply
            .primary_driver_value
            .contains("Trend continuation"));
        assert!(explanation_supply
            .secondary_drivers
            .iter()
            .any(|s| s.contains("Supply pressure, secondary.")));

        // 8. flow_neutral becomes ignored
        // 資金フローが中立の場合は Ignored Today になる
        let signal_flow = InterpretationNarrativeSignal {
            flow_acceleration: Some(0.01), // neutral
            ..Default::default()
        };
        let explanation_flow =
            build_todays_explanation(&signal_flow, &signal_context_default, Language::EnUs);
        assert!(explanation_flow
            .ignored_today
            .iter()
            .any(|s| s.contains("Flow ignored.")));
    }
}
