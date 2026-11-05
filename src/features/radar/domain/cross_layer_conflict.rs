use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 衝突する層の組合せ。将来の層追加に備えて列挙型にしているが、
/// 現時点ではS1+S2で実装済みの評価層対トレンド層の1組のみ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CrossLayerPair {
    ValuationVsTrend,
}

/// 衝突の方向の組合せ。方向対が変わると旧衝突が終了し新衝突として扱われる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CrossLayerDirectionPair {
    BullishValuationWeakeningTrend,
    BearishValuationImprovingTrend,
}

/// 当日の決定パケットから計算された、構造化された層間衝突。
/// 日報の表示用文字列(`"{symbol} · SIGNAL_CONFLICT"`)の元データであり、
/// 跨日観測ジャーナルへ記録する際の識別子でもある。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossLayerConflictEvent {
    pub symbol: String,
    pub layer_pair: CrossLayerPair,
    pub direction_pair: CrossLayerDirectionPair,
}

/// 跨日観測ジャーナルの1レコード。追記専用(append-only)で、過去日付への
/// 遡及書き込みはしない。ジャーナルはこのストア稼働開始時点から空で始まる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossLayerConflictObservation {
    pub symbol: String,
    pub layer_pair: CrossLayerPair,
    pub direction_pair: CrossLayerDirectionPair,
    pub observed_at: NaiveDate,
}

impl CrossLayerConflictObservation {
    pub fn from_event(event: &CrossLayerConflictEvent, observed_at: NaiveDate) -> Self {
        Self {
            symbol: event.symbol.clone(),
            layer_pair: event.layer_pair,
            direction_pair: event.direction_pair,
            observed_at,
        }
    }
}

/// 週報の未裁決衝突サマリに表示する1件。連続2営業日以上観測され、
/// かつ基準日(通常は週報生成日)にも観測されている衝突のみがこの形になる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedCrossLayerConflict {
    pub symbol: String,
    pub layer_pair: CrossLayerPair,
    pub direction_pair: CrossLayerDirectionPair,
    pub first_seen: NaiveDate,
    pub last_seen: NaiveDate,
    pub consecutive_days: u32,
}

/// 週報サマリへ表示するための最小連続観測日数。これ未満(単日/一回限り)は
/// ノイズとして扱い、サマリに出さない。
pub const MIN_CONSECUTIVE_DAYS_FOR_WEEKLY_SUMMARY: u32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_from_event_copies_identity_and_attaches_the_observed_date() {
        let event = CrossLayerConflictEvent {
            symbol: "NVDA".to_string(),
            layer_pair: CrossLayerPair::ValuationVsTrend,
            direction_pair: CrossLayerDirectionPair::BullishValuationWeakeningTrend,
        };
        let observed_at = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();

        let observation = CrossLayerConflictObservation::from_event(&event, observed_at);

        assert_eq!(observation.symbol, "NVDA");
        assert_eq!(observation.layer_pair, CrossLayerPair::ValuationVsTrend);
        assert_eq!(
            observation.direction_pair,
            CrossLayerDirectionPair::BullishValuationWeakeningTrend
        );
        assert_eq!(observation.observed_at, observed_at);
    }

    #[test]
    fn minimum_consecutive_days_threshold_excludes_single_day_noise() {
        assert_eq!(MIN_CONSECUTIVE_DAYS_FOR_WEEKLY_SUMMARY, 2);
    }
}
