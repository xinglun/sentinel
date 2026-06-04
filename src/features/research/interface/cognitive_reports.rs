use crate::config;
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoConfidence, CapitalAbsorptionAutoEvent,
    CapitalAbsorptionAutoEventCategory, CapitalAbsorptionAutoSnapshot, CapitalAbsorptionAutoStatus,
    CapitalAbsorptionAutoTrend, CapitalAbsorptionIpoQueueHistoryPoint,
    CapitalAbsorptionIpoQueueItem, CapitalAbsorptionIpoQueueStatus,
    CapitalAbsorptionObservationEventType, CapitalAbsorptionPotentialSupplyTrend,
    CapitalAbsorptionSourceHealth, CapitalAbsorptionSupplyEventCounts, CapitalAbsorptionSupplyKind,
};
use crate::features::research::interface::default_cognitive_localizations as defaults;
use crate::features::shared::interface::i18n::Language;

pub(crate) fn build_research_attention_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    let entries = match &app_config.research_attention {
        Some(entries) => entries,
        None => return research_attention_empty(language).to_string(),
    };

    let mut active_entries: Vec<_> = entries
        .iter()
        .filter(|(_, entry)| entry.enable.unwrap_or(true))
        .collect();
    active_entries.sort_by_key(|(symbol, _)| symbol.as_str());

    if active_entries.is_empty() {
        return research_attention_empty(language).to_string();
    }

    let mut high = Vec::new();
    let mut medium = Vec::new();
    let mut low = Vec::new();
    let mut degrading = Vec::new();

    for (symbol, entry) in active_entries {
        let line = format_research_attention_item(symbol, entry, language);
        match entry.cognitive_yield {
            config::CognitiveYield::High => high.push(line),
            config::CognitiveYield::Medium => medium.push(line),
            config::CognitiveYield::Low => low.push(line),
            config::CognitiveYield::Degrading => degrading.push(line),
        }
    }

    let mut out = String::new();
    out.push_str(research_attention_title(language));
    out.push_str("\n\n");
    push_research_attention_section(&mut out, research_attention_high_label(language), &high);
    push_research_attention_section(&mut out, research_attention_medium_label(language), &medium);
    push_research_attention_section(&mut out, research_attention_low_label(language), &low);
    push_research_attention_section(
        &mut out,
        research_attention_degrading_label(language),
        &degrading,
    );
    out.push('\n');
    out.push_str(research_attention_boundary(language));
    out
}

fn push_research_attention_section(out: &mut String, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    out.push_str(title);
    out.push_str(":\n");
    for item in items {
        out.push_str(item);
    }
    out.push('\n');
}

fn format_research_attention_item(
    symbol: &str,
    entry: &config::ResearchAttentionEntry,
    language: Language,
) -> String {
    format!(
        "• {} · {} {} · {} {}\n  {}\n",
        symbol,
        research_attention_density_label(language),
        information_density_label(entry.information_density),
        research_attention_cost_label(language),
        attention_cost_label(entry.attention_cost),
        localized_research_reason(symbol, entry, language)
    )
}

fn research_attention_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🧠 研究注意力",
        Language::EnUs => "🧠 Research Attention",
        Language::JaJp => "🧠 認知注目",
    }
}

fn research_attention_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "🧠 研究注意力\n\n未配置认知观察对象。\n\n边界: 认知收益低不等于股票不好；本报告只管理时间、注意力与认知带宽。"
        }
        Language::EnUs => {
            "🧠 Research Attention\n\nNo research attention entries configured.\n\nBoundary: low cognitive yield does not mean the stock is bad; this report only manages time, attention, and cognitive bandwidth."
        }
        Language::JaJp => {
            "🧠 認知注目\n\n認知観測対象は未設定です。\n\n境界: 認知收益が低いことは銘柄の否定ではない。このレポートは時間、注意力、認知帯域だけを管理する。"
        }
    }
}

fn research_attention_high_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "HIGH",
        Language::EnUs => "HIGH",
        Language::JaJp => "HIGH",
    }
}

fn research_attention_medium_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "MEDIUM",
        Language::EnUs => "MEDIUM",
        Language::JaJp => "MEDIUM",
    }
}

fn research_attention_low_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "LOW / SATURATED",
        Language::EnUs => "LOW / SATURATED",
        Language::JaJp => "LOW / SATURATED",
    }
}

fn research_attention_degrading_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "DEGRADING",
        Language::EnUs => "DEGRADING",
        Language::JaJp => "DEGRADING",
    }
}

fn research_attention_density_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "信息密度",
        Language::EnUs => "Density",
        Language::JaJp => "情報密度",
    }
}

fn research_attention_cost_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "注意力成本",
        Language::EnUs => "Attention Cost",
        Language::JaJp => "注意コスト",
    }
}

fn research_attention_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "校准边界: 认知收益低 ≠ 股票不好；注意力成本高 ≠ 不值得研究；信息密度低 ≠ 不值得持有。"
        }
        Language::EnUs => {
            "Calibration boundary: low cognitive yield != bad stock; high attention cost != not worth researching; low information density != not worth holding."
        }
        Language::JaJp => {
            "校正境界: 認知收益が低い ≠ 悪い銘柄；注意コストが高い ≠ 研究価値なし；情報密度が低い ≠ 保有価値なし。"
        }
    }
}

pub(crate) fn build_asset_thesis_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    let entries = match &app_config.asset_thesis {
        Some(entries) => entries,
        None => return asset_thesis_empty(language).to_string(),
    };

    let mut active_entries: Vec<_> = entries
        .iter()
        .filter(|(_, entry)| entry.enable.unwrap_or(true))
        .collect();
    active_entries.sort_by_key(|(symbol, _)| symbol.as_str());

    if active_entries.is_empty() {
        return asset_thesis_empty(language).to_string();
    }

    let mut out = String::new();
    out.push_str(asset_thesis_title(language));
    out.push_str("\n\n");

    for (symbol, entry) in active_entries {
        out.push_str(&format!(
            "• {} · {}\n",
            symbol,
            localized_asset_thesis(symbol, entry, language)
        ));
        let observation_focus = localized_asset_observation_focus(symbol, entry, language);
        push_asset_thesis_list(
            &mut out,
            asset_thesis_observation_focus_label(language),
            &observation_focus,
        );
        let invalidation = localized_asset_invalidation(symbol, entry, language);
        push_asset_thesis_list(
            &mut out,
            asset_thesis_invalidation_label(language),
            &invalidation,
        );
        push_asset_thesis_governance(&mut out, entry, language);
        out.push('\n');
    }

    out.push_str(asset_thesis_boundary(language));
    out
}

pub(crate) fn enabled_research_attention_count(app_config: &config::AppConfig) -> usize {
    app_config
        .research_attention
        .as_ref()
        .map(|entries| {
            entries
                .values()
                .filter(|entry| entry.enable.unwrap_or(true))
                .count()
        })
        .unwrap_or(0)
}

pub(crate) fn enabled_asset_thesis_count(app_config: &config::AppConfig) -> usize {
    app_config
        .asset_thesis
        .as_ref()
        .map(|entries| {
            entries
                .values()
                .filter(|entry| entry.enable.unwrap_or(true))
                .count()
        })
        .unwrap_or(0)
}

fn push_asset_thesis_list(out: &mut String, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("  {}:\n", title));
    for item in items {
        out.push_str(&format!("  - {}\n", item));
    }
}

fn push_asset_thesis_governance(
    out: &mut String,
    entry: &config::AssetThesisEntry,
    language: Language,
) {
    let mut lines = Vec::new();

    if let Some(narrative_state) = &entry.narrative_state {
        lines.push(format!(
            "{} {} / {} {} / {} {}",
            asset_thesis_consensus_label(language),
            consensus_level_label(narrative_state.consensus_level),
            asset_thesis_skepticism_label(language),
            skepticism_level_label(narrative_state.skepticism_level),
            asset_thesis_valuation_reflection_label(language),
            valuation_reflection_label(narrative_state.valuation_reflection)
        ));
    }

    if let Some(time_horizon) = entry.time_horizon {
        let mut line = format!(
            "{} {}",
            asset_thesis_time_horizon_label(language),
            time_horizon_label(time_horizon)
        );
        if let Some(window) = &entry.materialization_window {
            line.push_str(&format!(
                " / {} {}",
                asset_thesis_window_label(language),
                window
            ));
        }
        lines.push(line);
    }

    if let Some(reality_override) = &entry.reality_override {
        let contradiction = reality_override.observable_contradiction.unwrap_or(false);
        let decay = reality_override.confidence_decay.unwrap_or(true);
        lines.push(format!(
            "{} {} / {} {}",
            asset_thesis_reality_override_label(language),
            boolean_label(contradiction),
            asset_thesis_confidence_decay_label(language),
            boolean_label(decay)
        ));
    }

    if !lines.is_empty() {
        push_asset_thesis_list(out, asset_thesis_governance_label(language), &lines);
    }
}

fn localized_research_reason(
    symbol: &str,
    entry: &config::ResearchAttentionEntry,
    language: Language,
) -> String {
    localized_text(
        &entry.reason,
        entry
            .reason_zh
            .as_deref()
            .or_else(|| defaults::research_reason_zh(symbol, &entry.reason)),
        entry
            .reason_en
            .as_deref()
            .or_else(|| defaults::research_reason_en(symbol, &entry.reason)),
        entry.reason_ja.as_deref(),
        language,
    )
}

fn localized_asset_thesis(
    symbol: &str,
    entry: &config::AssetThesisEntry,
    language: Language,
) -> String {
    localized_text(
        &entry.thesis,
        entry
            .thesis_zh
            .as_deref()
            .or_else(|| defaults::asset_thesis_zh(symbol, &entry.thesis)),
        entry
            .thesis_en
            .as_deref()
            .or_else(|| defaults::asset_thesis_en(symbol, &entry.thesis)),
        entry.thesis_ja.as_deref(),
        language,
    )
}

fn localized_asset_observation_focus(
    symbol: &str,
    entry: &config::AssetThesisEntry,
    language: Language,
) -> Vec<String> {
    let default_zh = defaults::observation_focus_zh(symbol, &entry.thesis);
    let default_en = defaults::observation_focus_en(symbol, &entry.thesis);
    localized_list(
        &entry.observation_focus,
        entry
            .observation_focus_zh
            .as_deref()
            .or(default_zh.as_deref()),
        entry
            .observation_focus_en
            .as_deref()
            .or(default_en.as_deref()),
        entry.observation_focus_ja.as_deref(),
        language,
    )
}

fn localized_asset_invalidation(
    symbol: &str,
    entry: &config::AssetThesisEntry,
    language: Language,
) -> Vec<String> {
    let default_zh = defaults::invalidation_zh(symbol, &entry.thesis);
    let default_en = defaults::invalidation_en(symbol, &entry.thesis);
    localized_list(
        &entry.invalidation,
        entry.invalidation_zh.as_deref().or(default_zh.as_deref()),
        entry.invalidation_en.as_deref().or(default_en.as_deref()),
        entry.invalidation_ja.as_deref(),
        language,
    )
}

fn localized_text(
    legacy_ja_text: &str,
    zh: Option<&str>,
    en: Option<&str>,
    ja: Option<&str>,
    language: Language,
) -> String {
    match language {
        Language::ZhCn => zh.unwrap_or(localized_config_missing(language)).to_string(),
        Language::EnUs => en.unwrap_or(localized_config_missing(language)).to_string(),
        Language::JaJp => ja.unwrap_or(legacy_ja_text).to_string(),
    }
}

fn localized_list(
    legacy_ja_items: &[String],
    zh: Option<&[String]>,
    en: Option<&[String]>,
    ja: Option<&[String]>,
    language: Language,
) -> Vec<String> {
    match language {
        Language::ZhCn => zh
            .map(|items| items.to_vec())
            .unwrap_or_else(|| vec![localized_config_missing(language).to_string()]),
        Language::EnUs => en
            .map(|items| items.to_vec())
            .unwrap_or_else(|| vec![localized_config_missing(language).to_string()]),
        Language::JaJp => ja
            .map(|items| items.to_vec())
            .unwrap_or_else(|| legacy_ja_items.to_vec()),
    }
}

fn localized_config_missing(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "用户自定义观察说明未提供中文版本。",
        Language::EnUs => "User-defined observation text is not provided in English.",
        Language::JaJp => "ユーザー定義の観測説明は日本語で未提供です。",
    }
}

fn asset_thesis_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🧭 资产观察命题",
        Language::EnUs => "🧭 Asset Thesis Registry",
        Language::JaJp => "🧭 銘柄別観測命題",
    }
}

fn asset_thesis_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "🧭 资产观察命题\n\n未配置资产观察命题。\n\n边界: 资产命题只说明为什么观察，不生成买卖指令。"
        }
        Language::EnUs => {
            "🧭 Asset Thesis Registry\n\nNo asset thesis entries configured.\n\nBoundary: asset theses explain why to observe; they do not generate trade instructions."
        }
        Language::JaJp => {
            "🧭 銘柄別観測命題\n\n銘柄別の観測命題は未設定です。\n\n境界: 銘柄命題は観測理由を説明するだけで、売買指示は生成しない。"
        }
    }
}

fn asset_thesis_observation_focus_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察焦点",
        Language::EnUs => "Observation Focus",
        Language::JaJp => "観測焦点",
    }
}

fn asset_thesis_invalidation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "失效条件",
        Language::EnUs => "Invalidation",
        Language::JaJp => "失効条件",
    }
}

fn asset_thesis_governance_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "反叙事治理",
        Language::EnUs => "Anti-Narrative Governance",
        Language::JaJp => "反ナラティブ統制",
    }
}

fn asset_thesis_consensus_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "共识",
        Language::EnUs => "Consensus",
        Language::JaJp => "コンセンサス",
    }
}

fn asset_thesis_skepticism_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "怀疑",
        Language::EnUs => "Skepticism",
        Language::JaJp => "懐疑",
    }
}

fn asset_thesis_valuation_reflection_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "定价反映",
        Language::EnUs => "Valuation Reflection",
        Language::JaJp => "価格反映",
    }
}

fn asset_thesis_time_horizon_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "时间尺度",
        Language::EnUs => "Time Horizon",
        Language::JaJp => "時間軸",
    }
}

fn asset_thesis_window_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "兑现窗口",
        Language::EnUs => "Materialization Window",
        Language::JaJp => "実現ウィンドウ",
    }
}

fn asset_thesis_reality_override_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "现实覆盖",
        Language::EnUs => "Reality Override",
        Language::JaJp => "現実優先",
    }
}

fn asset_thesis_confidence_decay_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "置信衰减",
        Language::EnUs => "Confidence Decay",
        Language::JaJp => "確信度減衰",
    }
}

fn consensus_level_label(level: config::ConsensusLevel) -> &'static str {
    match level {
        config::ConsensusLevel::Early => "EARLY",
        config::ConsensusLevel::Developing => "DEVELOPING",
        config::ConsensusLevel::Crowded => "CROWDED",
        config::ConsensusLevel::Saturated => "SATURATED",
    }
}

fn skepticism_level_label(level: config::SkepticismLevel) -> &'static str {
    match level {
        config::SkepticismLevel::High => "HIGH",
        config::SkepticismLevel::Normal => "NORMAL",
        config::SkepticismLevel::Low => "LOW",
    }
}

fn valuation_reflection_label(reflection: config::ValuationReflection) -> &'static str {
    match reflection {
        config::ValuationReflection::Underreflected => "UNDERREFLECTED",
        config::ValuationReflection::Partial => "PARTIAL",
        config::ValuationReflection::FullyPriced => "FULLY_PRICED",
    }
}

fn time_horizon_label(horizon: config::AssetThesisTimeHorizon) -> &'static str {
    match horizon {
        config::AssetThesisTimeHorizon::Short => "SHORT",
        config::AssetThesisTimeHorizon::Medium => "MEDIUM",
        config::AssetThesisTimeHorizon::Long => "LONG",
        config::AssetThesisTimeHorizon::Civilization => "CIVILIZATION",
    }
}

fn boolean_label(value: bool) -> &'static str {
    if value {
        "TRUE"
    } else {
        "FALSE"
    }
}

fn asset_thesis_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 观察命题 ≠ 买入理由；命题失效 ≠ 自动卖出；叙事越顺，越需要现实覆盖。"
        }
        Language::EnUs => {
            "Boundary: an observation thesis is not a buy reason; thesis invalidation is not an automatic sell; the smoother the story, the stronger the reality override must be."
        }
        Language::JaJp => {
            "境界: 観測命題は買い理由ではない。命題失効は自動売却ではない。物語が滑らかになるほど、現実優先を強める。"
        }
    }
}

pub(crate) fn build_macro_gravity_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    let Some(macro_gravity) = app_config
        .macro_gravity
        .as_ref()
        .filter(|macro_gravity| macro_gravity.enable.unwrap_or(true))
    else {
        return macro_gravity_empty(language).to_string();
    };

    let mut out = String::new();
    out.push_str(macro_gravity_title(language));
    out.push_str("\n\n");
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_rate_pressure_label(language),
        macro_pressure_label(macro_gravity.rate_pressure)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_real_yield_label(language),
        macro_pressure_label(macro_gravity.real_yield_pressure)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_curve_label(language),
        yield_curve_label(macro_gravity.yield_curve)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_credit_label(language),
        credit_stress_label(macro_gravity.credit_stress)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_liquidity_label(language),
        liquidity_condition_label(macro_gravity.liquidity)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_growth_valuation_label(language),
        growth_valuation_impact_label(macro_gravity.growth_valuation_impact)
    ));
    out.push('\n');
    out.push_str(macro_gravity_boundary(language));
    out
}

fn macro_gravity_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🌐 宏观重力",
        Language::EnUs => "🌐 Macro Gravity",
        Language::JaJp => "🌐 マクロ重力",
    }
}

fn macro_gravity_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "🌐 宏观重力\n\n未配置宏观重力支线。\n\n边界: 债券与信用环境只解释折现率和流动性，不生成交易信号。"
        }
        Language::EnUs => {
            "🌐 Macro Gravity\n\nNo macro gravity context configured.\n\nBoundary: bond and credit context only explains discount-rate and liquidity conditions; it does not generate trade signals."
        }
        Language::JaJp => {
            "🌐 マクロ重力\n\nマクロ重力コンテキストは未設定です。\n\n境界: 債券と信用環境は割引率と流動性だけを説明し、売買シグナルは生成しない。"
        }
    }
}

fn macro_gravity_rate_pressure_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 利率压力:",
        Language::EnUs => "- Rate pressure:",
        Language::JaJp => "- 金利圧力:",
    }
}

fn macro_gravity_real_yield_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 实际利率:",
        Language::EnUs => "- Real yield:",
        Language::JaJp => "- 実質金利:",
    }
}

fn macro_gravity_curve_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 收益率曲线:",
        Language::EnUs => "- Yield curve:",
        Language::JaJp => "- イールドカーブ:",
    }
}

fn macro_gravity_credit_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 信用压力:",
        Language::EnUs => "- Credit stress:",
        Language::JaJp => "- 信用圧力:",
    }
}

fn macro_gravity_liquidity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 流动性:",
        Language::EnUs => "- Liquidity:",
        Language::JaJp => "- 流動性:",
    }
}

fn macro_gravity_growth_valuation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 成长股估值:",
        Language::EnUs => "- Growth valuation:",
        Language::JaJp => "- 成長株バリュエーション:",
    }
}

fn macro_gravity_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 宏观重力只解释市场折现率、流动性和估值压力；不参与 Gate，不生成交易指令。"
        }
        Language::EnUs => {
            "Boundary: macro gravity only explains discount rates, liquidity, and valuation pressure; it does not enter Gate or generate trade instructions."
        }
        Language::JaJp => {
            "境界: マクロ重力は割引率、流動性、バリュエーション圧力だけを説明し、Gate に入らず、売買指示も生成しない。"
        }
    }
}

pub(crate) fn build_capital_absorption_report(
    app_config: &config::AppConfig,
    auto_snapshot: Option<&CapitalAbsorptionAutoSnapshot>,
    language: Language,
) -> String {
    let manual = app_config
        .capital_absorption
        .as_ref()
        .filter(|capital_absorption| capital_absorption.enable.unwrap_or(true));
    let snapshot = if let Some(auto_snapshot) = auto_snapshot.filter(|snapshot| {
        snapshot.source_status.status != CapitalAbsorptionSourceHealth::Unavailable
    }) {
        CapitalAbsorptionRenderSnapshot::from_auto(auto_snapshot, language)
    } else if let Some(manual) = manual {
        CapitalAbsorptionRenderSnapshot::from_config(manual, language)
    } else if let Some(auto_snapshot) = auto_snapshot {
        CapitalAbsorptionRenderSnapshot::from_auto(auto_snapshot, language)
    } else {
        return capital_absorption_empty(language).to_string();
    };

    let mut out = String::new();
    out.push_str(capital_absorption_title(language));
    out.push_str("\n\n");
    if let Some(source_status) = &snapshot.source_status {
        out.push_str(&format!(
            "{} {} · {}\n\n",
            capital_absorption_source_label(language),
            source_status.provider,
            source_status.message
        ));
    }
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_status_label(language),
        snapshot.status
    ));
    push_supply_event_counts(&mut out, &snapshot.supply_event_counts, language);
    push_actual_capital_supply(&mut out, &snapshot.capital_demand, language);
    push_potential_supply_trend(&mut out, &snapshot.potential_supply_trend, language);
    push_ai_ipo_queue(&mut out, &snapshot.ai_ipo_queue, language);
    push_ipo_queue_history(&mut out, &snapshot.ipo_queue_history, language);
    push_capital_absorption_events(&mut out, &snapshot.observed_events, language);
    push_capital_supply(&mut out, &snapshot.capital_supply, language);
    push_capital_absorption_ratio(&mut out, &snapshot.absorption_ratio, language);
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_structural_impact_label(language),
        snapshot.structural_impact
    ));
    out.push_str(capital_absorption_current_phase_boundary(language));
    out.push_str("\n\n");
    out.push_str(capital_absorption_boundary(language));
    out
}

struct CapitalAbsorptionRenderSnapshot {
    source_status: Option<CapitalAbsorptionRenderSourceStatus>,
    status: String,
    observed_events: Vec<CapitalAbsorptionRenderEvent>,
    supply_event_counts: CapitalAbsorptionSupplyEventCounts,
    ai_ipo_queue: Vec<CapitalAbsorptionIpoQueueItem>,
    ipo_queue_history: Vec<CapitalAbsorptionIpoQueueHistoryPoint>,
    potential_supply_trend: String,
    capital_demand: CapitalDemandRenderSnapshot,
    capital_supply: CapitalSupplyRenderSnapshot,
    absorption_ratio: CapitalAbsorptionRenderRatio,
    structural_impact: String,
}

struct CapitalAbsorptionRenderSourceStatus {
    provider: String,
    message: String,
}

struct CapitalAbsorptionRenderEvent {
    category: String,
    subject: String,
    description: String,
    amount_usd_b: Option<f64>,
    ai_capex_related: bool,
    source_count: usize,
    confidence: String,
    supply_kind: CapitalAbsorptionSupplyKind,
    event_type: String,
}

struct CapitalDemandRenderSnapshot {
    rolling_12m_usd_b: Option<f64>,
    ipo_financing_usd_b: Option<f64>,
    secondary_offering_usd_b: Option<f64>,
    convertible_debt_usd_b: Option<f64>,
    ai_related_financing_usd_b: Option<f64>,
}

struct CapitalSupplyRenderSnapshot {
    rolling_12m_usd_b: Option<f64>,
    score: Option<f64>,
    trend: String,
    etf_net_inflow_usd_b: Option<f64>,
    mutual_fund_net_inflow_usd_b: Option<f64>,
    pension_allocation_flow_usd_b: Option<f64>,
    foreign_capital_inflow_usd_b: Option<f64>,
    corporate_buyback_usd_b: Option<f64>,
}

struct CapitalAbsorptionRenderRatio {
    value: Option<f64>,
    state: String,
}

impl CapitalAbsorptionRenderSnapshot {
    fn from_config(value: &config::CapitalAbsorptionConfig, language: Language) -> Self {
        let observed_events = value
            .observed_events
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|event| CapitalAbsorptionRenderEvent::from_config(event, language))
            .collect::<Vec<_>>();
        Self {
            source_status: None,
            status: capital_absorption_status_value(capped_config_status(value.status), language),
            supply_event_counts: supply_event_counts_from_render_events(&observed_events),
            ai_ipo_queue: default_capital_absorption_ipo_queue(),
            ipo_queue_history: Vec::new(),
            potential_supply_trend: capital_absorption_potential_supply_trend_value(
                CapitalAbsorptionPotentialSupplyTrend::Stable,
                language,
            ),
            observed_events,
            capital_demand: CapitalDemandRenderSnapshot::from_config(
                &value.capital_demand,
                language,
            ),
            capital_supply: CapitalSupplyRenderSnapshot::from_config(
                &value.capital_supply,
                language,
            ),
            absorption_ratio: CapitalAbsorptionRenderRatio {
                value: value.absorption_ratio.value,
                state: capital_absorption_ratio_state_value(value.absorption_ratio.state, language),
            },
            structural_impact: value
                .structural_impact
                .clone()
                .unwrap_or_else(|| "Observation Only".to_string()),
        }
    }

    fn from_auto(value: &CapitalAbsorptionAutoSnapshot, language: Language) -> Self {
        Self {
            source_status: Some(CapitalAbsorptionRenderSourceStatus {
                provider: value.source_status.provider.clone(),
                message: value.source_status.message.clone(),
            }),
            status: capital_absorption_auto_status_value(value.status, language),
            supply_event_counts: value.supply_event_counts.clone(),
            ai_ipo_queue: value.ai_ipo_queue.clone(),
            ipo_queue_history: value.ipo_queue_history.clone(),
            potential_supply_trend: capital_absorption_potential_supply_trend_value(
                value.potential_supply_trend,
                language,
            ),
            observed_events: value
                .observed_events
                .iter()
                .map(|event| CapitalAbsorptionRenderEvent::from_auto(event, language))
                .collect(),
            capital_demand: CapitalDemandRenderSnapshot::from_auto(&value.capital_demand, language),
            capital_supply: CapitalSupplyRenderSnapshot::from_auto(&value.capital_supply, language),
            absorption_ratio: CapitalAbsorptionRenderRatio {
                value: value.absorption_ratio.value,
                state: capital_absorption_auto_ratio_state_value(
                    value.absorption_ratio.state,
                    language,
                ),
            },
            structural_impact: capital_absorption_observation_only_value(language).to_string(),
        }
    }
}

impl CapitalAbsorptionRenderEvent {
    fn from_config(value: &config::CapitalAbsorptionEventConfig, language: Language) -> Self {
        let supply_kind = match value.category {
            config::CapitalAbsorptionEventCategory::IpoSupply => {
                CapitalAbsorptionSupplyKind::Potential
            }
            config::CapitalAbsorptionEventCategory::MegaCapFinancing
            | config::CapitalAbsorptionEventCategory::SecondaryLiquidity => {
                CapitalAbsorptionSupplyKind::Actual
            }
        };
        let event_type = match supply_kind {
            CapitalAbsorptionSupplyKind::Actual => CapitalAbsorptionObservationEventType::Confirmed,
            CapitalAbsorptionSupplyKind::Potential => CapitalAbsorptionObservationEventType::Rumor,
        };
        Self {
            category: capital_absorption_event_category_value(value.category, language),
            subject: value.subject.clone(),
            description: value.description.clone(),
            amount_usd_b: value.amount_usd_b,
            ai_capex_related: value.ai_capex_related.unwrap_or(false),
            source_count: 1,
            confidence: capital_absorption_confidence_value(
                CapitalAbsorptionAutoConfidence::Low,
                language,
            ),
            supply_kind,
            event_type: capital_absorption_event_type_value(event_type, language).to_string(),
        }
    }

    fn from_auto(value: &CapitalAbsorptionAutoEvent, language: Language) -> Self {
        Self {
            category: capital_absorption_auto_event_category_value(value.category, language),
            subject: value.subject.clone(),
            description: value.description.clone(),
            amount_usd_b: value.amount_usd_b,
            ai_capex_related: value.ai_capex_related,
            source_count: value.source_count,
            confidence: capital_absorption_confidence_value(value.confidence, language),
            supply_kind: value.supply_kind,
            event_type: capital_absorption_event_type_value(value.event_type, language).to_string(),
        }
    }
}

impl CapitalDemandRenderSnapshot {
    fn from_config(value: &config::CapitalDemandConfig, language: Language) -> Self {
        let _current_phase_hides_demand_trend_and_score = (
            capital_absorption_trend_value(value.trend, language),
            value.score,
        );
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            ipo_financing_usd_b: value.ipo_financing_usd_b,
            secondary_offering_usd_b: value.secondary_offering_usd_b,
            convertible_debt_usd_b: value.convertible_debt_usd_b,
            ai_related_financing_usd_b: value.ai_related_financing_usd_b,
        }
    }

    fn from_auto(
        value: &crate::features::research::application::capital_absorption::CapitalDemandAutoSnapshot,
        language: Language,
    ) -> Self {
        let _current_phase_hides_demand_trend_and_score = (
            capital_absorption_auto_trend_value(value.trend, language),
            value.score,
        );
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            ipo_financing_usd_b: value.ipo_financing_usd_b,
            secondary_offering_usd_b: value.secondary_offering_usd_b,
            convertible_debt_usd_b: value.convertible_debt_usd_b,
            ai_related_financing_usd_b: value.ai_related_financing_usd_b,
        }
    }
}

impl CapitalSupplyRenderSnapshot {
    fn from_config(value: &config::CapitalSupplyConfig, language: Language) -> Self {
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            score: value.score,
            trend: capital_absorption_trend_value(value.trend, language),
            etf_net_inflow_usd_b: value.etf_net_inflow_usd_b,
            mutual_fund_net_inflow_usd_b: value.mutual_fund_net_inflow_usd_b,
            pension_allocation_flow_usd_b: value.pension_allocation_flow_usd_b,
            foreign_capital_inflow_usd_b: value.foreign_capital_inflow_usd_b,
            corporate_buyback_usd_b: value.corporate_buyback_usd_b,
        }
    }

    fn from_auto(
        value: &crate::features::research::application::capital_absorption::CapitalSupplyAutoSnapshot,
        language: Language,
    ) -> Self {
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            score: value.score,
            trend: capital_absorption_auto_trend_value(value.trend, language),
            etf_net_inflow_usd_b: value.etf_net_inflow_usd_b,
            mutual_fund_net_inflow_usd_b: value.mutual_fund_net_inflow_usd_b,
            pension_allocation_flow_usd_b: value.pension_allocation_flow_usd_b,
            foreign_capital_inflow_usd_b: value.foreign_capital_inflow_usd_b,
            corporate_buyback_usd_b: value.corporate_buyback_usd_b,
        }
    }
}

fn push_capital_absorption_events(
    out: &mut String,
    events: &[CapitalAbsorptionRenderEvent],
    language: Language,
) {
    out.push_str(capital_absorption_events_label(language));
    out.push_str(":\n");
    if events.is_empty() {
        out.push_str(capital_absorption_no_events(language));
        out.push('\n');
        return;
    }
    for event in events {
        let amount = event
            .amount_usd_b
            .map(|value| format!(" (${value:.1}B)"))
            .unwrap_or_default();
        let ai_capex = if event.ai_capex_related {
            format!(" · {}", capital_absorption_ai_capex_label(language))
        } else {
            String::new()
        };
        out.push_str(&format!(
            "- {} · {} {} · {} · {}{}{} · {} · {} {} · {} {}\n",
            capital_absorption_supply_kind_value(event.supply_kind, language),
            capital_absorption_event_type_label(language),
            event.event_type,
            event.category,
            event.subject,
            amount,
            ai_capex,
            event.description,
            capital_absorption_sources_count_label(language),
            event.source_count,
            capital_absorption_confidence_label(language),
            event.confidence
        ));
    }
    out.push('\n');
}

fn push_actual_capital_supply(
    out: &mut String,
    demand: &CapitalDemandRenderSnapshot,
    language: Language,
) {
    out.push_str(capital_absorption_actual_supply_label(language));
    out.push_str(":\n");
    push_optional_usd(
        out,
        capital_absorption_observed_actual_amount_label(language),
        demand.rolling_12m_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_ipo_label(language),
        demand.ipo_financing_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_secondary_label(language),
        demand.secondary_offering_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_convertible_label(language),
        demand.convertible_debt_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_ai_related_label(language),
        demand.ai_related_financing_usd_b,
    );
    if demand.rolling_12m_usd_b.is_none()
        && demand.ipo_financing_usd_b.is_none()
        && demand.secondary_offering_usd_b.is_none()
        && demand.convertible_debt_usd_b.is_none()
        && demand.ai_related_financing_usd_b.is_none()
    {
        out.push_str(capital_absorption_no_actual_supply(language));
        out.push('\n');
    }
    out.push('\n');
}

fn push_potential_supply_trend(out: &mut String, trend: &str, language: Language) {
    out.push_str(capital_absorption_potential_supply_trend_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {} {}\n\n",
        capital_absorption_trend_label(language),
        trend
    ));
}

fn push_supply_event_counts(
    out: &mut String,
    counts: &CapitalAbsorptionSupplyEventCounts,
    language: Language,
) {
    out.push_str(capital_absorption_supply_event_count_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_mega_cap_financing_count_label(language),
        counts.mega_cap_financing
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_secondary_offering_count_label(language),
        counts.secondary_offering
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_convertible_debt_count_label(language),
        counts.convertible_debt
    ));
    out.push_str(&format!(
        "- {}: {}\n\n",
        capital_absorption_secondary_liquidity_count_label(language),
        counts.secondary_liquidity
    ));
}

fn push_ai_ipo_queue(
    out: &mut String,
    queue: &[CapitalAbsorptionIpoQueueItem],
    language: Language,
) {
    if queue.is_empty() {
        return;
    }
    out.push_str(capital_absorption_ai_ipo_queue_label(language));
    out.push_str(":\n");
    for item in queue {
        let sources = if item.source_count > 0 {
            format!(
                " · {} {}",
                capital_absorption_sources_count_label(language),
                item.source_count
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            "- {}: {} · {} {}{}\n",
            item.issuer,
            capital_absorption_ipo_queue_status_value(item.status, language),
            capital_absorption_event_type_label(language),
            capital_absorption_event_type_value(item.event_type, language),
            sources
        ));
    }
    out.push('\n');
}

fn push_ipo_queue_history(
    out: &mut String,
    history: &[CapitalAbsorptionIpoQueueHistoryPoint],
    language: Language,
) {
    if history.is_empty() {
        return;
    }
    out.push_str(capital_absorption_ipo_queue_history_label(language));
    out.push_str(":\n");
    for point in history {
        out.push_str(&format!(
            "- {} · {} = {}\n",
            point.observed_at,
            capital_absorption_queue_size_label(language),
            point.queue_size
        ));
    }
    out.push('\n');
}

fn push_capital_supply(out: &mut String, supply: &CapitalSupplyRenderSnapshot, language: Language) {
    out.push_str(capital_absorption_supply_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {} {}\n",
        capital_absorption_trend_label(language),
        supply.trend
    ));
    push_optional_usd(
        out,
        capital_absorption_rolling_12m_label(language),
        supply.rolling_12m_usd_b,
    );
    push_optional_score(out, capital_absorption_score_label(language), supply.score);
    push_optional_usd(
        out,
        capital_absorption_etf_label(language),
        supply.etf_net_inflow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_mutual_fund_label(language),
        supply.mutual_fund_net_inflow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_pension_label(language),
        supply.pension_allocation_flow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_foreign_capital_label(language),
        supply.foreign_capital_inflow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_buyback_label(language),
        supply.corporate_buyback_usd_b,
    );
    out.push('\n');
}

fn push_capital_absorption_ratio(
    out: &mut String,
    ratio: &CapitalAbsorptionRenderRatio,
    language: Language,
) {
    let _configured_ratio_is_intentionally_hidden = (&ratio.value, &ratio.state);
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_ratio_label(language),
        capital_absorption_ratio_disabled_label(language)
    ));
}

fn supply_event_counts_from_render_events(
    events: &[CapitalAbsorptionRenderEvent],
) -> CapitalAbsorptionSupplyEventCounts {
    let actual_events = events
        .iter()
        .filter(|event| event.supply_kind == CapitalAbsorptionSupplyKind::Actual)
        .collect::<Vec<_>>();
    CapitalAbsorptionSupplyEventCounts {
        mega_cap_financing: actual_events
            .iter()
            .filter(|event| event.category.contains("Mega Cap"))
            .count(),
        ai_ipo_candidate: actual_events
            .iter()
            .filter(|event| event.category.contains("IPO"))
            .count(),
        secondary_offering: actual_events
            .iter()
            .filter(|event| event.description.to_ascii_lowercase().contains("secondary"))
            .count(),
        convertible_debt: actual_events
            .iter()
            .filter(|event| {
                event
                    .description
                    .to_ascii_lowercase()
                    .contains("convertible")
            })
            .count(),
        secondary_liquidity: actual_events
            .iter()
            .filter(|event| {
                event.category.contains("Secondary Liquidity")
                    || event.category.contains("二级流动性")
                    || event.category.contains("セカンダリー流動性")
            })
            .count(),
    }
}

fn default_capital_absorption_ipo_queue() -> Vec<CapitalAbsorptionIpoQueueItem> {
    [
        "Anthropic",
        "OpenAI",
        "SpaceX",
        "Databricks",
        "Stripe",
        "Figure",
    ]
    .iter()
    .map(|issuer| CapitalAbsorptionIpoQueueItem {
        issuer: (*issuer).to_string(),
        status: CapitalAbsorptionIpoQueueStatus::Rumor,
        source_count: 0,
        event_type: CapitalAbsorptionObservationEventType::Rumor,
    })
    .collect()
}

fn push_optional_usd(out: &mut String, label: &str, value: Option<f64>) {
    if let Some(value) = value {
        out.push_str(&format!("- {label} ${value:.1}B\n"));
    }
}

fn push_optional_score(out: &mut String, label: &str, value: Option<f64>) {
    if let Some(value) = value {
        out.push_str(&format!("- {label} {value:.2}\n"));
    }
}

fn capital_absorption_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "📊 资本吸收早期预警传感器",
        Language::EnUs => "📊 Capital Absorption Early Warning Sensor",
        Language::JaJp => "📊 資本吸収早期警戒センサー",
    }
}

fn capital_absorption_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "📊 资本吸收早期预警传感器\n\n未配置资本吸收观察层。\n\n边界: 本模块当前只观察潜在未来资本供给，不生成交易信号。"
        }
        Language::EnUs => {
            "📊 Capital Absorption Early Warning Sensor\n\nNo capital absorption context configured.\n\nBoundary: this module only observes potential future capital supply in the current phase; it does not generate trade signals."
        }
        Language::JaJp => {
            "📊 資本吸収早期警戒センサー\n\n資本吸収観測レイヤーは未設定です。\n\n境界: 現段階では潜在的な将来資本供給だけを観測し、売買シグナルは生成しない。"
        }
    }
}

fn capital_absorption_status_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资本吸收状态:",
        Language::EnUs => "Capital Absorption Status:",
        Language::JaJp => "資本吸収状態:",
    }
}

fn capital_absorption_source_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "自动来源:",
        Language::EnUs => "Automatic Source:",
        Language::JaJp => "自動ソース:",
    }
}

fn capital_absorption_events_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "发现",
        Language::EnUs => "Observed Events",
        Language::JaJp => "観測イベント",
    }
}

fn capital_absorption_no_events(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 未观察到大型资本吸收事件。",
        Language::EnUs => "- No large capital absorption events observed.",
        Language::JaJp => "- 大型の資本吸収イベントは未観測です。",
    }
}

fn capital_absorption_actual_supply_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "实际资本供给",
        Language::EnUs => "Actual Capital Supply",
        Language::JaJp => "実際の資本供給",
    }
}

fn capital_absorption_potential_supply_trend_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "潜在供给趋势",
        Language::EnUs => "Potential Supply Trend",
        Language::JaJp => "潜在供給トレンド",
    }
}

fn capital_absorption_no_actual_supply(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 未观察到已发生的大型股权/可转债供给。",
        Language::EnUs => "- No completed large equity or convertible supply observed.",
        Language::JaJp => "- 発生済みの大型株式・転換社債供給は未観測です。",
    }
}

fn capital_absorption_supply_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资本供给趋势",
        Language::EnUs => "Capital Supply",
        Language::JaJp => "資本供給トレンド",
    }
}

fn capital_absorption_ratio_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资本吸收比率:",
        Language::EnUs => "Capital Absorption Ratio:",
        Language::JaJp => "資本吸収比率:",
    }
}

fn capital_absorption_structural_impact_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "结构影响:",
        Language::EnUs => "Structural Impact:",
        Language::JaJp => "構造的影響:",
    }
}

fn capital_absorption_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 本模块当前只作为 Early Warning Sensor，不测量实际资本吸收，不测量市场流动性，不产生市场结论，不生成交易信号，不进行风险评级升级，不影响 READY / EXECUTE / Position Sizing / Gate / Trend Layer。"
        }
        Language::EnUs => {
            "Boundary: this module is currently only an Early Warning Sensor. It does not measure actual capital absorption, market liquidity, or market conclusions; it does not generate trading signals or risk-rating upgrades; it does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer."
        }
        Language::JaJp => {
            "境界: このモジュールは現段階では Early Warning Sensor に限定する。実際の資本吸収、市場流動性、市場結論を測定せず、売買シグナルやリスク格上げを生成せず、READY / EXECUTE / Position Sizing / Gate / Trend Layer に影響しない。"
        }
    }
}

fn capital_absorption_current_phase_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "当前阶段: Narrative Observation Only。\n观察对象: Potential Future Capital Supply，而不是 Actual Capital Absorption。\n解释边界: IPO 新闻增加不等于资本供给增加；资本供给增加不等于市场吸收失败；市场吸收失败不等于市场风险上升。\n状态边界: 当前阶段仅允许 NORMAL / WATCH；ACTIVE / STRESSED 保留到接入 Capital Supply 数据与 Rolling 12M Capital Model 后再评估。"
        }
        Language::EnUs => {
            "Current Phase: Narrative Observation Only.\nObject: Potential Future Capital Supply, not Actual Capital Absorption.\nInterpretation boundary: more IPO news does not equal more actual capital supply; more supply does not equal failed market absorption; failed absorption does not equal higher market risk.\nStatus boundary: only NORMAL / WATCH are allowed in this phase; ACTIVE / STRESSED are reserved until Capital Supply data and a Rolling 12M Capital Model are connected."
        }
        Language::JaJp => {
            "現段階: Narrative Observation Only。\n観測対象: Actual Capital Absorption ではなく Potential Future Capital Supply。\n解釈境界: IPO ニュース増加は実際の資本供給増加と同義ではない。資本供給増加は市場吸収失敗と同義ではない。市場吸収失敗は市場リスク上昇と同義ではない。\n状態境界: 現段階では NORMAL / WATCH のみを許可し、ACTIVE / STRESSED は Capital Supply data と Rolling 12M Capital Model 接続後に再評価する。"
        }
    }
}

fn capital_absorption_trend_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "趋势:",
        Language::EnUs => "Trend:",
        Language::JaJp => "トレンド:",
    }
}

fn capital_absorption_rolling_12m_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "滚动 12 个月:",
        Language::EnUs => "Rolling 12M:",
        Language::JaJp => "ローリング 12 か月:",
    }
}

fn capital_absorption_observed_actual_amount_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已观察实际供给:",
        Language::EnUs => "Observed actual supply:",
        Language::JaJp => "観測済み実供給:",
    }
}

fn capital_absorption_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "评分:",
        Language::EnUs => "Score:",
        Language::JaJp => "スコア:",
    }
}

fn capital_absorption_ipo_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "IPO 融资:",
        Language::EnUs => "IPO financing:",
        Language::JaJp => "IPO 調達:",
    }
}

fn capital_absorption_secondary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "增发融资:",
        Language::EnUs => "Secondary offering:",
        Language::JaJp => "増資:",
    }
}

fn capital_absorption_convertible_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "可转债融资:",
        Language::EnUs => "Convertible debt:",
        Language::JaJp => "転換社債:",
    }
}

fn capital_absorption_ai_related_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "AI 相关融资:",
        Language::EnUs => "AI-related financing:",
        Language::JaJp => "AI 関連調達:",
    }
}

fn capital_absorption_etf_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "ETF 净流入:",
        Language::EnUs => "ETF net inflow:",
        Language::JaJp => "ETF 純流入:",
    }
}

fn capital_absorption_mutual_fund_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "共同基金净流入:",
        Language::EnUs => "Mutual fund net inflow:",
        Language::JaJp => "投資信託純流入:",
    }
}

fn capital_absorption_pension_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "养老金配置流:",
        Language::EnUs => "Pension allocation flow:",
        Language::JaJp => "年金配分フロー:",
    }
}

fn capital_absorption_foreign_capital_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "外资流入:",
        Language::EnUs => "Foreign capital inflow:",
        Language::JaJp => "海外資本流入:",
    }
}

fn capital_absorption_buyback_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司回购:",
        Language::EnUs => "Corporate buyback:",
        Language::JaJp => "自社株買い:",
    }
}

fn capital_absorption_ai_capex_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "AI CapEx 相关",
        Language::EnUs => "AI CapEx related",
        Language::JaJp => "AI CapEx 関連",
    }
}

fn capital_absorption_supply_event_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "实际供给事件",
        Language::EnUs => "Actual Supply Event Count",
        Language::JaJp => "実供給イベント数",
    }
}

fn capital_absorption_ai_ipo_queue_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "AI IPO 队列",
        Language::EnUs => "AI IPO Queue",
        Language::JaJp => "AI IPO キュー",
    }
}

fn capital_absorption_ipo_queue_history_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "IPO 队列历史",
        Language::EnUs => "IPO Queue History",
        Language::JaJp => "IPO キュー履歴",
    }
}

fn capital_absorption_queue_size_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Queue Size",
        Language::EnUs => "Queue Size",
        Language::JaJp => "Queue Size",
    }
}

fn capital_absorption_mega_cap_financing_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Mega Cap 融资",
        Language::EnUs => "Mega Cap Financing",
        Language::JaJp => "Mega Cap 調達",
    }
}

fn capital_absorption_secondary_offering_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "增发",
        Language::EnUs => "Secondary Offering",
        Language::JaJp => "増資",
    }
}

fn capital_absorption_convertible_debt_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "可转债",
        Language::EnUs => "Convertible Debt",
        Language::JaJp => "転換社債",
    }
}

fn capital_absorption_secondary_liquidity_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "二级流动性",
        Language::EnUs => "Secondary Liquidity",
        Language::JaJp => "セカンダリー流動性",
    }
}

fn capital_absorption_sources_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源",
        Language::EnUs => "Sources",
        Language::JaJp => "ソース数",
    }
}

fn capital_absorption_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "可信度",
        Language::EnUs => "Confidence",
        Language::JaJp => "信頼度",
    }
}

fn capital_absorption_event_type_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "事件类型",
        Language::EnUs => "Event Type",
        Language::JaJp => "イベント種別",
    }
}

fn capital_absorption_ratio_disabled_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "本阶段未启用完整量化",
        Language::EnUs => "Full quantification disabled in this phase",
        Language::JaJp => "本段階では完全な定量化を未使用",
    }
}

fn capital_absorption_observation_only_value(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "仅观察",
        Language::EnUs => "Observation Only",
        Language::JaJp => "観測のみ",
    }
}

fn capped_config_status(
    status: config::CapitalAbsorptionStatus,
) -> config::CapitalAbsorptionStatus {
    match status {
        config::CapitalAbsorptionStatus::Normal => config::CapitalAbsorptionStatus::Normal,
        config::CapitalAbsorptionStatus::Watch
        | config::CapitalAbsorptionStatus::Active
        | config::CapitalAbsorptionStatus::Stressed => config::CapitalAbsorptionStatus::Watch,
    }
}

fn capital_absorption_status_value(
    status: config::CapitalAbsorptionStatus,
    language: Language,
) -> String {
    match status {
        config::CapitalAbsorptionStatus::Normal => {
            capital_absorption_status_text("NORMAL", language)
        }
        config::CapitalAbsorptionStatus::Watch => capital_absorption_status_text("WATCH", language),
        config::CapitalAbsorptionStatus::Active => {
            capital_absorption_status_text("ACTIVE", language)
        }
        config::CapitalAbsorptionStatus::Stressed => {
            capital_absorption_status_text("STRESSED", language)
        }
    }
}

fn capital_absorption_trend_value(
    trend: config::CapitalAbsorptionTrend,
    language: Language,
) -> String {
    match trend {
        config::CapitalAbsorptionTrend::Decreasing => {
            capital_absorption_trend_text("FALLING", language)
        }
        config::CapitalAbsorptionTrend::Stable => capital_absorption_trend_text("STABLE", language),
        config::CapitalAbsorptionTrend::Increasing => {
            capital_absorption_trend_text("RISING", language)
        }
        config::CapitalAbsorptionTrend::Accelerating => {
            capital_absorption_trend_text("RISING", language)
        }
    }
}

fn capital_absorption_ratio_state_value(
    state: config::CapitalAbsorptionRatioState,
    language: Language,
) -> String {
    match state {
        config::CapitalAbsorptionRatioState::Low => capital_absorption_ratio_text("LOW", language),
        config::CapitalAbsorptionRatioState::Neutral => {
            capital_absorption_ratio_text("NEUTRAL", language)
        }
        config::CapitalAbsorptionRatioState::Elevated => {
            capital_absorption_ratio_text("ELEVATED", language)
        }
        config::CapitalAbsorptionRatioState::Stressed => {
            capital_absorption_ratio_text("STRESSED", language)
        }
    }
}

fn capital_absorption_event_category_value(
    category: config::CapitalAbsorptionEventCategory,
    language: Language,
) -> String {
    match category {
        config::CapitalAbsorptionEventCategory::MegaCapFinancing => {
            capital_absorption_auto_event_category_value(
                CapitalAbsorptionAutoEventCategory::MegaCapFinancing,
                language,
            )
        }
        config::CapitalAbsorptionEventCategory::IpoSupply => {
            capital_absorption_auto_event_category_value(
                CapitalAbsorptionAutoEventCategory::IpoSupply,
                language,
            )
        }
        config::CapitalAbsorptionEventCategory::SecondaryLiquidity => {
            capital_absorption_auto_event_category_value(
                CapitalAbsorptionAutoEventCategory::SecondaryLiquidity,
                language,
            )
        }
    }
}

fn capital_absorption_auto_status_value(
    status: CapitalAbsorptionAutoStatus,
    language: Language,
) -> String {
    match status {
        CapitalAbsorptionAutoStatus::Normal => capital_absorption_status_text("NORMAL", language),
        CapitalAbsorptionAutoStatus::Watch => capital_absorption_status_text("WATCH", language),
    }
}

fn capital_absorption_auto_trend_value(
    trend: CapitalAbsorptionAutoTrend,
    language: Language,
) -> String {
    match trend {
        CapitalAbsorptionAutoTrend::Decreasing => {
            capital_absorption_trend_text("FALLING", language)
        }
        CapitalAbsorptionAutoTrend::Stable => capital_absorption_trend_text("STABLE", language),
    }
}

fn capital_absorption_potential_supply_trend_value(
    trend: CapitalAbsorptionPotentialSupplyTrend,
    language: Language,
) -> String {
    match trend {
        CapitalAbsorptionPotentialSupplyTrend::Falling => {
            capital_absorption_trend_text("FALLING", language)
        }
        CapitalAbsorptionPotentialSupplyTrend::Stable => {
            capital_absorption_trend_text("STABLE", language)
        }
        CapitalAbsorptionPotentialSupplyTrend::Rising => {
            capital_absorption_trend_text("RISING", language)
        }
    }
}

fn capital_absorption_auto_ratio_state_value(
    state: crate::features::research::application::capital_absorption::CapitalAbsorptionAutoRatioState,
    language: Language,
) -> String {
    match state {
        crate::features::research::application::capital_absorption::CapitalAbsorptionAutoRatioState::Low => capital_absorption_ratio_text("LOW", language),
        crate::features::research::application::capital_absorption::CapitalAbsorptionAutoRatioState::Neutral => capital_absorption_ratio_text("NEUTRAL", language),
    }
}

fn capital_absorption_auto_event_category_value(
    category: CapitalAbsorptionAutoEventCategory,
    language: Language,
) -> String {
    match category {
        CapitalAbsorptionAutoEventCategory::MegaCapFinancing => match language {
            Language::ZhCn => "Mega Cap 融资".to_string(),
            Language::EnUs => "Mega Cap Financing".to_string(),
            Language::JaJp => "Mega Cap 調達".to_string(),
        },
        CapitalAbsorptionAutoEventCategory::IpoSupply => match language {
            Language::ZhCn => "IPO 供给".to_string(),
            Language::EnUs => "IPO Supply".to_string(),
            Language::JaJp => "IPO 供給".to_string(),
        },
        CapitalAbsorptionAutoEventCategory::SecondaryLiquidity => match language {
            Language::ZhCn => "二级流动性".to_string(),
            Language::EnUs => "Secondary Liquidity".to_string(),
            Language::JaJp => "セカンダリー流動性".to_string(),
        },
    }
}

fn capital_absorption_status_text(code: &str, language: Language) -> String {
    match (code, language) {
        ("NORMAL", Language::ZhCn) => "正常（NORMAL）".to_string(),
        ("WATCH", Language::ZhCn) => "观察（WATCH）".to_string(),
        ("ACTIVE", Language::ZhCn) => "结构观察（ACTIVE）".to_string(),
        ("STRESSED", Language::ZhCn) => "流动性压力（STRESSED）".to_string(),
        ("NORMAL", Language::JaJp) => "通常（NORMAL）".to_string(),
        ("WATCH", Language::JaJp) => "観察（WATCH）".to_string(),
        ("ACTIVE", Language::JaJp) => "構造観察（ACTIVE）".to_string(),
        ("STRESSED", Language::JaJp) => "流動性圧力（STRESSED）".to_string(),
        _ => code.to_string(),
    }
}

fn capital_absorption_trend_text(code: &str, language: Language) -> String {
    match (code, language) {
        ("FALLING", Language::ZhCn) => "下降（FALLING）".to_string(),
        ("RISING", Language::ZhCn) => "上升（RISING）".to_string(),
        ("DECREASING", Language::ZhCn) => "下降（DECREASING）".to_string(),
        ("STABLE", Language::ZhCn) => "稳定（STABLE）".to_string(),
        ("INCREASING", Language::ZhCn) => "上升（INCREASING）".to_string(),
        ("ACCELERATING", Language::ZhCn) => "加速（ACCELERATING）".to_string(),
        ("FALLING", Language::JaJp) => "低下（FALLING）".to_string(),
        ("RISING", Language::JaJp) => "上昇（RISING）".to_string(),
        ("DECREASING", Language::JaJp) => "低下（DECREASING）".to_string(),
        ("STABLE", Language::JaJp) => "安定（STABLE）".to_string(),
        ("INCREASING", Language::JaJp) => "上昇（INCREASING）".to_string(),
        ("ACCELERATING", Language::JaJp) => "加速（ACCELERATING）".to_string(),
        _ => code.to_string(),
    }
}

fn capital_absorption_supply_kind_value(
    supply_kind: CapitalAbsorptionSupplyKind,
    language: Language,
) -> &'static str {
    match (supply_kind, language) {
        (CapitalAbsorptionSupplyKind::Actual, Language::ZhCn) => "实际供给",
        (CapitalAbsorptionSupplyKind::Potential, Language::ZhCn) => "潜在队列",
        (CapitalAbsorptionSupplyKind::Actual, Language::EnUs) => "Actual Supply",
        (CapitalAbsorptionSupplyKind::Potential, Language::EnUs) => "Potential Queue",
        (CapitalAbsorptionSupplyKind::Actual, Language::JaJp) => "実供給",
        (CapitalAbsorptionSupplyKind::Potential, Language::JaJp) => "潜在キュー",
    }
}

fn capital_absorption_event_type_value(
    event_type: CapitalAbsorptionObservationEventType,
    language: Language,
) -> &'static str {
    match (event_type, language) {
        (CapitalAbsorptionObservationEventType::Confirmed, Language::ZhCn) => "确认（Confirmed）",
        (CapitalAbsorptionObservationEventType::Reported, Language::ZhCn) => "报道（Reported）",
        (CapitalAbsorptionObservationEventType::Rumor, Language::ZhCn) => "传闻（Rumor）",
        (CapitalAbsorptionObservationEventType::Confirmed, Language::EnUs) => "Confirmed",
        (CapitalAbsorptionObservationEventType::Reported, Language::EnUs) => "Reported",
        (CapitalAbsorptionObservationEventType::Rumor, Language::EnUs) => "Rumor",
        (CapitalAbsorptionObservationEventType::Confirmed, Language::JaJp) => "確認（Confirmed）",
        (CapitalAbsorptionObservationEventType::Reported, Language::JaJp) => "報道（Reported）",
        (CapitalAbsorptionObservationEventType::Rumor, Language::JaJp) => "噂（Rumor）",
    }
}

fn capital_absorption_ratio_text(code: &str, language: Language) -> String {
    match (code, language) {
        ("LOW", Language::ZhCn) => "低（LOW）".to_string(),
        ("NEUTRAL", Language::ZhCn) => "中性（NEUTRAL）".to_string(),
        ("ELEVATED", Language::ZhCn) => "偏高（ELEVATED）".to_string(),
        ("STRESSED", Language::ZhCn) => "压力（STRESSED）".to_string(),
        ("LOW", Language::JaJp) => "低い（LOW）".to_string(),
        ("NEUTRAL", Language::JaJp) => "中立（NEUTRAL）".to_string(),
        ("ELEVATED", Language::JaJp) => "高め（ELEVATED）".to_string(),
        ("STRESSED", Language::JaJp) => "圧迫（STRESSED）".to_string(),
        _ => code.to_string(),
    }
}

fn capital_absorption_confidence_value(
    confidence: CapitalAbsorptionAutoConfidence,
    language: Language,
) -> String {
    match (confidence, language) {
        (CapitalAbsorptionAutoConfidence::Low, Language::ZhCn) => "低".to_string(),
        (CapitalAbsorptionAutoConfidence::Medium, Language::ZhCn) => "中".to_string(),
        (CapitalAbsorptionAutoConfidence::High, Language::ZhCn) => "高".to_string(),
        (CapitalAbsorptionAutoConfidence::Low, Language::JaJp) => "低".to_string(),
        (CapitalAbsorptionAutoConfidence::Medium, Language::JaJp) => "中".to_string(),
        (CapitalAbsorptionAutoConfidence::High, Language::JaJp) => "高".to_string(),
        (CapitalAbsorptionAutoConfidence::Low, Language::EnUs) => "Low".to_string(),
        (CapitalAbsorptionAutoConfidence::Medium, Language::EnUs) => "Medium".to_string(),
        (CapitalAbsorptionAutoConfidence::High, Language::EnUs) => "High".to_string(),
    }
}

fn capital_absorption_ipo_queue_status_value(
    status: CapitalAbsorptionIpoQueueStatus,
    language: Language,
) -> String {
    match (status, language) {
        (CapitalAbsorptionIpoQueueStatus::Rumor, Language::ZhCn) => "传闻（Rumor）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Expected, Language::ZhCn) => {
            "预期（Expected）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Filed, Language::ZhCn) => "已提交（Filed）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Scheduled, Language::ZhCn) => {
            "已排期（Scheduled）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Completed, Language::ZhCn) => {
            "已完成（Completed）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Rumor, Language::JaJp) => "噂（Rumor）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Expected, Language::JaJp) => {
            "予想（Expected）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Filed, Language::JaJp) => "提出済み（Filed）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Scheduled, Language::JaJp) => {
            "予定済み（Scheduled）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Completed, Language::JaJp) => {
            "完了（Completed）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Rumor, Language::EnUs) => "Rumor".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Expected, Language::EnUs) => "Expected".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Filed, Language::EnUs) => "Filed".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Scheduled, Language::EnUs) => "Scheduled".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Completed, Language::EnUs) => "Completed".to_string(),
    }
}

pub(crate) fn daily_calibration_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🧭 每日认知校准",
        Language::EnUs => "🧭 Daily Cognitive Calibration",
        Language::JaJp => "🧭 日次認知校正",
    }
}

pub(crate) fn daily_calibration_audit_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 1. 今日审计摘要",
        Language::EnUs => "## 1. Daily Audit Summary",
        Language::JaJp => "## 1. 日次監査サマリー",
    }
}

pub(crate) fn daily_calibration_questions_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 2. 日报校准问题",
        Language::EnUs => "## 2. Daily Calibration Questions",
        Language::JaJp => "## 2. 日次校正質問",
    }
}

pub(crate) fn daily_calibration_attention_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 3. 认知关注校准",
        Language::EnUs => "## 3. Research Attention Calibration",
        Language::JaJp => "## 3. 認知注目の校正",
    }
}

pub(crate) fn daily_calibration_thesis_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 4. 资产观察命题",
        Language::EnUs => "## 4. Asset Observation Theses",
        Language::JaJp => "## 4. 銘柄別観測命題",
    }
}

pub(crate) fn daily_calibration_macro_gravity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 5. 宏观重力校准",
        Language::EnUs => "## 5. Macro Gravity Calibration",
        Language::JaJp => "## 5. マクロ重力校正",
    }
}

pub(crate) fn daily_calibration_gray_rhino_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 7. 灰犀牛升级监控",
        Language::EnUs => "## 7. Gray Rhino Escalation",
        Language::JaJp => "## 7. 灰色のサイ昇格監視",
    }
}

pub(crate) fn daily_calibration_capital_absorption_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 6. 市场资本吸收监控",
        Language::EnUs => "## 6. Capital Absorption Monitor",
        Language::JaJp => "## 6. 資本吸収モニター",
    }
}

pub(crate) fn daily_calibration_question_market(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "固定问题: 今天是市场理解变化，还是只是噪音变化？",
        Language::EnUs => "Fixed question: did market understanding change today, or only noise?",
        Language::JaJp => "固定質問: 今日変化したのは市場理解か、それともノイズだけか？",
    }
}

pub(crate) fn daily_calibration_question_gate(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 战术状态:",
        Language::EnUs => "- Tactical state:",
        Language::JaJp => "- 戦術状態:",
    }
}

pub(crate) fn daily_calibration_question_evidence(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 证据状态:",
        Language::EnUs => "- Evidence state:",
        Language::JaJp => "- 証拠状態:",
    }
}

pub(crate) fn daily_calibration_question_attention(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 需校准认知对象数:",
        Language::EnUs => "- Attention entries to calibrate:",
        Language::JaJp => "- 校正対象の認知項目数:",
    }
}

pub(crate) fn daily_calibration_question_thesis(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 需复查观察命题数:",
        Language::EnUs => "- Observation theses to review:",
        Language::JaJp => "- 再確認する観測命題数:",
    }
}

pub(crate) fn daily_calibration_question_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "校准口径: 战术状态、证据状态、认知对象和观察命题只用于复盘，不构成新信号。",
        Language::EnUs => {
            "Calibration rule: tactical state, evidence state, attention entries, and observation theses are for review only, not new signals."
        }
        Language::JaJp => {
            "校正口径: 戦術状態、証拠状態、認知項目、観測命題は復盤専用であり、新シグナルではない。"
        }
    }
}

pub(crate) fn daily_calibration_evidence_strong(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "结构证据较强，重点检查价格/扩散是否跟上",
        Language::EnUs => {
            "structural evidence is strong; check whether price/diffusion is following"
        }
        Language::JaJp => "構造証拠は強い。価格/拡散が追随しているか確認",
    }
}

pub(crate) fn daily_calibration_evidence_observed(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已有结构证据，重点检查质量而非数量",
        Language::EnUs => "structural evidence observed; check quality, not quantity",
        Language::JaJp => "構造証拠を観測中。数量ではなく品質を確認",
    }
}

pub(crate) fn daily_calibration_evidence_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无可用结构证据或审计记录",
        Language::EnUs => "no usable structural evidence or audit record",
        Language::JaJp => "利用可能な構造証拠または監査記録なし",
    }
}

pub(crate) fn daily_calibration_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 本日报只校准系统理解、证据质量、认知资源与观察命题；不生成新的交易指令。"
        }
        Language::EnUs => {
            "Boundary: this daily report only calibrates system understanding, evidence quality, cognitive resources, and observation theses; it does not generate new trade instructions."
        }
        Language::JaJp => {
            "境界: この日報はシステム理解、証拠品質、認知資源、観測命題だけを校正し、新しい売買指示は生成しない。"
        }
    }
}

fn information_density_label(value: config::InformationDensity) -> &'static str {
    match value {
        config::InformationDensity::Expanding => "EXPANDING",
        config::InformationDensity::Active => "ACTIVE",
        config::InformationDensity::Stable => "STABLE",
        config::InformationDensity::Saturated => "SATURATED",
    }
}

fn attention_cost_label(value: config::AttentionCost) -> &'static str {
    match value {
        config::AttentionCost::Low => "LOW",
        config::AttentionCost::Moderate => "MODERATE",
        config::AttentionCost::High => "HIGH",
        config::AttentionCost::Draining => "DRAINING",
    }
}

pub(crate) fn macro_pressure_label(value: config::MacroPressure) -> &'static str {
    match value {
        config::MacroPressure::Falling => "FALLING",
        config::MacroPressure::Neutral => "NEUTRAL",
        config::MacroPressure::Rising => "RISING",
        config::MacroPressure::Tight => "TIGHT",
    }
}

pub(crate) fn yield_curve_label(value: config::YieldCurveState) -> &'static str {
    match value {
        config::YieldCurveState::Normal => "NORMAL",
        config::YieldCurveState::Flat => "FLAT",
        config::YieldCurveState::Inverted => "INVERTED",
        config::YieldCurveState::Steepening => "STEEPENING",
    }
}

pub(crate) fn credit_stress_label(value: config::CreditStress) -> &'static str {
    match value {
        config::CreditStress::Normal => "NORMAL",
        config::CreditStress::Watch => "WATCH",
        config::CreditStress::Stress => "STRESS",
    }
}

pub(crate) fn liquidity_condition_label(value: config::LiquidityCondition) -> &'static str {
    match value {
        config::LiquidityCondition::Loose => "LOOSE",
        config::LiquidityCondition::Neutral => "NEUTRAL",
        config::LiquidityCondition::Tight => "TIGHT",
    }
}

pub(crate) fn growth_valuation_impact_label(value: config::GrowthValuationImpact) -> &'static str {
    match value {
        config::GrowthValuationImpact::Supportive => "SUPPORTIVE",
        config::GrowthValuationImpact::Neutral => "NEUTRAL",
        config::GrowthValuationImpact::Compressing => "COMPRESSING",
    }
}

#[cfg(test)]
mod capital_absorption_report_tests {
    use super::*;
    use crate::features::research::application::capital_absorption::{
        CapitalAbsorptionAutoRatio, CapitalAbsorptionAutoRatioState,
        CapitalAbsorptionIpoQueueHistoryPoint, CapitalAbsorptionSourceHealth,
        CapitalAbsorptionSourceStatus,
    };
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
            assert!(
                report.contains("Potential Supply Trend") || report.contains("潜在供給トレンド")
            );
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
}
