use crate::features::radar::interface::interpretation_read_model::InterpretationNarrativeSignal;
use crate::features::radar::interface::presentation::{
    SignalContextInformationContent, SignalContextPrimaryContext, SignalContextQuality,
};
use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
use crate::features::shared::interface::i18n::Language;
use chrono::{Datelike, NaiveDate};

#[derive(Debug)]
pub(crate) struct SignalContextReadModelInput {
    pub as_of_date: NaiveDate,
    pub signal: InterpretationNarrativeSignal,
    pub future_context: SignalContextEventReadModel,
    pub language: Language,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SignalContextAssessment {
    pub information_content: SignalContextInformationContent,
    pub primary_context: SignalContextPrimaryContext,
    pub context_quality: SignalContextQuality,
    pub event_fact: String,
    pub source_health: MacroEventSourceHealth,
    pub source_diagnostics_summary: String,
    pub source_diagnostics_appendix: String,
    pub interpretation: String,
    pub next_observation: String,
}

pub(crate) fn build_signal_context_assessment(
    input: SignalContextReadModelInput,
) -> SignalContextAssessment {
    let _signal = input.signal;
    let primary_context = derive_primary_context(input.as_of_date, &input.future_context);
    let information_content = derive_information_content(primary_context, &input.future_context);
    let context_quality = derive_context_quality(primary_context, &input.future_context);
    let event_fact = compose_event_fact(&input.future_context);
    let source_health = input.future_context.source_health;
    let (source_diagnostics_summary, source_diagnostics_appendix) =
        compose_source_diagnostics(&input.future_context, input.language);
    let interpretation = compose_interpretation(
        primary_context,
        information_content,
        context_quality,
        &input.future_context,
        input.language,
    );
    let next_observation = compose_next_observation(
        primary_context,
        information_content,
        &input.future_context,
        input.language,
    );

    SignalContextAssessment {
        information_content,
        primary_context,
        context_quality,
        event_fact,
        source_health,
        source_diagnostics_summary,
        source_diagnostics_appendix,
        interpretation,
        next_observation,
    }
}

pub(crate) fn signal_context_information_content_label(
    value: SignalContextInformationContent,
) -> &'static str {
    match value {
        SignalContextInformationContent::High => "HIGH",
        SignalContextInformationContent::Medium => "MEDIUM",
        SignalContextInformationContent::Low => "LOW",
        SignalContextInformationContent::Unknown => "UNKNOWN",
    }
}

pub(crate) fn signal_context_primary_context_label(
    value: SignalContextPrimaryContext,
) -> &'static str {
    match value {
        SignalContextPrimaryContext::QuarterEndRebalancing => "Quarter-end Rebalancing",
        SignalContextPrimaryContext::MonthEndRebalancing => "Month-end Rebalancing",
        SignalContextPrimaryContext::IndexReconstitution => "Index Reconstitution",
        SignalContextPrimaryContext::EtfRebalance => "ETF Rebalance",
        SignalContextPrimaryContext::HolidayLiquidity => "Holiday Liquidity",
        SignalContextPrimaryContext::PreEarningsWaiting => "Pre-Earnings Waiting",
        SignalContextPrimaryContext::MajorEventWaiting => "Major Event Waiting",
        SignalContextPrimaryContext::MacroEvent => "Macro Event",
        SignalContextPrimaryContext::None => "None",
    }
}

pub(crate) fn signal_context_quality_label(value: SignalContextQuality) -> &'static str {
    match value {
        SignalContextQuality::High => "HIGH",
        SignalContextQuality::Medium => "MEDIUM",
        SignalContextQuality::Low => "LOW",
        SignalContextQuality::Unavailable => "UNAVAILABLE",
    }
}

pub(crate) fn signal_context_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: Signal Context 只解释今天价格变化的信息含量，不进入 Gate、Execution、Trader、Action Matrix、READY / EXECUTE 或 Position Sizing，也不生成交易信号。"
        }
        Language::EnUs => {
            "Boundary: Signal Context only explains the information content of today's price change. It does not enter Gate, Execution, Trader, Action Matrix, READY / EXECUTE, or Position Sizing, and it does not generate trade signals."
        }
        Language::JaJp => {
            "境界: Signal Context は今日の価格変化の情報含量だけを説明し、Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing に入らず、売買シグナルも生成しない。"
        }
    }
}

fn derive_primary_context(
    as_of_date: NaiveDate,
    future_context: &SignalContextEventReadModel,
) -> SignalContextPrimaryContext {
    if is_quarter_end(as_of_date) {
        return SignalContextPrimaryContext::QuarterEndRebalancing;
    }

    if is_month_end(as_of_date) {
        return SignalContextPrimaryContext::MonthEndRebalancing;
    }

    if let Some(context) = future_context.detected_primary_context() {
        return context;
    }

    SignalContextPrimaryContext::None
}

fn derive_information_content(
    primary_context: SignalContextPrimaryContext,
    future_context: &SignalContextEventReadModel,
) -> SignalContextInformationContent {
    match primary_context {
        // 機械性コンテキストは公式ソースの状態に依存せず LOW とする。
        SignalContextPrimaryContext::QuarterEndRebalancing
        | SignalContextPrimaryContext::MonthEndRebalancing
        | SignalContextPrimaryContext::IndexReconstitution
        | SignalContextPrimaryContext::EtfRebalance
        | SignalContextPrimaryContext::HolidayLiquidity => SignalContextInformationContent::Low,
        SignalContextPrimaryContext::MacroEvent => SignalContextInformationContent::High,
        SignalContextPrimaryContext::MajorEventWaiting
        | SignalContextPrimaryContext::PreEarningsWaiting => {
            SignalContextInformationContent::Medium
        }
        SignalContextPrimaryContext::None => {
            if future_context.has_loaded_context() {
                SignalContextInformationContent::Low
            } else {
                SignalContextInformationContent::Unknown
            }
        }
    }
}

fn derive_context_quality(
    primary_context: SignalContextPrimaryContext,
    future_context: &SignalContextEventReadModel,
) -> SignalContextQuality {
    match primary_context {
        SignalContextPrimaryContext::QuarterEndRebalancing
        | SignalContextPrimaryContext::MonthEndRebalancing => SignalContextQuality::High,
        SignalContextPrimaryContext::IndexReconstitution
        | SignalContextPrimaryContext::EtfRebalance
        | SignalContextPrimaryContext::HolidayLiquidity
        | SignalContextPrimaryContext::PreEarningsWaiting
        | SignalContextPrimaryContext::MajorEventWaiting
        | SignalContextPrimaryContext::MacroEvent => future_context
            .evidence_quality_for(primary_context)
            .unwrap_or_else(|| {
                if future_context.has_loaded_context() {
                    SignalContextQuality::Low
                } else {
                    SignalContextQuality::Unavailable
                }
            }),
        SignalContextPrimaryContext::None => {
            if future_context.has_loaded_context() {
                SignalContextQuality::Low
            } else {
                SignalContextQuality::Unavailable
            }
        }
    }
}

fn compose_interpretation(
    primary_context: SignalContextPrimaryContext,
    information_content: SignalContextInformationContent,
    context_quality: SignalContextQuality,
    future_context: &SignalContextEventReadModel,
    language: Language,
) -> String {
    match primary_context {
        SignalContextPrimaryContext::MacroEvent => macro_event_text(
            future_context,
            information_content,
            context_quality,
            language,
        ),
        SignalContextPrimaryContext::MajorEventWaiting
        | SignalContextPrimaryContext::PreEarningsWaiting => waiting_event_text(
            primary_context,
            future_context,
            information_content,
            language,
        ),
        SignalContextPrimaryContext::QuarterEndRebalancing
        | SignalContextPrimaryContext::MonthEndRebalancing
        | SignalContextPrimaryContext::IndexReconstitution
        | SignalContextPrimaryContext::EtfRebalance
        | SignalContextPrimaryContext::HolidayLiquidity => mechanical_context_text(
            primary_context,
            future_context,
            information_content,
            language,
        ),
        SignalContextPrimaryContext::None => none_text(
            information_content,
            context_quality,
            future_context,
            language,
        ),
    }
}

fn compose_source_diagnostics(
    future_context: &SignalContextEventReadModel,
    language: Language,
) -> (String, String) {
    let attempts = future_context.source_attempts;
    let successes = future_context.source_successes;
    let failures = future_context.source_failures;
    let health = signal_context_source_health_label(future_context.source_health);
    let detail = future_context
        .source_diagnostic
        .as_deref()
        .unwrap_or(match language {
            Language::ZhCn => "没有额外诊断信息",
            Language::EnUs => "no extra diagnostic information",
            Language::JaJp => "追加の診断情報はない",
        });
    let summary = if attempts == 0 {
        match language {
            Language::ZhCn => "Official Calendar unavailable.".to_string(),
            Language::EnUs => "Official Calendar unavailable.".to_string(),
            Language::JaJp => "Official Calendar は利用不可。".to_string(),
        }
    } else if failures == 0 {
        String::new()
    } else {
        match language {
            Language::ZhCn => format!(
                "Official Calendar coverage {successes}/{attempts}; unavailable {failures}; health {health}."
            ),
            Language::EnUs => format!(
                "Official Calendar coverage {successes}/{attempts}; unavailable {failures}; health {health}."
            ),
            Language::JaJp => format!(
                "Official Calendar coverage {successes}/{attempts}; unavailable {failures}; health {health}."
            ),
        }
    };
    let appendix = if attempts == 0 {
        match language {
            Language::ZhCn => format!("Official Calendar source not loaded; {detail}"),
            Language::EnUs => format!("Official calendar source not loaded; {detail}"),
            Language::JaJp => format!("公式カレンダー source は未ロード; {detail}"),
        }
    } else if future_context.source_health == MacroEventSourceHealth::Succeeded {
        String::new()
    } else {
        match language {
            Language::ZhCn => format!(
                "Official calendar source health: {health}; coverage: {successes}/{attempts} succeeded, {failures} failed; {detail}"
            ),
            Language::EnUs => format!(
                "Official calendar source health: {health}; coverage: {successes}/{attempts} succeeded, {failures} failed; {detail}"
            ),
            Language::JaJp => format!(
                "公式カレンダーの source health: {health}; coverage: {successes}/{attempts} succeeded, {failures} failed; {detail}"
            ),
        }
    };
    (summary, appendix)
}

fn compose_next_observation(
    primary_context: SignalContextPrimaryContext,
    information_content: SignalContextInformationContent,
    future_context: &SignalContextEventReadModel,
    language: Language,
) -> String {
    let waiting_text = match language {
        Language::ZhCn => "等待官方公布。公布后系统将自动对比 Expected / Actual、计算 Surprise、更新 Narrative。".to_string(),
        Language::EnUs => "Waiting for the official release. After publication the system will automatically compare Expected / Actual, calculate Surprise, and refresh the Narrative.".to_string(),
        Language::JaJp => "公式発表を待機中。公表後は Expected / Actual を比較し、Surprise を計算して Narrative を更新する。".to_string(),
    };

    if future_context.source_health == MacroEventSourceHealth::Unavailable
        && !future_context.has_loaded_context()
    {
        return match language {
            Language::ZhCn => "官方来源当前不可用；待来源恢复后再确认是否存在事件".to_string(),
            Language::EnUs => "Official source is currently unavailable; verify if events exist once the source is restored.".to_string(),
            Language::JaJp => "公式ソースは現在利用できません。ソース復旧後にイベントが存在するかどうかを再確認します。".to_string(),
        };
    }

    match (primary_context, information_content) {
        (SignalContextPrimaryContext::MacroEvent, SignalContextInformationContent::High) => match language {
            Language::ZhCn => "等待官方公布。公布后系统将自动对比 Expected / Actual、计算 Surprise、更新 Narrative。".to_string(),
            Language::EnUs => "Waiting for the official release. After publication the system will automatically compare Expected / Actual, calculate Surprise, and refresh the Narrative.".to_string(),
            Language::JaJp => "公式発表を待機中。公表後は Expected / Actual を比較し、Surprise を計算して Narrative を更新する。".to_string(),
        },
        (_, SignalContextInformationContent::Medium) => match language {
            Language::ZhCn => "观察中等重要事件的后续公布，并在结果落地后重新评估预期修正。".to_string(),
            Language::EnUs => "Watching the next release for a medium-importance event, then re-evaluating the expectation adjustment once it lands.".to_string(),
            Language::JaJp => "中重要度イベントの次回公表を観察し、結果が出たら期待修正を再評価する。".to_string(),
        },
        (_, SignalContextInformationContent::Low) => match language {
            Language::ZhCn => "继续等待官方日历更新；当前更接近正常整理而非新的风险升级。".to_string(),
            Language::EnUs => "Continue waiting for the official calendar update; this is closer to normal consolidation than a new risk upgrade.".to_string(),
            Language::JaJp => "公式カレンダーの更新を待つ。現状は新しいリスク上昇というより通常の整理に近い。".to_string(),
        },
        _ => waiting_text,
    }
}

fn is_quarter_end(date: NaiveDate) -> bool {
    matches!(date.month(), 3 | 6 | 9 | 12) && date == last_day_of_month(date)
}

fn is_month_end(date: NaiveDate) -> bool {
    date == last_day_of_month(date)
}

fn last_day_of_month(date: NaiveDate) -> NaiveDate {
    let (year, month) = (date.year(), date.month());
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).expect("valid next month date")
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).expect("valid next month date")
    };
    next_month
        .pred_opt()
        .expect("previous day of first month day must exist")
}

fn compose_event_fact(future_context: &SignalContextEventReadModel) -> String {
    future_context
        .detected_primary_evidence_summary()
        .unwrap_or_default()
}

fn none_text(
    information_content: SignalContextInformationContent,
    context_quality: SignalContextQuality,
    future_context: &SignalContextEventReadModel,
    language: Language,
) -> String {
    match context_quality {
        SignalContextQuality::Unavailable => match language {
            Language::ZhCn => {
                let _ = (information_content, future_context);
                "官方来源暂时无法确认今天是否存在高信息量事件，Signal Context 只能标记为 UNKNOWN。"
                    .to_string()
            }
            Language::EnUs => {
                let _ = (information_content, future_context);
                "Official sources cannot confirm whether today has a high-information event, so Signal Context is UNKNOWN.".to_string()
            }
            Language::JaJp => {
                let _ = (information_content, future_context);
                "公式ソースでは今日は高情報量イベントの有無を確認できず、Signal Context は UNKNOWN でしか扱えない。".to_string()
            }
        },
        SignalContextQuality::Low => match language {
            Language::ZhCn => {
                let _ = (information_content, future_context);
                "今天未识别到高信息量宏观事件。官方经济日历未命中 CPI、FOMC、就业、GDP 等事件。今日价格变化更可能由企业消息、板块轮动、技术走势驱动。".to_string()
            }
            Language::EnUs => {
                let _ = (information_content, future_context);
                "Today no high-information macro event was identified. The official economic calendar did not match CPI, FOMC, jobs, GDP, or similar events. Price changes are more likely driven by company news, sector rotation, or technical action."
                    .to_string()
            }
            Language::JaJp => {
                let _ = (information_content, future_context);
                "今日は高情報量のマクロイベントは識別されていない。公式経済カレンダーは CPI、FOMC、雇用、GDP などにヒットしておらず、値動きは企業ニュース、セクターローテーション、テクニカル要因に由来する可能性が高い。".to_string()
            }
        },
        _ => match language {
            Language::ZhCn => {
                let _ = (information_content, future_context);
                "今天存在中等重要事件，但不是高信息量事件。市场更可能围绕局部信息和预期修正波动。"
                    .to_string()
            }
            Language::EnUs => {
                let _ = (information_content, future_context);
                "Today has a medium-importance event, but not a high-information macro event. The market is more likely to trade around localized information and expectation adjustments."
                    .to_string()
            }
            Language::JaJp => {
                let _ = (information_content, future_context);
                "今日は中重要度のイベントはあるが、高情報量のマクロイベントではない。市場は局所情報と期待修正を中心に動きやすい。".to_string()
            }
        },
    }
}

fn mechanical_context_text(
    primary_context: SignalContextPrimaryContext,
    future_context: &SignalContextEventReadModel,
    information_content: SignalContextInformationContent,
    language: Language,
) -> String {
    let _ = (future_context, information_content);
    match (primary_context, language) {
        (SignalContextPrimaryContext::QuarterEndRebalancing, Language::ZhCn) => {
            "近期价格波动更可能来自季度末的机械性再平衡。当前价格的信息含量较低，建议等资金流恢复常态后再重新评估趋势。".to_string()
        }
        (SignalContextPrimaryContext::QuarterEndRebalancing, Language::EnUs) => {
            "Recent price action is more likely driven by quarter-end mechanical rebalancing. Information content is low, so wait until normal trading resumes before re-evaluating the trend."
                .to_string()
        }
        (SignalContextPrimaryContext::QuarterEndRebalancing, Language::JaJp) => {
            "最近の値動きは四半期末の機械的なリバランスに由来する可能性が高い。情報含量は低く、通常の取引に戻ってから改めてトレンドを評価した方がよい。"
                .to_string()
        }
        (SignalContextPrimaryContext::MonthEndRebalancing, Language::ZhCn) => {
            "近期价格波动更可能来自月末再平衡。当前价格的信息含量较低，适合先观察正常交易恢复后的延续性。".to_string()
        }
        (SignalContextPrimaryContext::MonthEndRebalancing, Language::EnUs) => {
            "Recent price action is more likely driven by month-end rebalancing. Information content is low, so it is better to observe whether the move persists after normal trading resumes."
                .to_string()
        }
        (SignalContextPrimaryContext::MonthEndRebalancing, Language::JaJp) => {
            "最近の値動きは月末リバランスの影響である可能性が高い。情報含量は低く、通常取引に戻った後の持続性を先に観察したい。".to_string()
        }
        (SignalContextPrimaryContext::IndexReconstitution, Language::ZhCn) => {
            "当前价格更像是在反映指数成分调整，信息含量偏低。".to_string()
        }
        (SignalContextPrimaryContext::IndexReconstitution, Language::EnUs) => {
            "The move looks tied to index reconstitution flow, so its information content is low."
                .to_string()
        }
        (SignalContextPrimaryContext::IndexReconstitution, Language::JaJp) => {
            "現在の値動きは指数リコンスティテューションの影響に近く、情報含量は低い。".to_string()
        }
        (SignalContextPrimaryContext::EtfRebalance, Language::ZhCn) => {
            "当前价格更像是在反映 ETF 再平衡，信息含量偏低。".to_string()
        }
        (SignalContextPrimaryContext::EtfRebalance, Language::EnUs) => {
            "The move looks tied to ETF rebalancing flow, so its information content is low."
                .to_string()
        }
        (SignalContextPrimaryContext::EtfRebalance, Language::JaJp) => {
            "現在の値動きは ETF リバランスの影響に近く、情報含量は低い。".to_string()
        }
        (SignalContextPrimaryContext::HolidayLiquidity, Language::ZhCn) => {
            "节假日前后的流动性偏薄，当前价格信息含量偏低。".to_string()
        }
        (SignalContextPrimaryContext::HolidayLiquidity, Language::EnUs) => {
            "Liquidity is thinner around the holiday window, so the information content is low."
                .to_string()
        }
        (SignalContextPrimaryContext::HolidayLiquidity, Language::JaJp) => {
            "休日前後は流動性が薄く、現在の価格情報含量は低い。".to_string()
        }
        _ => none_text(information_content, SignalContextQuality::Low, future_context, language),
    }
}

fn waiting_event_text(
    primary_context: SignalContextPrimaryContext,
    future_context: &SignalContextEventReadModel,
    information_content: SignalContextInformationContent,
    language: Language,
) -> String {
    let _ = (primary_context, future_context, information_content);
    match language {
        Language::ZhCn => {
            "市场正在等待重要事件，当前价格信息含量处于中等水平，尚不足以直接定义为高信息量事件。".to_string()
        }
        Language::EnUs => {
            "The market is waiting for an important event. Information content is medium, but not yet high enough to qualify as a high-information day."
                .to_string()
        }
        Language::JaJp => {
            "市場は重要イベントを待っており、情報含量は中程度だが、高情報量日と断定するにはまだ足りない。".to_string()
        }
    }
}

fn macro_event_text(
    future_context: &SignalContextEventReadModel,
    information_content: SignalContextInformationContent,
    context_quality: SignalContextQuality,
    language: Language,
) -> String {
    let event_fact = future_context
        .detected_primary_evidence_summary()
        .unwrap_or_default();
    match language {
        Language::ZhCn => {
            let _ = context_quality;
            let info = signal_context_information_content_label(information_content);
            if event_fact.is_empty() {
                format!(
                    "今天识别到高信息量宏观事件，市场正在重新定价新的宏观信息。信息含量: {info}。"
                )
            } else {
                format!("今天识别到高信息量宏观事件: {event_fact}。市场正在重新定价新的宏观信息。信息含量: {info}。")
            }
        }
        Language::EnUs => {
            let _ = context_quality;
            let info = signal_context_information_content_label(information_content);
            if event_fact.is_empty() {
                format!("A high-information macro event was identified today, and the market is repricing the new macro information. Information content: {info}.")
            } else {
                format!("A high-information macro event was identified today: {event_fact}. The market is repricing the new macro information. Information content: {info}.")
            }
        }
        Language::JaJp => {
            let _ = context_quality;
            let info = signal_context_information_content_label(information_content);
            if event_fact.is_empty() {
                format!("今日は高情報量のマクロイベントが識別され、市場は新しいマクロ情報を再価格付けしている。情報含量: {info}。")
            } else {
                format!("今日は高情報量のマクロイベントが識別され: {event_fact}。市場は新しいマクロ情報を再価格付けしている。情報含量: {info}。")
            }
        }
    }
}

fn signal_context_source_health_label(value: MacroEventSourceHealth) -> &'static str {
    match value {
        MacroEventSourceHealth::Succeeded => "SUCCEEDED",
        MacroEventSourceHealth::Partial => "PARTIAL",
        MacroEventSourceHealth::Unavailable => "UNAVAILABLE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::interface::presentation::{
        InterpretationExpectationQuality, InterpretationExpectationQualityReason,
        InterpretationGravityDataQuality, InterpretationGravityDataQualityReason,
        InterpretationTrendState,
    };
    use crate::features::research::domain::expectation::ExpectationLifecycleState;
    use crate::features::research::interface::expectation_report_builder::{
        build_expectation_layer_fixture_snapshot, ExpectationLayerSnapshot,
    };
    use crate::features::research::interface::macro_event_calendar_adapter::MacroEventCalendarReadModel;
    use crate::features::research::interface::macro_event_observation::{
        FutureCalendarKind, FutureCalendarObservation, MacroEventImportance,
        MacroEventInformationContent, MacroEventLifecycle, MacroEventObservation,
        MacroEventSourceHealth, MacroEventSurpriseState, MacroEventType,
    };
    use crate::features::shared::interface::i18n::Language;
    use chrono::NaiveDate;

    fn signal(
        expectation_quality: InterpretationExpectationQuality,
        expectation_quality_reason: InterpretationExpectationQualityReason,
        gravity_data_quality: InterpretationGravityDataQuality,
        supply_pressure: bool,
        flow_acceleration: Option<f64>,
    ) -> InterpretationNarrativeSignal {
        InterpretationNarrativeSignal {
            trend_state: InterpretationTrendState::Stable,
            trend_available: true,
            expectation_quality,
            expectation_quality_reason,
            gravity_data_quality,
            gravity_data_quality_reason:
                InterpretationGravityDataQualityReason::ProviderUnavailable,
            gravity_status: None,
            supply_pressure,
            supply_available: true,
            flow_acceleration,
            gray_rhino_escalated: false,
        }
    }

    fn future_context_unavailable() -> crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel
    {
        Default::default()
    }

    fn future_context_loaded_without_hit() -> crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel
    {
        crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel {
            pre_earnings_waiting:
                crate::features::radar::interface::signal_context_event_read_model::SignalContextEventSlot::Loaded(
                    None,
                ),
            ..Default::default()
        }
    }

    fn future_context_with_pre_earnings(snapshot: &ExpectationLayerSnapshot) -> crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel
    {
        crate::features::radar::interface::signal_context_event_read_model::build_signal_context_event_read_model(
            crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModelInput {
                as_of_date: snapshot.as_of_date,
                expectation_snapshot: Some(snapshot),
                future_calendar: None,
            },
        )
    }

    fn macro_event_observation(
        event_date: NaiveDate,
        importance: MacroEventImportance,
        lifecycle: MacroEventLifecycle,
        source_health: MacroEventSourceHealth,
        information_content: MacroEventInformationContent,
    ) -> MacroEventObservation {
        MacroEventObservation {
            event_id: "cpi-2026-06-18".to_string(),
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            event_date,
            event_time: Some("08:30".to_string()),
            timezone: "America/New_York".to_string(),
            country: "US".to_string(),
            event_type: MacroEventType::Cpi,
            event_name: "CPI Release".to_string(),
            source: "BLS".to_string(),
            source_url: "https://www.bls.gov/schedule/news_release/cpi.htm".to_string(),
            importance,
            lifecycle,
            expected_value: Some("2.9%".to_string()),
            actual_value: None,
            previous_value: Some("2.8%".to_string()),
            unit: Some("%".to_string()),
            surprise_state: MacroEventSurpriseState::NotAvailable,
            information_content,
            source_health,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 17).unwrap(),
        }
    }

    fn future_context_with_macro_event(
        observation: MacroEventObservation,
    ) -> crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel
    {
        let calendar = MacroEventCalendarReadModel::from_observations(
            observation.as_of_date,
            "inline".to_string(),
            vec![observation],
        );
        crate::features::radar::interface::signal_context_event_read_model::build_signal_context_event_read_model(
            crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModelInput {
                as_of_date: calendar.as_of_date,
                expectation_snapshot: None,
                future_calendar: Some(&calendar),
            },
        )
    }

    fn future_calendar_fact(
        as_of_date: NaiveDate,
        kind: FutureCalendarKind,
        event_date: NaiveDate,
        importance: MacroEventImportance,
        lifecycle: MacroEventLifecycle,
        source_health: MacroEventSourceHealth,
        information_content: MacroEventInformationContent,
    ) -> FutureCalendarObservation {
        FutureCalendarObservation {
            kind,
            event_id: format!("fact-{:?}-{}", kind, event_date),
            as_of_date,
            event_date,
            event_time: Some("08:30".to_string()),
            timezone: "America/New_York".to_string(),
            country: "US".to_string(),
            event_type: MacroEventType::Gdp,
            event_name: format!("{:?}", kind),
            source: "Official Calendar".to_string(),
            source_url: "https://example.com/calendar".to_string(),
            importance,
            lifecycle,
            expected_value: None,
            actual_value: None,
            previous_value: None,
            unit: None,
            surprise_state: MacroEventSurpriseState::NotAvailable,
            information_content,
            source_health,
            observed_at: event_date,
        }
    }

    fn future_context_with_fact(
        fact: FutureCalendarObservation,
    ) -> crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel
    {
        let calendar = MacroEventCalendarReadModel::from_observations(
            fact.as_of_date,
            "inline".to_string(),
            vec![fact],
        );
        crate::features::radar::interface::signal_context_event_read_model::build_signal_context_event_read_model(
            crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModelInput {
                as_of_date: calendar.as_of_date,
                expectation_snapshot: None,
                future_calendar: Some(&calendar),
            },
        )
    }

    #[test]
    fn pending_expectation_event_outside_near_term_returns_unknown_low() {
        let mut snapshot = build_expectation_layer_fixture_snapshot();
        for observation in &mut snapshot.observations {
            if observation.lifecycle_state == ExpectationLifecycleState::Pending {
                observation.period = "2026Q3".to_string();
            }
        }
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: snapshot.as_of_date,
            signal: signal(
                InterpretationExpectationQuality::Medium,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                true,
                Some(0.03),
            ),
            future_context: future_context_with_pre_earnings(&snapshot),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::Low);
        assert!(assessment
            .interpretation
            .contains("no high-information macro event"));
    }

    #[test]
    fn quarter_end_returns_low_high() {
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.08),
            ),
            future_context: future_context_unavailable(),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::QuarterEndRebalancing
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::High);
        assert!(assessment.interpretation.contains("quarter-end"));
    }

    #[test]
    fn future_context_unavailable_exports_source_diagnostics() {
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.08),
            ),
            future_context: future_context_unavailable(),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.source_health,
            MacroEventSourceHealth::Unavailable
        );
        assert!(assessment.event_fact.is_empty());
        assert!(assessment
            .source_diagnostics_summary
            .contains("Official Calendar unavailable"));
        assert!(assessment
            .source_diagnostics_appendix
            .contains("Official calendar source not loaded"));
    }

    #[test]
    fn index_reconstitution_returns_low() {
        let fact = future_calendar_fact(
            NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
            FutureCalendarKind::IndexReconstitution,
            NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let as_of_date = fact.as_of_date;
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date,
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.08),
            ),
            future_context: future_context_with_fact(fact),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::IndexReconstitution
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::High);
    }

    #[test]
    fn holiday_liquidity_returns_low() {
        let fact = future_calendar_fact(
            NaiveDate::from_ymd_opt(2026, 12, 24).unwrap(),
            FutureCalendarKind::HolidayLiquidity,
            NaiveDate::from_ymd_opt(2026, 12, 24).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let as_of_date = fact.as_of_date;
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date,
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.08),
            ),
            future_context: future_context_with_fact(fact),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::HolidayLiquidity
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
    }

    #[test]
    fn major_event_waiting_returns_low() {
        let fact = future_calendar_fact(
            NaiveDate::from_ymd_opt(2026, 6, 29).unwrap(),
            FutureCalendarKind::MajorEventWaiting,
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 29).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.08),
            ),
            future_context: future_context_with_fact(fact),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::MajorEventWaiting
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Medium
        );
    }

    #[test]
    fn etf_rebalance_returns_low() {
        let fact = future_calendar_fact(
            NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
            FutureCalendarKind::EtfRebalance,
            NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.08),
            ),
            future_context: future_context_with_fact(fact),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::EtfRebalance
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
    }

    #[test]
    fn month_end_returns_low_high() {
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                true,
                Some(0.12),
            ),
            future_context: future_context_unavailable(),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::MonthEndRebalancing
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::High);
        assert!(assessment.interpretation.contains("month-end"));
    }

    #[test]
    fn pending_expectation_event_returns_pre_earnings_waiting_low() {
        let snapshot = build_expectation_layer_fixture_snapshot();
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: snapshot.as_of_date,
            signal: signal(
                InterpretationExpectationQuality::Medium,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                true,
                Some(0.03),
            ),
            future_context: future_context_with_pre_earnings(&snapshot),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::PreEarningsWaiting
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Medium
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::Low);
        assert!(assessment.interpretation.contains("important event"));
    }

    #[test]
    fn official_macro_event_today_returns_macro_event_high() {
        let observation = macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Released,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: observation.as_of_date,
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.09),
            ),
            future_context: future_context_with_macro_event(observation),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::MacroEvent
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::High
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::High);
        assert_eq!(assessment.event_fact, "CPI Release / 2026-06-18 / BLS");
        assert!(assessment.source_diagnostics_summary.is_empty());
        assert!(assessment.source_diagnostics_appendix.is_empty());
        assert!(assessment
            .interpretation
            .contains("high-information macro event"));
        assert!(assessment
            .next_observation
            .contains("Waiting for the official release"));
    }

    #[test]
    fn macro_event_with_low_importance_does_not_trigger_macro_event() {
        let observation = macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::Low,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: observation.as_of_date,
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.09),
            ),
            future_context: future_context_with_macro_event(observation),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
    }

    #[test]
    fn macro_event_with_archived_lifecycle_does_not_trigger_macro_event() {
        let observation = macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Archived,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: observation.as_of_date,
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.09),
            ),
            future_context: future_context_with_macro_event(observation),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
    }

    #[test]
    fn macro_event_unavailable_keeps_none_unknown() {
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::Unavailable,
                InterpretationExpectationQualityReason::SystemUnavailable,
                InterpretationGravityDataQuality::Unavailable,
                false,
                None,
            ),
            future_context: crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel::default(),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Unknown
        );
        assert_eq!(
            assessment.context_quality,
            SignalContextQuality::Unavailable
        );
    }

    #[test]
    fn quarter_end_still_wins_over_macro_event_if_same_day() {
        let observation = macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Released,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                true,
                Some(0.05),
            ),
            future_context: future_context_with_macro_event(observation),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::QuarterEndRebalancing
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
    }

    #[test]
    fn no_context_with_missing_inputs_returns_unknown_unavailable() {
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::Unavailable,
                InterpretationExpectationQualityReason::SystemUnavailable,
                InterpretationGravityDataQuality::Unavailable,
                false,
                None,
            ),
            future_context: future_context_unavailable(),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Unknown
        );
        assert_eq!(
            assessment.context_quality,
            SignalContextQuality::Unavailable
        );
        assert!(assessment.interpretation.contains("UNKNOWN"));
    }

    #[test]
    fn no_context_with_loaded_inputs_returns_unknown_low() {
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::Medium,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                true,
                Some(0.03),
            ),
            future_context: future_context_loaded_without_hit(),
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Low
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::Low);
        assert!(assessment
            .interpretation
            .contains("no high-information macro event"));
    }
}
