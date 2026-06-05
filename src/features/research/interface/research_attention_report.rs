use crate::config;
use crate::features::research::interface::asset_thesis_report::localized_research_reason;
use crate::features::shared::interface::i18n::Language;
use std::collections::BTreeMap;

pub(crate) fn build_research_attention_report_from_entries(
    entries: Option<&BTreeMap<String, config::ResearchAttentionEntry>>,
    language: Language,
) -> String {
    let entries = match entries {
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
