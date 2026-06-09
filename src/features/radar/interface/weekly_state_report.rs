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
    let latest_context = build_weekly_latest_context(pres_packet, context);
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
        // semantic shift warning: 'participation_ready_days' は現在 'trend_cohesion_ready_days' を出力する。
        // この key を読む downstream script は、従来の participation semantics ではなく cohesion gate semantics を受け取る。
        // script failure を避けるため、後方互換性のためだけにこの key を維持する。
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
    push_weekly_cognitive_calibration_snapshot(&mut review, context, text);

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
    cognitive_calibration_snapshot: &'static str,
    research_attention_entries: &'static str,
    asset_thesis_entries: &'static str,
    boundary_snapshot_only: &'static str,
    boundary_audit_facts: &'static str,
    boundary_macro: &'static str,
    boundary_macro_not_configured: &'static str,
    boundary_cognitive: &'static str,
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
    cognitive_calibration_snapshot: "## 认知校准快照",
    research_attention_entries: "研究关注条目",
    asset_thesis_entries: "资产命题条目",
    boundary_snapshot_only: "边界: 仅为快照；不生成评分、建议或交易判断。",
    boundary_audit_facts: "边界: 仅为审计事实；不生成评分、建议或交易判断。",
    boundary_macro: "边界: 仅说明贴现率与流动性上下文；不作为 Gate 输入或交易指令。",
    boundary_macro_not_configured: "边界: 宏观引力仅解释贴现率与流动性上下文。",
    boundary_cognitive: "边界: 认知校准只管理注意力和命题复核；不生成交易信号。",
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
    cognitive_calibration_snapshot: "## Cognitive Calibration Snapshot",
    research_attention_entries: "Research attention entries",
    asset_thesis_entries: "Asset thesis entries",
    boundary_snapshot_only: "Boundary: snapshot only; no score, advice, or trade decision.",
    boundary_audit_facts: "Boundary: audit facts only; no score, advice, or trade decision.",
    boundary_macro: "Boundary: context only; no Gate input or trade instruction.",
    boundary_macro_not_configured: "Boundary: macro gravity explains discount-rate and liquidity context only.",
    boundary_cognitive: "Boundary: cognitive calibration manages attention and thesis review only; it does not generate trade signals.",
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
    cognitive_calibration_snapshot: "## 認知校正スナップショット",
    research_attention_entries: "Research attention 件数",
    asset_thesis_entries: "Asset thesis 件数",
    boundary_snapshot_only: "境界: スナップショットのみ。スコア、助言、取引判断は生成しない。",
    boundary_audit_facts: "境界: 監査事実のみ。スコア、助言、取引判断は生成しない。",
    boundary_macro: "境界: コンテキストのみ。Gate 入力や取引指示ではない。",
    boundary_macro_not_configured:
        "境界: マクログラビティは割引率と流動性コンテキストだけを説明する。",
    boundary_cognitive: "境界: 認知校正は注意力と命題レビューだけを扱い、取引信号を生成しない。",
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

#[cfg(test)]
mod tests {
    use super::{load_weekly_state_machine_summaries, weekly_text};
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
    }
}
