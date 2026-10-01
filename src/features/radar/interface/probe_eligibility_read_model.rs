//! canonical permission を観測へ一方向に投影する。
use super::presentation::{ExecutionWindow, FinalExecutionDecision, ParticipationMode};
use crate::features::radar::domain::probe_eligibility_observation::{
    aggregate, Coverage, Permission, ProbeEligibilityObservation, ProbeFact,
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
            x.history_coverage = Coverage::Partial;
            x.quality = Coverage::Partial;
        }
    }
    windows
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
        let boundary=match language {Language::EnUs=>"Observation only; Gate / Probe / Eligibility / Execution / Position Sizing unchanged.",Language::ZhCn=>"仅观察；不改变 Gate / Probe / Eligibility / Execution / Position Sizing。",Language::JaJp=>"観測専用。Gate / Probe / Eligibility / Execution / Position Sizing は変更しない。"};
        output.push_str(&format!(
            "{boundary} decision_weight=0; trade_signal=false\n"
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
            window.quality = Coverage::Partial;
            window.history_coverage = Coverage::Partial;
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
}
