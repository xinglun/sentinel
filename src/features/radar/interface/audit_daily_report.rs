use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Weekday};
use std::collections::BTreeMap;

use crate::features::shared::interface::i18n::Language;

#[derive(Debug, Clone)]
pub(crate) struct TransitionAuditEntry {
    pub(crate) date: NaiveDate,
    pub(crate) timestamp: DateTime<FixedOffset>,
    pub(crate) log: crate::features::radar::domain::transition_log::StateTransitionLog,
}

#[derive(Debug, Clone)]
pub(crate) struct TransitionAuditDay {
    pub(crate) date: NaiveDate,
    pub(crate) events: Vec<TransitionAuditEntry>,
}

impl TransitionAuditDay {
    pub(crate) fn latest(&self) -> &TransitionAuditEntry {
        self.events
            .last()
            .expect("TransitionAuditDay must include at least one event")
    }
}

pub(crate) fn parse_transition_audit_entry(
    line: &str,
    language: Language,
) -> Result<Option<TransitionAuditEntry>> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    let timestamp = if let Some(ts) = value.get("timestamp").and_then(|v| v.as_str()) {
        DateTime::parse_from_rfc3339(ts)
            .with_context(|| format!("{}: {}", audit_error_invalid_timestamp(language), ts))?
    } else {
        return Ok(None);
    };

    let date = match value.get("date").and_then(|v| v.as_str()) {
        Some(raw_date) => NaiveDate::parse_from_str(raw_date, "%Y-%m-%d")
            .with_context(|| format!("{}: {}", audit_error_invalid_date(language), raw_date))?,
        None => timestamp.date_naive(),
    };

    let log_value = value
        .get("transition")
        .cloned()
        .or_else(|| value.get("log").cloned());
    let Some(log_json) = log_value else {
        return Ok(None);
    };

    let log: crate::features::radar::domain::transition_log::StateTransitionLog =
        serde_json::from_value(log_json)?;
    Ok(Some(TransitionAuditEntry {
        date,
        timestamp,
        log,
    }))
}

pub(crate) fn resolve_target_index(
    days: &[TransitionAuditDay],
    target_date: Option<NaiveDate>,
    language: Language,
) -> Result<usize> {
    if let Some(date) = target_date {
        days.iter()
            .position(|e| e.date == date)
            .with_context(|| format!("{} {}", audit_error_target_date_not_found(language), date))
    } else {
        Ok(days.len() - 1)
    }
}

#[cfg(test)]
pub(crate) fn build_audit_daily_report(
    days: &[TransitionAuditDay],
    target_idx: usize,
    window_days: usize,
    language: Language,
) -> String {
    build_audit_daily_report_with_evidence_status(days, target_idx, window_days, language, None)
}

pub(crate) fn build_audit_daily_report_with_evidence_status(
    days: &[TransitionAuditDay],
    target_idx: usize,
    window_days: usize,
    language: Language,
    evidence_collection_status: Option<
        &crate::features::shared::application::run_status::DeliveryStatus,
    >,
) -> String {
    let text = audit_text(language);
    let today = &days[target_idx];
    let today_latest = today.latest();
    let window_start = target_idx.saturating_sub(window_days.saturating_sub(1));
    let window = &days[window_start..=target_idx];
    let window_latest = window.iter().map(|d| d.latest()).collect::<Vec<_>>();

    let gate_is_ready = today_latest.log.trend_cohesion_gate.to;
    let gate_status = if gate_is_ready {
        text.status_ready
    } else {
        text.status_no_trade
    };
    let no_trade_mode = opportunity_mode_label(today_latest.log.opportunity_mode.to, language);
    let scout_days_without_expansion = today_latest.log.scout_days_without_expansion;
    let scout_abort_days = today_latest.log.scout_abort_days.max(1);
    let scout_streak_text = if today_latest.log.opportunity_mode.to
        == crate::features::radar::domain::transition_log::OpportunityMode::NoTradeScout
    {
        format!(
            "{} / {} {}",
            scout_days_without_expansion, scout_abort_days, text.day_unit
        )
    } else {
        text.none.to_string()
    };
    let gate_streak = consecutive_streak(days, target_idx, |log| {
        log.trend_cohesion_gate.to == gate_is_ready
    });

    let blocker_counts = summarize_blockers(&window_latest);
    let top_blockers = blocker_counts.into_iter().take(3).collect::<Vec<_>>();

    let breakout_today = summarize_breakout_changes_from_events(today);
    let no_trade_streak = consecutive_streak(days, target_idx, |log| !log.trend_cohesion_gate.to);
    let mainline_missing_streak = consecutive_streak(days, target_idx, |log| {
        log.trend_cohesion_status.to
            != crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed
    });

    let segment_type = if detect_no_trade_resets(window) {
        text.segment_reset
    } else {
        text.segment_continuous
    };

    let transition_state_change = yes_no(
        today.events.iter().any(|e| e.log.market_state.changed),
        language,
    );
    let transition_risk_change = yes_no(
        today.events.iter().any(|e| e.log.risk_overlay.changed),
        language,
    );
    let transition_trend_change = yes_no(
        today
            .events
            .iter()
            .any(|e| e.log.trend_cohesion_status.changed),
        language,
    );
    let transition_mode_change = yes_no(
        today.events.iter().any(|e| e.log.opportunity_mode.changed),
        language,
    );
    let transition_scout_reset = yes_no(
        today.events.iter().any(|e| e.log.scout_reset_triggered),
        language,
    );

    let blocker_text = if gate_is_ready || top_blockers.is_empty() {
        text.none.to_string()
    } else {
        top_blockers
            .iter()
            .map(|(name, _)| blocker_label(name, language))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    let breakout_text = summarize_breakout_sentence(&breakout_today, language);
    let mainline_text = trend_status_label(today_latest.log.trend_cohesion_status.to, language);

    let substantive_summaries = {
        let mut summaries = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        for event in &today.events {
            if let Some(ref rec) = event.log.trend_recognition {
                if let Some(ref substantive) = rec.substantive {
                    for record in &substantive.records {
                        let key = if record.dedupe_key().is_empty() {
                            format!(
                                "{:?}:{:?}:{:?}:{}:{}:{}",
                                record.source,
                                record.evidence_type,
                                record.symbol,
                                record.event_date,
                                record.source_url.as_deref().unwrap_or("NO_URL"),
                                record.description
                            )
                        } else {
                            record.dedupe_key().to_string()
                        };
                        if seen_keys.insert(key) {
                            let symbol_part = record
                                .symbol
                                .as_ref()
                                .map(|s| format!("[{}] ", s))
                                .unwrap_or_default();
                            let date_part = format!("[{}] ", record.event_date);
                            let type_part = format!("[{:?}] ", record.evidence_type);
                            let conf_part = format!("(conf:{:.2}) ", record.confidence);
                            let source_part = format!("[{:?}] ", record.source);

                            let url_part = record
                                .source_url
                                .as_deref()
                                .filter(|url| !is_fixture_source_url(url))
                                .map(|u| format!(" ({})", u))
                                .unwrap_or_default();

                            let description =
                                format_evidence_description(&record.description, language);

                            summaries.push(format!(
                                "- {}{}{}{}{}{}{}",
                                symbol_part,
                                date_part,
                                type_part,
                                conf_part,
                                source_part,
                                description,
                                url_part
                            ));
                        }
                    }
                }
            }
        }
        summaries
    };

    let audit_sentence = build_audit_sentence(
        language,
        gate_status,
        gate_streak,
        &blocker_text,
        &breakout_text,
        mainline_text,
        no_trade_mode,
    );

    let mut out = String::new();
    out.push_str(&format!(
        "# {} ({})\n\n",
        text.title,
        today_latest.date.format("%Y-%m-%d")
    ));

    out.push_str(&format!("1. {}\n", text.section_gate));
    out.push_str(&format!("- {}: {}\n", text.label_status, gate_status));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_no_trade_mode, no_trade_mode
    ));
    out.push_str(&format!(
        "- {}: {} {}\n",
        text.label_duration, gate_streak, text.day_unit
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_scout_streak, scout_streak_text
    ));
    out.push_str(&format!("- {}:\n", text.label_top_blockers));
    if top_blockers.is_empty() {
        out.push_str(&format!("- {}\n", text.none));
    } else {
        for (name, count) in &top_blockers {
            out.push_str(&format!(
                "- {} ({})\n",
                blocker_label(name, language),
                count
            ));
        }
    }

    out.push_str(&format!("\n2. {}\n", text.section_transition));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_state_change, transition_state_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_risk_change, transition_risk_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_trend_change, transition_trend_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_mode_change, transition_mode_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_scout_reset, transition_scout_reset
    ));

    out.push_str(&format!("\n3. {}\n", text.section_breakout));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_breakout_new,
        format_symbols(&breakout_today.new_symbols, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_breakout_continued,
        format_symbols(&breakout_today.continued_symbols, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_breakout_removed,
        format_symbols(&breakout_today.removed_symbols, language)
    ));

    out.push_str(&format!("\n4. {}\n", text.section_substantive));
    if let Some(status) = evidence_collection_status {
        out.push_str(&format!(
            "- {}: {}\n",
            text.label_evidence_collection,
            format_delivery_status(status, language)
        ));
    }
    out.push_str(&format!("- {}:\n", text.label_evidence_stock));
    if substantive_summaries.is_empty() {
        out.push_str(&format!("- {}\n", text.none));
    } else {
        for summary in substantive_summaries {
            out.push_str(&format!("{}\n", summary));
        }
    }

    out.push_str(&format!("\n5. {}\n", text.section_streaks));
    out.push_str(&format!(
        "- {}: {} {}\n",
        text.label_no_trade_streak, no_trade_streak, text.day_unit
    ));
    out.push_str(&format!(
        "- {}: {} {}\n",
        text.label_mainline_missing_streak, mainline_missing_streak, text.day_unit
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_recent_shape, segment_type
    ));
    out.push_str(&format!("- {}\n", text.methodology_note));

    out.push_str(&format!("\n6. {}\n", text.section_one_liner));
    out.push_str(&format!("- {}\n", audit_sentence));

    if let Some(evidence) = &today_latest.log.trend_recognition {
        let dict = crate::features::shared::interface::i18n::get_dictionary(language);
        let tr_dict = &dict.trend_recognition;

        let state_label = match evidence.state {
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::None => &tr_dict.state_none,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::StructuralPersistence => {
                &tr_dict.state_structural_persistence
            }
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::EarlyLeader => &tr_dict.state_early_leader,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::LeaderConfirmedFollowersLagging => &tr_dict.state_leader_confirmed_followers_lagging,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::Broadening => &tr_dict.state_broadening,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::Mature => &tr_dict.state_mature,
        };

        let lag_label = if evidence.lag_state {
            &tr_dict.lag_alert
        } else {
            text.none
        };

        let formatted = text
            .template_trend_recognition
            .replace("{state}", state_label)
            .replace("{score}", &format!("{:.2}", evidence.diffusion_score))
            .replace("{conviction}", &format!("{:.2}", evidence.conviction_score))
            .replace("{lag_state}", lag_label);

        out.push_str(&format!("{}\n", formatted));
    }

    out
}

struct AuditDailyText {
    title: &'static str,
    section_gate: &'static str,
    section_transition: &'static str,
    section_breakout: &'static str,
    section_streaks: &'static str,
    section_substantive: &'static str,
    section_one_liner: &'static str,
    label_status: &'static str,
    label_duration: &'static str,
    label_no_trade_mode: &'static str,
    label_scout_streak: &'static str,
    label_top_blockers: &'static str,
    label_state_change: &'static str,
    label_risk_change: &'static str,
    label_trend_change: &'static str,
    label_mode_change: &'static str,
    label_scout_reset: &'static str,
    label_breakout_new: &'static str,
    label_breakout_continued: &'static str,
    label_breakout_removed: &'static str,
    label_evidence_collection: &'static str,
    label_evidence_stock: &'static str,
    label_no_trade_streak: &'static str,
    label_mainline_missing_streak: &'static str,
    label_recent_shape: &'static str,
    methodology_note: &'static str,
    none: &'static str,
    yes: &'static str,
    no: &'static str,
    segment_reset: &'static str,
    segment_continuous: &'static str,
    status_no_trade: &'static str,
    status_ready: &'static str,
    mode_cold: &'static str,
    mode_scout: &'static str,
    mode_ready: &'static str,
    day_unit: &'static str,
    template_trend_recognition: &'static str,
}

fn audit_text(language: Language) -> AuditDailyText {
    match language {
        Language::ZhCn => AuditDailyText {
            title: "每日审计",
            section_gate: "门槛摘要",
            section_transition: "状态变化摘要",
            section_breakout: "突破摘要",
            section_streaks: "连续段统计",
            section_substantive: "实体证据摘要",
            section_one_liner: "审计一句话",
            label_status: "状态",
            label_duration: "持续天数",
            label_no_trade_mode: "NO TRADE 分层",
            label_scout_streak: "侦察未扩散计数",
            label_top_blockers: "最主要阻碍因子前三项",
            label_state_change: "今天是否有状态变化",
            label_risk_change: "今天是否有风险叠加变化",
            label_trend_change: "今天是否有主线状态变化",
            label_mode_change: "今天是否有 NO TRADE 分层变化",
            label_scout_reset: "今天是否触发侦察重置",
            label_breakout_new: "新增突破",
            label_breakout_continued: "延续突破",
            label_breakout_removed: "消失突破",
            label_evidence_collection: "今日证据采集状态",
            label_evidence_stock: "历史证据存量",
            label_no_trade_streak: "当前 NO TRADE 连续段长度",
            label_mainline_missing_streak: "当前主线缺失连续段长度",
            label_recent_shape: "最近一段 NO TRADE 形态",
            methodology_note: "口径: 连续段按日志连续计算（周末自动衔接）",
            none: "无",
            yes: "有",
            no: "无",
            segment_reset: "反复 reset",
            segment_continuous: "连续段",
            status_no_trade: "NO TRADE",
            status_ready: "READY",
            mode_cold: "初级（无信号）",
            mode_scout: "侦察态（有信号未验证）",
            mode_ready: "READY（可执行）",
            day_unit: "天",
            template_trend_recognition: "- 趋势识别质量: {state}; 扩散评分 {score}; 确信度 {conviction}; 滞后状态 {lag_state}",
        },
        Language::EnUs => AuditDailyText {
            title: "Audit Daily",
            section_gate: "Gate Summary",
            section_transition: "Transition Summary",
            section_breakout: "Breakout Summary",
            section_streaks: "Streak Metrics",
            section_substantive: "Substantive Evidence",
            section_one_liner: "Audit One-liner",
            label_status: "Status",
            label_duration: "Duration",
            label_no_trade_mode: "NO TRADE tier",
            label_scout_streak: "Scout non-expansion counter",
            label_top_blockers: "Top 3 blockers",
            label_state_change: "State changed today",
            label_risk_change: "Risk overlay changed today",
            label_trend_change: "Mainline status changed today",
            label_mode_change: "NO TRADE tier changed today",
            label_scout_reset: "Scout reset triggered today",
            label_breakout_new: "New breakout",
            label_breakout_continued: "Continued breakout",
            label_breakout_removed: "Removed breakout",
            label_evidence_collection: "Today's evidence collection status",
            label_evidence_stock: "Historical evidence stock",
            label_no_trade_streak: "Current NO TRADE streak",
            label_mainline_missing_streak: "Current missing-mainline streak",
            label_recent_shape: "Recent NO TRADE segment type",
            methodology_note:
                "Methodology: streaks are calculated by log continuity (weekends auto-bridged)",
            none: "None",
            yes: "Yes",
            no: "No",
            segment_reset: "Repeated resets",
            segment_continuous: "Continuous segment",
            status_no_trade: "NO TRADE",
            status_ready: "READY",
            mode_cold: "Cold (no signal)",
            mode_scout: "Scout (signal unverified)",
            mode_ready: "READY (executable)",
            day_unit: "days",
            template_trend_recognition: "- Trend Recognition Quality: {state}; Diffusion Score {score}; Conviction Score {conviction}; Lag State {lag_state}",
        },
        Language::JaJp => AuditDailyText {
            title: "日次監査",
            section_gate: "ゲートサマリー",
            section_transition: "状態遷移サマリー",
            section_breakout: "ブレイクアウトサマリー",
            section_streaks: "連続区間統計",
            section_substantive: "実体的な証拠サマリー",
            section_one_liner: "監査ワンライン要約",
            label_status: "状態",
            label_duration: "継続日数",
            label_no_trade_mode: "NO TRADE レイヤー",
            label_scout_streak: "偵察未拡散カウント",
            label_top_blockers: "主要阻害要因 Top 3",
            label_state_change: "本日の状態変化",
            label_risk_change: "本日のリスクオーバーレイ変化",
            label_trend_change: "本日の主線状態変化",
            label_mode_change: "本日の NO TRADE レイヤー変化",
            label_scout_reset: "本日の偵察リセット発生",
            label_breakout_new: "新規ブレイクアウト",
            label_breakout_continued: "継続ブレイクアウト",
            label_breakout_removed: "消失ブレイクアウト",
            label_evidence_collection: "本日の証拠収集状態",
            label_evidence_stock: "履歴証拠ストック",
            label_no_trade_streak: "現在の NO TRADE 連続日数",
            label_mainline_missing_streak: "現在の主線欠如連続日数",
            label_recent_shape: "直近 NO TRADE 区間の形態",
            methodology_note: "口径: 連続区間はログ連続で計算（週末は自動連結）",
            none: "なし",
            yes: "あり",
            no: "なし",
            segment_reset: "反復 reset",
            segment_continuous: "連続区間",
            status_no_trade: "NO TRADE",
            status_ready: "READY",
            mode_cold: "初級（シグナルなし）",
            mode_scout: "偵察（シグナル未検証）",
            mode_ready: "READY（実行可）",
            day_unit: "日",
            template_trend_recognition: "- トレンド認識品質: {state}; 拡散スコア {score}; 確信度 {conviction}; 遅行状態 {lag_state}",
        },
    }
}

fn opportunity_mode_label(
    mode: crate::features::radar::domain::transition_log::OpportunityMode,
    language: Language,
) -> &'static str {
    let text = audit_text(language);
    match mode {
        crate::features::radar::domain::transition_log::OpportunityMode::NoTradeCold => {
            text.mode_cold
        }
        crate::features::radar::domain::transition_log::OpportunityMode::NoTradeScout => {
            text.mode_scout
        }
        crate::features::radar::domain::transition_log::OpportunityMode::Ready => text.mode_ready,
    }
}

fn yes_no(flag: bool, language: Language) -> &'static str {
    let text = audit_text(language);
    if flag {
        text.yes
    } else {
        text.no
    }
}

fn format_delivery_status(
    status: &crate::features::shared::application::run_status::DeliveryStatus,
    language: Language,
) -> String {
    match status {
        crate::features::shared::application::run_status::DeliveryStatus::Succeeded => {
            match language {
                Language::ZhCn => "成功".to_string(),
                Language::EnUs => "succeeded".to_string(),
                Language::JaJp => "成功".to_string(),
            }
        }
        crate::features::shared::application::run_status::DeliveryStatus::Skipped => match language
        {
            Language::ZhCn => "跳过".to_string(),
            Language::EnUs => "skipped".to_string(),
            Language::JaJp => "スキップ".to_string(),
        },
        crate::features::shared::application::run_status::DeliveryStatus::Failed { reason } => {
            match language {
                Language::ZhCn => format!("失败 ({})", reason),
                Language::EnUs => format!("failed ({})", reason),
                Language::JaJp => format!("失敗 ({})", reason),
            }
        }
    }
}

fn format_symbols(symbols: &[String], language: Language) -> String {
    if symbols.is_empty() {
        audit_text(language).none.to_string()
    } else {
        symbols.join(", ")
    }
}

fn is_fixture_source_url(url: &str) -> bool {
    url.contains("tests/fixtures/") || url.contains("tests\\fixtures\\")
}

fn format_evidence_description(description: &str, language: Language) -> String {
    if is_fixture_source_url(description) {
        return match language {
            Language::ZhCn => "测试 fixture 证据说明已隐藏".to_string(),
            Language::EnUs => "Test fixture evidence description hidden".to_string(),
            Language::JaJp => "テスト fixture の証拠説明は非表示".to_string(),
        };
    }
    if description == "Manual ingestion via CLI" {
        return match language {
            Language::ZhCn => "通过 CLI 手动录入".to_string(),
            Language::EnUs => "Manual ingestion via CLI".to_string(),
            Language::JaJp => "CLI から手動入力".to_string(),
        };
    }
    match language {
        Language::ZhCn => "原始证据说明未提供中文版本".to_string(),
        Language::EnUs => description.to_string(),
        Language::JaJp => "元の証拠説明は日本語で未提供".to_string(),
    }
}

fn blocker_label(raw: &str, language: Language) -> String {
    match language {
        Language::ZhCn => match raw {
            "StabilityThreshold" => "稳定性不足".to_string(),
            "ContinuityThreshold" => "连续性不足".to_string(),
            "DirectionalCohesion" => "无主线".to_string(),
            "HighCandidateDispersion" => "候选过散".to_string(),
            "UnstableRotation" => "轮动不稳".to_string(),
            "WeakLeadership" => "领涨不足".to_string(),
            _ => raw.to_string(),
        },
        Language::EnUs => match raw {
            "StabilityThreshold" => "Low stability".to_string(),
            "ContinuityThreshold" => "Low continuity".to_string(),
            "DirectionalCohesion" => "No mainline".to_string(),
            "HighCandidateDispersion" => "Candidates too dispersed".to_string(),
            "UnstableRotation" => "Unstable rotation".to_string(),
            "WeakLeadership" => "Weak leadership".to_string(),
            _ => raw.to_string(),
        },
        Language::JaJp => match raw {
            "StabilityThreshold" => "安定性不足".to_string(),
            "ContinuityThreshold" => "連続性不足".to_string(),
            "DirectionalCohesion" => "主線未形成".to_string(),
            "HighCandidateDispersion" => "候補が分散しすぎ".to_string(),
            "UnstableRotation" => "ローテーション不安定".to_string(),
            "WeakLeadership" => "リーダーシップ不足".to_string(),
            _ => raw.to_string(),
        },
    }
}

fn summarize_blockers(window: &[&TransitionAuditEntry]) -> Vec<(String, usize)> {
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for entry in window {
        if entry.log.trend_cohesion_gate.to {
            continue;
        }
        let mut day_set = std::collections::HashSet::<String>::new();
        for item in &entry.log.trend_cohesion_gate.added {
            day_set.insert(item.clone());
        }
        for item in &entry.log.trend_cohesion_gate.persisting {
            day_set.insert(item.clone());
        }
        for key in day_set {
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut sorted = counts.into_iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
}

pub(crate) fn group_audit_days(entries: Vec<TransitionAuditEntry>) -> Vec<TransitionAuditDay> {
    let mut grouped = BTreeMap::<NaiveDate, Vec<TransitionAuditEntry>>::new();
    for entry in entries {
        grouped.entry(entry.date).or_default().push(entry);
    }

    let mut days = grouped
        .into_iter()
        .map(|(date, mut events)| {
            events.sort_by_key(|a| a.timestamp);
            TransitionAuditDay { date, events }
        })
        .collect::<Vec<_>>();
    days.sort_by_key(|a| a.date);
    days
}

struct BreakoutDailySummary {
    new_symbols: Vec<String>,
    continued_symbols: Vec<String>,
    removed_symbols: Vec<String>,
}

fn summarize_breakout_changes(
    changes: &[crate::features::radar::domain::transition_log::BreakoutTransition],
) -> BreakoutDailySummary {
    let mut new_symbols = Vec::new();
    let mut continued_symbols = Vec::new();
    let mut removed_symbols = Vec::new();
    for item in changes {
        let from = item.from_status;
        let to = item.to_status;
        if from == crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
            && to != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
        {
            new_symbols.push(item.symbol.clone());
        } else if from
            != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
            && to == crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
        {
            removed_symbols.push(item.symbol.clone());
        } else if from
            != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
            && to != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
        {
            continued_symbols.push(item.symbol.clone());
        }
    }
    new_symbols.sort();
    new_symbols.dedup();
    continued_symbols.sort();
    continued_symbols.dedup();
    removed_symbols.sort();
    removed_symbols.dedup();
    BreakoutDailySummary {
        new_symbols,
        continued_symbols,
        removed_symbols,
    }
}

fn summarize_breakout_changes_from_events(day: &TransitionAuditDay) -> BreakoutDailySummary {
    let mut merged = BreakoutDailySummary {
        new_symbols: Vec::new(),
        continued_symbols: Vec::new(),
        removed_symbols: Vec::new(),
    };
    for event in &day.events {
        let once = summarize_breakout_changes(&event.log.breakout_changes);
        merged.new_symbols.extend(once.new_symbols);
        merged.continued_symbols.extend(once.continued_symbols);
        merged.removed_symbols.extend(once.removed_symbols);
    }
    merged.new_symbols.sort();
    merged.new_symbols.dedup();
    merged.continued_symbols.sort();
    merged.continued_symbols.dedup();
    merged.removed_symbols.sort();
    merged.removed_symbols.dedup();
    merged
}

fn detect_no_trade_resets(window: &[TransitionAuditDay]) -> bool {
    window
        .iter()
        .flat_map(|day| day.events.iter())
        .any(|entry| entry.log.trend_cohesion_gate.from != entry.log.trend_cohesion_gate.to)
}

fn trend_status_label(
    status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus,
    language: Language,
) -> &'static str {
    match language {
        Language::ZhCn => match status {
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                "未形成"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                "形成中"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => "已形成",
        },
        Language::EnUs => match status {
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                "Not formed"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                "Forming"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => "Formed",
        },
        Language::JaJp => match status {
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                "未形成"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                "形成中"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => {
                "形成済み"
            }
        },
    }
}

fn summarize_breakout_sentence(summary: &BreakoutDailySummary, language: Language) -> String {
    let mut items = Vec::new();
    for symbol in &summary.new_symbols {
        items.push(format_breakout_item(symbol, language, "new"));
    }
    for symbol in &summary.continued_symbols {
        items.push(format_breakout_item(symbol, language, "continued"));
    }
    for symbol in &summary.removed_symbols {
        items.push(format_breakout_item(symbol, language, "removed"));
    }
    if items.is_empty() {
        audit_text(language).none.to_string()
    } else {
        items.join(", ")
    }
}

fn format_breakout_item(symbol: &str, language: Language, kind: &str) -> String {
    match language {
        Language::ZhCn => match kind {
            "new" => format!("{}（新增）", symbol),
            "continued" => format!("{}（延续）", symbol),
            _ => format!("{}（消失）", symbol),
        },
        Language::EnUs => match kind {
            "new" => format!("{} (new)", symbol),
            "continued" => format!("{} (continued)", symbol),
            _ => format!("{} (removed)", symbol),
        },
        Language::JaJp => match kind {
            "new" => format!("{}（新規）", symbol),
            "continued" => format!("{}（継続）", symbol),
            _ => format!("{}（消失）", symbol),
        },
    }
}

pub(crate) fn consecutive_streak<F>(
    days: &[TransitionAuditDay],
    target_idx: usize,
    predicate: F,
) -> usize
where
    F: Fn(&crate::features::radar::domain::transition_log::StateTransitionLog) -> bool,
{
    if !predicate(&days[target_idx].latest().log) {
        return 0;
    }

    let mut streak = 1usize;
    let mut idx = target_idx;
    while idx > 0 {
        let prev_idx = idx - 1;
        if !is_consecutive_trading_day(days[prev_idx].date, days[idx].date) {
            break;
        }
        if !predicate(&days[prev_idx].latest().log) {
            break;
        }
        streak += 1;
        idx = prev_idx;
    }
    streak
}

fn is_consecutive_trading_day(prev: NaiveDate, curr: NaiveDate) -> bool {
    if curr <= prev {
        return false;
    }

    let mut day = prev.succ_opt().unwrap_or(prev);
    while day < curr {
        match day.weekday() {
            Weekday::Sat | Weekday::Sun => {}
            _ => return false,
        }
        day = day.succ_opt().unwrap_or(day);
    }
    true
}

fn build_audit_sentence(
    language: Language,
    gate_status: &str,
    gate_streak: usize,
    blocker_text: &str,
    breakout_text: &str,
    mainline_text: &str,
    no_trade_mode: &str,
) -> String {
    match language {
        Language::ZhCn => format!(
            "{} 连续第 {} 天；主因：{}；NO TRADE 分层：{}；今日突破：{}；主线状态：{}。",
            gate_status, gate_streak, blocker_text, no_trade_mode, breakout_text, mainline_text
        ),
        Language::EnUs => format!(
            "{} day {} in a row; primary blockers: {}; NO TRADE tier: {}; today's breakout: {}; mainline status: {}.",
            gate_status, gate_streak, blocker_text, no_trade_mode, breakout_text, mainline_text
        ),
        Language::JaJp => format!(
            "{} 連続 {} 日目；主因：{}；NO TRADE レイヤー：{}；本日のブレイクアウト：{}；主線状態：{}。",
            gate_status, gate_streak, blocker_text, no_trade_mode, breakout_text, mainline_text
        ),
    }
}

pub(crate) fn audit_empty_log_message(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未找到可用的 state_transitions.jsonl 记录。",
        Language::EnUs => "No usable records found in state_transitions.jsonl.",
        Language::JaJp => "state_transitions.jsonl に有効な記録がありません。",
    }
}

pub(crate) fn audit_error_parse_date(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无法解析 --date",
        Language::EnUs => "Unable to parse --date",
        Language::JaJp => "--date を解析できません",
    }
}

pub(crate) fn audit_error_read_file(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无法读取文件",
        Language::EnUs => "Unable to read file",
        Language::JaJp => "ファイルを読み込めません",
    }
}

pub(crate) fn audit_error_parse_line(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "解析 state_transitions.jsonl 第",
        Language::EnUs => "Failed to parse state_transitions.jsonl line",
        Language::JaJp => "state_transitions.jsonl の行解析に失敗:",
    }
}

pub(crate) fn audit_error_invalid_timestamp(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无效 timestamp",
        Language::EnUs => "Invalid timestamp",
        Language::JaJp => "無効な timestamp",
    }
}

pub(crate) fn audit_error_invalid_date(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无效 date",
        Language::EnUs => "Invalid date",
        Language::JaJp => "無効な date",
    }
}

pub(crate) fn audit_error_target_date_not_found(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未找到目标日期的审计记录:",
        Language::EnUs => "No audit record found for target date:",
        Language::JaJp => "対象日の監査記録が見つかりません:",
    }
}

pub(crate) fn audit_daily_usage(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "用法:\n  cargo run -- audit_daily [--date YYYY-MM-DD] [--days N]\n  cargo run -- transition_audit_summary [--date YYYY-MM-DD] [--days N]"
        }
        Language::EnUs => {
            "Usage:\n  cargo run -- audit_daily [--date YYYY-MM-DD] [--days N]\n  cargo run -- transition_audit_summary [--date YYYY-MM-DD] [--days N]"
        }
        Language::JaJp => {
            "使い方:\n  cargo run -- audit_daily [--date YYYY-MM-DD] [--days N]\n  cargo run -- transition_audit_summary [--date YYYY-MM-DD] [--days N]"
        }
    }
}
