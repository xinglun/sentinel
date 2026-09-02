use crate::features::radar::domain::observation_timeline::ObservationTimeline;
use crate::features::radar::interface::presentation::PresentationPacket;
use crate::features::shared::interface::i18n::{get_dictionary, DisplayDictionary, Language};
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

#[derive(Clone, Copy)]
enum ExitItemSection {
    RiskSignal,
    PortfolioAction,
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
    card.push_str(&render_runtime_integrity_markdown(
        pres,
        detailed_transition,
    ));
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
    card.push_str(&render_effective_action_details(
        pres,
        &dict,
        RenderMode::Markdown,
    ));
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
    card.push_str(&render_current_relative_strength_section(
        pres.current_relative_strength.as_ref(),
        pres.language,
        RenderMode::Markdown,
    ));
    if compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }
    card.push_str(&render_interpretation_section(
        pres.interpretation_layer.as_ref(),
        pres.language,
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
    card.push_str(&render_runtime_integrity_html(pres));
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
    card.push_str(&render_effective_action_details(
        pres,
        &dict,
        RenderMode::Html,
    ));
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
    card.push_str(&render_current_relative_strength_section(
        pres.current_relative_strength.as_ref(),
        pres.language,
        RenderMode::Html,
    ));
    if compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }
    card.push_str(&render_interpretation_section(
        pres.interpretation_layer.as_ref(),
        pres.language,
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

fn render_runtime_integrity_markdown(pres: &PresentationPacket, detailed: bool) -> String {
    let mut out = String::new();
    if let Some(identity) = pres.runtime_identity.as_ref() {
        out.push_str("<!-- report_runtime_identity\n");
        out.push_str(
            &serde_json::to_string_pretty(identity).expect("runtime identity is serializable"),
        );
        out.push_str("\n-->\n\n");
    }
    if let Some(integrity) = pres.runtime_integrity.as_ref() {
        if !integrity.is_healthy() {
            let diagnostics = if integrity.diagnostics.is_empty() {
                "UNAVAILABLE".to_string()
            } else {
                integrity.diagnostics.join(", ")
            };
            out.push_str(&format!(
                "> ⚠️ Observation Integrity: {} (Runtime Integrity, decision_weight=0)\n> diagnostics: {}\n\n",
                runtime_integrity_status_label(integrity.status),
                diagnostics,
            ));
        }
    }
    if detailed {
        out.push_str("## Report Runtime Integrity\n\n");
        if let Some(provenance) = pres.data_provenance.as_ref() {
            out.push_str("### data_provenance\n\n```json\n");
            out.push_str(
                &serde_json::to_string_pretty(provenance).expect("provenance is serializable"),
            );
            out.push_str("\n```\n\n");
        }
        if let Some(integrity) = pres.runtime_integrity.as_ref() {
            out.push_str("### runtime_integrity\n\n```json\n");
            out.push_str(
                &serde_json::to_string_pretty(integrity).expect("integrity is serializable"),
            );
            out.push_str("\n```\n\n");
        }
        if let Some(lifecycle) = pres.report_lifecycle.as_ref() {
            out.push_str("### report_lifecycle\n\n```json\n");
            out.push_str(
                &serde_json::to_string_pretty(lifecycle).expect("lifecycle is serializable"),
            );
            out.push_str("\n```\n\n");
        }
        if let Some(leadership) = pres.leader_persistence.as_ref() {
            out.push_str("### leadership_provenance\n\n```json\n");
            out.push_str(
                &serde_json::to_string_pretty(leadership)
                    .expect("leadership provenance is serializable"),
            );
            out.push_str("\n```\n\n");
        }
    }
    out
}

fn render_runtime_integrity_html(pres: &PresentationPacket) -> String {
    let Some(integrity) = pres.runtime_integrity.as_ref() else {
        return String::new();
    };
    let mut out = String::new();
    if let Some(identity) = pres.runtime_identity.as_ref() {
        out.push_str(&format!(
            "<i>report_runtime_identity: {} | revision: {}</i>\n\n",
            identity.report_run_id, identity.git_commit_sha
        ));
    }
    if !integrity.is_healthy() {
        out.push_str(&format!(
            "<i>Runtime Integrity: {} (decision_weight=0; diagnostics: {})</i>\n\n",
            runtime_integrity_status_label(integrity.status),
            if integrity.diagnostics.is_empty() {
                "UNAVAILABLE".to_string()
            } else {
                integrity.diagnostics.join(", ")
            }
        ));
    }
    out
}

fn runtime_integrity_status_label(
    status: crate::features::shared::application::run_status::RuntimeIntegrityStatus,
) -> &'static str {
    match status {
        crate::features::shared::application::run_status::RuntimeIntegrityStatus::Healthy => {
            "HEALTHY"
        }
        crate::features::shared::application::run_status::RuntimeIntegrityStatus::Degraded => {
            "DEGRADED"
        }
        crate::features::shared::application::run_status::RuntimeIntegrityStatus::Unavailable => {
            "UNAVAILABLE"
        }
    }
}

fn render_exit_summary(pres: &PresentationPacket, mode: RenderMode) -> String {
    let mut out = String::new();
    match mode {
        RenderMode::Markdown => out.push_str(&format!("### {}\n\n", pres.exit_summary.title)),
        RenderMode::Html => out.push_str(&format!("<b>{}</b>\n\n", pres.exit_summary.title)),
    }
    if !pres.exit_summary.signal_items.is_empty() {
        match mode {
            RenderMode::Markdown => {
                out.push_str(&format!("#### {}\n\n", pres.exit_summary.signal_title))
            }
            RenderMode::Html => {
                out.push_str(&format!("<b>{}</b>\n\n", pres.exit_summary.signal_title))
            }
        }
        out.push_str(&render_exit_items(
            &pres.exit_summary.signal_items,
            mode,
            pres.language,
            ExitItemSection::RiskSignal,
        ));
    }

    match mode {
        RenderMode::Markdown => out.push_str(&format!(
            "#### {}\n\n",
            pres.exit_summary.actual_action_title
        )),
        RenderMode::Html => out.push_str(&format!(
            "<b>{}</b>\n\n",
            pres.exit_summary.actual_action_title
        )),
    }
    if !pres.exit_summary.items.is_empty() {
        out.push_str(&render_exit_items(
            &pres.exit_summary.items,
            mode,
            pres.language,
            ExitItemSection::PortfolioAction,
        ));
    } else if let Some(note) = pres
        .exit_summary
        .no_action_note
        .as_ref()
        .or(pres.exit_summary.empty_note.as_ref())
    {
        let (_, _, _, _, _, portfolio_action_label, portfolio_action_none) =
            exit_semantic_labels(pres.language);
        match mode {
            RenderMode::Markdown => {
                out.push_str(&format!(
                    "{}: {}\n",
                    portfolio_action_label, portfolio_action_none
                ));
            }
            RenderMode::Html => {
                out.push_str(&format!(
                    "{}: {}\n",
                    portfolio_action_label, portfolio_action_none
                ));
            }
        }
        for line in note.lines() {
            match mode {
                RenderMode::Markdown => out.push_str(&format!("> {}\n", line)),
                RenderMode::Html => out.push_str(&format!("{}\n", line)),
            }
        }
    }
    out.push('\n');
    out
}

fn render_exit_items(
    items: &[crate::features::radar::interface::presentation::ExitDecisionItemViewModel],
    mode: RenderMode,
    language: Language,
    section: ExitItemSection,
) -> String {
    let mut out = String::new();
    let (
        action_matrix_label,
        risk_adjustment_label,
        modifier_label,
        explanation_label,
        trigger_label,
        portfolio_action_label,
        _,
    ) = exit_semantic_labels(language);
    for item in items {
        match mode {
            RenderMode::Markdown => {
                out.push_str(&format!("- {}\n", item.symbol));
                match section {
                    ExitItemSection::RiskSignal => {
                        out.push_str(&format!(
                            "   {}: {}\n   {}: {}\n   {}: {}\n",
                            action_matrix_label,
                            item.action_state,
                            risk_adjustment_label,
                            risk_adjustment_code(item.intent),
                            trigger_label,
                            item.reason
                        ));
                        if let Some(modifier) = item.observation_modifier.as_deref() {
                            out.push_str(&format!("   {}: {}\n", modifier_label, modifier));
                        }
                        if let Some(explanation) = item.observation_explanation.as_deref() {
                            out.push_str(&format!("   {}: {}\n", explanation_label, explanation));
                        }
                    }
                    ExitItemSection::PortfolioAction => {
                        out.push_str(&format!(
                            "   {}: {}\n",
                            portfolio_action_label,
                            exit_intent_code(item.intent)
                        ));
                    }
                }
            }
            RenderMode::Html => {
                out.push_str(&format!("• {}\n", item.symbol));
                match section {
                    ExitItemSection::RiskSignal => {
                        out.push_str(&format!(
                            "  {}: {}\n  {}: {}\n  {}: {}\n",
                            action_matrix_label,
                            item.action_state,
                            risk_adjustment_label,
                            risk_adjustment_code(item.intent),
                            trigger_label,
                            item.reason
                        ));
                        if let Some(modifier) = item.observation_modifier.as_deref() {
                            out.push_str(&format!("  {}: {}\n", modifier_label, modifier));
                        }
                        if let Some(explanation) = item.observation_explanation.as_deref() {
                            out.push_str(&format!("  {}: {}\n", explanation_label, explanation));
                        }
                    }
                    ExitItemSection::PortfolioAction => {
                        out.push_str(&format!(
                            "  {}: {}\n",
                            portfolio_action_label,
                            exit_intent_code(item.intent)
                        ));
                    }
                }
            }
        }
    }
    out
}

fn exit_semantic_labels(
    language: Language,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    match language {
        Language::ZhCn => (
            "基础 Action Matrix",
            "风险调整信号",
            "观察修正",
            "解释",
            "触发原因",
            "实际组合动作",
            "NONE",
        ),
        Language::EnUs => (
            "Base Action Matrix",
            "Risk Adjustment Signal",
            "Observation Modifier",
            "Explanation",
            "Trigger",
            "Portfolio Action",
            "NONE",
        ),
        Language::JaJp => (
            "基本 Action Matrix",
            "リスク調整シグナル",
            "観測修正",
            "説明",
            "発動理由",
            "実際のポートフォリオアクション",
            "NONE",
        ),
    }
}

fn exit_intent_code(
    intent: crate::features::radar::interface::presentation::ExitDisplayIntent,
) -> &'static str {
    match intent {
        crate::features::radar::interface::presentation::ExitDisplayIntent::Hold => "HOLD",
        crate::features::radar::interface::presentation::ExitDisplayIntent::Trim => "TRIM",
        crate::features::radar::interface::presentation::ExitDisplayIntent::Exit => "EXIT",
        crate::features::radar::interface::presentation::ExitDisplayIntent::Watch => "WATCH",
    }
}

fn risk_adjustment_code(
    intent: crate::features::radar::interface::presentation::ExitDisplayIntent,
) -> &'static str {
    match intent {
        crate::features::radar::interface::presentation::ExitDisplayIntent::Trim => "REDUCE",
        crate::features::radar::interface::presentation::ExitDisplayIntent::Exit => "EXIT",
        crate::features::radar::interface::presentation::ExitDisplayIntent::Hold
        | crate::features::radar::interface::presentation::ExitDisplayIntent::Watch => "NONE",
    }
}

fn render_effective_action_details(
    pres: &PresentationPacket,
    dict: &DisplayDictionary,
    mode: RenderMode,
) -> String {
    let permission_range =
        final_execution_permission_range(pres, &pres.decision_summary.entry_cap_value);
    let effective_range =
        final_execution_position_range(pres, &pres.decision_summary.entry_cap_value);
    let permission = market_permission_label(pres);
    let action = final_execution_action_label(pres);
    match mode {
        RenderMode::Markdown => format!(
            "\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n",
            dict.decision.market_permission,
            permission,
            dict.decision.eligible_assets,
            pres.final_execution_decision.eligible_asset_count,
            dict.decision.effective_action,
            action,
            dict.decision.permission_budget,
            permission_range,
            dict.decision.effective_new_entry_cap,
            effective_range,
        ),
        RenderMode::Html => format!(
            "\n<b>{}</b>: {}\n<b>{}</b>: {}\n<b>{}</b>: {}\n<b>{}</b>: {}\n<b>{}</b>: {}\n",
            dict.decision.market_permission,
            permission,
            dict.decision.eligible_assets,
            pres.final_execution_decision.eligible_asset_count,
            dict.decision.effective_action,
            action,
            dict.decision.permission_budget,
            permission_range,
            dict.decision.effective_new_entry_cap,
            effective_range,
        ),
    }
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
    if pres.final_execution_decision.execution_window != ExecutionWindow::None
        && pres.final_execution_decision.eligible_asset_count == 0
    {
        return crate::features::shared::interface::i18n::get_dictionary(pres.language)
            .decision
            .no_new_entry;
    }
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

fn final_execution_permission_range(pres: &PresentationPacket, fallback: &str) -> String {
    if pres.final_execution_decision.reason.is_empty()
        || pres
            .final_execution_decision
            .permission_position_range
            .is_empty()
    {
        fallback.to_string()
    } else {
        pres.final_execution_decision
            .permission_position_range
            .clone()
    }
}

fn market_permission_label(pres: &PresentationPacket) -> String {
    use crate::features::radar::interface::presentation::{ExecutionWindow, ParticipationMode};
    use crate::features::shared::interface::i18n::Language;
    match (
        pres.final_execution_decision.execution_window,
        pres.final_execution_decision.participation_mode,
    ) {
        (ExecutionWindow::None, _) => match pres.language {
            Language::ZhCn => "无市场参与权限",
            Language::EnUs => "No Market Permission",
            Language::JaJp => "市場参加権限なし",
        },
        (ExecutionWindow::Limited, ParticipationMode::Probe) => match pres.language {
            Language::ZhCn => "有限参与窗口 / 仅 Probe",
            Language::EnUs => "Limited Participation Window / Probe Only",
            Language::JaJp => "限定参加ウィンドウ / Probe のみ",
        },
        (ExecutionWindow::Open, ParticipationMode::Add) => match pres.language {
            Language::ZhCn => "开放参与窗口 / 可加仓",
            Language::EnUs => "Open Participation Window / Add",
            Language::JaJp => "参加ウィンドウ開始 / 追加可",
        },
        _ => pres.final_execution_decision.reason.as_str(),
    }
    .to_string()
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
            out.push_str(&format!(
                "> {}: {}\n",
                s.confidence_breakdown_label, s.confidence_breakdown_value
            ));
            out.push_str(&format!(
                "> {}: {} · {}: {} · {}: {} · {}: {}\n",
                s.breadth_raw_label,
                s.breadth_raw_value,
                s.breadth_counts_label,
                s.breadth_counts_value,
                s.breadth_universe_label,
                s.breadth_universe_value,
                s.breadth_semantic_label,
                s.breadth_semantic_value
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
            out.push_str(&format!(
                "<i>{}: {}</i>\n",
                s.confidence_breakdown_label, s.confidence_breakdown_value
            ));
            out.push_str(&format!(
                "<i>{}: {} · {}: {} · {}: {} · {}: {}</i>\n",
                s.breadth_raw_label,
                s.breadth_raw_value,
                s.breadth_counts_label,
                s.breadth_counts_value,
                s.breadth_universe_label,
                s.breadth_universe_value,
                s.breadth_semantic_label,
                s.breadth_semantic_value
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
                out.push_str(&format!(
                    "> **{}**: {}\n> **{}**: {}\n",
                    pres.risk_opportunity_summary.execution_risk_label,
                    pres.risk_opportunity_summary.execution_risk_value,
                    pres.risk_opportunity_summary.portfolio_risk_label,
                    pres.risk_opportunity_summary.portfolio_risk_value
                ));
            } else {
                out.push_str(&format!(
                    "- **{}**: {}\n- **{}**: {}\n",
                    pres.risk_opportunity_summary.opportunity_label,
                    pres.risk_opportunity_summary.opportunity_value,
                    pres.risk_opportunity_summary.risk_label,
                    pres.risk_opportunity_summary.risk_value
                ));
                out.push_str(&format!(
                    "- **{}**: {}\n- **{}**: {}\n",
                    pres.risk_opportunity_summary.execution_risk_label,
                    pres.risk_opportunity_summary.execution_risk_value,
                    pres.risk_opportunity_summary.portfolio_risk_label,
                    pres.risk_opportunity_summary.portfolio_risk_value
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
                out.push_str(&format!(
                    "<i><b>{}</b>: {}\n<b>{}</b>: {}</i>\n",
                    pres.risk_opportunity_summary.execution_risk_label,
                    pres.risk_opportunity_summary.execution_risk_value,
                    pres.risk_opportunity_summary.portfolio_risk_label,
                    pres.risk_opportunity_summary.portfolio_risk_value
                ));
            } else {
                out.push_str(&format!(
                    "• <b>{}</b>: {}\n• <b>{}</b>: {}\n",
                    pres.risk_opportunity_summary.opportunity_label,
                    pres.risk_opportunity_summary.opportunity_value,
                    pres.risk_opportunity_summary.risk_label,
                    pres.risk_opportunity_summary.risk_value
                ));
                out.push_str(&format!(
                    "• <b>{}</b>: {}\n• <b>{}</b>: {}\n",
                    pres.risk_opportunity_summary.execution_risk_label,
                    pres.risk_opportunity_summary.execution_risk_value,
                    pres.risk_opportunity_summary.portfolio_risk_label,
                    pres.risk_opportunity_summary.portfolio_risk_value
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
                "  - Baseline Status: {}\n  - Change Status: {}\n",
                change_log.baseline_status, change_log.change_status
            ));
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
                "  - Baseline Status: {}\n  - Change Status: {}\n",
                change_log.baseline_status, change_log.change_status
            ));
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
    let dict = get_dictionary(language);
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
        breadth_raw_label,
        breadth_classification_label,
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
            dict.signals.universe_breadth_raw_sequence.as_str(),
            dict.signals.universe_breadth_score_sequence.as_str(),
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
            dict.signals.universe_breadth_raw_sequence.as_str(),
            dict.signals.universe_breadth_score_sequence.as_str(),
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
            dict.signals.universe_breadth_raw_sequence.as_str(),
            dict.signals.universe_breadth_score_sequence.as_str(),
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
    let supply_sequence =
        sequence(&|entry| timeline_supply_phase_value(&entry.supply_phase).to_string());
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
                "  - {breadth_raw_label}: {}\n",
                sequence(&|entry| {
                    entry
                        .breadth_raw_percent
                        .map(|value| format!("{value:.1}"))
                        .unwrap_or_else(|| "UNAVAILABLE".to_string())
                })
            ));
            block.push_str(&format!(
                "  - {breadth_classification_label}: {}\n",
                sequence(&|entry| {
                    entry
                        .breadth_score
                        .map(|value| format!("{value:.1}"))
                        .unwrap_or_else(|| "UNAVAILABLE".to_string())
                })
            ));
            block.push_str(&format!(
                "  - {confidence_label}: {}\n",
                sequence(&|entry| format!("{:.1}", entry.confidence_index))
            ));
            block.push_str(&format!("  - {supply_label}: {}\n", supply_sequence));
        }
    }
    block.push('\n');
    block
}

fn timeline_supply_phase_value(value: &str) -> &str {
    match value {
        "IDLE" | "ACCUMULATING" | "ABSORBING" | "STRESSED" | "OVERWHELMED" => value,
        _ => "UNAVAILABLE",
    }
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
    let absent = persistence.leader_state_value == "ABSENT";

    match mode {
        RenderMode::Markdown => {
            block.push_str(&format!("### {}\n\n", persistence.title));
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.primary_leader_label, persistence.primary_leader_value
            ));
            block.push_str(&format!(
                "  - Current Leader: {}\n",
                persistence.primary_leader_value
            ));
            if let Some(previous) = &persistence.previous_snapshot_leader_value {
                block.push_str(&format!("  - Previous Snapshot Leader: {previous}\n"));
            }
            if let Some(last_confirmed) = &persistence.last_confirmed_leader_value {
                block.push_str(&format!("  - Last Confirmed Leader: {last_confirmed}\n"));
            }
            if !persistence.tactical_leadership_structure_value.is_empty() {
                block.push_str(&format!(
                    "  - Tactical Leadership Structure: {}\n",
                    persistence.tactical_leadership_structure_value
                ));
            }
            if !absent {
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
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.history_coverage_label, persistence.history_coverage_value
            ));
            block.push_str(&format!(
                "  - Leadership Snapshot ID: {}\n",
                persistence
                    .leadership_snapshot_id
                    .as_deref()
                    .unwrap_or("UNAVAILABLE")
            ));
            block.push_str(&format!(
                "  - Previous Snapshot ID: {}\n",
                persistence
                    .previous_snapshot_id
                    .as_deref()
                    .unwrap_or("UNAVAILABLE")
            ));
            block.push_str(&format!(
                "  - calculation_mode: {}\n",
                if persistence.calculation_mode.is_empty() {
                    "UNAVAILABLE"
                } else {
                    persistence.calculation_mode.as_str()
                }
            ));
            if persistence.calculation_mode == "RECOMPUTED_FROM_PARTIAL_HISTORY" {
                block.push_str(
                    "  - absence_since and duration are estimated/reconstructed from partial history.\n",
                );
            }
            if let Some(note) = &persistence.history_note {
                block.push_str(&format!("  - {}\n", note));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.leader_state_label, persistence.leader_state_value
            ));
            if absent {
                block.push_str(&format!(
                    "  - Leader Absence Duration: {} trading days\n",
                    persistence.leader_absence_duration
                ));
                if let Some(absence_since) = &persistence.leader_absence_since_value {
                    block.push_str(&format!("  - Leader Absence Since: {absence_since}\n"));
                }
                block.push_str(&format!(
                    "  - Last Transition: {}\n",
                    persistence.change_from_yesterday_value
                ));
            } else {
                block.push_str(&format!(
                    "  - {}: {}\n",
                    persistence.leadership_score_label, persistence.leadership_score_value
                ));
                block.push_str(&format!(
                    "  - {}: {}\n",
                    persistence.change_from_yesterday_label,
                    persistence.change_from_yesterday_value
                ));
            }
            if !absent && !persistence.switch_history_values.is_empty() {
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
                "  - Current Leader: {}\n",
                persistence.primary_leader_value
            ));
            if let Some(previous) = &persistence.previous_snapshot_leader_value {
                block.push_str(&format!("  - Previous Snapshot Leader: {previous}\n"));
            }
            if let Some(last_confirmed) = &persistence.last_confirmed_leader_value {
                block.push_str(&format!("  - Last Confirmed Leader: {last_confirmed}\n"));
            }
            if !persistence.tactical_leadership_structure_value.is_empty() {
                block.push_str(&format!(
                    "  - Tactical Leadership Structure: {}\n",
                    persistence.tactical_leadership_structure_value
                ));
            }
            if !absent {
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
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.history_coverage_label, persistence.history_coverage_value
            ));
            block.push_str(&format!(
                "  - Leadership Snapshot ID: {}\n",
                persistence
                    .leadership_snapshot_id
                    .as_deref()
                    .unwrap_or("UNAVAILABLE")
            ));
            block.push_str(&format!(
                "  - Previous Snapshot ID: {}\n",
                persistence
                    .previous_snapshot_id
                    .as_deref()
                    .unwrap_or("UNAVAILABLE")
            ));
            block.push_str(&format!(
                "  - calculation_mode: {}\n",
                if persistence.calculation_mode.is_empty() {
                    "UNAVAILABLE"
                } else {
                    persistence.calculation_mode.as_str()
                }
            ));
            if persistence.calculation_mode == "RECOMPUTED_FROM_PARTIAL_HISTORY" {
                block.push_str(
                    "  - absence_since and duration are estimated/reconstructed from partial history.\n",
                );
            }
            if let Some(note) = &persistence.history_note {
                block.push_str(&format!("  - {}\n", note));
            }
            block.push_str(&format!(
                "  - {}: {}\n",
                persistence.leader_state_label, persistence.leader_state_value
            ));
            if absent {
                block.push_str(&format!(
                    "  - Leader Absence Duration: {} trading days\n",
                    persistence.leader_absence_duration
                ));
                if let Some(absence_since) = &persistence.leader_absence_since_value {
                    block.push_str(&format!("  - Leader Absence Since: {absence_since}\n"));
                }
                block.push_str(&format!(
                    "  - Last Transition: {}\n",
                    persistence.change_from_yesterday_value
                ));
            } else {
                block.push_str(&format!(
                    "  - {}: {}\n",
                    persistence.leadership_score_label, persistence.leadership_score_value
                ));
                block.push_str(&format!(
                    "  - {}: {}\n",
                    persistence.change_from_yesterday_label,
                    persistence.change_from_yesterday_value
                ));
            }
            if !absent && !persistence.switch_history_values.is_empty() {
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

fn render_current_relative_strength_section(
    strength: Option<
        &crate::features::radar::interface::presentation::CurrentRelativeStrengthViewModel,
    >,
    language: crate::features::shared::interface::i18n::Language,
    _mode: RenderMode,
) -> String {
    let Some(strength) = strength else {
        return String::new();
    };
    let heading = format!("### {}\n\n", strength.title);
    let bullet = "  - ";
    let boundary = "\n";
    let benchmark_symbol = if strength.benchmark_symbol.trim().is_empty() {
        "SPY"
    } else {
        strength.benchmark_symbol.trim()
    };
    let (leader_label, day_1_label, day_5_label) = match language {
        crate::features::shared::interface::i18n::Language::ZhCn => (
            "确认 Leader".to_string(),
            format!("1日相对 {benchmark_symbol}"),
            format!("5日相对 {benchmark_symbol}"),
        ),
        crate::features::shared::interface::i18n::Language::EnUs => (
            "Confirmed Leader".to_string(),
            format!("1d vs {benchmark_symbol}"),
            format!("5d vs {benchmark_symbol}"),
        ),
        crate::features::shared::interface::i18n::Language::JaJp => (
            "確認済み Leader".to_string(),
            format!("1日 {benchmark_symbol}比"),
            format!("5日 {benchmark_symbol}比"),
        ),
    };
    let recovery_strength_label = match language {
        crate::features::shared::interface::i18n::Language::ZhCn => "恢复强度",
        crate::features::shared::interface::i18n::Language::EnUs => "Recovery Strength",
        crate::features::shared::interface::i18n::Language::JaJp => "回復強度",
    };
    let mut out = heading;
    out.push_str(&format!(
        "{}{}: {}{}",
        bullet, leader_label, strength.confirmed_leader, boundary
    ));
    for item in &strength.items {
        out.push_str(&format!(
            "{}{}: {}{}",
            bullet, item.symbol, item.status, boundary
        ));
        out.push_str(&format!(
            "{}RS Observation Health: {}{}",
            bullet,
            if item.health.is_empty() {
                "UNAVAILABLE"
            } else {
                item.health.as_str()
            },
            boundary
        ));
        if let Some(diagnostic) = &item.diagnostic {
            out.push_str(&format!(
                "{}RS Diagnostic: {}{}",
                bullet, diagnostic, boundary
            ));
        }
        if !item.recovery_strength.is_empty() && item.recovery_strength != "NONE" {
            out.push_str(&format!(
                "{}{}: {}{}",
                bullet, recovery_strength_label, item.recovery_strength, boundary
            ));
        }
        if let Some(conflict_code) = &item.conflict_code {
            out.push_str(&format!(
                "{}{}: {}{}",
                bullet,
                match language {
                    crate::features::shared::interface::i18n::Language::ZhCn => "状态冲突",
                    crate::features::shared::interface::i18n::Language::EnUs => "Signal Conflict",
                    crate::features::shared::interface::i18n::Language::JaJp => "シグナル衝突",
                },
                conflict_code,
                boundary
            ));
            if item.recovery_watch {
                out.push_str(&format!(
                    "{}Recovery Watch: RECOVERY_WATCH{}",
                    bullet, boundary
                ));
            }
            if let Some(explanation) = &item.recovery_explanation {
                out.push_str(&format!("{}{}{}", bullet, explanation, boundary));
            }
        } else if let Some(explanation) = &item.recovery_explanation {
            out.push_str(&format!("{}{}{}", bullet, explanation, boundary));
        }
        if let Some(value) = item.relative_1d_vs_benchmark {
            out.push_str(&format!(
                "{}{day_1_label}: {value:+.2}%{}",
                bullet, boundary
            ));
        }
        if let Some(value) = item.relative_5d_vs_benchmark {
            out.push_str(&format!(
                "{}{day_5_label}: {value:+.2}%{}",
                bullet, boundary
            ));
        }
    }
    out.push_str(&format!("{}{}{}", bullet, strength.boundary, boundary));
    out
}

fn context_coverage_status(
    value: crate::features::radar::interface::presentation::SignalContextSourceStatus,
) -> &'static str {
    match value {
        crate::features::radar::interface::presentation::SignalContextSourceStatus::Healthy => {
            "HEALTHY"
        }
        crate::features::radar::interface::presentation::SignalContextSourceStatus::Partial => {
            "PARTIAL"
        }
        crate::features::radar::interface::presentation::SignalContextSourceStatus::Degraded => {
            "DEGRADED"
        }
        crate::features::radar::interface::presentation::SignalContextSourceStatus::Unavailable => {
            "UNAVAILABLE"
        }
    }
}

fn render_signal_context_coverage_markdown(
    block: &mut String,
    coverage: &crate::features::radar::interface::presentation::SignalContextCoverage,
) {
    block.push_str("  - Context Coverage:\n");
    block.push_str(&format!(
        "    - Scheduled Macro: {}\n    - Corporate: {}\n    - Geopolitical: {}\n    - Commodity: {}\n    - Rates/Credit: {}\n    - Market Structure: {}\n    - Overall: {}\n",
        context_coverage_status(coverage.scheduled_macro),
        context_coverage_status(coverage.corporate),
        context_coverage_status(coverage.geopolitical),
        context_coverage_status(coverage.commodity),
        context_coverage_status(coverage.rates_credit),
        context_coverage_status(coverage.market_structure),
        context_coverage_status(coverage.overall),
    ));
}

fn render_signal_context_coverage_html(
    block: &mut String,
    coverage: &crate::features::radar::interface::presentation::SignalContextCoverage,
) {
    block.push_str("  - Context Coverage:\n");
    block.push_str(&format!(
        "    - Scheduled Macro: {}\n    - Corporate: {}\n    - Geopolitical: {}\n    - Commodity: {}\n    - Rates/Credit: {}\n    - Market Structure: {}\n    - Overall: {}\n",
        context_coverage_status(coverage.scheduled_macro),
        context_coverage_status(coverage.corporate),
        context_coverage_status(coverage.geopolitical),
        context_coverage_status(coverage.commodity),
        context_coverage_status(coverage.rates_credit),
        context_coverage_status(coverage.market_structure),
        context_coverage_status(coverage.overall),
    ));
}

fn render_signal_context_facts_markdown(
    block: &mut String,
    layer: &crate::features::radar::interface::presentation::InterpretationLayerViewModel,
    language: Language,
) {
    let labels = match language {
        Language::ZhCn => ["生命周期", "预期", "实际", "意外值", "原因"],
        Language::EnUs => ["Lifecycle", "Expected", "Actual", "Surprise", "Reason"],
        Language::JaJp => ["ライフサイクル", "予想", "実績", "サプライズ", "理由"],
    };
    for (label, value) in labels.into_iter().zip([
        &layer.signal_context_lifecycle_value,
        &layer.signal_context_expected_value,
        &layer.signal_context_actual_value,
        &layer.signal_context_surprise_value,
        &layer.signal_context_reason_value,
    ]) {
        block.push_str(&format!("    - {label}: {value}\n"));
    }
}

fn render_interpretation_section(
    layer: Option<&crate::features::radar::interface::presentation::InterpretationLayerViewModel>,
    language: Language,
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
            if !layer.signal_context_type_label.is_empty()
                && !layer.signal_context_type_value.is_empty()
            {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.signal_context_type_label, layer.signal_context_type_value
                ));
            }
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
            render_signal_context_coverage_markdown(&mut block, &layer.signal_context_coverage);
            render_signal_context_facts_markdown(&mut block, layer, language);
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
            if !layer.signal_context_type_label.is_empty()
                && !layer.signal_context_type_value.is_empty()
            {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.signal_context_type_label, layer.signal_context_type_value
                ));
            }
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
            render_signal_context_coverage_html(&mut block, &layer.signal_context_coverage);
            render_signal_context_facts_markdown(&mut block, layer, language);
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
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.breadth_raw_label, layer.breadth_raw_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.breadth_semantic_label, layer.breadth_semantic_value
            ));
            if !layer.rs_recovery_breadth_label.is_empty() {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rs_recovery_breadth_label, layer.rs_recovery_breadth_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.strong_moderate_recovery_label, layer.strong_moderate_recovery_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rs_diffusion_label, layer.rs_diffusion_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.actionable_diffusion_label, layer.actionable_diffusion_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.diffusion_reason_label, layer.diffusion_reason_value
                ));
            }
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.breadth_score_label, layer.breadth_score_value
            ));
            block.push_str(&format!(
                "    - Tactical Leadership Structure: {}\n",
                layer.tactical_leadership_structure_value
            ));
            block.push_str(&format!(
                "    - Leader Absence Duration: {} trading days\n",
                layer.leader_absence_duration
            ));
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
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.breadth_raw_label, layer.breadth_raw_value
            ));
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.breadth_semantic_label, layer.breadth_semantic_value
            ));
            if !layer.rs_recovery_breadth_label.is_empty() {
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rs_recovery_breadth_label, layer.rs_recovery_breadth_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.strong_moderate_recovery_label, layer.strong_moderate_recovery_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.rs_diffusion_label, layer.rs_diffusion_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.actionable_diffusion_label, layer.actionable_diffusion_value
                ));
                block.push_str(&format!(
                    "    - {}: {}\n",
                    layer.diffusion_reason_label, layer.diffusion_reason_value
                ));
            }
            block.push_str(&format!(
                "    - {}: {}\n",
                layer.breadth_score_label, layer.breadth_score_value
            ));
            block.push_str(&format!(
                "    - Tactical Leadership Structure: {}\n",
                layer.tactical_leadership_structure_value
            ));
            block.push_str(&format!(
                "    - Leader Absence Duration: {} trading days\n",
                layer.leader_absence_duration
            ));
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
