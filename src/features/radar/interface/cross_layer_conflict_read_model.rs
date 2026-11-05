use crate::features::radar::domain::cross_layer_conflict::{
    CrossLayerConflictObservation, UnresolvedCrossLayerConflict,
    MIN_CONSECUTIVE_DAYS_FOR_WEEKLY_SUMMARY,
};
use crate::features::radar::interface::probe_eligibility_read_model::is_session;
use chrono::NaiveDate;
use std::collections::HashMap;

/// 直前の営業日を返す(土日・NYSE休日をスキップする)。
/// radar/domainはNYSE休日カレンダーを持つresearch/interfaceへ依存できないため、
/// 営業日判定はinterface層で行う。
fn previous_trading_day(date: NaiveDate) -> NaiveDate {
    let mut candidate = date.pred_opt().expect("date arithmetic underflow");
    while !is_session(candidate) {
        candidate = candidate.pred_opt().expect("date arithmetic underflow");
    }
    candidate
}

/// 跨日観測ジャーナルから、基準日時点で有効な(=基準日にも観測されている)
/// 未裁決衝突を導出する。
///
/// 連続性は「記録された観測日が直前の営業日と一致するか」で判定する。
/// 土日・NYSE休日を挟んでも連続は途切れないが、営業日を飛ばした観測の欠落が
/// あれば、そこで連続日数の遡りを打ち切る(滞留・誤った継続を防ぐ)。
/// 連続2営業日未満(単日/一回限り)の衝突はノイズとして除外する。
/// 基準日に観測されていない衝突(=解消済み)も除外する。
pub fn derive_unresolved_conflicts(
    observations: &[CrossLayerConflictObservation],
    as_of_date: NaiveDate,
) -> Vec<UnresolvedCrossLayerConflict> {
    let mut grouped: HashMap<(String, _, _), Vec<NaiveDate>> = HashMap::new();
    for observation in observations {
        grouped
            .entry((
                observation.symbol.clone(),
                observation.layer_pair,
                observation.direction_pair,
            ))
            .or_default()
            .push(observation.observed_at);
    }

    let mut result = Vec::new();
    for ((symbol, layer_pair, direction_pair), mut dates) in grouped {
        dates.sort_unstable();
        dates.dedup();
        if dates.last() != Some(&as_of_date) {
            // 基準日に観測されていない衝突は解消済みとして週報から除外する。
            continue;
        }
        let mut consecutive_days = 1u32;
        let mut cursor = as_of_date;
        for &date in dates.iter().rev().skip(1) {
            if date == previous_trading_day(cursor) {
                consecutive_days += 1;
                cursor = date;
            } else {
                break;
            }
        }
        if consecutive_days < MIN_CONSECUTIVE_DAYS_FOR_WEEKLY_SUMMARY {
            continue;
        }
        result.push(UnresolvedCrossLayerConflict {
            symbol,
            layer_pair,
            direction_pair,
            first_seen: cursor,
            last_seen: as_of_date,
            consecutive_days,
        });
    }
    result.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::cross_layer_conflict::CrossLayerDirectionPair;
    use crate::features::radar::domain::cross_layer_conflict::CrossLayerPair;
    use chrono::Datelike;

    fn observation(symbol: &str, observed_at: NaiveDate) -> CrossLayerConflictObservation {
        CrossLayerConflictObservation {
            symbol: symbol.to_string(),
            layer_pair: CrossLayerPair::ValuationVsTrend,
            direction_pair: CrossLayerDirectionPair::BullishValuationWeakeningTrend,
            observed_at,
        }
    }

    #[test]
    fn single_day_observation_is_excluded_as_noise() {
        let day1 = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        let unresolved = derive_unresolved_conflicts(&[observation("NVDA", day1)], day1);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn two_consecutive_trading_days_are_included() {
        let day1 = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        let unresolved = derive_unresolved_conflicts(
            &[observation("NVDA", day1), observation("NVDA", day2)],
            day2,
        );
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].symbol, "NVDA");
        assert_eq!(unresolved[0].first_seen, day1);
        assert_eq!(unresolved[0].last_seen, day2);
        assert_eq!(unresolved[0].consecutive_days, 2);
    }

    #[test]
    fn weekend_gap_does_not_break_consecutive_streak() {
        // 2026-11-06(金) -> 2026-11-09(月)
        let friday = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let monday = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        assert_eq!(friday.weekday(), chrono::Weekday::Fri);
        assert_eq!(monday.weekday(), chrono::Weekday::Mon);
        let unresolved = derive_unresolved_conflicts(
            &[observation("NVDA", friday), observation("NVDA", monday)],
            monday,
        );
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].consecutive_days, 2);
        assert_eq!(unresolved[0].first_seen, friday);
    }

    #[test]
    fn missed_trading_day_breaks_the_streak_and_restarted_single_day_is_excluded_as_noise() {
        let day1 = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 11, 3).unwrap();
        let day4 = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(); // day3(11/4)が欠落
        let unresolved = derive_unresolved_conflicts(
            &[
                observation("NVDA", day1),
                observation("NVDA", day2),
                observation("NVDA", day4),
            ],
            day4,
        );
        // 欠落で連続が途切れた結果、基準日時点の連続日数は1日に戻り、
        // 単日はノイズとして週報サマリから除外される(滞留させない)。
        assert!(unresolved.is_empty());
    }

    #[test]
    fn conflict_not_observed_on_as_of_date_is_resolved_and_excluded() {
        let day1 = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        let day3 = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        // NVDAは day1, day2 のみで観測され、基準日 day3 には観測されていない。
        let unresolved = derive_unresolved_conflicts(
            &[observation("NVDA", day1), observation("NVDA", day2)],
            day3,
        );
        assert!(unresolved.is_empty());
    }

    #[test]
    fn empty_journal_produces_no_unresolved_conflicts() {
        let day1 = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        assert!(derive_unresolved_conflicts(&[], day1).is_empty());
    }
}
