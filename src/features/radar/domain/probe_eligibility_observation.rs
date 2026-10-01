//! 取引判断へ戻さない Probe permission の監査集計。
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Permission {
    Probe,
    NoTrade,
    Ready,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Coverage {
    Complete,
    Partial,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeFact {
    pub market_date: NaiveDate,
    pub permission: Permission,
    #[serde(default)]
    pub eligible_asset_count: Option<usize>,
    pub report_run_id: String,
    pub observed_at: String,
    pub provenance: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExitState {
    EligibleReached,
    ReturnedToNoTrade,
    EscalatedToReady,
    StillOpen,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeEpisode {
    pub episode_id: String,
    pub started_at: NaiveDate,
    pub ended_at: Option<NaiveDate>,
    pub trading_day_count: usize,
    pub converted_to_eligible: bool,
    pub first_eligible_at: Option<NaiveDate>,
    pub eligible_latency_trading_days: Option<usize>,
    pub exit_state: ExitState,
    pub exit_permission: Option<Permission>,
    pub censored: bool,
    pub eligible_days: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeEligibilityObservation {
    pub as_of: Option<NaiveDate>,
    pub history_coverage: Coverage,
    pub quality: Coverage,
    pub probe_open_days: usize,
    pub probe_days_with_eligible_assets: usize,
    pub eligible_known_probe_days: usize,
    pub probe_day_conversion_rate: Option<f64>,
    pub completed_probe_episodes: usize,
    pub converted_probe_episodes: usize,
    pub probe_episode_conversion_rate: Option<f64>,
    pub current_probe_streak_days: usize,
    pub current_probe_streak_eligible_days: usize,
    pub avg_completed_probe_episode_days: Option<f64>,
    pub median_first_eligible_latency_days: Option<f64>,
    pub fallback_to_no_trade_count: usize,
    pub fallback_to_no_trade_rate: Option<f64>,
    pub escalated_to_ready_count: usize,
    pub escalated_to_ready_rate: Option<f64>,
    pub unknown_day_count: usize,
    pub excluded_unknown_days: usize,
    pub excluded_unknown_eligibility_days: usize,
    pub episodes: Vec<ProbeEpisode>,
    pub decision_weight: u8,
    pub trade_signal: bool,
}
fn rate(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}

/// session は calendar 側で確定する。欠損日は UNKNOWN とし、将来 fact は参照しない。
pub fn aggregate(facts: &[ProbeFact], sessions: &[NaiveDate]) -> ProbeEligibilityObservation {
    let mut by_date: BTreeMap<NaiveDate, Vec<&ProbeFact>> = BTreeMap::new();
    for fact in facts {
        by_date.entry(fact.market_date).or_default().push(fact);
    }
    let mut sessions = sessions.to_vec();
    sessions.sort_unstable();
    sessions.dedup();
    let mut episodes = Vec::new();
    let mut active: Option<ProbeEpisode> = None;
    let mut previous = Permission::Unknown;
    let mut probe_days = 0;
    let mut eligible_days = 0;
    let mut unknown = 0;
    let mut unknown_eligibility = 0;
    let mut partial = sessions.is_empty();
    for date in &sessions {
        // 同一日・同一 timestamp の矛盾を配列順で解決しない。
        let selected = by_date.get(date).and_then(|rows| {
            if rows.iter().any(|r| {
                r.report_run_id.is_empty()
                    || r.provenance != "canonical-final-execution-v1"
                    || chrono::DateTime::parse_from_rfc3339(&r.observed_at).is_err()
            }) {
                return None;
            }

            let latest = rows
                .iter()
                .filter_map(|r| chrono::DateTime::parse_from_rfc3339(&r.observed_at).ok())
                .max()?;
            let current: Vec<_> = rows
                .iter()
                .copied()
                .filter(|r| {
                    chrono::DateTime::parse_from_rfc3339(&r.observed_at).ok() == Some(latest)
                })
                .collect();
            let first = *current.first()?;
            if first.report_run_id.is_empty()
                || first.provenance != "canonical-final-execution-v1"
                || current.iter().any(|r| {
                    r.permission != first.permission
                        || r.eligible_asset_count != first.eligible_asset_count
                })
            {
                None
            } else {
                Some(first)
            }
        });
        let permission = selected
            .map(|r| r.permission)
            .unwrap_or(Permission::Unknown);
        if permission == Permission::Probe {
            probe_days += 1;
            let eligible = selected.and_then(|r| r.eligible_asset_count);
            if eligible.is_none() {
                unknown_eligibility += 1;
                partial = true;
            }
            if eligible.is_some_and(|n| n > 0) {
                eligible_days += 1;
            }
            let episode = active.get_or_insert_with(|| ProbeEpisode {
                episode_id: format!("probe-{date}"),
                started_at: *date,
                ended_at: None,
                trading_day_count: 0,
                converted_to_eligible: false,
                first_eligible_at: None,
                eligible_latency_trading_days: None,
                exit_state: ExitState::StillOpen,
                exit_permission: None,
                censored: previous == Permission::Unknown,
                eligible_days: 0,
            });
            if episode.censored {
                partial = true;
            }
            if eligible.is_none() {
                episode.censored = true;
            }
            if eligible.is_some_and(|n| n > 0) {
                episode.eligible_days += 1;
                if episode.first_eligible_at.is_none() {
                    episode.first_eligible_at = Some(*date);
                    episode.eligible_latency_trading_days = Some(episode.trading_day_count);
                    episode.converted_to_eligible = true;
                }
            }
            episode.trading_day_count += 1;
        } else {
            if permission == Permission::Unknown {
                unknown += 1;
                partial = true;
            }
            if let Some(mut episode) = active.take() {
                episode.ended_at = Some(*date);
                episode.exit_permission = Some(permission);
                if permission == Permission::Unknown {
                    episode.censored = true;
                }
                episode.exit_state = if episode.censored {
                    ExitState::Unavailable
                } else if episode.converted_to_eligible {
                    ExitState::EligibleReached
                } else if permission == Permission::NoTrade {
                    ExitState::ReturnedToNoTrade
                } else {
                    ExitState::EscalatedToReady
                };
                episodes.push(episode);
            }
        }
        previous = permission;
    }
    let streak = active.as_ref().map(|e| e.trading_day_count).unwrap_or(0);
    let streak_eligible = active.as_ref().map(|e| e.eligible_days).unwrap_or(0);
    if let Some(e) = active {
        episodes.push(e);
    }
    let completed: Vec<_> = episodes
        .iter()
        .filter(|e| !e.censored && e.ended_at.is_some())
        .collect();
    let converted = completed.iter().filter(|e| e.converted_to_eligible).count();
    let fallback = completed
        .iter()
        .filter(|e| e.exit_state == ExitState::ReturnedToNoTrade)
        .count();
    let escalated = completed
        .iter()
        .filter(|e| e.exit_permission == Some(Permission::Ready))
        .count();
    let mut latencies: Vec<_> = episodes
        .iter()
        .filter(|e| !e.censored)
        .filter_map(|e| e.eligible_latency_trading_days)
        .collect();
    latencies.sort_unstable();
    let median = if latencies.is_empty() {
        None
    } else {
        Some((latencies[(latencies.len() - 1) / 2] + latencies[latencies.len() / 2]) as f64 / 2.0)
    };
    let count = completed.len();
    let average = rate(completed.iter().map(|e| e.trading_day_count).sum(), count);
    let quality = if partial {
        Coverage::Partial
    } else {
        Coverage::Complete
    };
    ProbeEligibilityObservation {
        as_of: sessions.last().copied(),
        history_coverage: quality,
        quality: if count == 0 || probe_days == unknown_eligibility {
            Coverage::Partial
        } else {
            quality
        },
        probe_open_days: probe_days,
        probe_days_with_eligible_assets: eligible_days,
        eligible_known_probe_days: probe_days - unknown_eligibility,
        probe_day_conversion_rate: rate(eligible_days, probe_days - unknown_eligibility),
        completed_probe_episodes: count,
        converted_probe_episodes: converted,
        probe_episode_conversion_rate: rate(converted, count),
        current_probe_streak_days: streak,
        current_probe_streak_eligible_days: streak_eligible,
        avg_completed_probe_episode_days: average,
        median_first_eligible_latency_days: median,
        fallback_to_no_trade_count: fallback,
        fallback_to_no_trade_rate: rate(fallback, count),
        escalated_to_ready_count: escalated,
        escalated_to_ready_rate: rate(escalated, count),
        unknown_day_count: unknown,
        excluded_unknown_days: unknown,
        excluded_unknown_eligibility_days: unknown_eligibility,
        episodes,
        decision_weight: 0,
        trade_signal: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn day(n: u32, permission: Permission, eligible: Option<usize>) -> ProbeFact {
        ProbeFact {
            market_date: NaiveDate::from_ymd_opt(2026, 9, n).unwrap(),
            permission,
            eligible_asset_count: eligible,
            report_run_id: format!("run-{n}"),
            observed_at: format!("2026-09-{n:02}T21:00:00Z"),
            provenance: "canonical-final-execution-v1".into(),
        }
    }
    fn observe(facts: &[ProbeFact]) -> ProbeEligibilityObservation {
        let sessions: Vec<_> = facts.iter().map(|x| x.market_date).collect();
        aggregate(facts, &sessions)
    }
    #[test]
    fn conversion_keeps_whole_contiguous_run_and_zero_based_latency() {
        let x = observe(&[
            day(1, Permission::NoTrade, Some(0)),
            day(2, Permission::Probe, Some(0)),
            day(3, Permission::Probe, Some(0)),
            day(4, Permission::Probe, Some(1)),
            day(5, Permission::Probe, Some(1)),
            day(6, Permission::NoTrade, Some(0)),
        ]);
        assert_eq!(x.probe_open_days, 4);
        assert_eq!(x.probe_days_with_eligible_assets, 2);
        assert_eq!(x.completed_probe_episodes, 1);
        assert_eq!(x.converted_probe_episodes, 1);
        assert_eq!(x.median_first_eligible_latency_days, Some(2.0));
        assert_eq!(x.probe_day_conversion_rate, Some(0.5));
        assert_eq!(x.fallback_to_no_trade_count, 0);
        assert_eq!(x.current_probe_streak_days, 0);
        assert_eq!(x.decision_weight, 0);
        assert!(!x.trade_signal);
    }
    #[test]
    fn fallback_and_escalation_are_distinct() {
        for (exit, fallback, escalated) in [(Permission::NoTrade, 1, 0), (Permission::Ready, 0, 1)]
        {
            let x = observe(&[
                day(1, Permission::NoTrade, Some(0)),
                day(2, Permission::Probe, Some(0)),
                day(3, Permission::Probe, Some(0)),
                day(4, exit, Some(0)),
            ]);
            assert_eq!(x.fallback_to_no_trade_count, fallback);
            assert_eq!(x.escalated_to_ready_count, escalated);
            assert_eq!(x.completed_probe_episodes, 1);
        }
    }
    #[test]
    fn left_censored_episode_is_not_completed_and_open_streak_is_observed_only() {
        let x = observe(&[
            day(1, Permission::Probe, Some(0)),
            day(2, Permission::Probe, Some(0)),
            day(3, Permission::NoTrade, Some(0)),
        ]);
        assert_eq!(x.completed_probe_episodes, 0);
        assert_eq!(x.history_coverage, Coverage::Partial);
        let y = observe(&[
            day(1, Permission::NoTrade, Some(0)),
            day(2, Permission::Probe, Some(1)),
            day(3, Permission::Probe, Some(0)),
        ]);
        assert_eq!(y.current_probe_streak_days, 2);
        assert_eq!(y.current_probe_streak_eligible_days, 1);
        assert_eq!(y.completed_probe_episodes, 0);
    }
    #[test]
    fn unknown_permission_and_eligibility_never_become_false() {
        let x = observe(&[
            day(1, Permission::NoTrade, Some(0)),
            day(2, Permission::Probe, None),
            day(3, Permission::Unknown, None),
            day(4, Permission::Probe, Some(1)),
            day(5, Permission::NoTrade, Some(0)),
        ]);
        assert_eq!(x.excluded_unknown_days, 1);
        assert_eq!(x.excluded_unknown_eligibility_days, 1);
        assert_eq!(x.completed_probe_episodes, 0);
        assert_eq!(x.history_coverage, Coverage::Partial);
        assert_eq!(x.probe_day_conversion_rate, Some(1.0));
    }
    #[test]
    fn missing_session_censors_and_empty_denominator_is_unavailable() {
        let facts = [
            day(1, Permission::NoTrade, Some(0)),
            day(2, Permission::Probe, Some(0)),
            day(4, Permission::NoTrade, Some(0)),
        ];
        let sessions = (1..=4)
            .map(|n| day(n, Permission::Unknown, None).market_date)
            .collect::<Vec<_>>();
        let x = aggregate(&facts, &sessions);
        assert_eq!(x.excluded_unknown_days, 1);
        assert_eq!(x.completed_probe_episodes, 0);
        let y = aggregate(&[], &[]);
        assert_eq!(y.probe_day_conversion_rate, None);
        assert_eq!(y.probe_episode_conversion_rate, None);
    }
    #[test]
    fn same_day_conflict_is_unknown_and_exact_duplicate_counts_once() {
        let a = day(1, Permission::Probe, Some(1));
        let mut b = a.clone();
        b.permission = Permission::NoTrade;
        let x = aggregate(&[a.clone(), a.clone()], &[a.market_date]);
        assert_eq!(x.probe_open_days, 1);
        let y = aggregate(&[a.clone(), b], &[a.market_date]);
        assert_eq!(y.excluded_unknown_days, 1);
    }

    #[test]
    fn complete_coverage_with_no_completed_sample_has_partial_quality() {
        let x = observe(&[
            day(1, Permission::NoTrade, Some(0)),
            day(2, Permission::NoTrade, Some(0)),
        ]);
        assert_eq!(x.history_coverage, Coverage::Complete);
        assert_eq!(x.quality, Coverage::Partial);
        let x = observe(&[
            day(1, Permission::NoTrade, Some(0)),
            day(2, Permission::Probe, Some(0)),
        ]);
        assert_eq!(x.quality, Coverage::Partial);
    }
    #[test]
    fn converted_episode_still_records_ready_exit() {
        let x = observe(&[
            day(1, Permission::NoTrade, Some(0)),
            day(2, Permission::Probe, Some(1)),
            day(3, Permission::Ready, Some(1)),
        ]);
        assert_eq!(x.converted_probe_episodes, 1);
        assert_eq!(x.escalated_to_ready_count, 1);
        assert_eq!(x.escalated_to_ready_rate, Some(1.0));
        assert_eq!(x.fallback_to_no_trade_count, 0);
    }
    #[test]
    fn invalid_provenance_or_time_never_depends_on_input_order() {
        let a = day(1, Permission::Probe, Some(1));
        for field in ["provenance", "identity", "time"] {
            let mut b = a.clone();
            match field {
                "provenance" => b.provenance = "renderer".into(),
                "identity" => b.report_run_id.clear(),
                _ => b.observed_at = "invalid".into(),
            };
            for rows in [vec![a.clone(), b.clone()], vec![b.clone(), a.clone()]] {
                let x = aggregate(&rows, &[a.market_date]);
                assert_eq!(x.excluded_unknown_days, 1);
            }
        }
    }
}
