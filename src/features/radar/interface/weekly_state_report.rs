use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;
use std::collections::BTreeMap;

use crate::features::shared::interface::i18n::Language;

#[derive(Clone)]
pub(crate) struct WeeklyReportContext {
    pub macro_gravity: Option<WeeklyMacroGravityContext>,
    pub research_attention_entries: usize,
    pub asset_thesis_entries: usize,
    pub capital_absorption_ipo_queue: serde_json::Value,
    pub capital_dynamics_flow_layer: serde_json::Value,
    pub expectation_layer: serde_json::Value,
}

#[derive(Clone)]
pub(crate) struct WeeklyMacroGravityContext {
    pub rate_pressure: String,
    pub real_yield_pressure: String,
    pub yield_curve: String,
    pub credit_stress: String,
    pub liquidity: String,
    pub growth_valuation_impact: String,
}

pub(crate) fn persist_weekly_state_outputs(
    save_dir: &std::path::Path,
    history: &[crate::features::radar::domain::decision::DecisionPacket],
    current_packet: &crate::features::radar::domain::decision::DecisionPacket,
    include_current_packet: bool,
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    context: &WeeklyReportContext,
    current_state_machine: Option<
        &crate::features::shared::application::run_status::StateMachineSummary,
    >,
) -> Result<()> {
    let mut recent_packets: Vec<&crate::features::radar::domain::decision::DecisionPacket> =
        history.iter().rev().take(7).collect();
    recent_packets.reverse();
    if include_current_packet {
        recent_packets.push(current_packet);
    }
    if recent_packets.len() > 7 {
        recent_packets = recent_packets[recent_packets.len() - 7..].to_vec();
    }

    let mut market_state_counts = BTreeMap::<String, usize>::new();
    let mut risk_overlay_counts = BTreeMap::<String, usize>::new();
    let mut total_confidence = 0.0;
    let mut total_stability = 0.0;
    let mut trend_cohesion_ready_days = 0usize;

    for packet in &recent_packets {
        *market_state_counts
            .entry(format!("{:?}", packet.market_regime.market_state))
            .or_insert(0) += 1;
        *risk_overlay_counts
            .entry(format!("{:?}", packet.market_regime.risk_overlay))
            .or_insert(0) += 1;
        total_confidence += packet.market_features.system_confidence;
        total_stability += packet.market_features.stability_score;
        if packet.trend_cohesion.gate_passed {
            trend_cohesion_ready_days += 1;
        }
    }

    let day_count = recent_packets.len();
    let avg_confidence = if day_count > 0 {
        total_confidence / day_count as f64
    } else {
        0.0
    };
    let avg_stability = if day_count > 0 {
        total_stability / day_count as f64
    } else {
        0.0
    };
    let latest_context = build_weekly_latest_context(
        pres_packet,
        context,
        &context.capital_absorption_ipo_queue,
        &context.capital_dynamics_flow_layer,
        &context.expectation_layer,
    );
    let state_machine_summaries =
        load_weekly_state_machine_summaries(save_dir, current_packet.date, current_state_machine);
    let weekly_totals = build_weekly_totals(&state_machine_summaries);
    let daily_summaries = build_daily_summaries(&state_machine_summaries);

    let metrics = json!({
        "generated_at": chrono::Local::now().to_rfc3339(),
        "as_of_date": pres_packet.date_str,
        "days_analyzed": day_count,
        "include_current_packet": include_current_packet,
        "data_status": if include_current_packet { "OK" } else { "DATA_UNAVAILABLE" },
        "latest_market_state": format!("{:?}", current_packet.market_regime.market_state),
        "latest_risk_overlay": format!("{:?}", current_packet.market_regime.risk_overlay),
        "avg_confidence": avg_confidence,
        "avg_stability": avg_stability,
        "trend_cohesion_ready_days": trend_cohesion_ready_days,
        // 互換キーとして `participation_ready_days` を残すが、意味は `trend_cohesion_ready_days` です。
        // downstream script は従来の participation semantics ではなく、cohesion gate semantics を受け取ります。
        "participation_ready_days": trend_cohesion_ready_days,
        "market_state_counts": market_state_counts,
        "risk_overlay_counts": risk_overlay_counts,
        "weekly_totals": weekly_totals,
        "daily_summaries": daily_summaries,
        "latest_context": latest_context,
    });

    std::fs::write(
        save_dir.join("weekly_state_metrics.json"),
        serde_json::to_string_pretty(&metrics)?,
    )?;

    let text = weekly_text(pres_packet.language);
    let mut review = String::new();
    review.push_str(text.title);
    review.push_str("\n\n");
    review.push_str(&format!("- {}: {}\n", text.as_of, pres_packet.date_str));
    review.push_str(&format!(
        "- {}: {}\n",
        text.status,
        if include_current_packet {
            text.status_using_current
        } else {
            text.status_data_unavailable
        }
    ));
    review.push_str(&format!(
        "- {}: {} | {}\n",
        text.latest_headline,
        pres_packet.macro_display.headline,
        pres_packet.macro_display.bias_label
    ));
    review.push_str(&format!("- {}: {}\n", text.days_analyzed, day_count));
    review.push_str(&format!(
        "- {}: {:.1}\n",
        text.avg_confidence, avg_confidence
    ));
    review.push_str(&format!("- {}: {:.1}\n", text.avg_stability, avg_stability));
    review.push_str(&format!(
        "- {}: {}\n\n",
        text.trend_cohesion_ready_days, trend_cohesion_ready_days
    ));
    review.push_str(text.market_state_counts);
    review.push('\n');
    for (state, count) in metrics["market_state_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }
    review.push('\n');
    review.push_str(text.risk_overlay_counts);
    review.push('\n');
    for (state, count) in metrics["risk_overlay_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }
    push_weekly_state_machine_totals(
        &mut review,
        &weekly_totals,
        &metrics["daily_summaries"],
        text,
    );
    push_weekly_strategic_context_snapshot(&mut review, pres_packet, text);
    push_weekly_macro_gravity_snapshot(&mut review, context, text);
    push_weekly_capital_dynamics_snapshot(
        &mut review,
        &context.capital_absorption_ipo_queue,
        &context.capital_dynamics_flow_layer,
        text,
    );
    push_weekly_cognitive_calibration_snapshot(&mut review, context, text);
    push_weekly_expectation_snapshot(&mut review, &context.expectation_layer, text);

    std::fs::write(save_dir.join("weekly_state_review_auto.md"), review)?;
    Ok(())
}

#[derive(Clone)]
struct WeeklyStateMachineEntry {
    date: NaiveDate,
    summary: crate::features::shared::application::run_status::StateMachineSummary,
}

fn load_weekly_state_machine_summaries(
    save_dir: &std::path::Path,
    current_date: NaiveDate,
    current_state_machine: Option<
        &crate::features::shared::application::run_status::StateMachineSummary,
    >,
) -> Vec<WeeklyStateMachineEntry> {
    let mut entries = std::fs::read_dir(save_dir)
        .ok()
        .into_iter()
        .flat_map(|read_dir| read_dir.filter_map(std::result::Result::ok))
        .filter_map(|entry| load_state_machine_summary_from_run_status(&entry.path()))
        .filter(|(date, _)| *date <= current_date)
        .collect::<BTreeMap<_, _>>();

    if let Some(summary) = current_state_machine {
        entries.insert(current_date, summary.clone());
    }

    let mut recent = entries
        .into_iter()
        .map(|(date, summary)| WeeklyStateMachineEntry { date, summary })
        .collect::<Vec<_>>();
    if recent.len() > 7 {
        recent = recent.split_off(recent.len() - 7);
    }
    recent
}

fn load_state_machine_summary_from_run_status(
    path: &std::path::Path,
) -> Option<(
    NaiveDate,
    crate::features::shared::application::run_status::StateMachineSummary,
)> {
    let file_name = path.file_name()?.to_str()?;
    let raw_date = file_name
        .strip_prefix("run_status_")?
        .strip_suffix(".json")?;
    let date = NaiveDate::parse_from_str(raw_date, "%Y-%m-%d").ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let summary = value.get("state_machine")?;
    serde_json::from_value(summary.clone())
        .ok()
        .map(|summary| (date, summary))
}

fn build_weekly_totals(entries: &[WeeklyStateMachineEntry]) -> serde_json::Value {
    let mut reset_confirmed_total = 0usize;
    let mut reset_blocked_total = 0usize;
    let mut soft_reset_total = 0usize;
    let mut duration_lock_total = 0usize;
    let mut defensive_override_total = 0usize;
    let mut core_breakdown_total = 0usize;
    let mut reconciliation_mismatch_total = 0usize;
    let mut preflight_failed_total = 0usize;

    for entry in entries {
        let summary = &entry.summary;
        reset_confirmed_total += usize::from(summary.reset_confirmed);
        reset_blocked_total += usize::from(summary.reset_blocked);
        soft_reset_total += usize::from(summary.soft_reset_applied);
        duration_lock_total += usize::from(summary.duration_locked);
        defensive_override_total += usize::from(summary.defensive_override);
        core_breakdown_total += usize::from(summary.core_breakdown);
        reconciliation_mismatch_total += summary.reconciliation_mismatch_count;
        preflight_failed_total += usize::from(summary.preflight_failed);
    }

    json!({
        "days": entries.len(),
        "reset_confirmed_total": reset_confirmed_total,
        "reset_blocked_total": reset_blocked_total,
        "soft_reset_total": soft_reset_total,
        "duration_lock_total": duration_lock_total,
        "defensive_override_total": defensive_override_total,
        "core_breakdown_total": core_breakdown_total,
        "reconciliation_mismatch_total": reconciliation_mismatch_total,
        "preflight_failed_total": preflight_failed_total
    })
}

fn build_daily_summaries(entries: &[WeeklyStateMachineEntry]) -> serde_json::Value {
    json!(entries
        .iter()
        .map(|entry| {
            let summary = &entry.summary;
            json!({
                "date": entry.date.to_string(),
                "from_state": &summary.from_state,
                "to_state": &summary.to_state,
                "reset_confirmed": summary.reset_confirmed,
                "reset_blocked": summary.reset_blocked,
                "soft_reset_applied": summary.soft_reset_applied,
                "duration_locked": summary.duration_locked,
                "defensive_override": summary.defensive_override,
                "core_breakdown": summary.core_breakdown,
                "reconciliation_mismatch_count": summary.reconciliation_mismatch_count,
                "preflight_failed": summary.preflight_failed
            })
        })
        .collect::<Vec<_>>())
}

fn build_weekly_latest_context(
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    context: &WeeklyReportContext,
    capital_absorption_ipo_queue: &serde_json::Value,
    capital_dynamics_flow_layer: &serde_json::Value,
    expectation_layer: &serde_json::Value,
) -> serde_json::Value {
    let trend_breadth_mode = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| format!("{:?}", evidence.trend_breadth_mode));
    let market_cycle_position = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| format!("{:?}", evidence.market_cycle_position));
    let holding_efficiency = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| format!("{:?}", evidence.holding_efficiency));
    let strategic_context = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| evidence.strategic_context.clone())
        .unwrap_or_default();

    json!({
        "trend_breadth_mode": trend_breadth_mode,
        "market_cycle_position": market_cycle_position,
        "holding_efficiency": holding_efficiency,
        "strategic_context": strategic_context,
        "macro_gravity": build_weekly_macro_gravity_context(context),
        "capital_absorption_ipo_queue": capital_absorption_ipo_queue,
        "capital_dynamics": {
            "supply_layer": capital_absorption_ipo_queue,
            "flow_layer": capital_dynamics_flow_layer
        },
        "expectation_layer": expectation_layer,
        "cognitive_calibration": {
            "research_attention_entries": context.research_attention_entries,
            "asset_thesis_entries": context.asset_thesis_entries
        }
    })
}

fn build_weekly_macro_gravity_context(context: &WeeklyReportContext) -> serde_json::Value {
    let Some(macro_gravity) = context.macro_gravity.as_ref() else {
        return json!({
            "configured": false
        });
    };

    json!({
        "configured": true,
        "rate_pressure": macro_gravity.rate_pressure,
        "real_yield_pressure": macro_gravity.real_yield_pressure,
        "yield_curve": macro_gravity.yield_curve,
        "credit_stress": macro_gravity.credit_stress,
        "liquidity": macro_gravity.liquidity,
        "growth_valuation_impact": macro_gravity.growth_valuation_impact
    })
}

struct WeeklyText {
    title: &'static str,
    as_of: &'static str,
    status: &'static str,
    status_using_current: &'static str,
    status_data_unavailable: &'static str,
    latest_headline: &'static str,
    days_analyzed: &'static str,
    avg_confidence: &'static str,
    avg_stability: &'static str,
    trend_cohesion_ready_days: &'static str,
    market_state_counts: &'static str,
    risk_overlay_counts: &'static str,
    state_machine_totals: &'static str,
    state_summary_days: &'static str,
    reset_confirmed_blocked: &'static str,
    soft_reset_duration_lock_defensive_override: &'static str,
    core_breakdown_reconciliation_mismatch: &'static str,
    daily_state_timeline: &'static str,
    no_state_machine_summaries: &'static str,
    strategic_context_snapshot: &'static str,
    trend_breadth_mode: &'static str,
    market_cycle_position: &'static str,
    holding_efficiency: &'static str,
    strategic_context_lines: &'static str,
    strategic_context_none: &'static str,
    macro_gravity_snapshot: &'static str,
    macro_gravity_not_configured: &'static str,
    rate_pressure: &'static str,
    real_yield: &'static str,
    yield_curve: &'static str,
    credit_stress: &'static str,
    liquidity: &'static str,
    growth_valuation: &'static str,
    capital_dynamics_snapshot: &'static str,
    boundary_capital_dynamics: &'static str,
    capital_absorption_ipo_queue_snapshot: &'static str,
    capital_absorption_ipo_queue_not_configured: &'static str,
    capital_absorption_latest_date: &'static str,
    capital_absorption_near_term_latest: &'static str,
    capital_absorption_queue_latest: &'static str,
    capital_absorption_queue_min_max_7d: &'static str,
    capital_absorption_reported_confirmed: &'static str,
    capital_absorption_pressure: &'static str,
    boundary_capital_absorption: &'static str,
    flow_layer_snapshot: &'static str,
    flow_layer_not_configured: &'static str,
    flow_layer_latest_date: &'static str,
    flow_layer_observation_divergence: &'static str,
    flow_layer_positive_negative_divergence: &'static str,
    flow_layer_breadth: &'static str,
    flow_layer_market_breadth: &'static str,
    flow_layer_sector_breadth: &'static str,
    flow_layer_watchlist_breadth: &'static str,
    flow_layer_core_holding_breadth: &'static str,
    boundary_flow_layer: &'static str,
    cognitive_calibration_snapshot: &'static str,
    research_attention_entries: &'static str,
    asset_thesis_entries: &'static str,
    boundary_snapshot_only: &'static str,
    boundary_audit_facts: &'static str,
    boundary_macro: &'static str,
    boundary_macro_not_configured: &'static str,
    boundary_cognitive: &'static str,
    expectation_layer_snapshot: &'static str,
    expectation_layer_as_of: &'static str,
    expectation_layer_decision_weight: &'static str,
    expectation_layer_trade_signal: &'static str,
    expectation_layer_observation_count: &'static str,
    expectation_layer_subjects: &'static str,
    expectation_layer_boundary: &'static str,
}

fn weekly_text(language: Language) -> &'static WeeklyText {
    match language {
        Language::ZhCn => &WEEKLY_TEXT_ZH,
        Language::EnUs => &WEEKLY_TEXT_EN,
        Language::JaJp => &WEEKLY_TEXT_JA,
    }
}

static WEEKLY_TEXT_ZH: WeeklyText = WeeklyText {
    title: "# 周度状态复盘（自动草稿）",
    as_of: "截至",
    status: "状态",
    status_using_current: "使用当前市场判断",
    status_data_unavailable: "数据不可用；仅基于已保存历史",
    latest_headline: "最新摘要",
    days_analyzed: "分析天数",
    avg_confidence: "平均置信度",
    avg_stability: "平均稳定度",
    trend_cohesion_ready_days: "趋势凝聚 ready 天数",
    market_state_counts: "## 市场状态计数",
    risk_overlay_counts: "## 风险覆盖计数",
    state_machine_totals: "## 状态机周度汇总",
    state_summary_days: "有状态摘要的天数",
    reset_confirmed_blocked: "重置确认 / 阻止",
    soft_reset_duration_lock_defensive_override: "软重置 / duration lock / 防御覆盖",
    core_breakdown_reconciliation_mismatch: "核心破坏 / 对账不一致",
    daily_state_timeline: "## 日度状态机时间线",
    no_state_machine_summaries: "没有可用的状态机摘要。",
    strategic_context_snapshot: "## 战略上下文快照",
    trend_breadth_mode: "趋势广度模式",
    market_cycle_position: "市场周期位置",
    holding_efficiency: "持仓效率",
    strategic_context_lines: "战略上下文行",
    strategic_context_none: "无",
    macro_gravity_snapshot: "## 宏观引力快照",
    macro_gravity_not_configured: "宏观引力未配置",
    rate_pressure: "利率压力",
    real_yield: "实际收益率",
    yield_curve: "收益率曲线",
    credit_stress: "信用压力",
    liquidity: "流动性",
    growth_valuation: "成长估值",
    capital_dynamics_snapshot: "## Capital Dynamics（供需观察）",
    boundary_capital_dynamics:
        "边界: Capital Dynamics 仅作 Observation shell，Current decision weight 为 0%，不接入 Gate、Execution、Trader、Action Matrix 或 Position Sizing。",
    capital_absorption_ipo_queue_snapshot: "### 6.1 Supply Layer（Capital Absorption）",
    capital_absorption_ipo_queue_not_configured: "资金吸收 IPO 队列未保存",
    capital_absorption_latest_date: "最新观测日",
    capital_absorption_near_term_latest: "最新 Near-Term Supply 数量",
    capital_absorption_queue_latest: "最新 Future Queue 数量",
    capital_absorption_queue_min_max_7d: "7 日 Future Queue 最小值 / 最大值",
    capital_absorption_reported_confirmed: "已报道 / 已确认",
    capital_absorption_pressure: "潜在供给压力",
    boundary_capital_absorption: "边界: 仅为潜在未来供给观察；不生成市场结论、风险升级或交易信号。",
    flow_layer_snapshot: "### 6.2 Demand Layer（Flow Layer）",
    flow_layer_not_configured: "Flow Layer 未配置",
    flow_layer_latest_date: "最新 Flow 观察日",
    flow_layer_observation_divergence: "Observation / Divergence",
    flow_layer_positive_negative_divergence: "正向 / 负向背离",
    flow_layer_breadth: "Flow Breadth",
    flow_layer_market_breadth: "Market Breadth",
    flow_layer_sector_breadth: "Sector Breadth",
    flow_layer_watchlist_breadth: "Watchlist Breadth",
    flow_layer_core_holding_breadth: "Core Holding Breadth",
    boundary_flow_layer: "边界: Flow Layer 仅作 Observation Only 观察，decision weight 固定为 0%，不覆盖 Trend Layer，也不生成交易信号。",
    cognitive_calibration_snapshot: "## 认知校准快照",
    research_attention_entries: "研究关注条目",
    asset_thesis_entries: "资产命题条目",
    boundary_snapshot_only: "边界: 仅为快照；不生成评分、建议或交易判断。",
    boundary_audit_facts: "边界: 仅为审计事实；不生成评分、建议或交易判断。",
    boundary_macro: "边界: 仅说明贴现率与流动性上下文；不作为 Gate 输入或交易指令。",
    boundary_macro_not_configured: "边界: 宏观引力仅解释贴现率与流动性上下文。",
    boundary_cognitive: "边界: 认知校准只管理注意力和命题复核；不生成交易信号。",
    expectation_layer_snapshot: "## Expectation Layer（市场预期观测）",
    expectation_layer_as_of: "观测日",
    expectation_layer_decision_weight: "decision_weight",
    expectation_layer_trade_signal: "trade_signal",
    expectation_layer_observation_count: "observation_count",
    expectation_layer_subjects: "subjects",
    expectation_layer_boundary:
        "边界: Expectation Layer 仅用于观测市场预期，不进入 Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing，也不生成交易信号。",
};

static WEEKLY_TEXT_EN: WeeklyText = WeeklyText {
    title: "# Weekly State Review (Auto)",
    as_of: "As of",
    status: "Status",
    status_using_current: "using current market decision",
    status_data_unavailable: "data unavailable; based on prior persisted history only",
    latest_headline: "Latest headline",
    days_analyzed: "Days analyzed",
    avg_confidence: "Avg confidence",
    avg_stability: "Avg stability",
    trend_cohesion_ready_days: "Trend cohesion ready days",
    market_state_counts: "## Market State Counts",
    risk_overlay_counts: "## Risk Overlay Counts",
    state_machine_totals: "## State Machine Weekly Totals",
    state_summary_days: "Days with state summary",
    reset_confirmed_blocked: "Reset confirmed / blocked",
    soft_reset_duration_lock_defensive_override: "Soft reset / duration lock / defensive override",
    core_breakdown_reconciliation_mismatch: "Core breakdown / reconciliation mismatch",
    daily_state_timeline: "## Daily State Machine Timeline",
    no_state_machine_summaries: "No state machine summaries available.",
    strategic_context_snapshot: "## Strategic Context Snapshot",
    trend_breadth_mode: "Trend breadth mode",
    market_cycle_position: "Market cycle position",
    holding_efficiency: "Holding efficiency",
    strategic_context_lines: "Strategic context lines",
    strategic_context_none: "none",
    macro_gravity_snapshot: "## Macro Gravity Snapshot",
    macro_gravity_not_configured: "Macro gravity: not configured",
    rate_pressure: "Rate pressure",
    real_yield: "Real yield",
    yield_curve: "Yield curve",
    credit_stress: "Credit stress",
    liquidity: "Liquidity",
    growth_valuation: "Growth valuation",
    capital_dynamics_snapshot: "## Capital Dynamics (Supply / Demand Observation)",
    boundary_capital_dynamics:
        "Boundary: Capital Dynamics is an observation shell only. Current decision weight remains 0%, and it does not connect to Gate, Execution, Trader, Action Matrix, or Position Sizing.",
    capital_absorption_ipo_queue_snapshot: "### 6.1 Supply Layer (Capital Absorption)",
    capital_absorption_ipo_queue_not_configured: "Capital absorption IPO queue: not persisted",
    capital_absorption_latest_date: "Latest observation date",
    capital_absorption_near_term_latest: "Latest Near-Term Supply Count",
    capital_absorption_queue_latest: "Latest Future Queue Count",
    capital_absorption_queue_min_max_7d: "7D Future Queue min / max",
    capital_absorption_reported_confirmed: "Reported / Confirmed",
    capital_absorption_pressure: "Potential supply pressure",
    boundary_capital_absorption:
        "Boundary: potential future supply observation only; no market conclusion, risk upgrade, or trade signal.",
    flow_layer_snapshot: "### 6.2 Demand Layer (Flow Layer)",
    flow_layer_not_configured: "Flow Layer: not configured",
    flow_layer_latest_date: "Latest Flow observation date",
    flow_layer_observation_divergence: "Observations / Divergences",
    flow_layer_positive_negative_divergence: "Positive / Negative divergence",
    flow_layer_breadth: "Flow Breadth",
    flow_layer_market_breadth: "Market Breadth",
    flow_layer_sector_breadth: "Sector Breadth",
    flow_layer_watchlist_breadth: "Watchlist Breadth",
    flow_layer_core_holding_breadth: "Core Holding Breadth",
    boundary_flow_layer:
        "Boundary: Flow Layer is Observation Only. Decision weight remains 0%, it does not override Trend Layer, and it does not generate trade signals.",
    cognitive_calibration_snapshot: "## Cognitive Calibration Snapshot",
    research_attention_entries: "Research attention entries",
    asset_thesis_entries: "Asset thesis entries",
    boundary_snapshot_only: "Boundary: snapshot only; no score, advice, or trade decision.",
    boundary_audit_facts: "Boundary: audit facts only; no score, advice, or trade decision.",
    boundary_macro: "Boundary: context only; no Gate input or trade instruction.",
    boundary_macro_not_configured: "Boundary: macro gravity explains discount-rate and liquidity context only.",
    boundary_cognitive: "Boundary: cognitive calibration manages attention and thesis review only; it does not generate trade signals.",
    expectation_layer_snapshot: "## Expectation Layer (Market Expectation Observation)",
    expectation_layer_as_of: "As of",
    expectation_layer_decision_weight: "decision_weight",
    expectation_layer_trade_signal: "trade_signal",
    expectation_layer_observation_count: "observation_count",
    expectation_layer_subjects: "subjects",
    expectation_layer_boundary:
        "Boundary: Expectation Layer is for observing market expectations only. It does not enter Gate, Execution, Trader, Action Matrix, READY / EXECUTE, or Position Sizing, and it does not generate trade signals.",
};

static WEEKLY_TEXT_JA: WeeklyText = WeeklyText {
    title: "# 週次状態レビュー（自動下書き）",
    as_of: "基準日",
    status: "状態",
    status_using_current: "現在の市場判断を使用",
    status_data_unavailable: "データ利用不可。保存済み履歴のみを使用",
    latest_headline: "最新ヘッドライン",
    days_analyzed: "分析日数",
    avg_confidence: "平均確信度",
    avg_stability: "平均安定度",
    trend_cohesion_ready_days: "トレンド凝集 ready 日数",
    market_state_counts: "## 市場状態カウント",
    risk_overlay_counts: "## リスクオーバーレイカウント",
    state_machine_totals: "## 状態機械の週次集計",
    state_summary_days: "状態サマリーがある日数",
    reset_confirmed_blocked: "リセット確認 / ブロック",
    soft_reset_duration_lock_defensive_override: "ソフトリセット / duration lock / 防御 override",
    core_breakdown_reconciliation_mismatch: "core breakdown / reconciliation mismatch",
    daily_state_timeline: "## 日次状態機械タイムライン",
    no_state_machine_summaries: "利用可能な状態機械サマリーはありません。",
    strategic_context_snapshot: "## 戦略コンテキストスナップショット",
    trend_breadth_mode: "トレンド幅モード",
    market_cycle_position: "市場サイクル位置",
    holding_efficiency: "保有効率",
    strategic_context_lines: "戦略コンテキスト行",
    strategic_context_none: "なし",
    macro_gravity_snapshot: "## マクログラビティスナップショット",
    macro_gravity_not_configured: "マクログラビティ未設定",
    rate_pressure: "金利圧力",
    real_yield: "実質利回り",
    yield_curve: "イールドカーブ",
    credit_stress: "信用ストレス",
    liquidity: "流動性",
    growth_valuation: "成長評価",
    capital_dynamics_snapshot: "## Capital Dynamics（需給観測）",
    boundary_capital_dynamics:
        "境界: Capital Dynamics は Observation shell のみであり、Current decision weight は 0% に固定され、Gate、Execution、Trader、Action Matrix、Position Sizing へ接続しない。",
    capital_absorption_ipo_queue_snapshot: "### 6.1 Supply Layer（Capital Absorption）",
    capital_absorption_ipo_queue_not_configured: "資金吸収 IPO キューは未保存",
    capital_absorption_latest_date: "最新観測日",
    capital_absorption_near_term_latest: "最新 Near-Term Supply 数",
    capital_absorption_queue_latest: "最新 Future Queue 数",
    capital_absorption_queue_min_max_7d: "7 日 Future Queue 最小 / 最大",
    capital_absorption_reported_confirmed: "報道済み / 確認済み",
    capital_absorption_pressure: "潜在供給圧力",
    boundary_capital_absorption:
        "境界: 潜在的な将来供給の観測のみ。市場結論、リスク格上げ、取引信号は生成しない。",
    flow_layer_snapshot: "### 6.2 Demand Layer（Flow Layer）",
    flow_layer_not_configured: "Flow Layer は未設定",
    flow_layer_latest_date: "最新 Flow 観測日",
    flow_layer_observation_divergence: "Observation / Divergence",
    flow_layer_positive_negative_divergence: "正 / 負 divergence",
    flow_layer_breadth: "Flow Breadth",
    flow_layer_market_breadth: "Market Breadth",
    flow_layer_sector_breadth: "Sector Breadth",
    flow_layer_watchlist_breadth: "Watchlist Breadth",
    flow_layer_core_holding_breadth: "Core Holding Breadth",
    boundary_flow_layer:
        "境界: Flow Layer は Observation Only の観測であり、decision weight は 0% に固定され、Trend Layer を override せず、取引信号を生成しない。",
    cognitive_calibration_snapshot: "## 認知校正スナップショット",
    research_attention_entries: "Research attention 件数",
    asset_thesis_entries: "Asset thesis 件数",
    boundary_snapshot_only: "境界: スナップショットのみ。スコア、助言、取引判断は生成しない。",
    boundary_audit_facts: "境界: 監査事実のみ。スコア、助言、取引判断は生成しない。",
    boundary_macro: "境界: コンテキストのみ。Gate 入力や取引指示ではない。",
    boundary_macro_not_configured:
        "境界: マクログラビティは割引率と流動性コンテキストだけを説明する。",
    boundary_cognitive: "境界: 認知校正は注意力と命題レビューだけを扱い、取引信号を生成しない。",
    expectation_layer_snapshot: "## Expectation Layer（市場期待観測）",
    expectation_layer_as_of: "観測日",
    expectation_layer_decision_weight: "decision_weight",
    expectation_layer_trade_signal: "trade_signal",
    expectation_layer_observation_count: "observation_count",
    expectation_layer_subjects: "subjects",
    expectation_layer_boundary:
        "境界: Expectation Layer は市場期待の観測専用であり、Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing に入らず、売買シグナルも生成しない。",
};

fn push_weekly_strategic_context_snapshot(
    review: &mut String,
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.strategic_context_snapshot);
    review.push('\n');
    if let Some(evidence) = pres_packet.transition_evidence.as_ref() {
        review.push_str(&format!(
            "- {}: {:?}\n",
            text.trend_breadth_mode, evidence.trend_breadth_mode
        ));
        review.push_str(&format!(
            "- {}: {:?}\n",
            text.market_cycle_position, evidence.market_cycle_position
        ));
        review.push_str(&format!(
            "- {}: {:?}\n",
            text.holding_efficiency, evidence.holding_efficiency
        ));
        if evidence.strategic_context.is_empty() {
            review.push_str(&format!(
                "- {}: {}\n",
                text.strategic_context_lines, text.strategic_context_none
            ));
        } else {
            review.push_str(&format!("- {}:\n", text.strategic_context_lines));
            for line in &evidence.strategic_context {
                review.push_str(&format!("  - {}\n", line));
            }
        }
    } else {
        review.push_str(&format!("- {}: N/A\n", text.trend_breadth_mode));
        review.push_str(&format!("- {}: N/A\n", text.market_cycle_position));
        review.push_str(&format!("- {}: N/A\n", text.holding_efficiency));
        review.push_str(&format!(
            "- {}: {}\n",
            text.strategic_context_lines, text.strategic_context_none
        ));
    }
    review.push_str("- ");
    review.push_str(text.boundary_snapshot_only);
    review.push('\n');
}

fn push_weekly_macro_gravity_snapshot(
    review: &mut String,
    context: &WeeklyReportContext,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.macro_gravity_snapshot);
    review.push('\n');
    let Some(macro_gravity) = context.macro_gravity.as_ref() else {
        review.push_str(&format!("- {}\n", text.macro_gravity_not_configured));
        review.push_str("- ");
        review.push_str(text.boundary_macro_not_configured);
        review.push('\n');
        return;
    };

    review.push_str(&format!(
        "- {}: {}\n",
        text.rate_pressure, macro_gravity.rate_pressure
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.real_yield, macro_gravity.real_yield_pressure
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.yield_curve, macro_gravity.yield_curve
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.credit_stress, macro_gravity.credit_stress
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.liquidity, macro_gravity.liquidity
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.growth_valuation, macro_gravity.growth_valuation_impact
    ));
    review.push_str("- ");
    review.push_str(text.boundary_macro);
    review.push('\n');
}

fn push_weekly_capital_absorption_ipo_queue_snapshot(
    review: &mut String,
    summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.capital_absorption_ipo_queue_snapshot);
    review.push('\n');
    if !summary["configured"].as_bool().unwrap_or(false) {
        review.push_str(&format!(
            "- {}\n",
            text.capital_absorption_ipo_queue_not_configured
        ));
        review.push_str("- ");
        review.push_str(text.boundary_capital_absorption);
        review.push('\n');
        return;
    }
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_latest_date,
        summary["latest_date"].as_str().unwrap_or("unknown")
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_near_term_latest,
        summary["near_term_supply_count_latest"]
            .as_u64()
            .unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_queue_latest,
        summary["future_queue_count_latest"]
            .as_u64()
            .unwrap_or_else(|| summary["queue_count_latest"].as_u64().unwrap_or(0))
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.capital_absorption_queue_min_max_7d,
        summary["queue_count_min_7d"].as_u64().unwrap_or(0),
        summary["queue_count_max_7d"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.capital_absorption_reported_confirmed,
        summary["reported_count_latest"].as_u64().unwrap_or(0),
        summary["confirmed_count_latest"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_pressure,
        summary["pressure_latest"].as_str().unwrap_or("unknown")
    ));
    review.push_str("- ");
    review.push_str(text.boundary_capital_absorption);
    review.push('\n');
}

fn push_weekly_capital_dynamics_snapshot(
    review: &mut String,
    supply_summary: &serde_json::Value,
    flow_summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.capital_dynamics_snapshot);
    review.push('\n');
    review.push_str("- ");
    review.push_str(text.boundary_capital_dynamics);
    review.push('\n');
    push_weekly_capital_absorption_ipo_queue_snapshot(review, supply_summary, text);
    push_weekly_flow_layer_snapshot(review, flow_summary, text);
}

fn push_weekly_flow_layer_snapshot(
    review: &mut String,
    summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.flow_layer_snapshot);
    review.push('\n');
    if !summary["configured"].as_bool().unwrap_or(false) {
        review.push_str(&format!("- {}\n", text.flow_layer_not_configured));
        review.push_str("- ");
        review.push_str(text.boundary_flow_layer);
        review.push('\n');
        return;
    }
    review.push_str(&format!(
        "- {}: {}\n",
        text.flow_layer_latest_date,
        summary["as_of_date"].as_str().unwrap_or("unknown")
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.flow_layer_observation_divergence,
        summary["observation_count"].as_u64().unwrap_or(0),
        summary["divergence_count"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.flow_layer_positive_negative_divergence,
        summary["positive_divergence_count"].as_u64().unwrap_or(0),
        summary["negative_divergence_count"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!("- {}:\n", text.flow_layer_breadth));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_market_breadth,
        summary["breadth"]["market_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_sector_breadth,
        summary["breadth"]["sector_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_watchlist_breadth,
        summary["breadth"]["watchlist_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_core_holding_breadth,
        summary["breadth"]["core_holding_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str("- ");
    review.push_str(text.boundary_flow_layer);
    review.push('\n');
}

fn push_weekly_state_machine_totals(
    review: &mut String,
    totals: &serde_json::Value,
    daily_summaries: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.state_machine_totals);
    review.push('\n');
    review.push_str(&format!(
        "- {}: {}\n",
        text.state_summary_days,
        totals["days"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.reset_confirmed_blocked,
        totals["reset_confirmed_total"].as_u64().unwrap_or(0),
        totals["reset_blocked_total"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {} / {}\n",
        text.soft_reset_duration_lock_defensive_override,
        totals["soft_reset_total"].as_u64().unwrap_or(0),
        totals["duration_lock_total"].as_u64().unwrap_or(0),
        totals["defensive_override_total"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.core_breakdown_reconciliation_mismatch,
        totals["core_breakdown_total"].as_u64().unwrap_or(0),
        totals["reconciliation_mismatch_total"]
            .as_u64()
            .unwrap_or(0)
    ));

    review.push('\n');
    review.push_str(text.daily_state_timeline);
    review.push('\n');
    if let Some(items) = daily_summaries.as_array() {
        if items.is_empty() {
            review.push_str(&format!("- {}\n", text.no_state_machine_summaries));
        }
        for item in items {
            review.push_str(&format!(
                "- {}: {} -> {} | reset C/B {} / {} | soft_reset {} | duration_lock {} | defensive_override {} | mismatch {}\n",
                item["date"].as_str().unwrap_or("unknown"),
                item["from_state"].as_str().unwrap_or("unknown"),
                item["to_state"].as_str().unwrap_or("unknown"),
                item["reset_confirmed"].as_bool().unwrap_or(false),
                item["reset_blocked"].as_bool().unwrap_or(false),
                item["soft_reset_applied"].as_bool().unwrap_or(false),
                item["duration_locked"].as_bool().unwrap_or(false),
                item["defensive_override"].as_bool().unwrap_or(false),
                item["reconciliation_mismatch_count"].as_u64().unwrap_or(0)
            ));
        }
    }
    review.push_str("- ");
    review.push_str(text.boundary_audit_facts);
    review.push('\n');
}

fn push_weekly_cognitive_calibration_snapshot(
    review: &mut String,
    context: &WeeklyReportContext,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.cognitive_calibration_snapshot);
    review.push('\n');
    review.push_str(&format!(
        "- {}: {}\n",
        text.research_attention_entries, context.research_attention_entries
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.asset_thesis_entries, context.asset_thesis_entries
    ));
    review.push_str("- ");
    review.push_str(text.boundary_cognitive);
    review.push('\n');
}

fn push_weekly_expectation_snapshot(
    review: &mut String,
    summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.expectation_layer_snapshot);
    review.push('\n');
    if !summary["configured"].as_bool().unwrap_or(false) {
        review.push_str("- expectation layer not configured\n");
        review.push_str("- ");
        review.push_str(text.expectation_layer_boundary);
        review.push('\n');
        return;
    }

    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_as_of,
        summary["as_of_date"].as_str().unwrap_or("unknown")
    ));
    review.push_str(&format!(
        "- {}: {}%\n",
        text.expectation_layer_decision_weight,
        summary["decision_weight_percent"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_trade_signal,
        summary["trade_signal"].as_bool().unwrap_or(false)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_observation_count,
        summary["observation_count"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_subjects,
        summary["subjects"]
            .as_array()
            .map(|subjects| {
                subjects
                    .iter()
                    .filter_map(|value| value.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "unknown".to_string())
    ));
    review.push_str("- ");
    review.push_str(text.expectation_layer_boundary);
    review.push('\n');
}

#[cfg(test)]
mod tests {
    use super::{
        build_weekly_latest_context, load_weekly_state_machine_summaries,
        persist_weekly_state_outputs, push_weekly_capital_absorption_ipo_queue_snapshot,
        push_weekly_capital_dynamics_snapshot, push_weekly_expectation_snapshot,
        push_weekly_flow_layer_snapshot, weekly_text, WeeklyReportContext,
    };
    use crate::features::radar::interface::presentation::PresentationPacket;
    use crate::features::shared::application::run_status::StateMachineSummary;
    use crate::features::shared::interface::i18n::Language;
    use chrono::NaiveDate;
    use tempfile::tempdir;

    fn write_run_status(
        save_dir: &std::path::Path,
        date: &str,
        state: &str,
        reset_confirmed: bool,
    ) {
        let value = serde_json::json!({
            "state_machine": {
                "from_state": "PREVIOUS",
                "to_state": state,
                "reset_confirmed": reset_confirmed,
                "reset_blocked": false,
                "soft_reset_applied": false,
                "duration_locked": false,
                "defensive_override": false,
                "core_breakdown": false,
                "reconciliation_mismatch_count": 0,
                "preflight_failed": false
            }
        });
        std::fs::write(
            save_dir.join(format!("run_status_{date}.json")),
            serde_json::to_string(&value).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn weekly_state_machine_summaries_ignore_future_run_status_files() {
        let tmp = tempdir().unwrap();
        write_run_status(tmp.path(), "2026-06-07", "VALID_HISTORY", true);
        write_run_status(tmp.path(), "2026-06-10", "FUTURE_SHOULD_NOT_APPEAR", true);
        let current = StateMachineSummary {
            from_state: "VALID_HISTORY".to_string(),
            to_state: "CURRENT".to_string(),
            ..Default::default()
        };

        let entries = load_weekly_state_machine_summaries(
            tmp.path(),
            NaiveDate::from_ymd_opt(2026, 6, 9).unwrap(),
            Some(&current),
        );

        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.summary.to_state == "CURRENT"));
        assert!(entries
            .iter()
            .any(|entry| entry.summary.to_state == "VALID_HISTORY"));
        assert!(!entries
            .iter()
            .any(|entry| entry.summary.to_state == "FUTURE_SHOULD_NOT_APPEAR"));
    }

    #[test]
    fn weekly_review_text_uses_configured_language_labels() {
        assert_eq!(
            weekly_text(Language::ZhCn).title,
            "# 周度状态复盘（自动草稿）"
        );
        assert_eq!(
            weekly_text(Language::EnUs).title,
            "# Weekly State Review (Auto)"
        );
        assert_eq!(
            weekly_text(Language::JaJp).title,
            "# 週次状態レビュー（自動下書き）"
        );
        assert!(weekly_text(Language::ZhCn)
            .boundary_cognitive
            .contains("不生成交易信号"));
        assert!(weekly_text(Language::JaJp)
            .boundary_cognitive
            .contains("取引信号を生成しない"));
        assert!(weekly_text(Language::ZhCn)
            .expectation_layer_boundary
            .contains("不进入 Gate"));
        assert!(weekly_text(Language::JaJp)
            .expectation_layer_boundary
            .contains("売買シグナルも生成しない"));
        assert!(!weekly_text(Language::ZhCn)
            .capital_absorption_ipo_queue_snapshot
            .contains("Capital Absorption IPO Queue Snapshot"));
        assert!(!weekly_text(Language::ZhCn)
            .capital_absorption_queue_min_max_7d
            .contains("min / max"));
        assert!(!weekly_text(Language::ZhCn)
            .capital_absorption_reported_confirmed
            .contains("Reported / Confirmed"));
        assert!(!weekly_text(Language::JaJp)
            .capital_absorption_ipo_queue_snapshot
            .contains("Capital Absorption IPO Queue Snapshot"));
        assert!(!weekly_text(Language::JaJp)
            .capital_absorption_queue_min_max_7d
            .contains("min / max"));
        assert!(!weekly_text(Language::JaJp)
            .capital_absorption_reported_confirmed
            .contains("Reported / Confirmed"));
    }

    #[test]
    fn weekly_capital_absorption_labels_do_not_leak_english_in_zh_or_ja() {
        let blocked = [
            "Capital Absorption IPO Queue",
            "min / max",
            "Reported / Confirmed",
        ];
        let localized_labels = [
            weekly_text(Language::ZhCn).capital_absorption_ipo_queue_snapshot,
            weekly_text(Language::ZhCn).capital_absorption_queue_min_max_7d,
            weekly_text(Language::ZhCn).capital_absorption_reported_confirmed,
            weekly_text(Language::JaJp).capital_absorption_ipo_queue_snapshot,
            weekly_text(Language::JaJp).capital_absorption_queue_min_max_7d,
            weekly_text(Language::JaJp).capital_absorption_reported_confirmed,
        ];

        for label in localized_labels {
            for blocked_label in blocked {
                assert!(!label.contains(blocked_label));
            }
        }
    }

    #[test]
    fn weekly_capital_absorption_review_section_keeps_observation_boundary() {
        let summary = serde_json::json!({
            "configured": true,
            "latest_date": "2026-06-08",
            "near_term_supply_count_latest": 1,
            "future_queue_count_latest": 3,
            "queue_count_latest": 3,
            "queue_count_min_7d": 1,
            "queue_count_max_7d": 3,
            "reported_count_latest": 2,
            "confirmed_count_latest": 1,
            "pressure_latest": "ELEVATED"
        });
        let mut review = String::new();

        push_weekly_capital_absorption_ipo_queue_snapshot(
            &mut review,
            &summary,
            weekly_text(Language::ZhCn),
        );

        assert!(review.contains("### 6.1 Supply Layer（Capital Absorption）"));
        assert!(review.contains("最新 Near-Term Supply 数量: 1"));
        assert!(review.contains("最新 Future Queue 数量: 3"));
        assert!(review.contains("7 日 Future Queue 最小值 / 最大值: 1 / 3"));
        assert!(review.contains("已报道 / 已确认: 2 / 1"));
        assert!(review.contains("潜在供给压力: ELEVATED"));
        assert!(review.contains("不生成市场结论、风险升级或交易信号"));
        assert!(!review.contains("Capital Absorption IPO Queue"));
        assert!(!review.contains("min / max"));
        assert!(!review.contains("Reported / Confirmed"));
        assert!(!review.contains("READY"));
        assert!(!review.contains("EXECUTE"));
    }

    #[test]
    fn weekly_flow_layer_review_section_keeps_observation_only_boundary() {
        let summary = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-08",
            "observation_count": 2,
            "divergence_count": 1,
            "positive_divergence_count": 0,
            "negative_divergence_count": 1,
            "breadth": {
                "market_breadth": "UNAVAILABLE",
                "sector_breadth": "DIVERGENT",
                "watchlist_breadth": "SUPPORTIVE",
                "core_holding_breadth": "NEUTRAL"
            }
        });
        let mut review = String::new();

        push_weekly_flow_layer_snapshot(&mut review, &summary, weekly_text(Language::ZhCn));

        assert!(review.contains("### 6.2 Demand Layer（Flow Layer）"));
        assert!(review.contains("最新 Flow 观察日: 2026-06-08"));
        assert!(review.contains("Observation / Divergence: 2 / 1"));
        assert!(review.contains("正向 / 负向背离: 0 / 1"));
        assert!(review.contains("Market Breadth: UNAVAILABLE"));
        assert!(review.contains("Watchlist Breadth: SUPPORTIVE"));
        assert!(review.contains("decision weight 固定为 0%"));
        assert!(!review.contains("READY"));
        assert!(!review.contains("EXECUTE"));
    }

    #[test]
    fn weekly_capital_dynamics_review_shell_wraps_supply_and_flow() {
        let supply = serde_json::json!({
            "configured": true,
            "latest_date": "2026-06-08",
            "near_term_supply_count_latest": 1,
            "future_queue_count_latest": 3,
            "queue_count_latest": 3,
            "queue_count_min_7d": 1,
            "queue_count_max_7d": 3,
            "reported_count_latest": 2,
            "confirmed_count_latest": 1,
            "pressure_latest": "ELEVATED"
        });
        let flow = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-08",
            "observation_count": 2,
            "divergence_count": 1,
            "positive_divergence_count": 0,
            "negative_divergence_count": 1,
            "breadth": {
                "market_breadth": "UNAVAILABLE",
                "sector_breadth": "DIVERGENT",
                "watchlist_breadth": "SUPPORTIVE",
                "core_holding_breadth": "NEUTRAL"
            }
        });
        let mut review = String::new();

        push_weekly_capital_dynamics_snapshot(
            &mut review,
            &supply,
            &flow,
            weekly_text(Language::ZhCn),
        );

        assert!(review.contains("## Capital Dynamics（供需观察）"));
        assert!(review.contains("Current decision weight 为 0%"));
        assert!(review.contains("### 6.1 Supply Layer（Capital Absorption）"));
        assert!(review.contains("### 6.2 Demand Layer（Flow Layer）"));
        assert!(review.contains("不接入 Gate、Execution、Trader、Action Matrix 或 Position Sizing"));
    }

    #[test]
    fn weekly_expectation_review_section_keeps_read_only_boundary() {
        let summary = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-18",
            "decision_weight_percent": 0,
            "trade_signal": false,
            "gate_effect": "none",
            "execution_effect": "none",
            "position_sizing_effect": "none",
            "observation_count": 1,
            "subjects": ["TSLA"]
        });
        let mut review = String::new();

        push_weekly_expectation_snapshot(&mut review, &summary, weekly_text(Language::ZhCn));

        assert!(review.contains("## Expectation Layer（市场预期观测）"));
        assert!(review.contains("decision_weight: 0%"));
        assert!(review.contains("trade_signal: false"));
        assert!(review.contains("observation_count: 1"));
        assert!(review.contains("subjects: TSLA"));
        assert!(review.contains("不进入 Gate、Execution、Trader、Action Matrix"));
        assert!(!review.contains("BUY"));
        assert!(!review.contains("SELL"));
    }

    #[test]
    fn weekly_latest_context_keeps_supply_layer_and_legacy_alias_in_sync() {
        let supply = serde_json::json!({
            "configured": true,
            "latest_date": "2026-06-08",
            "near_term_supply_count_latest": 1
        });
        let flow = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-08",
            "observation_count": 2
        });
        let expectation = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-18",
            "decision_weight_percent": 0,
            "trade_signal": false,
            "gate_effect": "none",
            "execution_effect": "none",
            "position_sizing_effect": "none",
            "observation_count": 1,
            "subjects": ["TSLA"]
        });
        let latest = build_weekly_latest_context(
            &PresentationPacket::default(),
            &WeeklyReportContext {
                macro_gravity: None,
                research_attention_entries: 0,
                asset_thesis_entries: 0,
                capital_absorption_ipo_queue: supply.clone(),
                capital_dynamics_flow_layer: flow.clone(),
                expectation_layer: expectation.clone(),
            },
            &supply,
            &flow,
            &expectation,
        );

        assert_eq!(latest["capital_dynamics"]["supply_layer"], supply);
        assert_eq!(latest["capital_absorption_ipo_queue"], supply);
        assert_eq!(
            latest["capital_dynamics"]["supply_layer"],
            latest["capital_absorption_ipo_queue"]
        );
        assert_eq!(latest["capital_dynamics"]["flow_layer"], flow);
        assert_eq!(latest["expectation_layer"], expectation);
    }

    #[test]
    fn weekly_state_metrics_keep_trend_cohesion_alias_in_sync() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let packet_date = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        let market_features = crate::features::radar::domain::features::MarketFeatures {
            system_confidence: 0.8,
            stability_score: 0.7,
            ..Default::default()
        };
        let market_regime = crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
            market_state: crate::features::radar::domain::market_regime::MarketState::NEWBORN,
            lifecycle_state: crate::features::radar::domain::market_regime::LifecycleState::NEWBORN,
            risk_overlay: crate::features::radar::domain::market_regime::RiskOverlay::NORMAL,
            reasons: vec![],
            low_stability_streak: 0,
            duration_in_state: 1,
            transition_audit: None,
        };
        let packet = crate::features::radar::domain::decision::DecisionPacket::new(
            packet_date,
            market_features,
            market_regime,
            None,
            crate::features::radar::domain::portfolio_policy::PortfolioPolicy::default(),
            vec![],
            Vec::new(),
            false,
            crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                continuity_streak: 3,
                ..Default::default()
            },
            None,
            None,
        );
        let pres_packet = crate::features::radar::interface::presentation::PresentationPacket {
            date_str: "2026-06-18".to_string(),
            language: Language::ZhCn,
            ..Default::default()
        };
        let context = WeeklyReportContext {
            macro_gravity: None,
            research_attention_entries: 0,
            asset_thesis_entries: 0,
            capital_absorption_ipo_queue: serde_json::json!({
                "configured": false
            }),
            capital_dynamics_flow_layer: serde_json::json!({
                "configured": false
            }),
            expectation_layer: serde_json::json!({
                "configured": false
            }),
        };

        persist_weekly_state_outputs(&save_dir, &[], &packet, true, &pres_packet, &context, None)
            .unwrap();

        let metrics: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(save_dir.join("weekly_state_metrics.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metrics["trend_cohesion_ready_days"], 1);
        assert_eq!(metrics["participation_ready_days"], 1);
        assert_eq!(
            metrics["trend_cohesion_ready_days"],
            metrics["participation_ready_days"]
        );
    }
}
