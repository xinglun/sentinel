use crate::config;
use crate::features::research::interface::default_cognitive_localizations as defaults;
use crate::features::shared::interface::i18n::Language;
use std::collections::BTreeMap;

pub(crate) fn build_asset_thesis_report_from_entries(
    entries: Option<&BTreeMap<String, config::AssetThesisEntry>>,
    language: Language,
) -> String {
    let entries = match entries {
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

pub(crate) fn enabled_research_attention_count_from_entries(
    entries: Option<&BTreeMap<String, config::ResearchAttentionEntry>>,
) -> usize {
    entries
        .map(|entries| {
            entries
                .values()
                .filter(|entry| entry.enable.unwrap_or(true))
                .count()
        })
        .unwrap_or(0)
}

pub(crate) fn enabled_asset_thesis_count_from_entries(
    entries: Option<&BTreeMap<String, config::AssetThesisEntry>>,
) -> usize {
    entries
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

pub(super) fn localized_research_reason(
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
