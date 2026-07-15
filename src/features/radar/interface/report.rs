use crate::features::radar::domain::observation_timeline::ObservationTimeline;
use crate::features::radar::interface::presentation::PresentationPacket;
use crate::features::shared::interface::i18n::{get_dictionary, DisplayDictionary};
use std::collections::HashMap;

pub struct ReportResult {
    pub telegram_html_body: String,
    pub markdown_body: String,
    pub archival_markdown: String,
}

pub struct ReportRenderContext {
    pub compact_transition_in_no_trade: bool,
    pub compact_stability_threshold: String,
    pub compact_continuity_threshold: String,
    pub observation_timeline: Option<ObservationTimeline>,
}

#[derive(Clone, Copy)]
enum RenderMode {
    Markdown,
    Html,
}

pub fn generate_refined_report(
    context: &ReportRenderContext,
    pres: &PresentationPacket,
    _realized_pl: f64,
    _positions: &HashMap<String, (f64, f64)>,
    _prices: &HashMap<String, f64>,
) -> anyhow::Result<ReportResult> {
    let telegram_html = generate_telegram_html_report(
        pres,
        false,
        context.compact_transition_in_no_trade,
        &context.compact_stability_threshold,
        &context.compact_continuity_threshold,
        context.observation_timeline.as_ref(),
    );
    let markdown = generate_markdown_report(
        pres,
        false,
        context.compact_transition_in_no_trade,
        &context.compact_stability_threshold,
        &context.compact_continuity_threshold,
        context.observation_timeline.as_ref(),
    );
    let archival_markdown = generate_markdown_report(
        pres,
        true,
        context.compact_transition_in_no_trade,
        &context.compact_stability_threshold,
        &context.compact_continuity_threshold,
        context.observation_timeline.as_ref(),
    );

    Ok(ReportResult {
        telegram_html_body: telegram_html,
        markdown_body: markdown,
        archival_markdown,
    })
}

fn generate_markdown_report(
    pres: &PresentationPacket,
    detailed_transition: bool,
    compact_transition_in_no_trade: bool,
    compact_stability_threshold: &str,
    compact_continuity_threshold: &str,
    observation_timeline: Option<&ObservationTimeline>,
) -> String {
    let dict = get_dictionary(pres.language);
    let is_no_trade = final_execution_is_none(pres);
    let compact_no_trade_presentation =
        is_no_trade && compact_transition_in_no_trade && !detailed_transition;

    let mut card = String::new();
    card.push_str(&format!("## {}\n\n", dict.headers.market_summary));
    card.push_str(&format!(
        "**{}**: {} | **{}**: {}\n\n",
        dict.signals.regime_label,
        pres.macro_display.headline,
        dict.signals.bias,
        top_level_bias_label(pres)
    ));
    card.push_str(&format!("> {}\n\n", pres.macro_display.summary));

    let transition_block = render_transition_block(
        pres,
        &dict,
        detailed_transition,
        is_no_trade,
        compact_transition_in_no_trade,
        RenderMode::Markdown,
    );
    if detailed_transition {
        card.push_str(&transition_block);
    }

    let d = &pres.decision_summary;
    let final_action = final_execution_action_label(pres);
    let final_position_range = final_execution_position_range(pres, &d.entry_cap_value);
    card.push_str(&format!("**{}**\n\n", d.section_title));
    if !pres.final_execution_decision.reason.is_empty() {
        card.push_str(&format!("> {}\n\n", pres.final_execution_decision.reason));
    }
    if is_no_trade {
        card.push_str(&format!("### {}\n\n", d.action_status_value));
        card.push_str(&format!("> {}\n\n", d.hard_rule_note));
        card.push_str(&format!(
            "> {}：{}\n> {}：{}\n> {}：{}\n> {}：{}\n> {} · {}\n",
            d.trend_cohesion_label,
            d.trend_cohesion_value,
            d.trend_topology_label,
            d.trend_topology_value,
            d.state_tag_label,
            d.state_tag_value,
            d.action_tag_label,
            final_action,
            d.entry_cap_label,
            final_position_range
        ));
        if !d.gate_passed {
            if compact_no_trade_presentation {
                card.push_str(&render_compact_no_trade_reasons(
                    &d.readiness_reasons_label,
                    &d.trend_topology_value,
                    (
                        &pres.signal_summary.stability_label,
                        &d.compact_stability_value,
                        compact_stability_threshold,
                    ),
                    (
                        &pres.signal_summary.continuity_label,
                        &d.compact_continuity_value,
                        compact_continuity_threshold,
                    ),
                    RenderMode::Markdown,
                ));
            } else {
                card.push_str(&format!("> 🎯 **{}**:\n", d.formation_conditions_label));
                for fc in &d.formation_conditions {
                    card.push_str(&format!("> - {}\n", fc));
                }
                if !d.unmet_conditions.is_empty() {
                    card.push_str(&format!(">\n> ❌ **{}**:\n", d.unmet_conditions_label));
                    for uc in &d.unmet_conditions {
                        card.push_str(&format!("> - {}\n", uc));
                    }
                }
                card.push('\n');
            }
        }
        card.push_str(&format!(
            "\n> {} · {}\n> {} · {}\n> {} · {}\n",
            d.market_board_label,
            d.market_board_value,
            d.opportunity_snapshot_label,
            d.opportunity_snapshot_value,
            d.risk_snapshot_label,
            d.risk_snapshot_value
        ));
        if let Some(note) = &d.entry_cap_note {
            card.push_str(&format!("\n> {}\n", note));
        }
    } else {
        card.push_str(&format!(
            "- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n",
            d.trend_cohesion_label,
            d.trend_cohesion_value,
            d.trend_topology_label,
            d.trend_topology_value,
            d.action_status_label,
            final_action,
            d.state_tag_label,
            d.state_tag_value,
            d.action_tag_label,
            final_action,
            d.behavior_mode_label,
            final_action,
            d.exposure_label,
            final_position_range,
            d.market_board_label,
            d.market_board_value,
            d.opportunity_snapshot_label,
            d.opportunity_snapshot_value,
            d.risk_snapshot_label,
            d.risk_snapshot_value
        ));
        if !d.gate_passed {
            card.push_str(&format!("- 🎯 **{}**:\n", d.formation_conditions_label));
            for fc in &d.formation_conditions {
                card.push_str(&format!("  - {}\n", fc));
            }
            if !d.unmet_conditions.is_empty() {
                card.push_str(&format!("- ❌ **{}**:\n", d.unmet_conditions_label));
                for uc in &d.unmet_conditions {
                    card.push_str(&format!("  - {}\n", uc));
                }
            }
        }
        card.push_str(&format!("\n> {}\n", d.summary));
    }
    if !compact_no_trade_presentation && !d.readiness_reasons.is_empty() {
        card.push_str(&format!("\n- {}:\n", d.readiness_reasons_label));
        for reason in &d.readiness_reasons {
            card.push_str(&format!("  - {}\n", reason));
        }
    }
    if let Some(note) = &d.candidate_only_note {
        card.push_str(&format!("\n> {}\n", note));
    }
    card.push_str(&render_market_change_log_section(
        pres.market_change_log.as_ref(),
        RenderMode::Markdown,
    ));
    card.push_str(&render_observation_timeline_section(
        observation_timeline,
        pres.language,
        RenderMode::Markdown,
    ));
    if !compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }
    card.push_str("\n\n");

    let breakout_section = render_breakout_summary(pres, &dict, RenderMode::Markdown);

    if compact_no_trade_presentation {
        card.push_str(&breakout_section);
        card.push_str(&render_exit_summary(pres, RenderMode::Markdown));
    } else {
        card.push_str(&render_exit_summary(pres, RenderMode::Markdown));
        card.push_str(&breakout_section);
    }

    card.push_str(&render_top_actions_section(
        pres,
        &dict,
        is_no_trade,
        RenderMode::Markdown,
    ));
    card.push_str(&render_post_actions_sections(
        pres,
        &dict,
        is_no_trade,
        RenderMode::Markdown,
    ));
    card.push_str(&render_leadership_snapshot_section(
        pres.leadership_snapshot.as_ref(),
        RenderMode::Markdown,
    ));
    card.push_str(&render_leader_persistence_section(
        pres.leader_persistence.as_ref(),
        RenderMode::Markdown,
    ));
    if compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }
    card.push_str(&render_interpretation_section(
        pres.interpretation_layer.as_ref(),
        RenderMode::Markdown,
        detailed_transition,
    ));
    card.push_str(&render_market_interpretation_section(
        pres.market_interpretation.as_ref(),
        RenderMode::Markdown,
    ));
    card.push_str(&render_hypothesis_section(
        pres.hypothesis_layer.as_ref(),
        &dict,
        RenderMode::Markdown,
    ));

    card
}

fn generate_telegram_html_report(
    pres: &PresentationPacket,
    detailed_transition: bool,
    compact_transition_in_no_trade: bool,
    compact_stability_threshold: &str,
    compact_continuity_threshold: &str,
    observation_timeline: Option<&ObservationTimeline>,
) -> String {
    let dict = get_dictionary(pres.language);
    let is_no_trade = final_execution_is_none(pres);
    let compact_no_trade_presentation =
        is_no_trade && compact_transition_in_no_trade && !detailed_transition;

    let mut card = String::new();
    card.push_str(&format!("<b>{}</b>\n\n", dict.headers.market_summary));
    card.push_str(&format!(
        "<b>{}</b>: {} | <b>{}</b>: {}\n\n",
        dict.signals.regime_label,
        pres.macro_display.headline,
        dict.signals.bias,
        top_level_bias_label(pres)
    ));
    card.push_str(&format!("<i>{}</i>\n\n", pres.macro_display.summary));

    let transition_block = render_transition_block(
        pres,
        &dict,
        detailed_transition,
        is_no_trade,
        compact_transition_in_no_trade,
        RenderMode::Html,
    );
    if detailed_transition {
        card.push_str(&transition_block);
    }

    let d = &pres.decision_summary;
    let final_action = final_execution_action_label(pres);
    let final_position_range = final_execution_position_range(pres, &d.entry_cap_value);
    card.push_str(&format!("<b>{}</b>\n\n", d.section_title));
    if !pres.final_execution_decision.reason.is_empty() {
        card.push_str(&format!(
            "<i>{}</i>\n\n",
            pres.final_execution_decision.reason
        ));
    }
    if is_no_trade {
        card.push_str(&format!("<b>{}</b>\n\n", d.action_status_value));
        card.push_str(&format!("<i>{}</i>\n\n", d.hard_rule_note));
        card.push_str(&format!(
            "{}：{}\n{}：{}\n{}：{}\n{}：{}\n{} · {}\n\n",
            d.trend_cohesion_label,
            d.trend_cohesion_value,
            d.trend_topology_label,
            d.trend_topology_value,
            d.state_tag_label,
            d.state_tag_value,
            d.action_tag_label,
            final_action,
            d.entry_cap_label,
            final_position_range
        ));
        if !d.gate_passed {
            if compact_no_trade_presentation {
                card.push_str(&render_compact_no_trade_reasons(
                    &d.readiness_reasons_label,
                    &d.trend_topology_value,
                    (
                        &pres.signal_summary.stability_label,
                        &d.compact_stability_value,
                        compact_stability_threshold,
                    ),
                    (
                        &pres.signal_summary.continuity_label,
                        &d.compact_continuity_value,
                        compact_continuity_threshold,
                    ),
                    RenderMode::Html,
                ));
            } else {
                card.push_str(&format!("🎯 <b>{}</b>:\n", d.formation_conditions_label));
                for fc in &d.formation_conditions {
                    card.push_str(&format!("- <i>{}</i>\n", fc));
                }
                if !d.unmet_conditions.is_empty() {
                    card.push_str(&format!("\n❌ <b>{}</b>:\n", d.unmet_conditions_label));
                    for uc in &d.unmet_conditions {
                        card.push_str(&format!("- <i>{}</i>\n", uc));
                    }
                }
                card.push('\n');
            }
        }
        card.push_str(&format!(
            "{} · {}\n{} · {}\n{} · {}\n",
            d.market_board_label,
            d.market_board_value,
            d.opportunity_snapshot_label,
            d.opportunity_snapshot_value,
            d.risk_snapshot_label,
            d.risk_snapshot_value
        ));
        if let Some(note) = &d.entry_cap_note {
            card.push_str(&format!("\n<i>{}</i>\n", note));
        }
    } else {
        card.push_str(&format!(
            "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n",
            d.trend_cohesion_label,
            d.trend_cohesion_value,
            d.trend_topology_label,
            d.trend_topology_value,
            d.action_status_label,
            final_action,
            d.state_tag_label,
            d.state_tag_value,
            d.action_tag_label,
            final_action,
            d.behavior_mode_label,
            final_action,
            d.exposure_label,
            final_position_range,
            d.market_board_label,
            d.market_board_value,
            d.opportunity_snapshot_label,
            d.opportunity_snapshot_value,
            d.risk_snapshot_label,
            d.risk_snapshot_value
        ));
        if !d.gate_passed {
            card.push_str(&format!("• 🎯 <b>{}</b>:\n", d.formation_conditions_label));
            for fc in &d.formation_conditions {
                card.push_str(&format!("  - <i>{}</i>\n", fc));
            }
            if !d.unmet_conditions.is_empty() {
                card.push_str(&format!("• ❌ <b>{}</b>:\n", d.unmet_conditions_label));
                for uc in &d.unmet_conditions {
                    card.push_str(&format!("  - <i>{}</i>\n", uc));
                }
            }
        }
        card.push_str(&format!("\n<i>{}</i>\n", d.summary));
    }
    if !compact_no_trade_presentation && !d.readiness_reasons.is_empty() {
        card.push_str(&format!("\n• {}:\n", d.readiness_reasons_label));
        for reason in &d.readiness_reasons {
            card.push_str(&format!("  • {}\n", reason));
        }
    }
    if let Some(note) = &d.candidate_only_note {
        card.push_str(&format!("\n<i>{}</i>\n", note));
    }
    if !compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }
    card.push('\n');

    let breakout_section = render_breakout_summary(pres, &dict, RenderMode::Html);

    if compact_no_trade_presentation {
        card.push_str(&breakout_section);
        card.push_str(&render_exit_summary(pres, RenderMode::Html));
    } else {
        card.push_str(&render_exit_summary(pres, RenderMode::Html));
        card.push_str(&breakout_section);
    }

    card.push_str(&render_top_actions_section(
        pres,
        &dict,
        is_no_trade,
        RenderMode::Html,
    ));
    card.push_str(&render_post_actions_sections(
        pres,
        &dict,
        is_no_trade,
        RenderMode::Html,
    ));
    card.push_str(&render_market_change_log_section(
        pres.market_change_log.as_ref(),
        RenderMode::Html,
    ));
    card.push_str(&render_observation_timeline_section(
        observation_timeline,
        pres.language,
        RenderMode::Html,
    ));
    card.push_str(&render_leadership_snapshot_section(
        pres.leadership_snapshot.as_ref(),
        RenderMode::Html,
    ));
    card.push_str(&render_leader_persistence_section(
        pres.leader_persistence.as_ref(),
        RenderMode::Html,
    ));
    if compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }
    card.push_str(&render_interpretation_section(
        pres.interpretation_layer.as_ref(),
        RenderMode::Html,
        detailed_transition,
    ));
    card.push_str(&render_market_interpretation_section(
        pres.market_interpretation.as_ref(),
        RenderMode::Html,
    ));
    card.push_str(&render_hypothesis_section(
        pres.hypothesis_layer.as_ref(),
        &dict,
        RenderMode::Html,
    ));

    card
}

fn render_exit_summary(pres: &PresentationPacket, mode: RenderMode) -> String {
    let mut out = String::new();
    match mode {
        RenderMode::Markdown => out.push_str(&format!("### {}\n\n", pres.exit_summary.title)),
        RenderMode::Html => out.push_str(&format!("<b>{}</b>\n\n", pres.exit_summary.title)),
    }
    if pres.exit_summary.items.is_empty() {
        if let Some(note) = &pres.exit_summary.empty_note {
            for line in note.lines() {
                match mode {
                    RenderMode::Markdown => out.push_str(&format!("> {}\n", line)),
                    RenderMode::Html => out.push_str(&format!("{}\n", line)),
                }
            }
        }
    } else {
        for item in &pres.exit_summary.items {
            match mode {
                RenderMode::Markdown => {
                    out.push_str(&format!("- {} · {}\n", item.symbol, item.intent_label));
                    out.push_str(&format!("   {}\n", item.reason));
                }
                RenderMode::Html => {
                    out.push_str(&format!("• {} · {}\n", item.symbol, item.intent_label));
                    out.push_str(&format!("  {}\n", item.reason));
                }
            }
        }
    }
    out.push('\n');
    out
}

fn render_breakout_summary(
    pres: &PresentationPacket,
    dict: &DisplayDictionary,
    mode: RenderMode,
) -> String {
    let mut out = String::new();
    match mode {
        RenderMode::Markdown => out.push_str(&format!("### {}\n\n", pres.breakout_summary.title)),
        RenderMode::Html => out.push_str(&format!("<b>{}</b>\n\n", pres.breakout_summary.title)),
    }
    if pres.breakout_summary.items.is_empty() {
        if let Some(note) = &pres.breakout_summary.empty_note {
            match mode {
                RenderMode::Markdown => out.push_str(&format!("> {}\n", note)),
                RenderMode::Html => out.push_str(&format!("<i>{}</i>\n", note)),
            }
        }
    } else {
        for item in &pres.breakout_summary.items {
            match mode {
                RenderMode::Markdown => {
                    out.push_str(&format!("- {} · {}\n", item.symbol, item.status_label));
                    out.push_str(&format!(
                        "   {} · {} {} · {} {}\n",
                        item.reason,
                        dict.breakout.strength_label,
                        item.strength_value,
                        dict.breakout.quality_label,
                        item.quality_value
                    ));
                    if let Some(risk) = &item.failed_risk_value {
                        out.push_str(&format!(
                            "   {} {}\n",
                            dict.breakout.failed_risk_label, risk
                        ));
                    }
                }
                RenderMode::Html => {
                    out.push_str(&format!("• {} · {}\n", item.symbol, item.status_label));
                    out.push_str(&format!(
                        "  <i>{} · {} {} · {} {}</i>\n",
                        item.reason,
                        dict.breakout.strength_label,
                        item.strength_value,
                        dict.breakout.quality_label,
                        item.quality_value
                    ));
                    if let Some(risk) = &item.failed_risk_value {
                        out.push_str(&format!(
                            "  <i>{} {}</i>\n",
                            dict.breakout.failed_risk_label, risk
                        ));
                    }
                }
            }
        }
    }
    out.push('\n');
    out
}

fn render_top_actions_section(
    pres: &PresentationPacket,
    dict: &DisplayDictionary,
    is_no_trade: bool,
    mode: RenderMode,
) -> String {
    let candidate_only = is_no_trade
        || matches!(
            pres.final_execution_decision.participation_mode,
            crate::features::radar::interface::presentation::ParticipationMode::Probe
        );
    let top_actions_title = if candidate_only {
        dict.decision.candidate_watchlist.clone()
    } else {
        dict.headers.top_actions.clone()
    };

    let mut out = String::new();
    match mode {
        RenderMode::Markdown => out.push_str(&format!("### {}\n\n", top_actions_title)),
        RenderMode::Html => out.push_str(&format!("<b>{}</b>\n\n", top_actions_title)),
    }

    for (i, vm) in pres.top_actions.iter().enumerate() {
        let no_trade_secondary_desc =
            if candidate_only && vm.secondary_desc == dict.asset_states.optimal {
                format!("{} ({})", vm.secondary_desc, dict.asset_tags.candidate)
            } else {
                vm.secondary_desc.clone()
            };
        match mode {
            RenderMode::Markdown => {
                if candidate_only {
                    out.push_str(&format!("- {} · {}\n", vm.symbol, no_trade_secondary_desc));
                } else {
                    out.push_str(&format!(
                        "{}. {} **{}** - {}\n",
                        i + 1,
                        vm.indicator,
                        vm.symbol,
                        vm.primary_label
                    ));
                }
            }
            RenderMode::Html => {
                if candidate_only {
                    out.push_str(&format!("• {} · {}\n", vm.symbol, no_trade_secondary_desc));
                } else {
                    out.push_str(&format!(
                        "{}. {} <b>{}</b> - {}\n",
                        i + 1,
                        vm.indicator,
                        vm.symbol,
                        vm.primary_label
                    ));
                }
            }
        }

        let mut row2_parts = Vec::new();
        if !candidate_only {
            row2_parts.push(vm.secondary_desc.clone());
            for tag in &vm.tags {
                row2_parts.push(tag.clone());
            }
        } else if !vm.tags.is_empty() {
            row2_parts.extend(vm.tags.clone());
        }
        if let Some(ref diag) = vm.diagnostic {
            row2_parts.push(diag.clone());
        }
        if !row2_parts.is_empty() {
            match mode {
                RenderMode::Markdown => out.push_str(&format!("   *{}*\n", row2_parts.join(" · "))),
                RenderMode::Html => out.push_str(&format!("  <i>{}</i>\n", row2_parts.join(" · "))),
            }
        }
    }

    out.push('\n');
    out
}

fn final_execution_is_none(pres: &PresentationPacket) -> bool {
    if pres.final_execution_decision.reason.is_empty() {
        return pres.decision_summary.is_no_trade;
    }
    matches!(
        pres.final_execution_decision.execution_window,
        crate::features::radar::interface::presentation::ExecutionWindow::None
    )
}

fn top_level_bias_label(pres: &PresentationPacket) -> String {
    use crate::features::radar::interface::presentation::ExecutionWindow;
    use crate::features::shared::interface::i18n::Language;
    if pres.final_execution_decision.execution_window != ExecutionWindow::Limited {
        return pres.macro_display.bias_label.clone();
    }
    match pres.language {
        Language::ZhCn => "有限参与窗口 / 仅 Probe".to_string(),
        Language::EnUs => "Limited Participation Window / Probe Only".to_string(),
        Language::JaJp => "限定参加ウィンドウ / Probe のみ".to_string(),
    }
}

fn final_execution_action_label(pres: &PresentationPacket) -> String {
    use crate::features::radar::interface::presentation::{ExecutionWindow, ParticipationMode};
    use crate::features::shared::interface::i18n::Language;
    match (
        pres.final_execution_decision.execution_window,
        pres.final_execution_decision.participation_mode,
    ) {
        (ExecutionWindow::None, _) => match pres.language {
            Language::ZhCn => "仅候选 / 无执行窗口",
            Language::EnUs => "Candidate Only / No Execution Window",
            Language::JaJp => "候補のみ / 実行ウィンドウなし",
        }
        .to_string(),
        (ExecutionWindow::Limited, ParticipationMode::Probe) => match pres.language {
            Language::ZhCn => "有限参与窗口 / 仅 Probe",
            Language::EnUs => "Limited Participation Window / Probe Only",
            Language::JaJp => "限定参加ウィンドウ / Probe のみ",
        }
        .to_string(),
        (ExecutionWindow::Open, ParticipationMode::Add) => match pres.language {
            Language::ZhCn => "开放参与窗口 / 可加仓",
            Language::EnUs => "Open Participation Window / Add",
            Language::JaJp => "参加ウィンドウ開始 / 追加可",
        }
        .to_string(),
        _ => pres.final_execution_decision.reason.clone(),
    }
}

fn final_execution_position_range(pres: &PresentationPacket, fallback: &str) -> String {
    if pres.final_execution_decision.reason.is_empty() {
        fallback.to_string()
    } else {
        pres.final_execution_decision.position_range.clone()
    }
}

fn render_post_actions_sections(
    pres: &PresentationPacket,
    dict: &DisplayDictionary,
    is_no_trade: bool,
    mode: RenderMode,
) -> String {
    let mut out = String::new();
    let s = &pres.signal_summary;

    match mode {
        RenderMode::Markdown => {
            out.push_str(&format!("**{}**\n\n", dict.headers.monitoring_signals));
            out.push_str(&format!(
                "- **{}**: {}\n- **{}**: {}\n",
                s.cohesion_label, s.cohesion_value, s.stability_label, s.stability_value,
            ));
            out.push_str(&format!(
                "> {} {} · {} {} · {} {} · {} {}\n",
                s.confidence_label,
                s.confidence_value,
                s.continuity_label,
                s.continuity_value,
                s.regime_age_label,
                s.regime_age_value,
                s.flow_label,
                s.flow_value
            ));
        }
        RenderMode::Html => {
            out.push_str(&format!("<b>{}</b>\n\n", dict.headers.monitoring_signals));
            out.push_str(&format!(
                "• <b>{}</b>: {}\n• <b>{}</b>: {}\n",
                s.cohesion_label, s.cohesion_value, s.stability_label, s.stability_value,
            ));
            out.push_str(&format!(
                "<i>{} {} · {} {} · {} {} · {} {}</i>\n",
                s.confidence_label,
                s.confidence_value,
                s.continuity_label,
                s.continuity_value,
                s.regime_age_label,
                s.regime_age_value,
                s.flow_label,
                s.flow_value
            ));
        }
    }

    if !pres.tactical_buckets.is_empty() {
        match mode {
            RenderMode::Markdown => {
                out.push_str(&format!("\n**{}**\n\n", dict.headers.tactical_buckets));
                for bucket in &pres.tactical_buckets {
                    out.push_str(&format!(
                        "- **{} ({})**: {}\n",
                        bucket.display_name,
                        bucket.count,
                        bucket.items.join(" / ")
                    ));
                }
            }
            RenderMode::Html => {
                out.push_str(&format!("\n<b>{}</b>\n\n", dict.headers.tactical_buckets));
                for bucket in &pres.tactical_buckets {
                    out.push_str(&format!(
                        "• <b>{} ({})</b>: {}\n",
                        bucket.display_name,
                        bucket.count,
                        bucket.items.join(" / ")
                    ));
                }
            }
        }
    }

    match mode {
        RenderMode::Markdown => {
            out.push_str(&format!("\n**{}**\n\n", dict.headers.risks_opportunities));
            if is_no_trade {
                out.push_str(&format!(
                    "> **{}**: {}\n> **{}**: {}\n",
                    pres.risk_opportunity_summary.opportunity_label,
                    pres.risk_opportunity_summary.opportunity_value,
                    pres.risk_opportunity_summary.risk_label,
                    pres.risk_opportunity_summary.risk_value
                ));
            } else {
                out.push_str(&format!(
                    "- **{}**: {}\n- **{}**: {}\n",
                    pres.risk_opportunity_summary.opportunity_label,
                    pres.risk_opportunity_summary.opportunity_value,
                    pres.risk_opportunity_summary.risk_label,
                    pres.risk_opportunity_summary.risk_value
                ));
            }
            if !pres.risk_opportunities.is_empty() && !is_no_trade {
                for item in &pres.risk_opportunities {
                    out.push_str(&format!(
                        "- **{}**: {} · {}\n",
                        item.kind, item.symbol, item.reason
                    ));
                }
            }
        }
        RenderMode::Html => {
            out.push_str(&format!(
                "\n<b>{}</b>\n\n",
                dict.headers.risks_opportunities
            ));
            if is_no_trade {
                out.push_str(&format!(
                    "<i><b>{}</b>: {}\n<b>{}</b>: {}</i>\n",
                    pres.risk_opportunity_summary.opportunity_label,
                    pres.risk_opportunity_summary.opportunity_value,
                    pres.risk_opportunity_summary.risk_label,
                    pres.risk_opportunity_summary.risk_value
                ));
            } else {
                out.push_str(&format!(
                    "• <b>{}</b>: {}\n• <b>{}</b>: {}\n",
                    pres.risk_opportunity_summary.opportunity_label,
                    pres.risk_opportunity_summary.opportunity_value,
                    pres.risk_opportunity_summary.risk_label,
                    pres.risk_opportunity_summary.risk_value
                ));
            }
            if !pres.risk_opportunities.is_empty() && !is_no_trade {
                for item in &pres.risk_opportunities {
                    out.push_str(&format!(
                        "• <b>{}</b>: {} · {}\n",
                        item.kind, item.symbol, item.reason
                    ));
                }
            }
        }
    }

    if !pres.notices.is_empty() {
        out.push('\n');
        for notice in &pres.notices {
            out.push_str(&format!("{}\n", notice));
        }
    }

    if let Some(alert) = &pres.data_alert {
        out.push_str(&format!(
            "\n{} {}: {} ({})\n",
            alert.prefix,
            alert.label,
            alert.message,
            alert.symbols.join(", ")
        ));
    }

    out
}

fn render_transition_block(
    pres: &PresentationPacket,
    dict: &DisplayDictionary,
    detailed_transition: bool,
    is_no_trade: bool,
    compact_transition_in_no_trade: bool,
    mode: RenderMode,
) -> String {
    let mut block = String::new();
    let Some(evidence) = &pres.transition_evidence else {
        return block;
    };
    let has_scout_status = evidence.scout_continuity.is_some()
        || evidence.scout_expansion.is_some()
        || evidence.scout_reset.is_some();
    if !(evidence.has_significant_change
        || evidence.no_trade_persists
        || has_scout_status
        || evidence.trend_recognition_state.is_some()
        || !evidence.strategic_context.is_empty())
    {
        return block;
    }

    let te_dict = &dict.transition_evidence;
    let compact_transition = !detailed_transition && is_no_trade && compact_transition_in_no_trade;
    match mode {
        RenderMode::Markdown => block.push_str(&format!("### {}\n\n", te_dict.title)),
        RenderMode::Html => block.push_str(&format!("<b>{}</b>\n", te_dict.title)),
    }
    if evidence.no_trade_persists {
        match mode {
            RenderMode::Markdown => block.push_str(&format!("> {}\n", te_dict.no_trade_persists)),
            RenderMode::Html => block.push_str(&format!("<i>{}</i>\n", te_dict.no_trade_persists)),
        }
    }
    if let Some(m) = &evidence.market_state_change {
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**: {}\n", te_dict.market_state_change, m))
            }
            RenderMode::Html => {
                block.push_str(&format!("• {}: {}\n", te_dict.market_state_change, m))
            }
        }
    }
    if let Some(r) = &evidence.risk_overlay_change {
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**: {}\n", te_dict.risk_overlay_change, r))
            }
            RenderMode::Html => {
                block.push_str(&format!("• {}: {}\n", te_dict.risk_overlay_change, r))
            }
        }
    }

    if evidence.trend_cohesion_gate_change.is_some() || evidence.trend_unmet_diff.is_some() {
        let status_text = evidence
            .trend_cohesion_gate_change
            .as_ref()
            .cloned()
            .unwrap_or_else(|| {
                if evidence.trend_cohesion_gate_passed {
                    te_dict.gate_pass.clone()
                } else {
                    te_dict.gate_fail.clone()
                }
            });
        match mode {
            RenderMode::Markdown => block.push_str(&format!(
                "- **{}**: {}\n",
                te_dict.trend_cohesion_gate_change, status_text
            )),
            RenderMode::Html => block.push_str(&format!(
                "• {}: {}\n",
                te_dict.trend_cohesion_gate_change, status_text
            )),
        }
        if !compact_transition {
            if let Some(diff) = &evidence.trend_unmet_diff {
                render_transition_diff(
                    &mut block,
                    te_dict.gate_unmet_added.as_str(),
                    &diff.added,
                    mode,
                );
                render_transition_diff(
                    &mut block,
                    te_dict.gate_unmet_removed.as_str(),
                    &diff.removed,
                    mode,
                );
                render_transition_diff(
                    &mut block,
                    te_dict.gate_unmet_persisting.as_str(),
                    &diff.persisting,
                    mode,
                );
            }
        }
    }

    if let Some(s) = &evidence.trend_cohesion_status_change {
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**: {}\n", te_dict.trend_status_change, s))
            }
            RenderMode::Html => {
                block.push_str(&format!("• {}: {}\n", te_dict.trend_status_change, s))
            }
        }
    }
    if let Some(tp) = &evidence.trend_cohesion_topology_change {
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**: {}\n", te_dict.topology_change, tp))
            }
            RenderMode::Html => block.push_str(&format!("• {}: {}\n", te_dict.topology_change, tp)),
        }
    }
    if !evidence.breakout_changes.is_empty() {
        if compact_transition {
            match mode {
                RenderMode::Markdown => block.push_str(&format!(
                    "- **{}**: {}\n",
                    te_dict.breakout_changes,
                    evidence.breakout_changes.join(", ")
                )),
                RenderMode::Html => block.push_str(&format!(
                    "• {}: {}\n",
                    te_dict.breakout_changes,
                    evidence.breakout_changes.join(", ")
                )),
            }
        } else {
            match mode {
                RenderMode::Markdown => {
                    block.push_str(&format!("- **{}**:\n", te_dict.breakout_changes));
                    for b in &evidence.breakout_changes {
                        block.push_str(&format!("  - {}\n", b));
                    }
                }
                RenderMode::Html => {
                    block.push_str(&format!("• {}:\n", te_dict.breakout_changes));
                    for b in &evidence.breakout_changes {
                        block.push_str(&format!("  - <i>{}</i>\n", b));
                    }
                }
            }
        }
    }
    if !evidence.risk_taxonomy.is_empty() {
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**:\n", te_dict.risk_taxonomy));
                for item in &evidence.risk_taxonomy {
                    block.push_str(&format!("  - {}\n", item));
                }
            }
            RenderMode::Html => {
                block.push_str(&format!("• {}:\n", te_dict.risk_taxonomy));
                for item in &evidence.risk_taxonomy {
                    block.push_str(&format!("  - <i>{}</i>\n", item));
                }
            }
        }
    }
    if has_scout_status {
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**:\n", te_dict.scout_status));
                if let Some(value) = &evidence.scout_continuity {
                    block.push_str(&format!("  - {}: {}\n", te_dict.scout_continuity, value));
                }
                if let Some(value) = &evidence.scout_expansion {
                    block.push_str(&format!("  - {}: {}\n", te_dict.scout_expansion, value));
                }
                if let Some(value) = &evidence.scout_reset {
                    block.push_str(&format!("  - {}: {}\n", te_dict.scout_reset, value));
                }
            }
            RenderMode::Html => {
                block.push_str(&format!("• {}:\n", te_dict.scout_status));
                if let Some(value) = &evidence.scout_continuity {
                    block.push_str(&format!(
                        "  - <i>{}: {}</i>\n",
                        te_dict.scout_continuity, value
                    ));
                }
                if let Some(value) = &evidence.scout_expansion {
                    block.push_str(&format!(
                        "  - <i>{}: {}</i>\n",
                        te_dict.scout_expansion, value
                    ));
                }
                if let Some(value) = &evidence.scout_reset {
                    block.push_str(&format!("  - <i>{}: {}</i>\n", te_dict.scout_reset, value));
                }
            }
        }
    }

    if !evidence.strategic_context.is_empty() {
        let tr_dict = &dict.trend_recognition;
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**:\n", tr_dict.strategic_context_title));
                for line in &evidence.strategic_context {
                    block.push_str(&format!("  - {}\n", line));
                }
            }
            RenderMode::Html => {
                block.push_str(&format!("• {}:\n", tr_dict.strategic_context_title));
                for line in &evidence.strategic_context {
                    block.push_str(&format!("  - <i>{}</i>\n", line));
                }
            }
        }
    }

    if evidence.trend_recognition_state.is_some() {
        let tr_dict = &dict.trend_recognition;
        match mode {
            RenderMode::Markdown => {
                block.push_str(&format!("- **{}**:\n", tr_dict.title));
                if let Some(value) = &evidence.trend_recognition_state {
                    block.push_str(&format!("  - {}: {}\n", tr_dict.continuation_state, value));
                }
                if let Some(value) = &evidence.trend_recognition_diffusion_score {
                    block.push_str(&format!("  - {}: {:.2}\n", tr_dict.diffusion_score, value));
                }
                if let Some(value) = &evidence.trend_recognition_conviction_score {
                    block.push_str(&format!("  - {}: {:.2}\n", tr_dict.conviction_score, value));
                }
                if let Some(value) = &evidence.structural_strength {
                    block.push_str(&format!("  - {}: {}\n", tr_dict.structural_strength, value));
                }
                if let Some(value) = &evidence.evidence_quality_summary {
                    block.push_str(&format!("  - {}: {}\n", tr_dict.evidence_quality, value));
                }
                if !evidence.substantive_signals.is_empty() {
                    block.push_str(&format!(
                        "  - {}: {}\n",
                        tr_dict.substantive_evidence,
                        evidence.substantive_signals.join(", ")
                    ));
                }
                if !evidence.substantive_details.is_empty() {
                    for detail in &evidence.substantive_details {
                        block.push_str(&format!("    - {}\n", detail));
                    }
                }
                if let Some(value) = &evidence.trend_recognition_lag_state {
                    block.push_str(&format!("  - {}: {}\n", tr_dict.lag_state, value));
                }
                if let Some(value) = &evidence.trend_recognition_single_asset_decay {
                    block.push_str(&format!("  - {}: {}\n", tr_dict.single_asset_decay, value));
                }
            }
            RenderMode::Html => {
                block.push_str(&format!("• {}:\n", tr_dict.title));
                if let Some(value) = &evidence.trend_recognition_state {
                    block.push_str(&format!(
                        "  - <i>{}: {}</i>\n",
                        tr_dict.continuation_state, value
                    ));
                }
                if let Some(value) = &evidence.trend_recognition_diffusion_score {
                    block.push_str(&format!(
                        "  - <i>{}: {:.2}</i>\n",
                        tr_dict.diffusion_score, value
                    ));
                }
                if let Some(value) = &evidence.trend_recognition_conviction_score {
                    block.push_str(&format!(
                        "  - <i>{}: {:.2}</i>\n",
                        tr_dict.conviction_score, value
                    ));
                }
                if let Some(value) = &evidence.structural_strength {
                    block.push_str(&format!(
                        "  - <i>{}: {}</i>\n",
                        tr_dict.structural_strength, value
                    ));
                }
                if let Some(value) = &evidence.evidence_quality_summary {
                    block.push_str(&format!(
                        "  - <i>{}: {}</i>\n",
                        tr_dict.evidence_quality, value
                    ));
                }
                if !evidence.substantive_signals.is_empty() {
                    block.push_str(&format!(
                        "  - <i>{}: {}</i>\n",
                        tr_dict.substantive_evidence,
                        evidence.substantive_signals.join(", ")
                    ));
                }
                if let Some(value) = &evidence.trend_recognition_lag_state {
                    block.push_str(&format!("  - <i>{}: {}</i>\n", tr_dict.lag_state, value));
                }
                if let Some(value) = &evidence.trend_recognition_single_asset_decay {
                    block.push_str(&format!(
                        "  - <i>{}: {}</i>\n",
                        tr_dict.single_asset_decay, value
                    ));
                }
            }
        }
    }

    block.push('\n');
    block
}

fn render_hypothesis_section(
    layer: Option<&crate::features::radar::interface::presentation::HypothesisLayerViewModel>,
    dict: &DisplayDictionary,
    mode: RenderMode,
) -> String {
    let mut block = String::new();
    let Some(layer) = layer else {
        return block;
    };
    let candidates: Vec<_> = layer
        .candidates
        .iter()
        .filter(|candidate| !candidate.failure_risks.is_empty())
        .collect();
    if candidates.is_empty() {
        return block;
    }

    let h = &dict.hypothesis;
    match mode {
        RenderMode::Markdown => {
            block.push_str(&format!("### {}\n\n", layer.title));
            block.push_str(&format!("  - {}\n", layer.notice));
            for candidate in candidates {
                block.push_str(&format!(
                    "  - {}: {}\n",
                    h.hypothesis_label, candidate.title
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.summary_label, candidate.summary
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.status_label, candidate.confidence_label
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.consensus_label, candidate.consensus_state
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.pricing_label, candidate.pricing_state
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.horizon_label, candidate.time_horizon
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.materialization_window_label, candidate.materialization_window
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.tactical_isolation_label, candidate.tactical_isolation_notice
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.narrative_saturation_label, candidate.narrative_saturation
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.reality_override_label, candidate.reality_override_notice
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.reality_override_priority_label, candidate.reality_override_priority
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.confidence_decay_label, candidate.confidence_decay_notice
                ));
                block.push_str(&format!("    - {}: {}\n", h.age_label, candidate.age_label));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.validation_label, candidate.validation_summary
                ));
                for check in &candidate.validation_checks {
                    let mark = if check.passed { "✓" } else { "✗" };
                    block.push_str(&format!("      - {mark} {}\n", check.label));
                }
                block.push_str(&format!("    - {}:\n", h.evidence_chain_label));
                for evidence in &candidate.evidence_chain {
                    block.push_str(&format!(
                        "      - {} · {} · {} · {}\n",
                        evidence.label,
                        evidence.evidence_type,
                        evidence.strength,
                        evidence.source_layer
                    ));
                }
                block.push_str(&format!("    - {}:\n", h.beneficiaries_label));
                for beneficiary in &candidate.candidate_beneficiaries {
                    block.push_str(&format!(
                        "      - {}: {} / {}\n",
                        beneficiary.symbol, beneficiary.role, beneficiary.rationale
                    ));
                }
                block.push_str(&format!("    - {}:\n", h.failure_risks_label));
                for risk in &candidate.failure_risks {
                    block.push_str(&format!(
                        "      - {} [{}]: {}\n",
                        risk.label, risk.severity, risk.description
                    ));
                }
                block.push_str(&format!(
                    "    - {}: {}\n",
                    h.responsibility_label, candidate.responsibility_notice
                ));
            }
        }
        RenderMode::Html => {
            block.push_str(&format!("\n<b>{}</b>\n", layer.title));
            block.push_str(&format!("  - <i>{}</i>\n", layer.notice));
            for candidate in candidates {
                block.push_str(&format!(
                    "  - <i>{}: {}</i>\n",
                    h.hypothesis_label, candidate.title
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.summary_label, candidate.summary
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.status_label, candidate.confidence_label
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.consensus_label, candidate.consensus_state
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.pricing_label, candidate.pricing_state
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.horizon_label, candidate.time_horizon
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.materialization_window_label, candidate.materialization_window
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.tactical_isolation_label, candidate.tactical_isolation_notice
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.narrative_saturation_label, candidate.narrative_saturation
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.reality_override_label, candidate.reality_override_notice
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.reality_override_priority_label, candidate.reality_override_priority
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.confidence_decay_label, candidate.confidence_decay_notice
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.age_label, candidate.age_label
                ));
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.validation_label, candidate.validation_summary
                ));
                for check in &candidate.validation_checks {
                    let mark = if check.passed { "✓" } else { "✗" };
                    block.push_str(&format!("      - <i>{mark} {}</i>\n", check.label));
                }
                block.push_str(&format!("    - <i>{}:</i>\n", h.evidence_chain_label));
                for evidence in &candidate.evidence_chain {
                    block.push_str(&format!(
                        "      - <i>{} · {} · {} · {}</i>\n",
                        evidence.label,
                        evidence.evidence_type,
                        evidence.strength,
                        evidence.source_layer
                    ));
                }
                block.push_str(&format!("    - <i>{}:</i>\n", h.beneficiaries_label));
                for beneficiary in &candidate.candidate_beneficiaries {
                    block.push_str(&format!(
                        "      - <i>{}: {} / {}</i>\n",
                        beneficiary.symbol, beneficiary.role, beneficiary.rationale
                    ));
                }
                block.push_str(&format!("    - <i>{}:</i>\n", h.failure_risks_label));
                for risk in &candidate.failure_risks {
                    block.push_str(&format!(
                        "      - <i>{} [{}]: {}</i>\n",
                        risk.label, risk.severity, risk.description
                    ));
                }
                block.push_str(&format!(
                    "    - <i>{}: {}</i>\n",
                    h.responsibility_label, candidate.responsibility_notice
                ));
            }
        }
    }
    block.push('\n');
    block
}

fn render_market_change_log_section(
    change_log: Option<&crate::features::radar::interface::presentation::MarketChangeLogViewModel>,
    mode: RenderMode,
) -> String {
    let mut block = String::new();
    let Some(change_log) = change_log else {
        return block;
    };

    match mode {
        RenderMode::Markdown => {
            block.push_str(&format!("### {}\n\n", change_log.title));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.leader_label, change_log.leader_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.breadth_label, change_log.breadth_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.risk_label, change_log.risk_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.supply_phase_label, change_log.supply_phase_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.confidence_label, change_log.confidence_value
            ));
            block.push_str(&format!(
                "  - Change Level: {}\n  - Change Drivers: {}\n  - Unchanged Dimensions: {}\n",
                change_log.change_level,
                change_log.change_drivers.join(", "),
                change_log.unchanged_dimensions.join(", ")
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.interpretation_label, change_log.interpretation_value
            ));
            block.push_str(&format!("  - {}:\n", change_log.summary_label));
            for line in &change_log.summary_values {
                block.push_str(&format!("    - {}\n", line));
            }
            block.push_str(&format!("  - {}\n", change_log.boundary));
        }
        RenderMode::Html => {
            block.push_str(&format!("\n<b>{}</b>\n\n", change_log.title));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.leader_label, change_log.leader_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.breadth_label, change_log.breadth_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.risk_label, change_log.risk_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.supply_phase_label, change_log.supply_phase_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.confidence_label, change_log.confidence_value
            ));
            block.push_str(&format!(
                "  - Change Level: {}\n  - Change Drivers: {}\n  - Unchanged Dimensions: {}\n",
                change_log.change_level,
                change_log.change_drivers.join(", "),
                change_log.unchanged_dimensions.join(", ")
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                change_log.interpretation_label, change_log.interpretation_value
            ));
            block.push_str(&format!("  - {}:\n", change_log.summary_label));
            for line in &change_log.summary_values {
                block.push_str(&format!("    - {}\n", line));
            }
            block.push_str(&format!("  - {}\n", change_log.boundary));
        }
    }
    block.push('\n');
    block
}

fn render_observation_timeline_section(
    timeline: Option<&ObservationTimeline>,
    language: crate::features::shared::interface::i18n::Language,
    mode: RenderMode,
) -> String {
    let Some(timeline) = timeline else {
        return String::new();
    };
    let coverage = match timeline.history_coverage {
        crate::features::radar::domain::observation_timeline::HistoryCoverage::Complete => {
            "COMPLETE"
        }
        crate::features::radar::domain::observation_timeline::HistoryCoverage::Partial => "PARTIAL",
        crate::features::radar::domain::observation_timeline::HistoryCoverage::Unavailable => {
            "UNAVAILABLE"
        }
    };
    let (
        title,
        coverage_label,
        summary_label,
        leader_label,
        breadth_label,
        confidence_label,
        supply_label,
        change_summary,
        points_label,
    ) = match language {
        crate::features::shared::interface::i18n::Language::ZhCn => (
            "市场演化观察",
            "历史覆盖",
            "7日摘要",
            "主导者序列",
            "市场广度序列",
            "置信度序列",
            "供给阶段序列",
            if timeline.has_structural_change() {
                "过去 7 个交易日出现结构性变化。"
            } else {
                "过去 7 个交易日未出现结构性变化。"
            },
            "观测点数",
        ),
        crate::features::shared::interface::i18n::Language::EnUs => (
            "Observation Timeline",
            "History Coverage",
            "7-Day Summary",
            "Leader sequence",
            "Breadth sequence",
            "Confidence sequence",
            "Supply sequence",
            if timeline.has_structural_change() {
                "Structural change observed across the last 7 trading days."
            } else {
                "No structural change observed across the last 7 trading days."
            },
            "Observed points",
        ),
        crate::features::shared::interface::i18n::Language::JaJp => (
            "市場進化観測",
            "履歴カバレッジ",
            "7日間サマリー",
            "主導銘柄の推移",
            "市場の広がりの推移",
            "確信度の推移",
            "供給局面の推移",
            if timeline.has_structural_change() {
                "直近 7 取引日に構造変化が観測されました。"
            } else {
                "直近 7 取引日に構造変化は観測されませんでした。"
            },
            "観測点数",
        ),
    };
    let change_summary = match timeline.history_coverage {
        crate::features::radar::domain::observation_timeline::HistoryCoverage::Unavailable => {
            match language {
                crate::features::shared::interface::i18n::Language::ZhCn => format!(
                    "7日时间轴尚未形成。\n当前仅有 {}/7 个有效交易日观测点。",
                    timeline.entries.len()
                ),
                crate::features::shared::interface::i18n::Language::EnUs => format!(
                    "The 7-day timeline has not formed.\nOnly {}/7 valid trading-day observations are available.",
                    timeline.entries.len()
                ),
                crate::features::shared::interface::i18n::Language::JaJp => format!(
                    "7日間のタイムラインは未形成です。\n有効な取引日観測は {}/7 件のみです。",
                    timeline.entries.len()
                ),
            }
        }
        crate::features::radar::domain::observation_timeline::HistoryCoverage::Partial
            if timeline.entries.len() < 5 => match language {
            crate::features::shared::interface::i18n::Language::ZhCn => format!(
                "7日趋势结论暂不生成。当前仅有 {}/7 个有效交易日观测点，覆盖不足。",
                timeline.entries.len()
            ),
            crate::features::shared::interface::i18n::Language::EnUs => format!(
                "The 7-day trend conclusion is not generated. Only {}/7 valid trading-day observations are available; coverage is insufficient.",
                timeline.entries.len()
            ),
            crate::features::shared::interface::i18n::Language::JaJp => format!(
                "7日間のトレンド結論は生成しません。有効な取引日観測は {}/7 件のみで、カバレッジが不足しています。",
                timeline.entries.len()
            ),
        },
        crate::features::radar::domain::observation_timeline::HistoryCoverage::Partial => {
            let limited_change = timeline.summary
                == crate::features::radar::domain::observation_timeline::SUMMARY_LIMITED_COVERAGE_STRUCTURAL_CHANGE;
            match language {
                crate::features::shared::interface::i18n::Language::ZhCn => format!(
                    "有限结论：{}，但当前仅有 {}/7 个有效交易日观测点，覆盖不足。",
                    if limited_change {
                        "观察到结构性变化"
                    } else {
                        "目前未见结构性变化"
                    },
                    timeline.entries.len()
                ),
                crate::features::shared::interface::i18n::Language::EnUs => format!(
                    "Limited conclusion: {}, but only {}/7 valid trading-day observations are available; coverage is insufficient.",
                    if limited_change {
                        "structural change observed"
                    } else {
                        "no structural change observed"
                    },
                    timeline.entries.len()
                ),
                crate::features::shared::interface::i18n::Language::JaJp => format!(
                    "限定的な結論：{}、ただし有効な取引日観測は {}/7 件のみで、カバレッジが不足しています。",
                    if limited_change {
                        "構造変化が観測されました"
                    } else {
                        "構造変化は観測されませんでした"
                    },
                    timeline.entries.len()
                ),
            }
        }
        crate::features::radar::domain::observation_timeline::HistoryCoverage::Complete => {
            change_summary.to_string()
        }
    };
    let sequence = |value: &dyn Fn(
        &crate::features::radar::domain::observation_timeline::ObservationTimelineEntry,
    ) -> String| {
        timeline
            .entries
            .iter()
            .map(value)
            .collect::<Vec<_>>()
            .join(" → ")
    };
    let mut block = String::new();
    match mode {
        RenderMode::Markdown | RenderMode::Html => {
            block.push_str(&format!("### {title}\n\n"));
            block.push_str(&format!("  - {coverage_label}: {coverage}\n"));
            block.push_str(&format!("  - {summary_label}: {change_summary}\n"));
            block.push_str(&format!("  - {points_label}: {}\n", timeline.entries.len()));
            block.push_str(&format!(
                "  - {leader_label}: {}\n",
                sequence(&|entry| entry.primary_leader.clone())
            ));
            block.push_str(&format!(
                "  - {breadth_label}: {}\n",
                sequence(&|entry| format!("{:.1}", entry.breadth_score))
            ));
            block.push_str(&format!(
                "  - {confidence_label}: {}\n",
                sequence(&|entry| format!("{:.1}", entry.confidence_index))
            ));
            block.push_str(&format!(
                "  - {supply_label}: {}\n",
                sequence(&|entry| entry.supply_phase.clone())
            ));
        }
    }
    block.push('\n');
    block
}

fn render_leadership_snapshot_section(
    snapshot: Option<&crate::features::radar::interface::presentation::LeadershipSnapshotViewModel>,
    mode: RenderMode,
) -> String {
    let mut block = String::new();
    let Some(snapshot) = snapshot else {
        return block;
    };

    match mode {
        RenderMode::Markdown => {
            block.push_str(&format!("### {}\n\n", snapshot.title));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.primary_leader_label, snapshot.primary_leader_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.secondary_leaders_label,
                format_values(&snapshot.secondary_leaders_values)
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.watchlist_leaders_label,
                format_watch_candidates(
                    &snapshot.watchlist_leaders_values,
                    &snapshot.watchlist_leaders_reasons
                )
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.leadership_confidence_label, snapshot.leadership_confidence_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.leadership_conflict_label, snapshot.leadership_conflict_value
            ));
            block.push_str(&format!("  - {}\n", snapshot.boundary));
        }
        RenderMode::Html => {
            block.push_str(&format!("\n<b>{}</b>\n\n", snapshot.title));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.primary_leader_label, snapshot.primary_leader_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.secondary_leaders_label,
                format_values(&snapshot.secondary_leaders_values)
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.watchlist_leaders_label,
                format_watch_candidates(
                    &snapshot.watchlist_leaders_values,
                    &snapshot.watchlist_leaders_reasons
                )
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.leadership_confidence_label, snapshot.leadership_confidence_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                snapshot.leadership_conflict_label, snapshot.leadership_conflict_value
            ));
            block.push_str(&format!("  - {}\n", snapshot.boundary));
        }
    }
    block.push('\n');
    block
}

fn render_leader_persistence_section(
    persistence: Option<
        &crate::features::radar::interface::presentation::LeaderPersistenceViewModel,
    >,
    mode: RenderMode,
) -> String {
    let mut block = String::new();
    let Some(persistence) = persistence else {
        return block;
    };

    match mode {
        RenderMode::Markdown => {
            block.push_str(&format!("### {}\n\n", persistence.title));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.primary_leader_label, persistence.primary_leader_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.persistence_label, persistence.persistence_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.observed_days_label, persistence.observed_days_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.breakout_continuity_label, persistence.breakout_continuity_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.history_coverage_label, persistence.history_coverage_value
            ));
            if let Some(note) = &persistence.history_note {
                block.push_str(&format!("  - {}\n", note));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.leadership_score_label, persistence.leadership_score_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.leader_state_label, persistence.leader_state_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.change_from_yesterday_label, persistence.change_from_yesterday_value
            ));
            if !persistence.switch_history_values.is_empty() {
                block.push_str(&format!("  - {}:\n", persistence.switch_history_label));
                for value in &persistence.switch_history_values {
                    block.push_str(&format!("    - {}\n", value));
                }
            }
            block.push_str(&format!("  - {}\n", persistence.boundary));
        }
        RenderMode::Html => {
            block.push_str(&format!("\n<b>{}</b>\n\n", persistence.title));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.primary_leader_label, persistence.primary_leader_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.persistence_label, persistence.persistence_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.observed_days_label, persistence.observed_days_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.breakout_continuity_label, persistence.breakout_continuity_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.history_coverage_label, persistence.history_coverage_value
            ));
            if let Some(note) = &persistence.history_note {
                block.push_str(&format!("  - {}\n", note));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.leadership_score_label, persistence.leadership_score_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.leader_state_label, persistence.leader_state_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.change_from_yesterday_label, persistence.change_from_yesterday_value
            ));
            if !persistence.switch_history_values.is_empty() {
                block.push_str(&format!("  - {}:\n", persistence.switch_history_label));
                for value in &persistence.switch_history_values {
                    block.push_str(&format!("    - {}\n", value));
                }
            }
            block.push_str(&format!("  - {}\n", persistence.boundary));
        }
    }
    block.push('\n');
    block
}

fn render_interpretation_section(
    layer: Option<&crate::features::radar::interface::presentation::InterpretationLayerViewModel>,
    mode: RenderMode,
    include_appendix: bool,
) -> String {
    let mut block = String::new();
    let Some(layer) = layer else {
        return block;
    };

    match mode {
        RenderMode::Markdown => {
            block.push_str(&format!("### {}\n\n", layer.title));
            block.push_str(&format!("  - {}\n", layer.notice));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.current_decision_weight_label, layer.current_decision_weight_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.todays_explanation_label, layer.todays_explanation_navigation_value
            ));
            block.push_str(&format!("  - {}:\n", layer.signal_context_label));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_information_content_label,
                layer.signal_context_information_content_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_primary_context_label,
                layer.signal_context_primary_context_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_quality_label, layer.signal_context_quality_value
            ));
            if !layer.signal_context_event_fact_value.is_empty() {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.signal_context_event_fact_label, layer.signal_context_event_fact_value
                ));
            }
            if !layer.signal_context_source_diagnostics_value.is_empty() {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.signal_context_source_diagnostics_label,
                    layer.signal_context_source_diagnostics_value
                ));
            }
            if include_appendix
                && !layer
                    .signal_context_source_diagnostics_appendix_value
                    .is_empty()
            {
                block.push_str(&format!(
                    "    - {}:\n",
                    layer.signal_context_source_diagnostics_appendix_label
                ));
                for line in layer
                    .signal_context_source_diagnostics_appendix_value
                    .lines()
                {
                    block.push_str(&format!("      - {}\n", line));
                }
            }
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_interpretation_label,
                layer.signal_context_interpretation_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_next_observation_label,
                layer.signal_context_next_observation_value
            ));
            block.push_str(&format!("  - {}\n", layer.signal_context_boundary));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_quality_label, layer.expectation_quality_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_quality_reason_label, layer.expectation_quality_reason_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_lifecycle_label, layer.expectation_lifecycle_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_next_observation_label, layer.expectation_next_observation_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.gravity_data_quality_label, layer.gravity_data_quality_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.gravity_data_quality_reason_label, layer.gravity_data_quality_reason_value
            ));
            block.push_str(&format!("  - {}:\n", layer.observation_health_label));
            for line in layer.observation_health_value.lines() {
                block.push_str(&format!("    - {}\n", line));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.interpretation_quality_label, layer.interpretation_quality_value
            ));
            block.push_str(&format!(
                "  - {}: See Market Interpretation.\n",
                layer.narrative_components_label
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.interpretation_label, layer.interpretation_value
            ));
            if !layer.decision_explanation_reasons.is_empty() {
                block.push_str(&format!("  - {}:\n", layer.decision_explanation_label));
                block.push_str(&format!("    - {}\n", layer.decision_explanation_intro));
                for reason in &layer.decision_explanation_reasons {
                    block.push_str(&format!("    - {}\n", reason));
                }
                block.push_str(&format!(
                    "    - {}\n",
                    layer.decision_explanation_conclusion
                ));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.subjects_label, layer.subjects_value
            ));
            block.push_str(&format!("  - {}\n", layer.boundary));
        }
        RenderMode::Html => {
            block.push_str(&format!("\n<b>{}</b>\n", layer.title));
            block.push_str(&format!("  - <i>{}</i>\n", layer.notice));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.current_decision_weight_label, layer.current_decision_weight_value
            ));
            block.push_str(&format!(
                "  - <b>{}:</b> {}\n",
                layer.todays_explanation_label, layer.todays_explanation_navigation_value
            ));
            block.push_str(&format!("  - {}:\n", layer.signal_context_label));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_information_content_label,
                layer.signal_context_information_content_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_primary_context_label,
                layer.signal_context_primary_context_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_quality_label, layer.signal_context_quality_value
            ));
            if !layer.signal_context_event_fact_value.is_empty() {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.signal_context_event_fact_label, layer.signal_context_event_fact_value
                ));
            }
            if !layer.signal_context_source_diagnostics_value.is_empty() {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.signal_context_source_diagnostics_label,
                    layer.signal_context_source_diagnostics_value
                ));
            }
            if include_appendix
                && !layer
                    .signal_context_source_diagnostics_appendix_value
                    .is_empty()
            {
                block.push_str(&format!(
                    "    - {}:\n",
                    layer.signal_context_source_diagnostics_appendix_label
                ));
                for line in layer
                    .signal_context_source_diagnostics_appendix_value
                    .lines()
                {
                    block.push_str(&format!("      - <i>{}</i>\n", line));
                }
            }
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_interpretation_label,
                layer.signal_context_interpretation_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.signal_context_next_observation_label,
                layer.signal_context_next_observation_value
            ));
            block.push_str(&format!("  - {}\n", layer.signal_context_boundary));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_quality_label, layer.expectation_quality_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_quality_reason_label, layer.expectation_quality_reason_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_lifecycle_label, layer.expectation_lifecycle_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.expectation_next_observation_label, layer.expectation_next_observation_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.gravity_data_quality_label, layer.gravity_data_quality_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.gravity_data_quality_reason_label, layer.gravity_data_quality_reason_value
            ));
            block.push_str(&format!("  - {}:\n", layer.observation_health_label));
            for line in layer.observation_health_value.lines() {
                block.push_str(&format!("    - <i>{}</i>\n", line));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.interpretation_quality_label, layer.interpretation_quality_value
            ));
            block.push_str(&format!(
                "  - {}: See Market Interpretation.\n",
                layer.narrative_components_label
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.interpretation_label, layer.interpretation_value
            ));
            if !layer.decision_explanation_reasons.is_empty() {
                block.push_str(&format!("  - {}:\n", layer.decision_explanation_label));
                block.push_str(&format!(
                    "    - <i>{}</i>\n",
                    layer.decision_explanation_intro
                ));
                for reason in &layer.decision_explanation_reasons {
                    block.push_str(&format!("    - <i>{}</i>\n", reason));
                }
                block.push_str(&format!(
                    "    - <i>{}</i>\n",
                    layer.decision_explanation_conclusion
                ));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.subjects_label, layer.subjects_value
            ));
            block.push_str(&format!("  - {}\n", layer.boundary));
        }
    }
    block.push('\n');
    block
}

fn render_market_interpretation_section(
    layer: Option<&crate::features::radar::interface::presentation::MarketInterpretationViewModel>,
    mode: RenderMode,
) -> String {
    let mut block = String::new();
    let Some(layer) = layer else {
        return block;
    };

    match mode {
        RenderMode::Markdown => {
            block.push_str(&format!("### {}\n\n", layer.title));
            block.push_str(&format!("  - {}\n", layer.notice));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.current_decision_weight_label, layer.current_decision_weight_value
            ));
            if !layer.narrative_values.is_empty() {
                block.push_str(&format!("  - {}:\n", layer.narrative_label));
                for value in &layer.narrative_values {
                    block.push_str(&format!("    - {}\n", value));
                }
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.day_type_label, layer.day_type_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.day_type_reason_label, layer.day_type_reason_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.exceptional_factors_label,
                format_values(&layer.exceptional_factors_values)
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.leadership_classification_label, layer.leadership_classification_value
            ));
            block.push_str(&format!("  - {}:\n", layer.leadership_metrics_label));
            if is_leadership_unavailable(&layer.leadership_classification_value) {
                block.push_str(
                    "  - Leadership detail suppressed because the leadership sets conflict.\n",
                );
            } else {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.primary_label,
                    format_values(&layer.primary_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.supporting_label,
                    format_values(&layer.supporting_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.weakening_label,
                    format_values(&layer.weakening_values)
                ));
            }
            block.push_str(&format!("    - Breadth: {}\n", layer.breadth_score_value));
            block.push_str(&format!(
                "    - Concentration: {}\n",
                layer.concentration_score_value
            ));
            block.push_str(&format!("    - Rotation: {}\n", layer.rotation_score_value));
            if !is_leadership_unavailable(&layer.leadership_classification_value) {
                block.push_str(&format!("  - {}:\n", layer.rotation_label));
                block.push_str(&format!(
                    "    - rotationType: {}\n",
                    layer.rotation_type_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rotation_from_label,
                    format_values(&layer.rotation_from_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rotation_to_label,
                    format_values(&layer.rotation_to_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rotation_interpretation_label, layer.rotation_interpretation_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.observation_only_label, layer.observation_only_value
                ));
            }
            block.push_str(&format!("  - {}:\n", layer.confidence_label));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.trend_confidence_label, layer.trend_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.macro_confidence_label, layer.macro_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.supply_confidence_label, layer.supply_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.expectation_confidence_label, layer.expectation_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.gravity_confidence_label, layer.gravity_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.flow_confidence_label, layer.flow_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.overall_confidence_label, layer.overall_confidence_value
            ));
            block.push_str(&format!("  - {}:\n", layer.interpretation_priority_label));
            for item in &layer.interpretation_priority_values {
                block.push_str(&format!("    - {}\n", item));
            }
            block.push_str(&format!("  - {}\n", layer.boundary));
        }
        RenderMode::Html => {
            block.push_str(&format!("\n<b>{}</b>\n\n", layer.title));
            block.push_str(&format!("  - <i>{}</i>\n", layer.notice));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.current_decision_weight_label, layer.current_decision_weight_value
            ));
            if !layer.narrative_values.is_empty() {
                block.push_str(&format!("  - {}:\n", layer.narrative_label));
                for value in &layer.narrative_values {
                    block.push_str(&format!("    - {}\n", value));
                }
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.day_type_label, layer.day_type_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.day_type_reason_label, layer.day_type_reason_value
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.exceptional_factors_label,
                format_values(&layer.exceptional_factors_values)
            ));
            block.push_str(&format!(
                "  - {}: {}\n",
                layer.leadership_classification_label, layer.leadership_classification_value
            ));
            block.push_str(&format!("  - {}:\n", layer.leadership_metrics_label));
            if is_leadership_unavailable(&layer.leadership_classification_value) {
                block.push_str(
                    "  - Leadership detail suppressed because the leadership sets conflict.\n",
                );
            } else {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.primary_label,
                    format_values(&layer.primary_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.supporting_label,
                    format_values(&layer.supporting_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.weakening_label,
                    format_values(&layer.weakening_values)
                ));
            }
            block.push_str(&format!("    - Breadth: {}\n", layer.breadth_score_value));
            block.push_str(&format!(
                "    - Concentration: {}\n",
                layer.concentration_score_value
            ));
            block.push_str(&format!("    - Rotation: {}\n", layer.rotation_score_value));
            if !is_leadership_unavailable(&layer.leadership_classification_value) {
                block.push_str(&format!("  - {}:\n", layer.rotation_label));
                block.push_str(&format!(
                    "    - rotationType: {}\n",
                    layer.rotation_type_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rotation_from_label,
                    format_values(&layer.rotation_from_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rotation_to_label,
                    format_values(&layer.rotation_to_values)
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rotation_interpretation_label, layer.rotation_interpretation_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.observation_only_label, layer.observation_only_value
                ));
            }
            block.push_str(&format!("  - {}:\n", layer.confidence_label));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.trend_confidence_label, layer.trend_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.macro_confidence_label, layer.macro_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.supply_confidence_label, layer.supply_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.expectation_confidence_label, layer.expectation_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.gravity_confidence_label, layer.gravity_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.flow_confidence_label, layer.flow_confidence_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.overall_confidence_label, layer.overall_confidence_value
            ));
            block.push_str(&format!("  - {}:\n", layer.interpretation_priority_label));
            for item in &layer.interpretation_priority_values {
                block.push_str(&format!("    - <i>{}</i>\n", item));
            }
            block.push_str(&format!("  - <i>{}</i>\n", layer.boundary));
        }
    }

    block.push('\n');
    block
}

fn is_leadership_unavailable(value: &str) -> bool {
    value == "Leadership unavailable"
}

fn format_values(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", values.join(", "))
    }
}

fn format_watch_candidates(values: &[String], reasons: &[String]) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    values
        .iter()
        .enumerate()
        .map(|(index, symbol)| match reasons.get(index) {
            Some(reason) if !reason.is_empty() => format!("{symbol} ({reason})"),
            _ => symbol.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_transition_diff(target: &mut String, label: &str, values: &[String], mode: RenderMode) {
    if values.is_empty() {
        return;
    }
    match mode {
        RenderMode::Markdown => target.push_str(&format!("  - {}: {}\n", label, values.join(", "))),
        RenderMode::Html => {
            target.push_str(&format!("  <i>• {}: {}</i>\n", label, values.join(", ")))
        }
    }
}

fn render_compact_no_trade_reasons(
    reasons_label: &str,
    topology_value: &str,
    stability: (&str, &str, &str),
    continuity: (&str, &str, &str),
    mode: RenderMode,
) -> String {
    let mut out = String::new();
    match mode {
        RenderMode::Markdown => out.push_str(&format!("\n> {}：\n", reasons_label)),
        RenderMode::Html => out.push_str(&format!("\n<b>{}</b>:\n", reasons_label)),
    }

    let stability_display = format_ratio_display(stability.1, stability.2);
    let continuity_display = continuity.1.to_string();
    match mode {
        RenderMode::Markdown => {
            out.push_str(&format!("> - {} {}\n", stability.0, stability_display));
            out.push_str(&format!("> - {} {}\n", continuity.0, continuity_display));
            out.push_str(&format!("> - {}\n\n", topology_value));
        }
        RenderMode::Html => {
            out.push_str(&format!("• <i>{} {}</i>\n", stability.0, stability_display));
            out.push_str(&format!(
                "• <i>{} {}</i>\n",
                continuity.0, continuity_display
            ));
            out.push_str(&format!("• <i>{}</i>\n\n", topology_value));
        }
    }
    out
}

fn format_ratio_display(value: &str, denominator: &str) -> String {
    let normalized = value.trim();
    if normalized.is_empty() || normalized == "-" || normalized.eq_ignore_ascii_case("n/a") {
        "N/A".to_string()
    } else {
        format!("{}/{}", normalized, denominator)
    }
}
