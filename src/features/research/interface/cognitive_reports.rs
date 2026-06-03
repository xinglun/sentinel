use crate::config;
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
    language: Language,
) -> String {
    let Some(capital_absorption) = app_config
        .capital_absorption
        .as_ref()
        .filter(|capital_absorption| capital_absorption.enable.unwrap_or(true))
    else {
        return capital_absorption_empty(language).to_string();
    };

    let mut out = String::new();
    out.push_str(capital_absorption_title(language));
    out.push_str("\n\n");
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_status_label(language),
        capital_absorption_status_value(capital_absorption.status)
    ));
    push_capital_absorption_events(
        &mut out,
        capital_absorption.observed_events.as_deref().unwrap_or(&[]),
        language,
    );
    push_capital_demand(&mut out, &capital_absorption.capital_demand, language);
    push_capital_supply(&mut out, &capital_absorption.capital_supply, language);
    out.push_str(&format!(
        "{} {}{}\n\n",
        capital_absorption_ratio_label(language),
        capital_absorption_ratio_state_value(capital_absorption.absorption_ratio.state),
        capital_absorption
            .absorption_ratio
            .value
            .map(|value| format!(" ({value:.2})"))
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_structural_impact_label(language),
        capital_absorption
            .structural_impact
            .as_deref()
            .unwrap_or_else(|| capital_absorption_default_impact(language))
    ));
    push_capital_absorption_conditions(
        &mut out,
        capital_absorption_upgrade_active_label(language),
        capital_absorption
            .upgrade_to_active
            .as_deref()
            .unwrap_or(&[]),
    );
    push_capital_absorption_conditions(
        &mut out,
        capital_absorption_upgrade_stressed_label(language),
        capital_absorption
            .upgrade_to_stressed
            .as_deref()
            .unwrap_or(&[]),
    );
    out.push_str(capital_absorption_boundary(language));
    out
}

fn push_capital_absorption_events(
    out: &mut String,
    events: &[config::CapitalAbsorptionEventConfig],
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
        let ai_capex = if event.ai_capex_related.unwrap_or(false) {
            format!(" · {}", capital_absorption_ai_capex_label(language))
        } else {
            String::new()
        };
        out.push_str(&format!(
            "- {} · {}{}{} · {}\n",
            capital_absorption_event_category_value(event.category),
            event.subject,
            amount,
            ai_capex,
            event.description
        ));
        if let Some(source_url) = &event.source_url {
            out.push_str(&format!("  {}\n", source_url));
        }
    }
    out.push('\n');
}

fn push_capital_demand(out: &mut String, demand: &config::CapitalDemandConfig, language: Language) {
    out.push_str(capital_absorption_demand_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {} {}\n",
        capital_absorption_trend_label(language),
        capital_absorption_trend_value(demand.trend)
    ));
    push_optional_usd(
        out,
        capital_absorption_rolling_12m_label(language),
        demand.rolling_12m_usd_b,
    );
    push_optional_score(out, capital_absorption_score_label(language), demand.score);
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
    out.push('\n');
}

fn push_capital_supply(out: &mut String, supply: &config::CapitalSupplyConfig, language: Language) {
    out.push_str(capital_absorption_supply_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {} {}\n",
        capital_absorption_trend_label(language),
        capital_absorption_trend_value(supply.trend)
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

fn push_capital_absorption_conditions(out: &mut String, title: &str, conditions: &[String]) {
    if conditions.is_empty() {
        return;
    }
    out.push_str(title);
    out.push_str(":\n");
    for condition in conditions {
        out.push_str(&format!("- {condition}\n"));
    }
    out.push('\n');
}

fn capital_absorption_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "📊 Capital Absorption Monitor",
        Language::EnUs => "📊 Capital Absorption Monitor",
        Language::JaJp => "📊 Capital Absorption Monitor",
    }
}

fn capital_absorption_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "📊 Capital Absorption Monitor\n\n未配置资本吸收观察层。\n\n边界: 本模块只观察资本供需结构，不生成交易信号。"
        }
        Language::EnUs => {
            "📊 Capital Absorption Monitor\n\nNo capital absorption context configured.\n\nBoundary: this module only observes capital supply-demand structure; it does not generate trade signals."
        }
        Language::JaJp => {
            "📊 Capital Absorption Monitor\n\n資本吸収観測レイヤーは未設定です。\n\n境界: このモジュールは資本需給構造だけを観測し、売買シグナルは生成しない。"
        }
    }
}

fn capital_absorption_status_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Capital Absorption Status:",
        Language::EnUs => "Capital Absorption Status:",
        Language::JaJp => "Capital Absorption Status:",
    }
}

fn capital_absorption_events_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Observed Events",
        Language::EnUs => "Observed Events",
        Language::JaJp => "Observed Events",
    }
}

fn capital_absorption_no_events(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 未观察到大型资本吸收事件。",
        Language::EnUs => "- No large capital absorption events observed.",
        Language::JaJp => "- 大型の資本吸収イベントは未観測です。",
    }
}

fn capital_absorption_demand_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Capital Demand",
        Language::EnUs => "Capital Demand",
        Language::JaJp => "Capital Demand",
    }
}

fn capital_absorption_supply_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Capital Supply",
        Language::EnUs => "Capital Supply",
        Language::JaJp => "Capital Supply",
    }
}

fn capital_absorption_ratio_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Capital Absorption Ratio:",
        Language::EnUs => "Capital Absorption Ratio:",
        Language::JaJp => "Capital Absorption Ratio:",
    }
}

fn capital_absorption_structural_impact_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Structural Impact:",
        Language::EnUs => "Structural Impact:",
        Language::JaJp => "Structural Impact:",
    }
}

fn capital_absorption_default_impact(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Observation Only",
        Language::EnUs => "Observation Only",
        Language::JaJp => "Observation Only",
    }
}

fn capital_absorption_upgrade_active_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Upgrade To ACTIVE",
        Language::EnUs => "Upgrade To ACTIVE",
        Language::JaJp => "Upgrade To ACTIVE",
    }
}

fn capital_absorption_upgrade_stressed_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Upgrade To STRESSED",
        Language::EnUs => "Upgrade To STRESSED",
        Language::JaJp => "Upgrade To STRESSED",
    }
}

fn capital_absorption_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "Boundary: This module observes capital supply-demand dynamics and does not generate trading signals. It does not affect READY / EXECUTE."
        }
        Language::EnUs => {
            "Boundary: This module observes capital supply-demand dynamics and does not generate trading signals. It does not affect READY / EXECUTE."
        }
        Language::JaJp => {
            "Boundary: This module observes capital supply-demand dynamics and does not generate trading signals. It does not affect READY / EXECUTE."
        }
    }
}

fn capital_absorption_trend_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Trend:",
        Language::EnUs => "Trend:",
        Language::JaJp => "Trend:",
    }
}

fn capital_absorption_rolling_12m_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Rolling 12M:",
        Language::EnUs => "Rolling 12M:",
        Language::JaJp => "Rolling 12M:",
    }
}

fn capital_absorption_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Score:",
        Language::EnUs => "Score:",
        Language::JaJp => "Score:",
    }
}

fn capital_absorption_ipo_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "IPO financing:",
        Language::EnUs => "IPO financing:",
        Language::JaJp => "IPO financing:",
    }
}

fn capital_absorption_secondary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Secondary offering:",
        Language::EnUs => "Secondary offering:",
        Language::JaJp => "Secondary offering:",
    }
}

fn capital_absorption_convertible_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Convertible debt:",
        Language::EnUs => "Convertible debt:",
        Language::JaJp => "Convertible debt:",
    }
}

fn capital_absorption_ai_related_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "AI-related financing:",
        Language::EnUs => "AI-related financing:",
        Language::JaJp => "AI-related financing:",
    }
}

fn capital_absorption_etf_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "ETF net inflow:",
        Language::EnUs => "ETF net inflow:",
        Language::JaJp => "ETF net inflow:",
    }
}

fn capital_absorption_mutual_fund_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Mutual fund net inflow:",
        Language::EnUs => "Mutual fund net inflow:",
        Language::JaJp => "Mutual fund net inflow:",
    }
}

fn capital_absorption_pension_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Pension allocation flow:",
        Language::EnUs => "Pension allocation flow:",
        Language::JaJp => "Pension allocation flow:",
    }
}

fn capital_absorption_foreign_capital_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Foreign capital inflow:",
        Language::EnUs => "Foreign capital inflow:",
        Language::JaJp => "Foreign capital inflow:",
    }
}

fn capital_absorption_buyback_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Corporate buyback:",
        Language::EnUs => "Corporate buyback:",
        Language::JaJp => "Corporate buyback:",
    }
}

fn capital_absorption_ai_capex_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "AI CapEx related",
        Language::EnUs => "AI CapEx related",
        Language::JaJp => "AI CapEx related",
    }
}

fn capital_absorption_status_value(status: config::CapitalAbsorptionStatus) -> &'static str {
    match status {
        config::CapitalAbsorptionStatus::Normal => "NORMAL",
        config::CapitalAbsorptionStatus::Watch => "WATCH",
        config::CapitalAbsorptionStatus::Active => "ACTIVE",
        config::CapitalAbsorptionStatus::Stressed => "STRESSED",
    }
}

fn capital_absorption_trend_value(trend: config::CapitalAbsorptionTrend) -> &'static str {
    match trend {
        config::CapitalAbsorptionTrend::Decreasing => "DECREASING",
        config::CapitalAbsorptionTrend::Stable => "STABLE",
        config::CapitalAbsorptionTrend::Increasing => "INCREASING",
        config::CapitalAbsorptionTrend::Accelerating => "ACCELERATING",
    }
}

fn capital_absorption_ratio_state_value(
    state: config::CapitalAbsorptionRatioState,
) -> &'static str {
    match state {
        config::CapitalAbsorptionRatioState::Low => "LOW",
        config::CapitalAbsorptionRatioState::Neutral => "NEUTRAL",
        config::CapitalAbsorptionRatioState::Elevated => "ELEVATED",
        config::CapitalAbsorptionRatioState::Stressed => "STRESSED",
    }
}

fn capital_absorption_event_category_value(
    category: config::CapitalAbsorptionEventCategory,
) -> &'static str {
    match category {
        config::CapitalAbsorptionEventCategory::MegaCapFinancing => "Mega Cap Financing",
        config::CapitalAbsorptionEventCategory::IpoSupply => "IPO Supply",
        config::CapitalAbsorptionEventCategory::SecondaryLiquidity => "Secondary Liquidity",
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
