use crate::config::AppConfig;
use crate::core::i18n::get_dictionary;
use crate::core::presentation::PresentationPacket;
use std::collections::HashMap;

pub struct ReportResult {
    pub telegram_html_body: String,
    pub markdown_body: String,
    pub archival_markdown: String,
}

pub fn generate_refined_report(
    _config: &AppConfig,
    pres: &PresentationPacket,
    _realized_pl: f64,
    _positions: &HashMap<String, (f64, f64)>,
    _prices: &HashMap<String, f64>,
) -> anyhow::Result<ReportResult> {
    let telegram_html = generate_telegram_html_report(pres);
    let markdown = generate_markdown_report(pres);

    Ok(ReportResult {
        telegram_html_body: telegram_html,
        markdown_body: markdown.clone(),
        archival_markdown: markdown,
    })
}

fn generate_markdown_report(pres: &PresentationPacket) -> String {
    let dict = get_dictionary(pres.language);
    let is_no_trade = pres.decision_summary.is_no_trade;

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
    if !d.readiness_reasons.is_empty() {
        card.push_str(&format!("\n- {}:\n", d.readiness_reasons_label));
        for reason in &d.readiness_reasons {
            card.push_str(&format!("  - {}\n", reason));
        }
    }
    if let Some(note) = &d.candidate_only_note {
        card.push_str(&format!("\n> {}\n", note));
    }
    card.push_str("\n\n");

    card.push_str(&format!("### {}\n\n", pres.exit_summary.title));
    if pres.exit_summary.items.is_empty() {
        if let Some(note) = &pres.exit_summary.empty_note {
            for line in note.lines() {
                card.push_str(&format!("> {}\n", line));
            }
        }
    } else {
        for item in &pres.exit_summary.items {
            card.push_str(&format!("- {} · {}\n", item.symbol, item.intent_label));
            card.push_str(&format!("   {}\n", item.reason));
        }
    }
    card.push('\n');

    let top_actions_title = if d.action_status_value.contains("NO TRADE") {
        dict.decision.candidate_watchlist.clone()
    } else {
        dict.headers.top_actions.clone()
    };
    card.push_str(&format!("### {}\n\n", top_actions_title));
    for (i, vm) in pres.top_actions.iter().enumerate() {
        if is_no_trade {
            card.push_str(&format!("- {} · {}\n", vm.symbol, vm.secondary_desc));
        } else {
            card.push_str(&format!(
                "{}. {} **{}** - {}\n",
                i + 1,
                vm.indicator,
                vm.symbol,
                vm.primary_label
            ));
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
            card.push_str(&format!("   <i>{}</i>\n", row2_parts.join(" · ")));
        }
    }

    card.push('\n');
    card.push_str(&format!("**{}**\n\n", dict.headers.monitoring_signals));
    let s = &pres.signal_summary;
    card.push_str(&format!(
        "- **{}**: {}\n- **{}**: {}\n",
        s.participation_label, s.participation_value, s.stability_label, s.stability_value,
    ));
    card.push_str(&format!(
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

    if !pres.tactical_buckets.is_empty() {
        card.push_str(&format!("\n**{}**\n\n", dict.headers.tactical_buckets));
        for bucket in &pres.tactical_buckets {
            card.push_str(&format!(
                "- **{} ({})**: {}\n",
                bucket.display_name,
                bucket.count,
                bucket.items.join(" / ")
            ));
        }
    }

    card.push_str(&format!("\n**{}**\n\n", dict.headers.risks_opportunities));
    if is_no_trade {
        card.push_str(&format!(
            "> **{}**: {}\n> **{}**: {}\n",
            pres.risk_opportunity_summary.opportunity_label,
            pres.risk_opportunity_summary.opportunity_value,
            pres.risk_opportunity_summary.risk_label,
            pres.risk_opportunity_summary.risk_value
        ));
    } else {
        card.push_str(&format!(
            "- **{}**: {}\n- **{}**: {}\n",
            pres.risk_opportunity_summary.opportunity_label,
            pres.risk_opportunity_summary.opportunity_value,
            pres.risk_opportunity_summary.risk_label,
            pres.risk_opportunity_summary.risk_value
        ));
    }
    if !pres.risk_opportunities.is_empty() && !is_no_trade {
        for item in &pres.risk_opportunities {
            card.push_str(&format!(
                "- **{}**: {} · {}\n",
                item.kind, item.symbol, item.reason
            ));
        }
    }

    if !pres.notices.is_empty() {
        card.push('\n');
        for notice in &pres.notices {
            card.push_str(&format!("{}\n", notice));
        }
    }

    if let Some(alert) = &pres.data_alert {
        card.push_str(&format!(
            "\n{} {}: {} ({})\n",
            alert.prefix,
            alert.label,
            alert.message,
            alert.symbols.join(", ")
        ));
    }

    card
}

fn generate_telegram_html_report(pres: &PresentationPacket) -> String {
    let dict = get_dictionary(pres.language);
    let is_no_trade = pres.decision_summary.is_no_trade;

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
    if !d.readiness_reasons.is_empty() {
        card.push_str(&format!("\n• {}:\n", d.readiness_reasons_label));
        for reason in &d.readiness_reasons {
            card.push_str(&format!("  • {}\n", reason));
        }
    }
    if let Some(note) = &d.candidate_only_note {
        card.push_str(&format!("\n<i>{}</i>\n", note));
    }
    card.push('\n');

    card.push_str(&format!("<b>{}</b>\n\n", pres.exit_summary.title));
    if pres.exit_summary.items.is_empty() {
        if let Some(note) = &pres.exit_summary.empty_note {
            for line in note.lines() {
                card.push_str(&format!("{}\n", line));
            }
        }
    } else {
        for item in &pres.exit_summary.items {
            card.push_str(&format!("• {} · {}\n", item.symbol, item.intent_label));
            card.push_str(&format!("  {}\n", item.reason));
        }
    }
    card.push('\n');

    let top_actions_title = if d.action_status_value.contains("NO TRADE") {
        dict.decision.candidate_watchlist.clone()
    } else {
        dict.headers.top_actions.clone()
    };
    card.push_str(&format!("<b>{}</b>\n\n", top_actions_title));
    for (i, vm) in pres.top_actions.iter().enumerate() {
        if is_no_trade {
            card.push_str(&format!("• {} · {}\n", vm.symbol, vm.secondary_desc));
        } else {
            card.push_str(&format!(
                "{}. {} <b>{}</b> - {}\n",
                i + 1,
                vm.indicator,
                vm.symbol,
                vm.primary_label
            ));
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
            card.push_str(&format!("  <i>{}</i>\n", row2_parts.join(" · ")));
        }
    }

    card.push('\n');
    card.push_str(&format!("<b>{}</b>\n\n", dict.headers.monitoring_signals));
    let s = &pres.signal_summary;
    card.push_str(&format!(
        "• <b>{}</b>: {}\n• <b>{}</b>: {}\n",
        s.participation_label, s.participation_value, s.stability_label, s.stability_value,
    ));
    card.push_str(&format!(
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

    if !pres.tactical_buckets.is_empty() {
        card.push_str(&format!("\n<b>{}</b>\n\n", dict.headers.tactical_buckets));
        for bucket in &pres.tactical_buckets {
            card.push_str(&format!(
                "• <b>{} ({})</b>: {}\n",
                bucket.display_name,
                bucket.count,
                bucket.items.join(" / ")
            ));
        }
    }

    card.push_str(&format!(
        "\n<b>{}</b>\n\n",
        dict.headers.risks_opportunities
    ));
    if is_no_trade {
        card.push_str(&format!(
            "<i><b>{}</b>: {}\n<b>{}</b>: {}</i>\n",
            pres.risk_opportunity_summary.opportunity_label,
            pres.risk_opportunity_summary.opportunity_value,
            pres.risk_opportunity_summary.risk_label,
            pres.risk_opportunity_summary.risk_value
        ));
    } else {
        card.push_str(&format!(
            "• <b>{}</b>: {}\n• <b>{}</b>: {}\n",
            pres.risk_opportunity_summary.opportunity_label,
            pres.risk_opportunity_summary.opportunity_value,
            pres.risk_opportunity_summary.risk_label,
            pres.risk_opportunity_summary.risk_value
        ));
    }
    if !pres.risk_opportunities.is_empty() && !is_no_trade {
        for item in &pres.risk_opportunities {
            card.push_str(&format!(
                "• <b>{}</b>: {} · {}\n",
                item.kind, item.symbol, item.reason
            ));
        }
    }

    if !pres.notices.is_empty() {
        card.push('\n');
        for notice in &pres.notices {
            card.push_str(&format!("{}\n", notice));
        }
    }

    if let Some(alert) = &pres.data_alert {
        card.push_str(&format!(
            "\n{} {}: {} ({})\n",
            alert.prefix,
            alert.label,
            alert.message,
            alert.symbols.join(", ")
        ));
    }

    card
}
