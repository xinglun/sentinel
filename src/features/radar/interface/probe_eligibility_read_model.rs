//! canonical permission を観測へ一方向に投影する。
use super::presentation::{ExecutionWindow, FinalExecutionDecision, ParticipationMode};
use crate::features::radar::domain::probe_eligibility_observation::{
    aggregate, Coverage, HistoryQualityReason, Permission, ProbeEligibilityObservation, ProbeFact,
};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeObservationWindows {
    pub session_20: ProbeEligibilityObservation,
    pub session_60: ProbeEligibilityObservation,
    pub all_history: ProbeEligibilityObservation,
    pub history_read_failed: bool,
}

pub(crate) fn canonical_fact(
    decision: &FinalExecutionDecision,
    market_date: NaiveDate,
    run_id: &str,
    observed_at: &str,
) -> ProbeFact {
    let permission = match (decision.execution_window, decision.participation_mode) {
        (ExecutionWindow::Limited, ParticipationMode::Probe) => Permission::Probe,
        (ExecutionWindow::Open, ParticipationMode::Add) => Permission::Ready,
        (ExecutionWindow::None, ParticipationMode::None) => Permission::NoTrade,
        _ => Permission::Unknown,
    };
    ProbeFact {
        market_date,
        permission,
        eligible_asset_count: Some(decision.eligible_asset_count),
        report_run_id: run_id.into(),
        observed_at: observed_at.into(),
        provenance: "canonical-final-execution-v1".into(),
    }
}
pub(crate) fn is_session(date: NaiveDate) -> bool {
    !matches!(date.weekday(),chrono::Weekday::Sat|chrono::Weekday::Sun) && !crate::features::research::interface::macro_event_official_calendar_adapter::nyse_market_holidays(date.year()).contains(&date)
}
fn sessions_between(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut date = start;
    let mut dates = Vec::new();
    while date <= end {
        if is_session(date) {
            dates.push(date);
        }
        let Some(next) = date.succ_opt() else { break };
        date = next;
    }
    dates
}
fn mark_history_io_unavailable(observation: &mut ProbeEligibilityObservation) {
    observation.history_coverage = Coverage::Partial;
    observation.quality = Coverage::Partial;
    if !observation
        .history_quality_reasons
        .contains(&HistoryQualityReason::HistoryIoUnavailable)
    {
        observation
            .history_quality_reasons
            .push(HistoryQualityReason::HistoryIoUnavailable);
    }
}
pub(crate) fn build_windows(
    facts: &[ProbeFact],
    as_of: NaiveDate,
    history_start: Option<NaiveDate>,
    failed: bool,
) -> ProbeObservationWindows {
    let mut last60 = Vec::new();
    let mut date = as_of;
    while last60.len() < 60 {
        if is_session(date) {
            last60.push(date);
        }
        let Some(previous) = date.pred_opt() else {
            break;
        };
        date = previous;
    }
    last60.reverse();
    let first = facts
        .iter()
        .filter(|r| r.market_date <= as_of)
        .map(|r| r.market_date)
        .chain(history_start.filter(|d| *d <= as_of))
        .min()
        .unwrap_or(as_of);
    let all = sessions_between(first, as_of);
    let mut windows = ProbeObservationWindows {
        session_20: aggregate(facts, &last60[last60.len().saturating_sub(20)..]),
        session_60: aggregate(facts, &last60),
        all_history: aggregate(facts, &all),
        history_read_failed: failed,
    };
    if failed {
        for x in [
            &mut windows.session_20,
            &mut windows.session_60,
            &mut windows.all_history,
        ] {
            mark_history_io_unavailable(x);
        }
    }
    windows
}

fn history_quality_reason_text(
    reason: HistoryQualityReason,
    language: crate::features::shared::interface::i18n::Language,
) -> &'static str {
    use crate::features::shared::interface::i18n::Language;
    match (language, reason) {
        (Language::EnUs, HistoryQualityReason::MissingCanonicalPermissionFacts) => {
            "Canonical permission facts are missing for some sessions"
        }
        (Language::ZhCn, HistoryQualityReason::MissingCanonicalPermissionFacts) => {
            "部分交易日缺少 canonical permission fact"
        }
        (Language::JaJp, HistoryQualityReason::MissingCanonicalPermissionFacts) => {
            "一部の取引日に canonical permission fact がありません"
        }
        (Language::EnUs, HistoryQualityReason::MissingProbeEligibilityFacts) => {
            "Probe eligible asset counts are missing for some known Probe days"
        }
        (Language::ZhCn, HistoryQualityReason::MissingProbeEligibilityFacts) => {
            "部分已知 Probe 日缺少 Eligible 资产数量"
        }
        (Language::JaJp, HistoryQualityReason::MissingProbeEligibilityFacts) => {
            "一部の既知 Probe 日で Eligible 資産数が不明です"
        }
        (Language::EnUs, HistoryQualityReason::UnknownProbeEpisodeContinuity) => {
            "Probe episode continuity is unknown"
        }
        (Language::ZhCn, HistoryQualityReason::UnknownProbeEpisodeContinuity) => {
            "Probe 区间连续性未知"
        }
        (Language::JaJp, HistoryQualityReason::UnknownProbeEpisodeContinuity) => {
            "Probe 区間の連続性は不明です"
        }
        (Language::EnUs, HistoryQualityReason::InsufficientCompletedProbeEpisodes) => {
            "No completed Probe episode sample"
        }
        (Language::ZhCn, HistoryQualityReason::InsufficientCompletedProbeEpisodes) => {
            "没有已完成 Probe 区间样本"
        }
        (Language::JaJp, HistoryQualityReason::InsufficientCompletedProbeEpisodes) => {
            "完了した Probe 区間の標本がありません"
        }
        (Language::EnUs, HistoryQualityReason::HistoryIoUnavailable) => {
            "Canonical history I/O is unavailable"
        }
        (Language::ZhCn, HistoryQualityReason::HistoryIoUnavailable) => "canonical 历史读取不可用",
        (Language::JaJp, HistoryQualityReason::HistoryIoUnavailable) => {
            "canonical 履歴を読み取れません"
        }
    }
}

fn permission_label(permission: Permission) -> &'static str {
    match permission {
        Permission::Probe => "PROBE",
        Permission::NoTrade => "NO_TRADE",
        Permission::Ready => "READY",
        Permission::Unknown => "UNKNOWN",
    }
}

fn html_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        escaped.push_str(match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&#39;",
            _ => {
                escaped.push(character);
                continue;
            }
        });
    }
    escaped
}

pub(crate) fn render(
    windows: &ProbeObservationWindows,
    language: crate::features::shared::interface::i18n::Language,
    html: bool,
    archive: bool,
) -> String {
    use crate::features::shared::interface::i18n::Language;
    let labels = match language {
        Language::EnUs => [
            "Probe Effectiveness Observation",
            "Current Probe Streak",
            "Eligible Days in Current Streak",
            "Probe Open Days",
            "Probe Days with Eligible Assets",
            "Probe Day Conversion",
            "Completed Probe Episodes",
            "Converted Episodes",
            "Episode Conversion",
            "Average Completed Episode Days",
            "Median First Eligible Latency",
            "NO TRADE Fallback",
            "Ready Escalation",
            "Excluded Unknown Permission Days",
            "Excluded Unknown Eligibility Days",
            "History Coverage",
            "trading sessions",
            "all available history",
        ],
        Language::ZhCn => [
            "Probe 有效性观察",
            "当前 Probe 连续交易日",
            "当前连续区间 Eligible 日",
            "Probe 开放日",
            "存在 Eligible 资产的 Probe 日",
            "Probe 日转化率",
            "已完成 Probe 区间",
            "已转化区间",
            "区间转化率",
            "已完成区间平均交易日",
            "首次 Eligible 延迟中位交易日",
            "回退 NO TRADE",
            "升级 Ready",
            "排除权限未知日",
            "排除资格未知日",
            "历史覆盖",
            "交易日窗口",
            "全部可用历史",
        ],
        Language::JaJp => [
            "Probe 有効性観測",
            "現在の Probe 連続取引日",
            "現在区間の Eligible 日",
            "Probe 開放日",
            "Eligible 資産ありの Probe 日",
            "Probe 日転換率",
            "完了 Probe 区間",
            "転換済み区間",
            "区間転換率",
            "完了区間の平均取引日",
            "初回 Eligible 遅延の中央値（取引日）",
            "NO TRADE への復帰",
            "Ready への移行",
            "権限不明の除外日",
            "資格不明の除外日",
            "履歴網羅性",
            "取引日窓",
            "利用可能な全履歴",
        ],
    };
    let detail_labels = match language {
        Language::EnUs => [
            "Window Sessions",
            "Permission Known Days",
            "Unknown Permission Days",
            "Permission Coverage",
            "Known Probe Eligibility Days",
            "Unknown Probe Eligibility Days",
            "Canonical History Start",
            "History Quality Reason",
            "Canonical Fact Sources",
        ],
        Language::ZhCn => [
            "窗口交易日数",
            "权限已知日",
            "权限未知日",
            "权限覆盖",
            "已知 Probe 资格日",
            "未知 Probe 资格日",
            "Canonical 历史开始日",
            "历史质量原因",
            "Canonical 事实来源",
        ],
        Language::JaJp => [
            "取引セッション数",
            "権限既知日",
            "権限不明日",
            "権限カバレッジ",
            "既知の Probe 適格日",
            "未知の Probe 適格日",
            "Canonical 履歴開始日",
            "履歴品質の理由",
            "Canonical fact の出典",
        ],
    };
    let mut output = String::new();
    let mut choices = vec![(format!("20 {}", labels[16]), &windows.session_20)];
    if archive {
        choices.push((format!("60 {}", labels[16]), &windows.session_60));
        choices.push((labels[17].into(), &windows.all_history));
    }
    for (window, x) in choices {
        let percent = |n: Option<f64>| {
            n.map(|v| format!("{:.1}%", v * 100.0))
                .unwrap_or_else(|| "UNAVAILABLE".into())
        };
        let decimal = |n: Option<f64>| {
            n.map(|v| format!("{v:.1}"))
                .unwrap_or_else(|| "UNAVAILABLE".into())
        };
        let values = [
            x.current_probe_streak_days.to_string(),
            x.current_probe_streak_eligible_days.to_string(),
            x.probe_open_days.to_string(),
            x.probe_days_with_eligible_assets.to_string(),
            format!(
                "{} ({}/{})",
                percent(x.probe_day_conversion_rate),
                x.probe_days_with_eligible_assets,
                x.eligible_known_probe_days
            ),
            x.completed_probe_episodes.to_string(),
            x.converted_probe_episodes.to_string(),
            percent(x.probe_episode_conversion_rate),
            decimal(x.avg_completed_probe_episode_days),
            decimal(x.median_first_eligible_latency_days),
            format!(
                "{}/{} ({})",
                x.fallback_to_no_trade_count,
                x.completed_probe_episodes,
                percent(x.fallback_to_no_trade_rate)
            ),
            format!(
                "{}/{} ({})",
                x.escalated_to_ready_count,
                x.completed_probe_episodes,
                percent(x.escalated_to_ready_rate)
            ),
            x.excluded_unknown_days.to_string(),
            x.excluded_unknown_eligibility_days.to_string(),
            match x.history_coverage {
                Coverage::Complete => "COMPLETE",
                Coverage::Partial => "PARTIAL",
            }
            .into(),
        ];
        if html {
            output.push_str(&format!("\n<b>{} ({window})</b>\n", labels[0]));
        } else {
            output.push_str(&format!("\n### {} ({window})\n\n", labels[0]));
        }
        for (label, value) in labels[1..16].iter().zip(values) {
            output.push_str(&format!("- {label}: {value}\n"));
        }
        let quality_reasons = if x.history_quality_reasons.is_empty() {
            "NONE".to_string()
        } else {
            x.history_quality_reasons
                .iter()
                .map(|reason| history_quality_reason_text(*reason, language))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let canonical_sources = if x.canonical_provenance.is_empty() {
            "NONE".to_string()
        } else {
            x.canonical_provenance
                .iter()
                .map(|source| {
                    format!(
                        "{} provenance={} report_run_id={} observed_at={} permission={} eligible_asset_count={}",
                        source.market_date,
                        source.provenance,
                        source.report_run_id,
                        source.observed_at,
                        permission_label(source.permission),
                        source.eligible_asset_count
                            .map(|count| count.to_string())
                            .unwrap_or_else(|| "UNKNOWN".into())
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let detail_values = [
            x.history_window_sessions.to_string(),
            x.known_permission_days.to_string(),
            x.unknown_permission_days.to_string(),
            format!(
                "{} ({}/{})",
                percent(x.permission_coverage_rate),
                x.known_permission_days,
                x.history_window_sessions
            ),
            x.known_probe_eligibility_days.to_string(),
            x.unknown_probe_eligibility_days.to_string(),
            x.canonical_history_started_at
                .map(|date| date.to_string())
                .unwrap_or_else(|| "UNKNOWN".into()),
            quality_reasons,
            canonical_sources,
        ];
        for (index, (label, value)) in detail_labels.iter().zip(detail_values).enumerate() {
            let value = if html && index == 8 {
                html_escape(&value)
            } else {
                value
            };
            output.push_str(&format!("- {label}: {value}\n"));
        }
        let quality_label = match language {
            Language::EnUs => "Statistical Quality",
            Language::ZhCn => "统计质量",
            Language::JaJp => "統計品質",
        };
        output.push_str(&format!(
            "- {quality_label}: {}\n",
            match x.quality {
                Coverage::Complete => "COMPLETE",
                Coverage::Partial => "PARTIAL",
            }
        ));
        let boundary = match language {
            Language::EnUs => {
                "Observation only; Gate / Probe / Eligibility / Execution / Position Sizing unchanged."
            }
            Language::ZhCn => {
                "仅观察；不改变 Gate / Probe / Eligibility / Execution / Position Sizing。"
            }
            Language::JaJp => {
                "観測専用。Gate / Probe / Eligibility / Execution / Position Sizing は変更しない。"
            }
        };
        output.push_str(&format!(
            "{boundary} decision_weight=0; trade_signal=false; gate_effect=none; execution_effect=none; position_sizing_effect=none\n"
        ));
        if windows.history_read_failed {
            output.push_str("HISTORY_IO_UNAVAILABLE\n");
        }
    }
    output
}

pub(crate) fn observe_after_decision(
    persistence: &crate::features::radar::infrastructure::persistence::PersistenceLayer,
    decision: &FinalExecutionDecision,
    market_date: NaiveDate,
    run_id: &str,
    observed_at: &str,
    persist: bool,
) -> ProbeObservationWindows {
    let mut failed = false;
    let mut facts = persistence.load_probe_facts().unwrap_or_else(|_| {
        failed = true;
        Vec::new()
    });
    let start = persistence
        .load_trading_day_snapshots()
        .map(|rows| rows.iter().map(|r| r.market_date).min())
        .unwrap_or_else(|_| {
            failed = true;
            None
        });
    if let Ok(cutoff) = chrono::DateTime::parse_from_rfc3339(observed_at) {
        facts.retain(|r| {
            chrono::DateTime::parse_from_rfc3339(&r.observed_at)
                .map(|t| t <= cutoff)
                .unwrap_or(true)
        });
    } else {
        failed = true;
        facts.clear();
    }
    if persist && is_session(market_date) {
        let fact = canonical_fact(decision, market_date, run_id, observed_at);
        if persistence.save_probe_fact(&fact).is_err() {
            failed = true;
        }
        facts.push(fact);
    }
    let mut windows = build_windows(&facts, market_date, start, failed);
    if persist
        && persistence
            .save_probe_observation_archive(run_id, &windows)
            .is_err()
    {
        windows.history_read_failed = true;
        for window in [
            &mut windows.session_20,
            &mut windows.session_60,
            &mut windows.all_history,
        ] {
            mark_history_io_unavailable(window);
        }
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::super::presentation::{ExecutionWindow, FinalExecutionDecision, ParticipationMode};
    use super::*;
    #[test]
    fn canonical_projection_preserves_decision_bytes() {
        let decision = FinalExecutionDecision {
            execution_window: ExecutionWindow::Limited,
            participation_mode: ParticipationMode::Probe,
            eligible_asset_count: 2,
            ..Default::default()
        };
        let before = serde_json::to_vec(&decision).unwrap();
        let fact = canonical_fact(
            &decision,
            chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
            "run-1",
            "2026-09-30T21:00:00Z",
        );
        assert_eq!(fact.permission, Permission::Probe);
        assert_eq!(fact.eligible_asset_count, Some(2));
        assert_eq!(before, serde_json::to_vec(&decision).unwrap());
    }
    #[test]
    fn nyse_sessions_exclude_weekend_and_labor_day() {
        let dates = sessions_between(
            chrono::NaiveDate::from_ymd_opt(2026, 9, 4).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 8).unwrap(),
        );
        assert_eq!(
            dates.iter().map(|d| d.day()).collect::<Vec<_>>(),
            vec![4, 8]
        );
    }
    #[test]
    fn windows_use_sessions_and_keep_all_history_separate() {
        let as_of = chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let x = build_windows(
            &[],
            as_of,
            Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            false,
        );
        assert_eq!(x.session_20.excluded_unknown_days, 20);
        assert_eq!(x.session_60.excluded_unknown_days, 60);
        assert!(x.all_history.excluded_unknown_days > 60);
    }

    #[test]
    fn history_io_failure_is_distinct_from_missing_canonical_facts() {
        let as_of = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let missing = build_windows(&[], as_of, Some(as_of), false).session_20;
        let failed = build_windows(&[], as_of, Some(as_of), true).session_20;

        assert_eq!(missing.history_window_sessions, 20);
        assert_eq!(missing.unknown_permission_days, 20);
        assert_eq!(missing.history_coverage, Coverage::Partial);
        assert!(missing
            .history_quality_reasons
            .contains(&crate::features::radar::domain::probe_eligibility_observation::HistoryQualityReason::MissingCanonicalPermissionFacts));
        assert!(!missing
            .history_quality_reasons
            .contains(&crate::features::radar::domain::probe_eligibility_observation::HistoryQualityReason::HistoryIoUnavailable));

        assert_eq!(failed.history_window_sessions, 20);
        assert_eq!(failed.unknown_permission_days, 20);
        assert_eq!(failed.history_coverage, Coverage::Partial);
        assert!(failed
            .history_quality_reasons
            .contains(&crate::features::radar::domain::probe_eligibility_observation::HistoryQualityReason::MissingCanonicalPermissionFacts));
        assert!(failed
            .history_quality_reasons
            .contains(&crate::features::radar::domain::probe_eligibility_observation::HistoryQualityReason::HistoryIoUnavailable));
    }
}
