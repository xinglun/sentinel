use crate::config::AppConfig;
use crate::core::i18n::get_dictionary;
use crate::core::presentation::PresentationPacket;
use std::collections::HashMap;

pub struct ReportResult {
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
        card.push_str(&format!("> {}\n\n", d.summary));
        card.push_str(&format!(
            "> {}\n> {} · {}\n> {} · {}\n",
            d.behavior_mode_value,
            d.exposure_label,
            d.exposure_value,
            d.market_board_label,
            d.market_board_value
        ));
        card.push_str(&format!(
            "\n> **{}**: {}\n> **{}**: {}\n",
            d.opportunity_snapshot_label,
            d.opportunity_snapshot_value,
            d.risk_snapshot_label,
            d.risk_snapshot_value
        ));
    } else {
        card.push_str(&format!(
            "- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n",
            d.action_status_label,
            d.action_status_value,
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
        card.push_str(&format!("\n> {}\n", d.summary));
    }
    if !d.readiness_reasons.is_empty() {
        card.push_str(&format!("\n- **{}**:\n", d.readiness_reasons_label));
        for reason in &d.readiness_reasons {
            card.push_str(&format!("  - {}\n", reason));
        }
    }
    if let Some(note) = &d.candidate_only_note {
        card.push_str(&format!("\n> {}\n", note));
    }
    card.push_str("\n\n");

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

    card.push_str("\n---\n");

    Ok(ReportResult {
        markdown_body: card.clone(),
        archival_markdown: card,
    })
}
