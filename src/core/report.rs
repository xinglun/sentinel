use crate::config::AppConfig;
use crate::core::action_matrix::AssetAction;
use crate::core::asset_state::AssetState;
use crate::core::decision::DecisionPacket;
use crate::core::market_regime::MarketState;
use anyhow::Result;
use tabled::{settings::Style, Table, Tabled};

#[derive(Tabled)]
pub struct TerminalRow {
    #[tabled(rename = "Symbol")]
    pub symbol: String,
    #[tabled(rename = "State")]
    pub state: String,
    #[tabled(rename = "Action")]
    pub action: String,
    #[tabled(rename = "Dist")]
    pub owner_dev: String,
    #[tabled(rename = "Z-Score")]
    pub strength_z: String,
}

#[derive(Tabled)]
pub struct TerminalRowHeld {
    pub symbol: String,
    pub qty: f64,
    pub avg_price: f64,
    pub current_price: f64,
    pub pl: String,
}

pub struct ReportResult {
    pub markdown_body: String,
    pub archival_markdown: String,
    #[allow(dead_code)]
    pub state_code: String,
}

pub fn generate_refined_report(
    _config: &AppConfig,
    packet: &DecisionPacket,
    realized_pl: f64,
    positions: &std::collections::HashMap<String, (f64, f64)>,
    mode: &crate::core::runtime_mode::ExecutionMode,
    failed_symbols: Vec<String>,
) -> Result<ReportResult> {
    let date_str = packet.date.format("%Y-%m-%d").to_string();

    let telegram_card = format_telegram_card(packet, mode, &failed_symbols, positions);

    let mut rows = Vec::new();
    for asset in &packet.assets {
        let emoji = match asset.asset_state.state {
            AssetState::OPTIMAL => "🔥",
            AssetState::PULLBACK => "🏹",
            AssetState::OVERHEAT => "🌋",
            AssetState::DEFEND => "🛡️",
            _ => "▫️",
        };

        rows.push(TerminalRow {
            symbol: asset.symbol.clone(),
            state: format!("{} {:?}", emoji, asset.asset_state.state),
            action: format!("{:?}", asset.action),
            owner_dev: format!("{:+.1}%", asset.deviation.unwrap_or(0.0)),
            strength_z: format!("{:.1}σ", asset.z_score.unwrap_or(0.0)),
        });
    }

    let mut table = Table::new(&rows);
    table.with(Style::modern());

    println!("\n--- 🐕 Stock Sentinel Decision Packet ({}) ---", date_str);
    println!("Headline: {}", packet.telegram.headline);
    println!("Summary:  {}", packet.telegram.summary);
    println!("-----------------------------------------------");
    println!("{}", table);

    // V1.2 Transition Summary (CLI)
    if let Some(audit) = &packet.market_regime.transition_audit {
        println!("\n🔄 Transition: {:?} -> {:?}", audit.from, audit.to);
        if audit.duration_locked {
            println!("   ⚠️ Duration Lock: Triggered (Stay in {:?})", audit.from);
        }
        if audit.is_reset_blocked {
            println!("   🚫 Reset Gate: Blocked (Step-down to {:?})", audit.to);
        }
        if audit.soft_reset_applied {
            println!("   🧠 Soft Reset: Applied (Age reduced)");
        }
        if audit.core_breakdown {
            println!("   🏚️ Core Assets: Breakdown Detected");
        }
        if audit.defensive_override {
            println!("   🛡️ Safety: Defensive Override Triggered");
        }
    }

    let mut _total_equity = 0.0;

    let mut held_rows = Vec::new();
    for (sym, (qty, avg)) in positions {
        if *qty > 0.0 {
            let current_price = packet
                .assets
                .iter()
                .find(|a| a.symbol == *sym)
                .map(|a| a.price)
                .unwrap_or(0.0);

            let open_pl = (current_price - avg) * qty;
            _total_equity += current_price * qty;

            held_rows.push(TerminalRowHeld {
                symbol: sym.clone(),
                qty: *qty,
                avg_price: *avg,
                current_price,
                pl: format!("{:+.2}", open_pl),
            });
        }
    }

    if !held_rows.is_empty() {
        println!("\n💼 Portfolio Status");
        let mut held_table = Table::new(&held_rows);
        held_table.with(Style::sharp());
        println!("{}", held_table);
        println!(" • Total Realized P/L: {:+.2}", realized_pl);
    }

    // Archival Markdown (Clean formatting for reports/YYYY-MM-DD.md)
    let mut archival_md = format!("# Sentinel Decision Report: {}\n\n", date_str);
    archival_md.push_str(&format!(
        "## 📋 Headline\n> {}\n\n",
        packet.telegram.headline
    ));
    archival_md.push_str(&format!("## 📝 Summary\n{}\n\n", packet.telegram.summary));

    archival_md.push_str("## 📈 Market & Asset Decisions\n\n");

    // V1.2 Transition Summary (Archival)
    if let Some(audit) = &packet.market_regime.transition_audit {
        archival_md.push_str("### 🔄 State Transition Audit\n");
        archival_md.push_str(&format!(
            "- **Path**: `{:?}` -> `{:?}`\n",
            audit.from, audit.to
        ));
        archival_md.push_str(&format!(
            "- **Reset**: {}\n",
            if audit.reset_gate_passed {
                "Confirmed"
            } else if audit.is_reset_blocked {
                "Blocked"
            } else {
                "N/A"
            }
        ));
        archival_md.push_str(&format!(
            "- **Duration Lock**: {}\n",
            if audit.duration_locked {
                "Yes (Blocked)"
            } else {
                "No"
            }
        ));
        archival_md.push_str(&format!(
            "- **Core Breakdown**: {}\n",
            if audit.core_breakdown { "Yes" } else { "No" }
        ));
        archival_md.push_str(&format!(
            "- **Trend Dominant**: {}\n",
            if audit.trend_dominant { "Yes" } else { "No" }
        ));
        archival_md.push_str(&format!(
            "- **Soft Reset**: {}\n",
            if audit.soft_reset_applied {
                "Applied"
            } else {
                "No"
            }
        ));
        if audit.defensive_override {
            archival_md.push_str("- **Safety**: Defensive Override Triggered\n");
        }
        archival_md.push('\n');
    }

    archival_md.push_str("| Symbol | State | Action | Deviation | Z-Score |\n");
    archival_md.push_str("|---|---|---|---|---|\n");
    for row in &rows {
        archival_md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.symbol, row.state, row.action, row.owner_dev, row.strength_z
        ));
    }
    archival_md.push_str("\n\n");

    if !held_rows.is_empty() {
        archival_md.push_str("## 💼 Portfolio Status\n\n");
        archival_md.push_str("| Symbol | Qty | Avg Price | Current Price | Open P/L |\n");
        archival_md.push_str("|---|---|---|---|---|\n");
        for h in &held_rows {
            archival_md.push_str(&format!(
                "| {} | {:.2} | {:.2} | {:.2} | {} |\n",
                h.symbol, h.qty, h.avg_price, h.current_price, h.pl
            ));
        }
        archival_md.push_str(&format!("\n**Total Realized P/L**: {:+.2}\n", realized_pl));
    }

    archival_md.push_str(&format!(
        "\n\n---\n*Generated by Sentinel Engine 2.0 at {}*",
        chrono::Local::now().to_rfc3339()
    ));

    let state_code = format!("{:?}", packet.market_regime.market_state);

    Ok(ReportResult {
        markdown_body: telegram_card,
        archival_markdown: archival_md,
        state_code,
    })
}

fn format_telegram_card(
    packet: &DecisionPacket,
    mode: &crate::core::runtime_mode::ExecutionMode,
    failed_symbols: &[String],
    positions: &std::collections::HashMap<String, (f64, f64)>,
) -> String {
    let date_str = packet.date.format("%Y-%m-%d").to_string();
    let state = packet.market_regime.market_state;
    let risk = packet.market_regime.risk_overlay;

    // 0. Categorize Assets (Unified Source of Truth)
    let buckets = categorize_assets(&packet.assets);

    let stability_fragile = packet.market_features.stability_score < 15.0;
    let is_ignition = state == MarketState::IGNITION;
    let needs_restraint = stability_fragile && is_ignition;

    // 1. Header & Strategy
    let state_str = format!("{:?}", state).to_uppercase();
    let risk_str = match risk {
        crate::core::market_regime::RiskOverlay::NORMAL => "Risk Normal",
        crate::core::market_regime::RiskOverlay::DEFENSIVE => "Risk Defensive",
        _ => "Risk Mixed",
    };

    let mut card = format!("<b>{} | {}</b>\n", state_str, risk_str);
    card.push_str(&format!("{} · {}\n\n", packet.telegram.summary, date_str));

    // 2. Exposure
    let min_exp = (packet.portfolio_policy.target_exposure_min * 100.0) as i32;
    let max_exp = (packet.portfolio_policy.target_exposure_max * 100.0) as i32;

    card.push_str(&format!("<b>仓位 {}-{}%</b>\n", min_exp, max_exp));

    if !positions.is_empty() {
        card.push_str(&format!("持仓 {} positions\n", positions.len()));
    }
    card.push('\n');

    // 3. Top Actions (Derived from Buckets)
    card.push_str("<b>🎯 Top Actions</b>\n");
    let top_assets = select_top_actions_v4(&buckets, state, needs_restraint);

    for (idx, asset) in top_assets.iter().enumerate() {
        let tag = if asset.action_changed {
            if asset.prev_action.is_none() {
                " [NEW]"
            } else {
                " [CHANGED]"
            }
        } else {
            ""
        };

        let local_action = match asset.action {
            AssetAction::ACCUMULATE => "加仓",
            AssetAction::REDUCE => "减仓",
            AssetAction::FREEZE => "冻结",
            AssetAction::AVOID => "回避",
            AssetAction::HOLD => "持有",
            AssetAction::OBSERVE => "观察",
            AssetAction::WAIT => "等待",
        };

        let state_icon = match asset.asset_state.state {
            AssetState::OPTIMAL => "◎ ",
            AssetState::PULLBACK => "↘ ",
            AssetState::FORMING => "△ ",
            AssetState::DEFEND => "! ",
            _ => "",
        };

        card.push_str(&format!(
            "{}. {}  {}  {}{:?}{}\n",
            idx + 1,
            asset.symbol,
            local_action,
            state_icon,
            asset.asset_state.state,
            tag
        ));
        let reason = telegram_reason(asset, needs_restraint);
        card.push_str(&format!("   {}\n", reason));
    }
    card.push('\n');

    // 4. Signals (Compact)
    card.push_str("<b>📡 Signals</b>\n");
    let f = &packet.market_features;
    card.push_str(&format!(
        "Confidence {:.0} ({}) · Stability {:.0} ({})\n",
        f.system_confidence,
        confidence_label(f.system_confidence),
        f.stability_score,
        stability_label(f.stability_score)
    ));

    let flow_str = f
        .flow_acceleration
        .map(|x| format!("{:+.2}", x))
        .unwrap_or_else(|| "N/A".to_string());
    let mode_str = match mode {
        crate::core::runtime_mode::ExecutionMode::Live => "Live",
        crate::core::runtime_mode::ExecutionMode::DryRun => "Dry Run",
        crate::core::runtime_mode::ExecutionMode::Disabled => "Disabled",
    };

    card.push_str(&format!(
        "Regime Age {}d · Flow {} · Execution {}\n",
        f.regime_age, flow_str, mode_str
    ));

    // 5. Data Warning
    if !failed_symbols.is_empty() {
        let warning_type = match failed_symbols.len() {
            1 => "Notice",
            2..=3 => "Warning",
            _ => "Critical",
        };
        let symbols_str = failed_symbols.join(", ");
        card.push_str(&format!(
            "\nData {}: {} fetch failed\n",
            warning_type, symbols_str
        ));
    }

    // 6. Summary Layers
    card.push_str("\n<b>🌍 市场摘要</b>\n");
    card.push_str(&format_macro_summary(packet));
    card.push('\n');

    card.push_str("<b>🧭 战术分区</b>\n");
    card.push_str(&format_tactical_summary_v4(&buckets));
    card.push('\n');

    card.push_str("<b>⚠️ 风险与机会</b>\n");
    card.push_str(&format_risk_opportunity_v4(&buckets));

    card
}

struct AssetBuckets {
    accumulate: Vec<crate::core::action_matrix::AssetActionDecision>,
    hold: Vec<crate::core::action_matrix::AssetActionDecision>,
    watch: Vec<crate::core::action_matrix::AssetActionDecision>,
    defend: Vec<crate::core::action_matrix::AssetActionDecision>,
}

fn categorize_assets(assets: &[crate::core::action_matrix::AssetActionDecision]) -> AssetBuckets {
    let mut buckets = AssetBuckets {
        accumulate: Vec::new(),
        hold: Vec::new(),
        watch: Vec::new(),
        defend: Vec::new(),
    };

    for asset in assets {
        match asset.action {
            AssetAction::ACCUMULATE => buckets.accumulate.push(asset.clone()),
            AssetAction::HOLD => buckets.hold.push(asset.clone()),
            AssetAction::OBSERVE | AssetAction::WAIT => buckets.watch.push(asset.clone()),
            AssetAction::AVOID | AssetAction::FREEZE | AssetAction::REDUCE => {
                buckets.defend.push(asset.clone())
            }
        }
    }

    // Sort within buckets for display consistency (by z-score/strength)
    let sort_fn = |a: &crate::core::action_matrix::AssetActionDecision,
                   b: &crate::core::action_matrix::AssetActionDecision| {
        let ka = (
            if a.action_changed { 1 } else { 0 },
            a.z_score.unwrap_or(0.0).abs() as i64,
        );
        let kb = (
            if b.action_changed { 1 } else { 0 },
            b.z_score.unwrap_or(0.0).abs() as i64,
        );
        kb.cmp(&ka)
    };

    buckets.accumulate.sort_by(sort_fn);
    buckets.hold.sort_by(sort_fn);
    buckets.watch.sort_by(sort_fn);
    buckets.defend.sort_by(sort_fn);

    buckets
}

fn format_macro_summary(packet: &DecisionPacket) -> String {
    let mut s = String::new();
    let cap_state = match packet.market_regime.market_state {
        MarketState::ESTABLISHED | MarketState::CONFIRMED => "Expanding",
        MarketState::DEFENSIVE => "Protect",
        _ => "Neutral",
    };
    let momentum = if packet.market_features.flow_acceleration.unwrap_or(0.0) > 0.0 {
        "Stable Uptrend"
    } else {
        "Trend Neutral"
    };

    s.push_str(&format!("• 市场状态: {}\n", cap_state));
    s.push_str(&format!("• 动量: {}\n", momentum));
    s.push_str(&format!("• 当前倾向: {}\n", packet.telegram.bias));
    s
}

fn format_tactical_summary_v4(buckets: &AssetBuckets) -> String {
    let mut s = String::new();

    if !buckets.accumulate.is_empty() {
        s.push_str(&format!(
            "• 加仓区: {}\n",
            join_symbols(
                buckets
                    .accumulate
                    .iter()
                    .map(|a| a.symbol.clone())
                    .collect()
            )
        ));
    }
    if !buckets.hold.is_empty() {
        s.push_str(&format!(
            "• 持有区: {}\n",
            join_symbols(buckets.hold.iter().map(|a| a.symbol.clone()).collect())
        ));
    }
    if !buckets.watch.is_empty() {
        s.push_str(&format!(
            "• 观察区: {}\n",
            join_symbols(buckets.watch.iter().map(|a| a.symbol.clone()).collect())
        ));
    }
    if !buckets.defend.is_empty() {
        s.push_str(&format!(
            "• 防御区: {}\n",
            join_symbols(buckets.defend.iter().map(|a| a.symbol.clone()).collect())
        ));
    }

    s
}

fn format_risk_opportunity_v4(buckets: &AssetBuckets) -> String {
    let mut s = String::new();

    let best_opp = buckets
        .accumulate
        .first()
        .map(|a| format!("{} ({:?})", a.symbol, a.asset_state.state))
        .unwrap_or_else(|| "None".to_string());

    let best_risk = buckets
        .defend
        .first()
        .map(|a| format!("{} ({:?})", a.symbol, a.asset_state.state))
        .unwrap_or_else(|| "无明显高危标的".to_string());

    s.push_str(&format!("• 机会: {}\n", best_opp));
    s.push_str(&format!("• 风险: {}\n", best_risk));
    s
}

fn join_symbols(symbols: Vec<String>) -> String {
    if symbols.is_empty() {
        return "None".to_string();
    }
    let len = symbols.len();
    if len <= 3 {
        symbols.join(" / ")
    } else {
        format!("{} +{}", symbols[..3].join(" / "), len - 3)
    }
}

fn confidence_label(val: f64) -> &'static str {
    if val >= 80.0 {
        "High"
    } else if val >= 65.0 {
        "Moderate"
    } else {
        "Low"
    }
}

fn stability_label(val: f64) -> &'static str {
    if val >= 25.0 {
        "Stable"
    } else if val >= 15.0 {
        "Mixed"
    } else {
        "Fragile"
    }
}

fn telegram_reason(
    asset: &crate::core::action_matrix::AssetActionDecision,
    is_restrained: bool,
) -> String {
    if asset.action_changed {
        return match asset.prev_action {
            Some(prev) => format!("由 {:?} -> {:?}，今日信号变化", prev, asset.action),
            None => "今日新进入关注列表".to_string(),
        };
    }
    match asset.asset_state.state {
        AssetState::PULLBACK => {
            if is_restrained && asset.action == AssetAction::ACCUMULATE {
                "适合轻仓跟踪".to_string()
            } else {
                "趋势内回撤，允许补仓".to_string()
            }
        }
        AssetState::OPTIMAL => {
            if is_restrained && asset.action == AssetAction::ACCUMULATE {
                "结构占优，但不宜追高".to_string()
            } else {
                "结构最强，适合持有".to_string()
            }
        }
        AssetState::DEFEND => "结构转弱，避免参与".to_string(),
        AssetState::OVERHEAT => "偏离过热，避免追高".to_string(),
        AssetState::CRUISE => "趋势延续，持有为主".to_string(),
        AssetState::CAUTION => "信号转弱，暂不加仓".to_string(),
        AssetState::FORMING => {
            if is_restrained && asset.action == AssetAction::ACCUMULATE {
                "仅限试探性配置".to_string()
            } else {
                "结构未完成，继续观察".to_string()
            }
        }
    }
}

fn select_top_actions_v4(
    buckets: &AssetBuckets,
    state: MarketState,
    _is_restrained: bool,
) -> Vec<crate::core::action_matrix::AssetActionDecision> {
    let mut selected = Vec::new();
    let limit = if state == MarketState::DEFENSIVE {
        4
    } else {
        3
    };

    // Sequential pick from buckets
    for asset in &buckets.accumulate {
        if selected.len() >= limit {
            break;
        }
        selected.push(asset.clone());
    }
    for asset in &buckets.hold {
        if selected.len() >= limit {
            break;
        }
        selected.push(asset.clone());
    }
    for asset in &buckets.watch {
        if selected.len() >= limit {
            break;
        }
        selected.push(asset.clone());
    }
    for asset in &buckets.defend {
        if selected.len() >= limit {
            break;
        }
        selected.push(asset.clone());
    }

    selected
}
