use crate::features::research::application::gray_rhino_monitoring_state::{
    GrayRhinoMonitoringDirection, GrayRhinoMonitoringStatus,
};
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
};
use crate::features::research::interface::gray_rhino_read_model_builder::{
    group_company_candidates, group_company_statuses,
};
use crate::features::shared::interface::i18n::Language;
use std::collections::BTreeSet;

pub(crate) fn render_gray_rhino_inline_reference(candidates: &[GrayRhinoCandidate]) -> String {
    if candidates.is_empty() {
        return "Gray Rhino Inline Reference: none auto-discovered.\nBoundary: reference only; no trading, Gate, trend, or market-state mutation.".to_string();
    }
    let mut out = String::from("Gray Rhino Inline Reference (semantic isolation)\n");
    for candidate in candidates {
        out.push_str(&format!(
            "- {} / {:?} / {:?} / {:?}: {}\n",
            candidate.subject,
            candidate.scope,
            candidate.kind,
            candidate.state,
            candidate.evidence.join(" ")
        ));
        if !candidate.watch_triggers.is_empty() {
            out.push_str(&format!(
                "  Trigger watch: {}\n",
                candidate.watch_triggers.join(" / ")
            ));
        }
    }
    out.push_str("Boundary: reference only; no trading, Gate, trend, or market-state mutation.");
    out
}

pub(crate) fn render_auto_discovery_inline_reference(
    watch_symbols: &[String],
    display_candidates: &[GrayRhinoCandidate],
    monitoring_statuses: &[GrayRhinoMonitoringStatus],
    language: Language,
) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        render_gray_rhino_compact_summary(display_candidates, monitoring_statuses, language),
        render_watchlist_inline_candidates(watch_symbols, display_candidates, language),
        render_watchlist_inline_monitoring(watch_symbols, monitoring_statuses, language)
    )
}

fn render_gray_rhino_compact_summary(
    candidates: &[GrayRhinoCandidate],
    statuses: &[GrayRhinoMonitoringStatus],
    language: Language,
) -> String {
    let _ = candidates;
    let active_statuses = statuses
        .iter()
        .filter(|status| is_active_monitoring_state(status.current_state))
        .collect::<Vec<_>>();
    let market_active = active_statuses
        .iter()
        .filter(|status| status.scope == GrayRhinoCandidateScope::Market)
        .count();
    let company_subjects = active_statuses
        .iter()
        .filter(|status| status.scope == GrayRhinoCandidateScope::Company)
        .map(|status| status.subject.to_uppercase())
        .collect::<BTreeSet<_>>();
    let cooling_subjects = statuses
        .iter()
        .filter(|status| {
            status.scope == GrayRhinoCandidateScope::Company
                && status.current_state == GrayRhinoCandidateState::Cooling
        })
        .map(|status| status.subject.to_uppercase())
        .collect::<BTreeSet<_>>();
    let resolved_subjects = statuses
        .iter()
        .filter(|status| {
            status.scope == GrayRhinoCandidateScope::Company
                && status.current_state == GrayRhinoCandidateState::Resolved
        })
        .map(|status| status.subject.to_uppercase())
        .collect::<BTreeSet<_>>();
    let intensifying_subjects = statuses
        .iter()
        .filter(|status| {
            status.scope == GrayRhinoCandidateScope::Company
                && status.direction == GrayRhinoMonitoringDirection::Intensifying
        })
        .map(|status| status.subject.to_uppercase())
        .collect::<BTreeSet<_>>();

    let company_summary = if company_subjects.is_empty() {
        none_label(language).to_string()
    } else {
        company_subjects.into_iter().collect::<Vec<_>>().join(", ")
    };
    let intensifying_summary = if intensifying_subjects.is_empty() {
        none_label(language).to_string()
    } else {
        intensifying_subjects
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let cooling_summary = format_subject_set(cooling_subjects, language);
    let resolved_summary = format_subject_set(resolved_subjects, language);

    format!(
        "{}\n- {}: {market_active}\n- {}: {company_summary}\n- {}: {cooling_summary}\n- {}: {resolved_summary}\n- {}: {intensifying_summary}\n{}",
        gray_rhino_summary_title(language),
        market_active_label(language),
        company_active_label(language),
        company_cooling_label(language),
        company_resolved_label(language),
        company_intensifying_label(language),
        summary_boundary_label(language)
    )
}

fn is_active_monitoring_state(state: GrayRhinoCandidateState) -> bool {
    matches!(
        state,
        GrayRhinoCandidateState::Visible
            | GrayRhinoCandidateState::Expanding
            | GrayRhinoCandidateState::Critical
    )
}

fn format_subject_set(subjects: BTreeSet<String>, language: Language) -> String {
    if subjects.is_empty() {
        none_label(language).to_string()
    } else {
        subjects.into_iter().collect::<Vec<_>>().join(", ")
    }
}

fn render_watchlist_inline_candidates(
    watch_symbols: &[String],
    candidates: &[GrayRhinoCandidate],
    language: Language,
) -> String {
    if candidates.is_empty() {
        return format!(
            "{}: {}\n{}",
            inline_reference_title(language),
            none_auto_discovered_label(language),
            reference_boundary_label(language)
        );
    }

    let mut out = format!("{}\n", inline_reference_title(language));
    let market_candidates = candidates
        .iter()
        .filter(|candidate| candidate.scope == GrayRhinoCandidateScope::Market)
        .collect::<Vec<_>>();
    out.push_str(market_reference_title(language));
    out.push('\n');
    if market_candidates.is_empty() {
        out.push_str(&format!("- {}\n", none_label(language)));
    } else {
        for candidate in market_candidates {
            append_candidate_line(&mut out, candidate, language);
        }
    }

    out.push('\n');
    out.push_str(watchlist_reference_title(language));
    out.push('\n');
    let by_subject = group_company_candidates(candidates);
    let watch_symbol_keys = watch_symbols
        .iter()
        .map(|symbol| symbol.to_uppercase())
        .collect::<BTreeSet<_>>();
    for symbol in watch_symbols {
        out.push_str(&format!("- {symbol}\n"));
        if let Some(items) = by_subject.get(&symbol.to_uppercase()) {
            for candidate in items {
                append_candidate_line(&mut out, candidate, language);
            }
        } else {
            out.push_str(&format!(
                "  {}: {}\n",
                company_gray_rhino_label(language),
                none_label(language)
            ));
        }
    }
    let other_subjects = by_subject
        .keys()
        .filter(|subject| !watch_symbol_keys.contains(*subject))
        .collect::<Vec<_>>();
    if !other_subjects.is_empty() {
        out.push('\n');
        out.push_str(other_company_reference_title(language));
        out.push('\n');
        for subject in other_subjects {
            out.push_str(&format!("- {subject}\n"));
            if let Some(items) = by_subject.get(subject) {
                for candidate in items {
                    append_candidate_line(&mut out, candidate, language);
                }
            }
        }
    }
    out.push_str(reference_boundary_label(language));
    out
}

fn render_watchlist_inline_monitoring(
    watch_symbols: &[String],
    statuses: &[GrayRhinoMonitoringStatus],
    language: Language,
) -> String {
    if statuses.is_empty() {
        return format!(
            "{}: {}.\n{}",
            monitoring_status_title(language),
            none_label(language),
            reference_boundary_label(language)
        );
    }

    let mut out = format!("{}\n", monitoring_state_title(language));
    let market_statuses = statuses
        .iter()
        .filter(|status| status.scope == GrayRhinoCandidateScope::Market)
        .collect::<Vec<_>>();
    out.push_str(market_reference_title(language));
    out.push('\n');
    if market_statuses.is_empty() {
        out.push_str(&format!("- {}\n", none_label(language)));
    } else {
        for status in market_statuses {
            append_monitoring_line(&mut out, status, language);
        }
    }

    out.push('\n');
    out.push_str(watchlist_monitoring_title(language));
    out.push('\n');
    let by_subject = group_company_statuses(statuses);
    let watch_symbol_keys = watch_symbols
        .iter()
        .map(|symbol| symbol.to_uppercase())
        .collect::<BTreeSet<_>>();
    for symbol in watch_symbols {
        out.push_str(&format!("- {symbol}\n"));
        if let Some(items) = by_subject.get(&symbol.to_uppercase()) {
            for status in items {
                append_monitoring_line(&mut out, status, language);
            }
        } else {
            out.push_str(&format!(
                "  {}: {}\n",
                company_gray_rhino_monitoring_label(language),
                none_label(language)
            ));
        }
    }
    let other_subjects = by_subject
        .keys()
        .filter(|subject| !watch_symbol_keys.contains(*subject))
        .collect::<Vec<_>>();
    if !other_subjects.is_empty() {
        out.push('\n');
        out.push_str(other_company_monitoring_title(language));
        out.push('\n');
        for subject in other_subjects {
            out.push_str(&format!("- {subject}\n"));
            if let Some(items) = by_subject.get(subject) {
                for status in items {
                    append_monitoring_line(&mut out, status, language);
                }
            }
        }
    }
    out.push_str(reference_boundary_label(language));
    out
}

fn append_candidate_line(out: &mut String, candidate: &GrayRhinoCandidate, language: Language) {
    out.push_str(&format!(
        "  - {} / {} / {} / {}: {}\n",
        candidate.subject,
        candidate_scope_label(candidate.scope, language),
        candidate_kind_label(candidate.kind, language),
        candidate_state_label(candidate.state, language),
        candidate
            .evidence
            .iter()
            .map(|item| localized_structural_text(item, language))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    if !candidate.watch_triggers.is_empty() {
        out.push_str(&format!(
            "    {}: {}\n",
            trigger_watch_label(language),
            candidate
                .watch_triggers
                .iter()
                .map(|item| localized_structural_text(item, language))
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
}

fn append_monitoring_line(
    out: &mut String,
    status: &GrayRhinoMonitoringStatus,
    language: Language,
) {
    out.push_str(&format!(
        "  - {} / {} / {}: {} ({}, {}: {}, {}: {}, {}: {})\n",
        status.subject,
        candidate_scope_label(status.scope, language),
        candidate_kind_label(status.kind, language),
        candidate_state_label(status.current_state, language),
        monitoring_direction_label(status.direction, language),
        observations_label(language),
        status.observation_count,
        latest_label(language),
        status.latest_observed_at,
        stale_days_label(language),
        status.stale_days
    ));
    if let Some(previous_state) = status.previous_state {
        out.push_str(&format!(
            "    {}: {}\n",
            previous_state_label(language),
            candidate_state_label(previous_state, language)
        ));
    }
}

fn gray_rhino_summary_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛摘要（语义隔离）",
        Language::EnUs => "Gray Rhino Summary (semantic isolation)",
        Language::JaJp => "灰色のサイ要約（意味的に隔離）",
    }
}

fn market_active_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "市场活跃候选",
        Language::EnUs => "Market active candidates",
        Language::JaJp => "市場の有効候補",
    }
}

fn company_active_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司活跃候选",
        Language::EnUs => "Company active candidates",
        Language::JaJp => "企業の有効候補",
    }
}

fn company_cooling_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司降温候选",
        Language::EnUs => "Company cooling candidates",
        Language::JaJp => "企業の冷却中候補",
    }
}

fn company_resolved_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司已解除候选",
        Language::EnUs => "Company resolved candidates",
        Language::JaJp => "企業の解消済み候補",
    }
}

fn company_intensifying_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司升温观察",
        Language::EnUs => "Company intensifying watch",
        Language::JaJp => "企業の強まり観測",
    }
}

fn inline_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛内联参考（语义隔离）",
        Language::EnUs => "Gray Rhino Inline Reference (semantic isolation)",
        Language::JaJp => "灰色のサイ内訳参考（意味的に隔離）",
    }
}

fn market_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "市场参考",
        Language::EnUs => "Market Reference",
        Language::JaJp => "市場参考",
    }
}

fn watchlist_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察列表内联参考",
        Language::EnUs => "Watchlist Inline Reference",
        Language::JaJp => "監視リスト内訳参考",
    }
}

fn other_company_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "其他公司参考",
        Language::EnUs => "Other Company Reference",
        Language::JaJp => "その他企業参考",
    }
}

fn monitoring_status_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛监控状态",
        Language::EnUs => "Gray Rhino Monitoring Status",
        Language::JaJp => "灰色のサイ監視状態",
    }
}

fn monitoring_state_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛监控状态（语义隔离）",
        Language::EnUs => "Gray Rhino Monitoring State (semantic isolation)",
        Language::JaJp => "灰色のサイ監視状態（意味的に隔離）",
    }
}

fn watchlist_monitoring_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察列表内联监控",
        Language::EnUs => "Watchlist Inline Monitoring",
        Language::JaJp => "監視リスト内訳監視",
    }
}

fn other_company_monitoring_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "其他公司监控",
        Language::EnUs => "Other Company Monitoring",
        Language::JaJp => "その他企業監視",
    }
}

fn reference_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "边界声明: 仅作结构风险参考；不改变交易、闸门、趋势或市场状态。",
        Language::EnUs => {
            "Boundary: reference only; no trading, Gate, trend, or market-state mutation."
        }
        Language::JaJp => {
            "境界声明: 構造リスクの参考のみで、取引、ゲート、トレンド、市場状態は変更しない。"
        }
    }
}

fn summary_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "边界声明: 摘要仅供参考；不改变交易、闸门、趋势或市场状态。",
        Language::EnUs => {
            "Boundary: summary only; no trading, Gate, trend, or market-state mutation."
        }
        Language::JaJp => {
            "境界声明: 要約は参考のみで、取引、ゲート、トレンド、市場状態は変更しない。"
        }
    }
}

fn none_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无",
        Language::EnUs => "none",
        Language::JaJp => "なし",
    }
}

fn none_auto_discovered_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未发现自动候选",
        Language::EnUs => "none auto-discovered",
        Language::JaJp => "自動発見候補なし",
    }
}

fn company_gray_rhino_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司灰犀牛",
        Language::EnUs => "Company Gray Rhino",
        Language::JaJp => "企業灰色のサイ",
    }
}

fn company_gray_rhino_monitoring_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司灰犀牛监控",
        Language::EnUs => "Company Gray Rhino monitoring",
        Language::JaJp => "企業灰色のサイ監視",
    }
}

fn trigger_watch_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "触发观察",
        Language::EnUs => "Trigger watch",
        Language::JaJp => "トリガー観測",
    }
}

fn observations_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观测次数",
        Language::EnUs => "observations",
        Language::JaJp => "観測回数",
    }
}

fn latest_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "最新",
        Language::EnUs => "latest",
        Language::JaJp => "最新",
    }
}

fn stale_days_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "陈旧天数",
        Language::EnUs => "stale_days",
        Language::JaJp => "古さ（日）",
    }
}

fn previous_state_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "前次状态",
        Language::EnUs => "Previous state",
        Language::JaJp => "前回状態",
    }
}

fn candidate_scope_label(scope: GrayRhinoCandidateScope, language: Language) -> &'static str {
    match (scope, language) {
        (GrayRhinoCandidateScope::Company, Language::ZhCn) => "公司",
        (GrayRhinoCandidateScope::Market, Language::ZhCn) => "市场",
        (GrayRhinoCandidateScope::Company, Language::EnUs) => "Company",
        (GrayRhinoCandidateScope::Market, Language::EnUs) => "Market",
        (GrayRhinoCandidateScope::Company, Language::JaJp) => "企業",
        (GrayRhinoCandidateScope::Market, Language::JaJp) => "市場",
    }
}

fn candidate_kind_label(kind: GrayRhinoCandidateKind, language: Language) -> &'static str {
    match (kind, language) {
        (GrayRhinoCandidateKind::GovernanceConcentration, Language::ZhCn) => "治理集中",
        (GrayRhinoCandidateKind::DependencyConcentration, Language::ZhCn) => "依赖集中",
        (GrayRhinoCandidateKind::InstitutionalMaturityGap, Language::ZhCn) => "制度成熟缺口",
        (GrayRhinoCandidateKind::RedundancyGap, Language::ZhCn) => "冗余缺口",
        (GrayRhinoCandidateKind::MarketConcentration, Language::ZhCn) => "市场集中",
        (GrayRhinoCandidateKind::NarrativeCrowding, Language::ZhCn) => "叙事拥挤",
        (GrayRhinoCandidateKind::LiquidityFragility, Language::ZhCn) => "流动性脆弱",
        (GrayRhinoCandidateKind::CapexPaybackFragility, Language::ZhCn) => "资本开支回收脆弱",
        (GrayRhinoCandidateKind::GovernanceConcentration, Language::EnUs) => {
            "Governance Concentration"
        }
        (GrayRhinoCandidateKind::DependencyConcentration, Language::EnUs) => {
            "Dependency Concentration"
        }
        (GrayRhinoCandidateKind::InstitutionalMaturityGap, Language::EnUs) => {
            "Institutional Maturity Gap"
        }
        (GrayRhinoCandidateKind::RedundancyGap, Language::EnUs) => "Redundancy Gap",
        (GrayRhinoCandidateKind::MarketConcentration, Language::EnUs) => "Market Concentration",
        (GrayRhinoCandidateKind::NarrativeCrowding, Language::EnUs) => "Narrative Crowding",
        (GrayRhinoCandidateKind::LiquidityFragility, Language::EnUs) => "Liquidity Fragility",
        (GrayRhinoCandidateKind::CapexPaybackFragility, Language::EnUs) => {
            "Capex Payback Fragility"
        }
        (GrayRhinoCandidateKind::GovernanceConcentration, Language::JaJp) => "ガバナンス集中",
        (GrayRhinoCandidateKind::DependencyConcentration, Language::JaJp) => "依存集中",
        (GrayRhinoCandidateKind::InstitutionalMaturityGap, Language::JaJp) => "制度成熟度ギャップ",
        (GrayRhinoCandidateKind::RedundancyGap, Language::JaJp) => "冗長性ギャップ",
        (GrayRhinoCandidateKind::MarketConcentration, Language::JaJp) => "市場集中",
        (GrayRhinoCandidateKind::NarrativeCrowding, Language::JaJp) => "ナラティブ過密",
        (GrayRhinoCandidateKind::LiquidityFragility, Language::JaJp) => "流動性脆弱性",
        (GrayRhinoCandidateKind::CapexPaybackFragility, Language::JaJp) => "設備投資回収脆弱性",
    }
}

fn candidate_state_label(state: GrayRhinoCandidateState, language: Language) -> &'static str {
    match (state, language) {
        (GrayRhinoCandidateState::Background, Language::ZhCn) => "背景观察",
        (GrayRhinoCandidateState::Visible, Language::ZhCn) => "可见",
        (GrayRhinoCandidateState::Expanding, Language::ZhCn) => "扩张",
        (GrayRhinoCandidateState::Critical, Language::ZhCn) => "临界",
        (GrayRhinoCandidateState::Cooling, Language::ZhCn) => "降温",
        (GrayRhinoCandidateState::Resolved, Language::ZhCn) => "解除",
        (GrayRhinoCandidateState::Background, Language::EnUs) => "Background",
        (GrayRhinoCandidateState::Visible, Language::EnUs) => "Visible",
        (GrayRhinoCandidateState::Expanding, Language::EnUs) => "Expanding",
        (GrayRhinoCandidateState::Critical, Language::EnUs) => "Critical",
        (GrayRhinoCandidateState::Cooling, Language::EnUs) => "Cooling",
        (GrayRhinoCandidateState::Resolved, Language::EnUs) => "Resolved",
        (GrayRhinoCandidateState::Background, Language::JaJp) => "背景観測",
        (GrayRhinoCandidateState::Visible, Language::JaJp) => "可視",
        (GrayRhinoCandidateState::Expanding, Language::JaJp) => "拡張",
        (GrayRhinoCandidateState::Critical, Language::JaJp) => "臨界",
        (GrayRhinoCandidateState::Cooling, Language::JaJp) => "低下",
        (GrayRhinoCandidateState::Resolved, Language::JaJp) => "解消",
    }
}

fn monitoring_direction_label(
    direction: GrayRhinoMonitoringDirection,
    language: Language,
) -> &'static str {
    match (direction, language) {
        (GrayRhinoMonitoringDirection::New, Language::ZhCn) => "新增",
        (GrayRhinoMonitoringDirection::Stable, Language::ZhCn) => "稳定",
        (GrayRhinoMonitoringDirection::Intensifying, Language::ZhCn) => "升温",
        (GrayRhinoMonitoringDirection::Cooling, Language::ZhCn) => "降温",
        (GrayRhinoMonitoringDirection::Resolved, Language::ZhCn) => "解除",
        (GrayRhinoMonitoringDirection::New, Language::EnUs) => "New",
        (GrayRhinoMonitoringDirection::Stable, Language::EnUs) => "Stable",
        (GrayRhinoMonitoringDirection::Intensifying, Language::EnUs) => "Intensifying",
        (GrayRhinoMonitoringDirection::Cooling, Language::EnUs) => "Cooling",
        (GrayRhinoMonitoringDirection::Resolved, Language::EnUs) => "Resolved",
        (GrayRhinoMonitoringDirection::New, Language::JaJp) => "新規",
        (GrayRhinoMonitoringDirection::Stable, Language::JaJp) => "安定",
        (GrayRhinoMonitoringDirection::Intensifying, Language::JaJp) => "強まり",
        (GrayRhinoMonitoringDirection::Cooling, Language::JaJp) => "低下",
        (GrayRhinoMonitoringDirection::Resolved, Language::JaJp) => "解消",
    }
}

fn localized_structural_text(value: &str, language: Language) -> String {
    if matches!(language, Language::EnUs) {
        return value.to_string();
    }
    let lower = value.to_lowercase();
    let translated = match language {
        Language::ZhCn => {
            if lower.contains("market-level structural concentration") {
                Some("检测到市场层面的结构集中。")
            } else if lower.contains("liquidity or rate-pressure fragility") {
                Some("检测到流动性或利率压力脆弱性。")
            } else if lower.contains("capex payback fragility") {
                Some("检测到资本开支回收脆弱性。")
            } else if lower.contains("narrative crowding") {
                Some("检测到叙事拥挤。")
            } else if lower.contains("single dependency") || lower.contains("missing fallback") {
                Some("检测到单一依赖或后备路径缺失。")
            } else if lower.contains("founder") && lower.contains("voting control") {
                Some("检测到创始人或单一主体投票控制。")
            } else if lower.contains("governance check-and-balance weakness") {
                Some("检测到治理制衡弱点。")
            } else if lower.contains("ipo voting terms") {
                Some("IPO 投票条款")
            } else if lower.contains("board composition changes") {
                Some("董事会构成变化")
            } else if lower.contains("related-party transactions") {
                Some("关联交易")
            } else if lower.contains("founder control changes") {
                Some("创始人控制权变化")
            } else if lower.contains("supplier disruption") {
                Some("供应商中断")
            } else if lower.contains("cloud outage") {
                Some("云服务中断")
            } else if lower.contains("fallback disclosure change") {
                Some("后备路径披露变化")
            } else if lower.contains("breadth deterioration") {
                Some("市场广度恶化")
            } else if lower.contains("liquidity tightening") {
                Some("流动性收紧")
            } else if lower.contains("capex payback disappointment") {
                Some("资本开支回收不及预期")
            } else if lower.contains("yield curve deterioration") {
                Some("收益率曲线恶化")
            } else if lower.contains("credit spread widening") {
                Some("信用利差扩大")
            } else if lower.contains("central-bank liquidity shift") {
                Some("央行流动性变化")
            } else if lower.contains("utilization gap") {
                Some("利用率缺口")
            } else if lower.contains("earnings disappointment") {
                Some("盈利不及预期")
            } else if lower.contains("capex guidance revision") {
                Some("资本开支指引修正")
            } else if lower.contains("headline concentration") {
                Some("新闻标题集中")
            } else if lower.contains("single-theme leadership") {
                Some("单一主题领涨")
            } else if lower.contains("positioning reversal") {
                Some("仓位反转")
            } else {
                None
            }
        }
        Language::JaJp => {
            if lower.contains("market-level structural concentration") {
                Some("市場レベルの構造集中を検出。")
            } else if lower.contains("liquidity or rate-pressure fragility") {
                Some("流動性または金利圧力の脆弱性を検出。")
            } else if lower.contains("capex payback fragility") {
                Some("設備投資回収の脆弱性を検出。")
            } else if lower.contains("narrative crowding") {
                Some("ナラティブ過密を検出。")
            } else if lower.contains("single dependency") || lower.contains("missing fallback") {
                Some("単一依存または代替経路の不足を検出。")
            } else if lower.contains("founder") && lower.contains("voting control") {
                Some("創業者または単一主体の議決権支配を検出。")
            } else if lower.contains("governance check-and-balance weakness") {
                Some("ガバナンスの牽制不足を検出。")
            } else if lower.contains("ipo voting terms") {
                Some("IPO 議決権条件")
            } else if lower.contains("board composition changes") {
                Some("取締役会構成の変化")
            } else if lower.contains("related-party transactions") {
                Some("関連当事者取引")
            } else if lower.contains("founder control changes") {
                Some("創業者支配の変化")
            } else if lower.contains("supplier disruption") {
                Some("supplier 障害")
            } else if lower.contains("cloud outage") {
                Some("cloud 障害")
            } else if lower.contains("fallback disclosure change") {
                Some("代替経路開示の変化")
            } else if lower.contains("breadth deterioration") {
                Some("市場 breadth 悪化")
            } else if lower.contains("liquidity tightening") {
                Some("流動性引き締まり")
            } else if lower.contains("capex payback disappointment") {
                Some("設備投資回収の未達")
            } else if lower.contains("yield curve deterioration") {
                Some("イールドカーブ悪化")
            } else if lower.contains("credit spread widening") {
                Some("信用スプレッド拡大")
            } else if lower.contains("central-bank liquidity shift") {
                Some("中央銀行流動性の変化")
            } else if lower.contains("utilization gap") {
                Some("稼働率ギャップ")
            } else if lower.contains("earnings disappointment") {
                Some("利益未達")
            } else if lower.contains("capex guidance revision") {
                Some("設備投資 guidance 修正")
            } else if lower.contains("headline concentration") {
                Some("headline 集中")
            } else if lower.contains("single-theme leadership") {
                Some("単一テーマ主導")
            } else if lower.contains("positioning reversal") {
                Some("positioning 反転")
            } else {
                None
            }
        }
        Language::EnUs => None,
    };
    translated.unwrap_or(value).to_string()
}
