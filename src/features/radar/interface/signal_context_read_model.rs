use crate::features::radar::interface::interpretation_read_model::InterpretationNarrativeSignal;
use crate::features::radar::interface::presentation::{
    SignalContextInformationContent, SignalContextInformationLevel, SignalContextPrimaryContext,
    SignalContextQuality,
};
use crate::features::radar::interface::presentation::{
    SignalContextItem, SignalContextType, SignalContextV1,
};
use crate::features::radar::interface::signal_context_coverage::{
    build_v1_from_event_context, corporate_event_item, corporate_events_match,
};
use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
use crate::features::research::application::corporate_event_evidence_resolver::{
    CorporateEventEvidenceLifecycle, CorporateEventEvidenceResolution,
};
use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
use crate::features::shared::interface::i18n::Language;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeSet;

#[derive(Debug)]
pub(crate) struct SignalContextReadModelInput {
    pub as_of_date: NaiveDate,
    pub signal: InterpretationNarrativeSignal,
    pub future_context: SignalContextEventReadModel,
    pub language: Language,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SignalContextAssessment {
    pub v1: SignalContextV1,
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
    let v1 = build_v1_from_event_context(input.as_of_date, &input.future_context);
    let primary_context = derive_primary_context(input.as_of_date, &input.future_context, &v1);
    let information_content =
        derive_information_content(primary_context, &input.future_context, &v1);
    let context_quality = derive_context_quality(primary_context, &input.future_context, &v1);
    let event_fact = compose_event_fact(&input.future_context, &v1);
    let source_health = input.future_context.source_health;
    let (source_diagnostics_summary, mut source_diagnostics_appendix) =
        compose_source_diagnostics(input.as_of_date, &input.future_context, &v1, input.language);
    let coverage_line = format_signal_context_coverage(&v1.coverage);
    if source_diagnostics_appendix.is_empty() {
        source_diagnostics_appendix = coverage_line;
    } else {
        source_diagnostics_appendix = format!("{coverage_line}\n{source_diagnostics_appendix}");
    }
    let interpretation = compose_interpretation(
        primary_context,
        information_content,
        context_quality,
        &input.future_context,
        &v1,
        input.language,
    );
    let next_observation = compose_next_observation(
        input.as_of_date,
        primary_context,
        information_content,
        &input.future_context,
        &v1,
        input.language,
    );

    SignalContextAssessment {
        v1,
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

fn format_signal_context_coverage(
    coverage: &crate::features::radar::interface::presentation::SignalContextCoverage,
) -> String {
    format!(
        "Coverage: {:?}; scheduled_macro={:?}, corporate={:?}, geopolitical={:?}, commodity={:?}, rates_credit={:?}, market_structure={:?}",
        coverage.overall,
        coverage.scheduled_macro,
        coverage.corporate,
        coverage.geopolitical,
        coverage.commodity,
        coverage.rates_credit,
        coverage.market_structure,
    )
    .to_uppercase()
}

pub(crate) fn signal_context_information_content_label(
    value: SignalContextInformationContent,
) -> &'static str {
    match value {
        SignalContextInformationContent::High => "HIGH",
        SignalContextInformationContent::Medium => "MEDIUM",
        SignalContextInformationContent::Low => "LOW",
        SignalContextInformationContent::Unknown => "UNAVAILABLE",
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
        SignalContextPrimaryContext::CorporateEvent => "Corporate Event",
        SignalContextPrimaryContext::None => "None",
    }
}

pub(crate) fn signal_context_type_value(item: Option<&SignalContextItem>) -> String {
    let Some(item) = item else {
        return "UNAVAILABLE".to_string();
    };

    let category = match item.context_type {
        SignalContextType::ScheduledMacro => "SCHEDULED MACRO",
        SignalContextType::Corporate => "CORPORATE",
        SignalContextType::Geopolitical => "GEOPOLITICAL",
        SignalContextType::Commodity => "COMMODITY",
        SignalContextType::RatesCredit => "RATES/CREDIT",
        SignalContextType::MarketStructure => "MARKET STRUCTURE",
    };
    let detail = item
        .evidence
        .iter()
        .map(|evidence| evidence.event_type.trim())
        .find(|event_type| !event_type.is_empty())
        .map(normalize_signal_context_type_detail);

    match detail {
        Some(detail) if detail != category => format!("{category} / {detail}"),
        _ => category.to_string(),
    }
}

fn normalize_signal_context_type_detail(value: &str) -> String {
    value
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

pub(crate) fn signal_context_quality_label(value: SignalContextQuality) -> &'static str {
    match value {
        SignalContextQuality::High => "HIGH",
        SignalContextQuality::Medium => "MEDIUM",
        SignalContextQuality::Low => "LOW",
        SignalContextQuality::Unavailable => "UNAVAILABLE",
    }
}

pub(crate) fn signal_context_lifecycle_label(
    value: crate::features::radar::interface::presentation::SignalContextLifecycle,
) -> &'static str {
    match value {
        crate::features::radar::interface::presentation::SignalContextLifecycle::Upcoming => "UPCOMING",
        crate::features::radar::interface::presentation::SignalContextLifecycle::Released => "RELEASED",
        crate::features::radar::interface::presentation::SignalContextLifecycle::ActiveRepricing => "ACTIVE_REPRICING",
        crate::features::radar::interface::presentation::SignalContextLifecycle::Aftermath => "AFTERMATH",
        crate::features::radar::interface::presentation::SignalContextLifecycle::Expired => "EXPIRED",
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
    v1: &SignalContextV1,
) -> SignalContextPrimaryContext {
    if let Some(context) = future_context.detected_primary_context() {
        return context;
    }

    if let Some(item) = external_primary_item(future_context, v1) {
        if matches!(
            item.information_content,
            SignalContextInformationLevel::High | SignalContextInformationLevel::Medium
        ) {
            return match item.context_type {
                SignalContextType::Corporate => SignalContextPrimaryContext::CorporateEvent,
                _ => SignalContextPrimaryContext::MacroEvent,
            };
        }
    }

    if is_quarter_end(as_of_date) {
        return SignalContextPrimaryContext::QuarterEndRebalancing;
    }

    if is_month_end(as_of_date) {
        return SignalContextPrimaryContext::MonthEndRebalancing;
    }

    SignalContextPrimaryContext::None
}

fn derive_information_content(
    primary_context: SignalContextPrimaryContext,
    future_context: &SignalContextEventReadModel,
    v1: &SignalContextV1,
) -> SignalContextInformationContent {
    match primary_context {
        // 機械性コンテキストも六つのソースが HEALTHY である場合だけ LOW とする。
        SignalContextPrimaryContext::QuarterEndRebalancing
        | SignalContextPrimaryContext::MonthEndRebalancing
        | SignalContextPrimaryContext::IndexReconstitution
        | SignalContextPrimaryContext::EtfRebalance
        | SignalContextPrimaryContext::HolidayLiquidity => {
            if v1.coverage.overall == crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy {
                SignalContextInformationContent::Low
            } else {
                SignalContextInformationContent::Unknown
            }
        }
        SignalContextPrimaryContext::MacroEvent | SignalContextPrimaryContext::CorporateEvent => {
            if future_context.detected_primary_context()
                == Some(SignalContextPrimaryContext::MacroEvent)
            {
                SignalContextInformationContent::High
            } else {
                match v1.overall_information_content {
                    SignalContextInformationLevel::High => SignalContextInformationContent::High,
                    SignalContextInformationLevel::Medium => {
                        SignalContextInformationContent::Medium
                    }
                    SignalContextInformationLevel::Low => SignalContextInformationContent::Low,
                    SignalContextInformationLevel::Unavailable => {
                        SignalContextInformationContent::Unknown
                    }
                }
            }
        }
        SignalContextPrimaryContext::MajorEventWaiting
        | SignalContextPrimaryContext::PreEarningsWaiting => {
            SignalContextInformationContent::Medium
        }
        SignalContextPrimaryContext::None => {
            // 公式日历が成功しても、企業・地政学・商品・金利/信用・市場構造を
            // 全て走査した証拠がない限り、LOW や「無事件」には降格しない。
            let _ = future_context;
            SignalContextInformationContent::Unknown
        }
    }
}

fn derive_context_quality(
    primary_context: SignalContextPrimaryContext,
    future_context: &SignalContextEventReadModel,
    v1: &SignalContextV1,
) -> SignalContextQuality {
    match primary_context {
        SignalContextPrimaryContext::QuarterEndRebalancing
        | SignalContextPrimaryContext::MonthEndRebalancing => {
            if v1.coverage.overall
                == crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy
            {
                SignalContextQuality::High
            } else {
                SignalContextQuality::Unavailable
            }
        }
        SignalContextPrimaryContext::CorporateEvent => external_primary_item(future_context, v1)
            .map(|item| match item.evidence_quality {
                SignalContextInformationLevel::High => SignalContextQuality::High,
                SignalContextInformationLevel::Medium => SignalContextQuality::Medium,
                SignalContextInformationLevel::Low => SignalContextQuality::Low,
                SignalContextInformationLevel::Unavailable => SignalContextQuality::Unavailable,
            })
            .unwrap_or(SignalContextQuality::Unavailable),
        SignalContextPrimaryContext::IndexReconstitution
        | SignalContextPrimaryContext::EtfRebalance
        | SignalContextPrimaryContext::HolidayLiquidity
        | SignalContextPrimaryContext::PreEarningsWaiting
        | SignalContextPrimaryContext::MajorEventWaiting
        | SignalContextPrimaryContext::MacroEvent => {
            if external_primary_item(future_context, v1).is_some() {
                return v1
                    .context_quality
                    .map_high_quality_to_medium(v1.coverage.overall);
            }
            future_context
                .evidence_quality_for(primary_context)
                .unwrap_or_else(|| {
                    if future_context.has_loaded_context() {
                        SignalContextQuality::Low
                    } else {
                        SignalContextQuality::Unavailable
                    }
                })
                .map_high_quality_to_medium(v1.coverage.overall)
        }
        SignalContextPrimaryContext::None => {
            let _ = future_context;
            SignalContextQuality::Unavailable
        }
    }
}

trait CoverageQualityGuard {
    fn map_high_quality_to_medium(
        self,
        coverage: crate::features::radar::interface::presentation::SignalContextSourceStatus,
    ) -> SignalContextQuality;
}

impl CoverageQualityGuard for SignalContextQuality {
    fn map_high_quality_to_medium(
        self,
        coverage: crate::features::radar::interface::presentation::SignalContextSourceStatus,
    ) -> SignalContextQuality {
        if coverage
            != crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy
            && self == SignalContextQuality::High
        {
            SignalContextQuality::Medium
        } else {
            self
        }
    }
}

fn external_primary_item<'a>(
    future_context: &SignalContextEventReadModel,
    v1: &'a SignalContextV1,
) -> Option<&'a crate::features::radar::interface::presentation::SignalContextItem> {
    let item = v1.primary_context.as_ref()?;
    if item.context_type
        != crate::features::radar::interface::presentation::SignalContextType::ScheduledMacro
        || !future_context
            .timeline_entries
            .iter()
            .any(|entry| entry.event_name == item.title)
    {
        Some(item)
    } else {
        None
    }
}

fn is_provider_backed_corporate_item(
    future_context: &SignalContextEventReadModel,
    item: &crate::features::radar::interface::presentation::SignalContextItem,
) -> bool {
    if item.context_type
        != crate::features::radar::interface::presentation::SignalContextType::Corporate
    {
        return false;
    }
    future_context
        .corporate_event_provider
        .events
        .iter()
        .any(|event| {
            let provider_item = corporate_event_item(event, event.market_date);
            corporate_events_match(&provider_item, item)
        })
        || future_context
            .corporate_event_evidence
            .events
            .iter()
            .any(|evidence| {
                evidence.lifecycle != CorporateEventEvidenceLifecycle::Unavailable
                    && item.symbol.as_deref() == Some(evidence.subject.as_str())
            })
}

fn compose_interpretation(
    primary_context: SignalContextPrimaryContext,
    information_content: SignalContextInformationContent,
    context_quality: SignalContextQuality,
    future_context: &SignalContextEventReadModel,
    v1: &SignalContextV1,
    language: Language,
) -> String {
    match primary_context {
        SignalContextPrimaryContext::MacroEvent => macro_event_text(
            future_context,
            information_content,
            context_quality,
            language,
        ),
        SignalContextPrimaryContext::CorporateEvent => {
            corporate_event_text(v1, information_content, context_quality, language)
        }
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
            if context_quality == SignalContextQuality::Unavailable {
                SignalContextInformationContent::Unknown
            } else {
                information_content
            },
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
    as_of_date: NaiveDate,
    future_context: &SignalContextEventReadModel,
    v1: &SignalContextV1,
    language: Language,
) -> (String, String) {
    if let Some(item) = external_primary_item(future_context, v1) {
        let provider_backed = is_provider_backed_corporate_item(future_context, item);
        let mut appendix = future_context
            .corporate_event_provider
            .diagnostic
            .as_deref()
            .map(|diagnostic| format!("{}: {}", provider_diagnostic_label(language), diagnostic))
            .unwrap_or_default();
        append_source_diagnostics_line(
            &mut appendix,
            corporate_event_evidence_appendix(&future_context.corporate_event_evidence, language),
        );
        return (
            match language {
                Language::ZhCn if provider_backed => {
                    format!("企业事件 Provider 已加载：{}。", item.title)
                }
                Language::ZhCn => format!("已加载外部企业事件上下文：{}。", item.title),
                Language::EnUs if provider_backed => {
                    format!("Corporate event Provider loaded: {}.", item.title)
                }
                Language::EnUs => {
                    format!("External corporate event context loaded: {}.", item.title)
                }
                Language::JaJp if provider_backed => {
                    format!("企業イベント Provider を読み込んだ: {}。", item.title)
                }
                Language::JaJp => format!("外部企業イベント文脈を読み込んだ: {}。", item.title),
            },
            appendix,
        );
    }
    let timeline_lines = format_event_timeline_lines(as_of_date, future_context, language);
    let runtime_coverage_incomplete = future_context.runtime_coverage.as_ref().is_some_and(
        |coverage| {
            coverage.overall
                != crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy
        },
    );
    if future_context.source_health == MacroEventSourceHealth::Succeeded
        && !runtime_coverage_incomplete
        && future_context.corporate_event_provider.diagnostic.is_none()
        && future_context.corporate_event_evidence.events.is_empty()
        && future_context
            .corporate_event_evidence
            .provider_health
            .is_empty()
    {
        return (String::new(), String::new());
    }
    let detail = future_context
        .source_diagnostic
        .as_deref()
        .or(future_context
            .corporate_event_provider
            .diagnostic
            .as_deref())
        .unwrap_or(match language {
            Language::ZhCn => "没有额外诊断信息",
            Language::EnUs => "no extra diagnostic information",
            Language::JaJp => "追加の診断情報はない",
        });
    let summary = if timeline_lines.is_empty() {
        if runtime_coverage_incomplete {
            match language {
                Language::ZhCn => {
                    "来源覆盖不完整，当前无法确认是否存在高信息量事件；不作无事件结论。".to_string()
                }
                Language::EnUs => {
                    "Available source coverage is incomplete; whether a high-information event exists cannot be confirmed, so no absence conclusion is made.".to_string()
                }
                Language::JaJp => {
                    "利用可能なソースのカバレッジが不完全なため、高情報量イベントの有無を確認できず、無イベントの結論は出さない。".to_string()
                }
            }
        } else {
            match future_context.source_health {
            MacroEventSourceHealth::Unavailable => match language {
                Language::ZhCn => {
                    "Official Calendar unavailable; current monitoring remains idle.".to_string()
                }
                Language::EnUs => {
                    "Official Calendar unavailable; current monitoring remains idle.".to_string()
                }
                Language::JaJp => "利用可能なソース上では高情報量イベントを特定できず、監視は待機中。".to_string(),
            },
            _ => match language {
                Language::ZhCn => {
                    "No high-information event identified from available sources. Current monitoring remains idle.".to_string()
                }
                Language::EnUs => {
                    "No high-information event identified from available sources. Current monitoring remains idle.".to_string()
                }
                Language::JaJp => "利用可能なソース上では高情報量イベントを特定できず、監視は待機中。".to_string(),
            },
            }
        }
    } else {
        let first = &timeline_lines[0];
        if timeline_lines.len() == 1 {
            first.clone()
        } else {
            format!("{} {}", first, timeline_lines[1])
        }
    };
    let mut appendix = if timeline_lines.is_empty() {
        match language {
            Language::ZhCn => format!("No timeline available; {detail}"),
            Language::EnUs => format!("No timeline available; {detail}"),
            Language::JaJp => format!("タイムラインなし; {detail}"),
        }
    } else {
        timeline_lines.join("\n")
    };
    if let Some(diagnostic) = future_context
        .corporate_event_provider
        .diagnostic
        .as_deref()
    {
        let provider_line = format!("{}: {}", provider_diagnostic_label(language), diagnostic);
        if !appendix.contains(diagnostic) {
            if !appendix.is_empty() {
                appendix.push('\n');
            }
            appendix.push_str(&provider_line);
        }
    }
    if future_context.source_health != MacroEventSourceHealth::Succeeded {
        append_source_diagnostics_line(
            &mut appendix,
            match language {
                Language::ZhCn => {
                    format!(
                        "Official calendar source health: {}",
                        signal_context_source_health_label(future_context.source_health)
                    )
                }
                Language::EnUs => {
                    format!(
                        "Official calendar source health: {}",
                        signal_context_source_health_label(future_context.source_health)
                    )
                }
                Language::JaJp => {
                    format!(
                        "公式カレンダー source health: {}",
                        signal_context_source_health_label(future_context.source_health)
                    )
                }
            },
        );
    }
    append_source_diagnostics_line(
        &mut appendix,
        corporate_event_evidence_appendix(&future_context.corporate_event_evidence, language),
    );
    (summary, appendix)
}

fn append_source_diagnostics_line(appendix: &mut String, line: String) {
    if line.is_empty() {
        return;
    }
    if !appendix.is_empty() {
        appendix.push('\n');
    }
    appendix.push_str(&line);
}

fn corporate_event_evidence_appendix(
    resolution: &CorporateEventEvidenceResolution,
    language: Language,
) -> String {
    if resolution.events.is_empty() && resolution.provider_health.is_empty() {
        return String::new();
    }
    let health = if resolution.provider_health.is_empty() {
        "UNAVAILABLE".to_string()
    } else {
        resolution
            .provider_health
            .iter()
            .map(|provider| {
                let diagnostic = provider
                    .diagnostic
                    .as_deref()
                    .map(|value| format!(" ({value})"))
                    .unwrap_or_default();
                format!(
                    "{}={:?}{}",
                    provider.provider_id, provider.health, diagnostic
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    let header = match language {
        Language::ZhCn => "企业事件证据健康度",
        Language::EnUs => "Corporate Event Evidence Health",
        Language::JaJp => "企業イベント証拠 health",
    };
    let mut lines = vec![format!("{header}: {health}")];
    for event in &resolution.events {
        let sources = event
            .evidence
            .iter()
            .map(|evidence| evidence.source.provider_id.clone())
            .filter(|provider_id| !provider_id.is_empty())
            .collect::<BTreeSet<_>>();
        let sources = if sources.is_empty() {
            "UNAVAILABLE".to_string()
        } else {
            sources.into_iter().collect::<Vec<_>>().join(", ")
        };
        lines.push(format!(
            "{}: lifecycle={:?}; expected={}; confirmed={}; sources={}",
            event.subject,
            event.lifecycle,
            event
                .expected_date
                .map(|date| date.to_string())
                .unwrap_or_else(|| "UNAVAILABLE".to_string()),
            event
                .confirmed_event_date
                .map(|date| date.to_string())
                .unwrap_or_else(|| "UNAVAILABLE".to_string()),
            sources,
        ));
    }
    lines.join("\n")
}

fn provider_diagnostic_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "企业事件 Provider 诊断",
        Language::EnUs => "Corporate event Provider diagnostic",
        Language::JaJp => "企業イベント Provider 診断",
    }
}

fn compose_next_observation(
    as_of_date: NaiveDate,
    primary_context: SignalContextPrimaryContext,
    information_content: SignalContextInformationContent,
    future_context: &SignalContextEventReadModel,
    v1: &SignalContextV1,
    language: Language,
) -> String {
    if primary_context == SignalContextPrimaryContext::CorporateEvent {
        let title = external_primary_item(future_context, v1)
            .map(|item| item.title.as_str())
            .filter(|title| !title.trim().is_empty())
            .unwrap_or("the corporate event");
        return match language {
            Language::ZhCn => {
                format!("观察 {title} 后的市场反应能否持续；当前持续性尚未确认。")
            }
            Language::EnUs => format!(
                "Observe whether the market response persists after {title}; persistence is not yet confirmed."
            ),
            Language::JaJp => {
                format!("{title} 後の市場反応が持続するかを観察する。持続性はまだ確認されていない。")
            }
        };
    }
    let timeline_lines = format_event_timeline_lines(as_of_date, future_context, language);
    if timeline_lines.is_empty() {
        return match (primary_context, information_content, future_context.source_health) {
            (_, _, MacroEventSourceHealth::Unavailable) if !future_context.has_loaded_context() => {
                match language {
                    Language::ZhCn => "官方来源当前不可用；待来源恢复后再确认是否存在事件".to_string(),
                    Language::EnUs => "Official source is currently unavailable; verify if events exist once the source is restored.".to_string(),
                    Language::JaJp => "公式ソースは現在利用できません。ソース復旧後にイベントが存在するかどうかを再確認します。".to_string(),
                }
            }
            (_, SignalContextInformationContent::Medium, _) => match language {
                Language::ZhCn => "观察中等重要事件的后续公布，并在结果落地后重新评估预期修正。".to_string(),
                Language::EnUs => "Watching the next release for a medium-importance event, then re-evaluating the expectation adjustment once it lands.".to_string(),
                Language::JaJp => "中重要度イベントの次回公表を観察し、結果が出たら期待修正を再評価する。".to_string(),
            },
            (_, SignalContextInformationContent::Low, _) => match language {
                Language::ZhCn => "No high-information event identified from available sources. Current monitoring remains idle.".to_string(),
                Language::EnUs => "No high-information event identified from available sources. Current monitoring remains idle.".to_string(),
            Language::JaJp => "利用可能なソース上では高情報量イベントを特定できず、監視は待機中。".to_string(),
            },
            _ => match language {
                Language::ZhCn => "等待官方公布。公布后系统将自动对比 Expected / Actual、计算 Surprise、更新 Narrative。".to_string(),
                Language::EnUs => "Waiting for the official release. After publication the system will automatically compare Expected / Actual, calculate Surprise, and refresh the Narrative.".to_string(),
                Language::JaJp => "公式発表を待機中。公表後は Expected / Actual を比較し、Surprise を計算して Narrative を更新する。".to_string(),
            },
        };
    }

    let head = if timeline_lines[0].starts_with("Today:")
        || timeline_lines[0].starts_with("今日:")
        || timeline_lines[0].starts_with("本日:")
    {
        timeline_lines[0].clone()
    } else {
        match language {
            Language::ZhCn => format!("Available event context: {}", timeline_lines[0]),
            Language::EnUs => format!("Available event context: {}", timeline_lines[0]),
            Language::JaJp => format!("利用可能なイベント文脈: {}", timeline_lines[0]),
        }
    };
    if timeline_lines.len() == 1 {
        return head;
    }
    format!("{} {}", head, timeline_lines[1..].join(" "))
}

fn format_event_timeline_lines(
    as_of_date: NaiveDate,
    future_context: &SignalContextEventReadModel,
    language: Language,
) -> Vec<String> {
    let mut entries = future_context
        .timeline_entries
        .iter()
        .map(|entry| {
            let offset = trading_day_offset(as_of_date, entry.event_date);
            let prefix = timeline_prefix(offset, language);
            let title = if entry.importance.is_some() {
                entry.event_name.clone()
            } else {
                entry.summary.clone()
            };
            let mut line = format!("{}: {}", prefix, title);
            if entry.high_information {
                line.push_str(match language {
                    Language::ZhCn => " (high information)",
                    Language::EnUs => " (high information)",
                    Language::JaJp => " (high information)",
                });
            }
            line
        })
        .collect::<Vec<_>>();
    entries.truncate(3);
    entries
}

fn timeline_prefix(offset: i64, language: Language) -> String {
    match (offset, language) {
        (0, Language::ZhCn) => "今天".to_string(),
        (0, Language::EnUs) => "Today".to_string(),
        (0, Language::JaJp) => "本日".to_string(),
        (1, Language::ZhCn) => "UpcomingTomorrow".to_string(),
        (1, Language::EnUs) => "UpcomingTomorrow".to_string(),
        (1, Language::JaJp) => "UpcomingTomorrow".to_string(),
        (n, Language::ZhCn) => format!("{} 个交易日后", n),
        (n, Language::EnUs) => format!("In {} trading days", n),
        (n, Language::JaJp) => format!("{} 営業日後", n),
    }
}

fn trading_day_offset(from: NaiveDate, to: NaiveDate) -> i64 {
    if to <= from {
        return 0;
    }

    let mut current = from;
    let mut count = 0i64;
    while current < to {
        current = current.succ_opt().unwrap_or(current);
        match current.weekday() {
            chrono::Weekday::Sat | chrono::Weekday::Sun => {}
            _ => count += 1,
        }
    }
    count
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

fn compose_event_fact(
    future_context: &SignalContextEventReadModel,
    v1: &SignalContextV1,
) -> String {
    external_primary_item(future_context, v1)
        .map(|item| item.event_fact.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| future_context.detected_primary_evidence_summary())
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
                "当前来源无法确认今天是否存在高信息量事件，Signal Context 标记为 UNAVAILABLE。"
                    .to_string()
            }
            Language::EnUs => {
                let _ = (information_content, future_context);
                "Available sources cannot confirm whether today has a high-information event, so Signal Context is UNAVAILABLE.".to_string()
            }
            Language::JaJp => {
                let _ = (information_content, future_context);
                "利用可能なソースでは今日は高情報量イベントの有無を確認できず、Signal Context は UNAVAILABLE とする。".to_string()
            }
        },
        SignalContextQuality::Low => match language {
            Language::ZhCn => {
                let _ = (information_content, future_context);
                "今天未识别到高信息量宏观事件。官方经济日历未命中 CPI、FOMC、就业、GDP 等事件。今日价格变化更可能由企业消息、板块轮动、技术走势驱动。".to_string()
            }
            Language::EnUs => {
                let _ = (information_content, future_context);
                "No high-information event identified from available sources. Macro, corporate, geopolitical, commodity, rates/credit, and market-structure scans found no HIGH or MEDIUM event; price changes are more likely driven by ordinary rotation or technical structure."
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
    let _ = future_context;
    if information_content == SignalContextInformationContent::Unknown {
        return match language {
            Language::ZhCn => {
                "当前来源覆盖不完整，无法确认机械性再平衡是否为主要驱动。".to_string()
            }
            Language::EnUs => {
                "Available source coverage is incomplete; the mechanical context cannot be confirmed as the primary driver.".to_string()
            }
            Language::JaJp => {
                "利用可能なソースのカバレッジが不完全なため、機械的要因を主要ドライバーとして確認できない。".to_string()
            }
        };
    }
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
                    "今天识别到高信息量宏观事件；如有同步市场反应，该反应可能与新的宏观信息重新定价一致。信息含量: {info}。"
                )
            } else {
                format!("今天识别到高信息量宏观事件: {event_fact}。观察到的市场反应可能与新的宏观信息重新定价一致。信息含量: {info}。")
            }
        }
        Language::EnUs => {
            let _ = context_quality;
            let info = signal_context_information_content_label(information_content);
            if event_fact.is_empty() {
                format!("A high-information macro event was identified today. Observed market reactions, when available, are consistent with repricing the new macro information. Information content: {info}.")
            } else {
                format!("A high-information macro event was identified today: {event_fact}. Observed market reactions may be consistent with repricing the new macro information. Information content: {info}.")
            }
        }
        Language::JaJp => {
            let _ = context_quality;
            let info = signal_context_information_content_label(information_content);
            if event_fact.is_empty() {
                format!("今日は高情報量のマクロイベントが識別された。観測された市場反応があれば、新しいマクロ情報の再価格付けと整合的である可能性がある。情報含量: {info}。")
            } else {
                format!("今日は高情報量のマクロイベントが識別された: {event_fact}。観測された市場反応は新しいマクロ情報の再価格付けと整合的である可能性がある。情報含量: {info}。")
            }
        }
    }
}

fn corporate_event_text(
    v1: &SignalContextV1,
    information_content: SignalContextInformationContent,
    context_quality: SignalContextQuality,
    language: Language,
) -> String {
    let _ = context_quality;
    let event = v1.primary_context.as_ref();
    let title = event
        .map(|item| item.title.as_str())
        .filter(|title| !title.trim().is_empty())
        .unwrap_or("the corporate event");
    let fact = event
        .and_then(|item| (!item.event_fact.trim().is_empty()).then_some(item.event_fact.as_str()))
        .unwrap_or("");
    let info = signal_context_information_content_label(information_content);
    let (event_prefix_zh, event_prefix_en, event_prefix_ja) =
        if information_content == SignalContextInformationContent::High {
            (
                "今天识别到高信息量企业事件",
                "A high-information corporate event was identified",
                "高情報量の企業イベントが識別された",
            )
        } else {
            (
                "今天识别到中等信息量企业事件",
                "A medium-information corporate event was identified",
                "中情報量の企業イベントが識別された",
            )
        };
    match language {
        Language::ZhCn => {
            if fact.is_empty() {
                format!("{event_prefix_zh}：{title}。观察到的市场反应可能受该事件驱动，但事件后的持续性尚未确认。信息含量：{info}。")
            } else {
                format!("{event_prefix_zh}：{title}。事件事实：{fact} 观察到的市场反应可能受该事件驱动，但事件后的持续性尚未确认。信息含量：{info}。")
            }
        }
        Language::EnUs => {
            if fact.is_empty() {
                format!("{event_prefix_en}: {title}. The observed market reaction may be event-driven, but persistence after the event is not yet confirmed. Information content: {info}.")
            } else {
                format!("{event_prefix_en}: {title}. Event fact: {fact} The observed market reaction may be event-driven, but persistence after the event is not yet confirmed. Information content: {info}.")
            }
        }
        Language::JaJp => {
            if fact.is_empty() {
                format!("{event_prefix_ja}: {title}。観測された市場反応はこのイベントに起因する可能性があるが、イベント後の持続性はまだ確認されていない。情報含量: {info}。")
            } else {
                format!("{event_prefix_ja}: {title}。イベント事実: {fact} 観測された市場反応はこのイベントに起因する可能性があるが、イベント後の持続性はまだ確認されていない。情報含量: {info}。")
            }
        }
    }
}

fn signal_context_source_health_label(value: MacroEventSourceHealth) -> &'static str {
    match value {
        MacroEventSourceHealth::Succeeded => "HEALTHY",
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
    use crate::features::research::application::corporate_event_evidence_resolver::{
        CorporateEventEvidence, CorporateEventEvidenceLifecycle,
        CorporateEventEvidenceProviderHealth, CorporateEventEvidenceProviderHealthRecord,
        CorporateEventEvidenceRef, CorporateEventEvidenceResolution, EvidenceConfidence,
    };
    use crate::features::research::application::corporate_event_provider::{
        CorporateEventObservation, CorporateEventProviderHealth, CorporateEventProviderReadModel,
        CorporateEventReleaseWindow, CorporateEventSource, CorporateEventSourceKind,
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

    fn finnhub_source(url: Option<&str>) -> CorporateEventSource {
        CorporateEventSource {
            provider_id: "finnhub".to_string(),
            source_kind: CorporateEventSourceKind::EarningsCalendar,
            source_url: url.map(str::to_string),
        }
    }

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

    #[test]
    fn provider_earnings_is_rendered_as_high_corporate_context_with_weak_causality() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let future_context = crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel {
            corporate_event_provider: CorporateEventProviderReadModel {
                health: CorporateEventProviderHealth::Healthy,
                source: finnhub_source(Some("https://finnhub.io/api/v1/calendar/earnings")),
                events: vec![CorporateEventObservation {
                    symbol: "NVDA".to_string(),
                    market_date,
                    market_timezone: "America/New_York".to_string(),
                    release_window: CorporateEventReleaseWindow::AfterMarketClose,
                    fiscal_quarter: 2,
                    fiscal_year: 2027,
                    revenue_actual: Some(96_200_000_000.0),
                    revenue_estimate: Some(95_000_000_000.0),
                    source: finnhub_source(Some("https://finnhub.io/api/v1/calendar/earnings")),
                    observed_at: "2026-08-27T20:00:00Z".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: market_date,
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                None,
            ),
            future_context,
            language: Language::ZhCn,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::CorporateEvent
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::High
        );
        assert!(assessment.interpretation.contains("可能受该事件驱动"));
        assert!(assessment.interpretation.contains("持续性尚未确认"));
        assert!(assessment
            .source_diagnostics_summary
            .contains("企业事件 Provider"));
        assert!(!assessment
            .source_diagnostics_summary
            .contains("外部企业事件"));
    }

    #[test]
    fn provider_failure_diagnostic_is_rendered_without_changing_decision_boundary() {
        let mut future_context = future_context_unavailable();
        future_context.corporate_event_provider = CorporateEventProviderReadModel::unavailable(
            finnhub_source(Some("https://finnhub.io/api/v1/calendar/earnings")),
            "Finnhub earnings API returned HTTP 429",
        );
        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::Unavailable,
                InterpretationExpectationQualityReason::SystemUnavailable,
                InterpretationGravityDataQuality::Unavailable,
                false,
                None,
            ),
            future_context,
            language: Language::EnUs,
        });

        assert!(assessment
            .source_diagnostics_appendix
            .contains("Finnhub earnings API returned HTTP 429"));
        assert_eq!(assessment.v1.decision_weight, 0);
        assert!(!assessment.v1.trade_signal);
        assert_eq!(assessment.v1.gate_effect, "none");
        assert_eq!(assessment.v1.execution_effect, "none");
        assert_eq!(assessment.v1.position_sizing_effect, "none");
    }

    #[test]
    fn provider_failure_diagnostic_is_preserved_alongside_macro_timeline() {
        let mut future_context = future_context_unavailable();
        future_context.source_health = MacroEventSourceHealth::Succeeded;
        future_context.timeline_entries = vec![
            crate::features::radar::interface::signal_context_event_read_model::SignalContextTimelineEntry {
                event_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                event_name: "FOMC decision".to_string(),
                event_type: "MacroEvent".to_string(),
                source: "Federal Reserve".to_string(),
                summary: "FOMC decision".to_string(),
                high_information: true,
                ..Default::default()
            },
        ];
        future_context.corporate_event_provider = CorporateEventProviderReadModel::unavailable(
            finnhub_source(Some("https://finnhub.io/api/v1/calendar/earnings")),
            "Finnhub earnings API returned HTTP 429",
        );
        let assessment = without_external_fixture(|| {
            build_signal_context_assessment(SignalContextReadModelInput {
                as_of_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                signal: signal(
                    InterpretationExpectationQuality::Unavailable,
                    InterpretationExpectationQualityReason::SystemUnavailable,
                    InterpretationGravityDataQuality::Unavailable,
                    false,
                    None,
                ),
                future_context,
                language: Language::EnUs,
            })
        });

        assert!(assessment
            .source_diagnostics_appendix
            .contains("FOMC decision"));
        assert!(assessment
            .source_diagnostics_appendix
            .contains("Finnhub earnings API returned HTTP 429"));
    }

    #[test]
    fn corporate_event_evidence_health_and_provenance_are_rendered_in_appendix() {
        let observed_at = chrono::DateTime::parse_from_rfc3339("2026-08-27T18:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let mut future_context = future_context_unavailable();
        future_context.source_health = MacroEventSourceHealth::Succeeded;
        future_context.corporate_event_evidence = CorporateEventEvidenceResolution {
            events: vec![CorporateEventEvidence {
                subject: "NVDA".to_string(),
                event_type:
                    crate::features::research::application::corporate_event_provider::CorporateEventType::Earnings,
                lifecycle: CorporateEventEvidenceLifecycle::Scheduled,
                expected_date: Some(NaiveDate::from_ymd_opt(2026, 8, 28).unwrap()),
                confirmed_event_date: None,
                confirmed_at: None,
                confidence: EvidenceConfidence::Medium,
                evidence: vec![CorporateEventEvidenceRef {
                    source: CorporateEventSource {
                        provider_id: "alpha_vantage".to_string(),
                        source_kind: CorporateEventSourceKind::EarningsCalendar,
                        source_url: None,
                    },
                    event_date: NaiveDate::from_ymd_opt(2026, 8, 28).unwrap(),
                    observed_at,
                    accepted_at: None,
                    source_timestamp: None,
                    fact_kind: "ExpectedEvent".to_string(),
                }],
                diagnostics: Vec::new(),
                expected_value: None,
                actual_value: None,
                theme: None,
                importance: None,
                structured_explanation: None,
            }],
            provider_health: vec![
                CorporateEventEvidenceProviderHealthRecord {
                    provider_id: "alpha_vantage".to_string(),
                    health: CorporateEventEvidenceProviderHealth::Healthy,
                    diagnostic: None,
                },
                CorporateEventEvidenceProviderHealthRecord {
                    provider_id: "sec-edgar".to_string(),
                    health: CorporateEventEvidenceProviderHealth::Healthy,
                    diagnostic: None,
                },
                CorporateEventEvidenceProviderHealthRecord {
                    provider_id: "finnhub".to_string(),
                    health: CorporateEventEvidenceProviderHealth::Unavailable,
                    diagnostic: Some("credential unavailable".to_string()),
                },
            ],
        };

        let assessment = without_external_fixture(|| {
            build_signal_context_assessment(SignalContextReadModelInput {
                as_of_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                signal: signal(
                    InterpretationExpectationQuality::Unavailable,
                    InterpretationExpectationQualityReason::SystemUnavailable,
                    InterpretationGravityDataQuality::Unavailable,
                    false,
                    None,
                ),
                future_context,
                language: Language::EnUs,
            })
        });

        assert!(assessment
            .source_diagnostics_appendix
            .contains("Corporate Event Evidence Health"));
        assert!(assessment
            .source_diagnostics_appendix
            .contains("finnhub=Unavailable"));
        assert!(assessment
            .source_diagnostics_appendix
            .contains("NVDA: lifecycle=Scheduled"));
        assert!(assessment
            .source_diagnostics_appendix
            .contains("sources=alpha_vantage"));
    }

    #[test]
    fn medium_corporate_event_does_not_claim_high_information() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let future_context = crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel {
            corporate_event_provider: CorporateEventProviderReadModel {
                health: CorporateEventProviderHealth::Healthy,
                events: vec![CorporateEventObservation {
                    symbol: "NVDA".to_string(),
                    market_date,
                    release_window: CorporateEventReleaseWindow::AfterMarketClose,
                    fiscal_quarter: 2,
                    fiscal_year: 2027,
                    source: finnhub_source(Some("https://finnhub.io/api/v1/calendar/earnings")),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        let assessment = without_external_fixture(|| {
            build_signal_context_assessment(SignalContextReadModelInput {
                as_of_date: market_date,
                signal: signal(
                    InterpretationExpectationQuality::Unavailable,
                    InterpretationExpectationQualityReason::SystemUnavailable,
                    InterpretationGravityDataQuality::Unavailable,
                    false,
                    None,
                ),
                future_context,
                language: Language::EnUs,
            })
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::CorporateEvent
        );
        assert!(assessment.interpretation.contains("medium-information"));
        assert!(!assessment
            .interpretation
            .contains("high-information corporate event"));
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

    fn with_nvidia_fixture<T>(callback: impl FnOnce() -> T) -> T {
        let _guard =
            crate::features::radar::interface::signal_context_coverage::SIGNAL_CONTEXT_ENV_MUTEX
                .lock()
                .unwrap();
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/signal_context/2026-08-27-nvidia-earnings.json");
        let previous = std::env::var_os("SENTINEL_SIGNAL_CONTEXT_JSON_PATH");
        std::env::set_var("SENTINEL_SIGNAL_CONTEXT_JSON_PATH", path);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
        match previous {
            Some(value) => std::env::set_var("SENTINEL_SIGNAL_CONTEXT_JSON_PATH", value),
            None => std::env::remove_var("SENTINEL_SIGNAL_CONTEXT_JSON_PATH"),
        }
        result.unwrap()
    }

    fn without_external_fixture<T>(callback: impl FnOnce() -> T) -> T {
        let _guard =
            crate::features::radar::interface::signal_context_coverage::SIGNAL_CONTEXT_ENV_MUTEX
                .lock()
                .unwrap();
        let previous = std::env::var_os("SENTINEL_SIGNAL_CONTEXT_JSON_PATH");
        std::env::remove_var("SENTINEL_SIGNAL_CONTEXT_JSON_PATH");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
        if let Some(value) = previous {
            std::env::set_var("SENTINEL_SIGNAL_CONTEXT_JSON_PATH", value);
        }
        result.unwrap()
    }

    #[test]
    fn corporate_earnings_fixture_is_not_reported_as_a_macro_event() {
        let assessment = with_nvidia_fixture(|| {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/signal_context/2026-08-27-nvidia-earnings.json");
            let loaded = crate::features::radar::interface::signal_context_coverage::load_external_signal_context_from_path(
                path.to_str().unwrap(),
                NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            )
            .expect("NVIDIA earnings fixture must deserialize")
            .expect("NVIDIA earnings fixture must be present");
            assert_eq!(loaded.corporate_events.len(), 1);
            let serialized = serde_json::to_value(&loaded).unwrap();
            assert_eq!(serialized["corporate_events"][0]["symbol"], "NVDA");
            build_signal_context_assessment(SignalContextReadModelInput {
                as_of_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                signal: signal(
                    InterpretationExpectationQuality::High,
                    InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    InterpretationGravityDataQuality::Ready,
                    false,
                    Some(0.08),
                ),
                future_context: future_context_unavailable(),
                language: Language::EnUs,
            })
        });

        let primary_event = assessment.v1.primary_context.as_ref().unwrap();
        assert_eq!(
            primary_event.context_type,
            crate::features::radar::interface::presentation::SignalContextType::Corporate
        );
        assert_eq!(primary_event.title, "NVIDIA EARNINGS");
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::High
        );
        assert_eq!(
            signal_context_primary_context_label(assessment.primary_context),
            "Corporate Event"
        );
        assert!(assessment.interpretation.contains("may be event-driven"));
        assert!(assessment.next_observation.contains("persistence"));
        assert!(!assessment
            .source_diagnostics_summary
            .contains("No high-information event identified"));
        assert_eq!(assessment.v1.decision_weight, 0);
        assert!(!assessment.v1.trade_signal);
        assert_eq!(assessment.v1.gate_effect, "none");
        assert_eq!(assessment.v1.execution_effect, "none");
        assert_eq!(assessment.v1.position_sizing_effect, "none");
    }

    #[test]
    fn missing_external_context_remains_unavailable() {
        let assessment = without_external_fixture(|| {
            build_signal_context_assessment(SignalContextReadModelInput {
                as_of_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                signal: signal(
                    InterpretationExpectationQuality::High,
                    InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    InterpretationGravityDataQuality::Ready,
                    false,
                    Some(0.08),
                ),
                future_context: future_context_unavailable(),
                language: Language::EnUs,
            })
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
        assert!(assessment.interpretation.contains("UNAVAILABLE"));
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
        assert_eq!(
            assessment.context_quality,
            SignalContextQuality::Unavailable
        );
        assert!(assessment.interpretation.contains("UNAVAILABLE"));
    }

    #[test]
    fn quarter_end_with_incomplete_coverage_is_unavailable() {
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
            SignalContextInformationContent::Unknown
        );
        assert_eq!(
            assessment.context_quality,
            SignalContextQuality::Unavailable
        );
        assert!(assessment
            .interpretation
            .contains("Available source coverage is incomplete"));
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
            .contains("Official calendar source health: UNAVAILABLE"));
    }

    #[test]
    fn signal_context_incomplete_coverage_does_not_claim_absence() {
        let mut future_context = future_context_unavailable();
        future_context.source_health = MacroEventSourceHealth::Partial;
        future_context.runtime_coverage = Some(
            crate::features::radar::interface::presentation::SignalContextCoverage {
                scheduled_macro: crate::features::radar::interface::presentation::SignalContextSourceStatus::Partial,
                corporate: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                geopolitical: crate::features::radar::interface::presentation::SignalContextSourceStatus::Unavailable,
                commodity: crate::features::radar::interface::presentation::SignalContextSourceStatus::Unavailable,
                rates_credit: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                market_structure: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                overall: crate::features::radar::interface::presentation::SignalContextSourceStatus::Unavailable,
            },
        );

        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 8, 12).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::Unavailable,
                InterpretationExpectationQualityReason::SystemUnavailable,
                InterpretationGravityDataQuality::Unavailable,
                false,
                None,
            ),
            future_context,
            language: Language::EnUs,
        });

        assert!(assessment
            .source_diagnostics_summary
            .contains("coverage is incomplete"));
        assert!(!assessment
            .source_diagnostics_summary
            .contains("No high-information event identified"));
        assert!(!assessment
            .source_diagnostics_summary
            .contains("monitoring remains idle"));
    }

    #[test]
    fn normal_no_event_with_healthy_coverage_preserves_absence() {
        let mut future_context = future_context_unavailable();
        future_context.source_health = MacroEventSourceHealth::Succeeded;
        future_context.runtime_coverage = Some(
            crate::features::radar::interface::presentation::SignalContextCoverage {
                scheduled_macro: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                corporate: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                geopolitical: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                commodity: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                rates_credit: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                market_structure: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
                overall: crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy,
            },
        );

        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 8, 12).unwrap(),
            signal: signal(
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                false,
                Some(0.08),
            ),
            future_context,
            language: Language::EnUs,
        });

        assert!(assessment.source_diagnostics_summary.is_empty());
    }

    #[test]
    fn index_reconstitution_with_incomplete_coverage_is_unavailable() {
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
            SignalContextInformationContent::Unknown
        );
        assert_eq!(assessment.context_quality, SignalContextQuality::Medium);
    }

    #[test]
    fn holiday_liquidity_with_incomplete_coverage_is_unavailable() {
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
            SignalContextInformationContent::Unknown
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
    fn etf_rebalance_with_incomplete_coverage_is_unavailable() {
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
            SignalContextInformationContent::Unknown
        );
    }

    #[test]
    fn month_end_with_incomplete_coverage_is_unavailable() {
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
            SignalContextInformationContent::Unknown
        );
        assert_eq!(
            assessment.context_quality,
            SignalContextQuality::Unavailable
        );
        assert!(assessment
            .interpretation
            .contains("Available source coverage is incomplete"));
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
        assert_eq!(assessment.context_quality, SignalContextQuality::Medium);
        assert_eq!(assessment.event_fact, "CPI Release / 2026-06-18 / BLS");
        assert!(assessment.source_diagnostics_summary.is_empty());
        assert!(assessment
            .source_diagnostics_appendix
            .contains("COVERAGE: UNAVAILABLE"));
        assert!(assessment
            .interpretation
            .contains("high-information macro event"));
        assert!(assessment.next_observation.contains("Today:"));
        assert!(assessment.next_observation.contains("CPI"));
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
            SignalContextInformationContent::Unknown
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
            SignalContextInformationContent::Unknown
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
    fn high_information_macro_event_wins_over_quarter_end_if_same_day() {
        let mut observation = macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Released,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        observation.as_of_date = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();
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
            SignalContextPrimaryContext::MacroEvent
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::High
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
        assert!(assessment.interpretation.contains("UNAVAILABLE"));
    }

    #[test]
    fn no_context_with_loaded_inputs_returns_unavailable_without_full_scan() {
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
            SignalContextInformationContent::Unknown
        );
        assert_eq!(
            assessment.context_quality,
            SignalContextQuality::Unavailable
        );
        assert!(assessment.interpretation.contains("UNAVAILABLE"));
    }
}
