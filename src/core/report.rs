use crate::config::AppConfig;
use crate::core::i18n::{get_dictionary, DisplayDictionary};
use crate::core::presentation::PresentationPacket;
use crate::core::threshold_format::format_threshold_value;
use std::collections::HashMap;

pub struct ReportResult {
    pub telegram_html_body: String,
    pub markdown_body: String,
    pub archival_markdown: String,
}

#[derive(Clone, Copy)]
enum RenderMode {
    Markdown,
    Html,
}

pub fn generate_refined_report(
    config: &AppConfig,
    pres: &PresentationPacket,
    _realized_pl: f64,
    _positions: &HashMap<String, (f64, f64)>,
    _prices: &HashMap<String, f64>,
) -> anyhow::Result<ReportResult> {
    let compact_transition_in_no_trade = config.output.compact_transition_evidence_in_no_trade;
    let rules = config.get_parsed_rules();
    let compact_stability_threshold =
        format_threshold_value(rules.trend_cohesion.gate_stability_threshold);
    let compact_continuity_threshold = rules.trend_cohesion.gate_continuity_threshold.to_string();
    let telegram_html = generate_telegram_html_report(
        pres,
        false,
        compact_transition_in_no_trade,
        &compact_stability_threshold,
        &compact_continuity_threshold,
    );
    let markdown = generate_markdown_report(
        pres,
        false,
        compact_transition_in_no_trade,
        &compact_stability_threshold,
        &compact_continuity_threshold,
    );
    let archival_markdown = generate_markdown_report(
        pres,
        true,
        compact_transition_in_no_trade,
        &compact_stability_threshold,
        &compact_continuity_threshold,
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
) -> String {
    let dict = get_dictionary(pres.language);
    let is_no_trade = pres.decision_summary.is_no_trade;
    let compact_no_trade_presentation =
        is_no_trade && compact_transition_in_no_trade && !detailed_transition;

    let mut card = String::new();
    card.push_str(&format!("## {}\n\n", dict.headers.market_summary));
    card.push_str(&format!(
        "**{}**: {} | **{}**: {}\n\n",
        dict.signals.regime_label,
        pres.macro_display.headline,
        dict.signals.bias,
        pres.macro_display.bias_label
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
    card.push_str(&format!("**{}**\n\n", d.section_title));
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
            d.action_tag_value,
            d.entry_cap_label,
            d.entry_cap_value
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
            d.action_status_value,
            d.state_tag_label,
            d.state_tag_value,
            d.action_tag_label,
            d.action_tag_value,
            d.behavior_mode_label,
            d.behavior_mode_value,
            d.exposure_label,
            d.exposure_value,
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
    if compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }

    card
}

fn generate_telegram_html_report(
    pres: &PresentationPacket,
    detailed_transition: bool,
    compact_transition_in_no_trade: bool,
    compact_stability_threshold: &str,
    compact_continuity_threshold: &str,
) -> String {
    let dict = get_dictionary(pres.language);
    let is_no_trade = pres.decision_summary.is_no_trade;
    let compact_no_trade_presentation =
        is_no_trade && compact_transition_in_no_trade && !detailed_transition;

    let mut card = String::new();
    card.push_str(&format!("<b>{}</b>\n\n", dict.headers.market_summary));
    card.push_str(&format!(
        "<b>{}</b>: {} | <b>{}</b>: {}\n\n",
        dict.signals.regime_label,
        pres.macro_display.headline,
        dict.signals.bias,
        pres.macro_display.bias_label
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
    card.push_str(&format!("<b>{}</b>\n\n", d.section_title));
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
            d.action_tag_value,
            d.entry_cap_label,
            d.entry_cap_value
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
            d.action_status_value,
            d.state_tag_label,
            d.state_tag_value,
            d.action_tag_label,
            d.action_tag_value,
            d.behavior_mode_label,
            d.behavior_mode_value,
            d.exposure_label,
            d.exposure_value,
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
    if compact_no_trade_presentation && !detailed_transition && !transition_block.is_empty() {
        card.push('\n');
        card.push_str(&transition_block);
    }

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
    let top_actions_title = if is_no_trade {
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
            if is_no_trade && vm.secondary_desc == dict.asset_states.optimal {
                format!("{} ({})", vm.secondary_desc, dict.asset_tags.candidate)
            } else {
                vm.secondary_desc.clone()
            };
        match mode {
            RenderMode::Markdown => {
                if is_no_trade {
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
                if is_no_trade {
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
        if !is_no_trade {
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
        || evidence.trend_recognition_state.is_some())
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
    let continuity_display = format_ratio_display(
        &extract_first_number(continuity.1).unwrap_or_else(|| continuity.1.to_string()),
        continuity.2,
    );
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

fn extract_first_number(input: &str) -> Option<String> {
    let mut buf = String::new();
    let mut capturing = false;
    for ch in input.chars() {
        if ch.is_ascii_digit() {
            capturing = true;
            buf.push(ch);
        } else if capturing {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}
