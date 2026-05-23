use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 証拠ソースの種別。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvidenceSourceType {
    #[default]
    Manual,
    OfficialIR,
    NewsMedia,
    PriceAction,
}

/// 実体的証拠の分類。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvidenceType {
    #[default]
    CapexPayoff,
    EarningsValidation,
    OrderVisibility,
    FollowThrough,
}

/// 自動または手動で取り込まれた実体的証拠レコード。
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct AutomatedEvidenceRecord {
    pub source: EvidenceSourceType,
    pub evidence_type: EvidenceType,
    pub confidence: f64,
    pub description: String,
    pub event_date: String,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub dedupe_key: String,
}

/// 証拠の時間減衰を表す domain policy。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvidenceDecayPolicy {
    pub decay_limit_days: u32,
    pub long_memory_multiplier: f64,
}

impl EvidenceDecayPolicy {
    pub fn new(decay_limit_days: u32) -> Self {
        Self {
            decay_limit_days,
            long_memory_multiplier: 0.1,
        }
    }

    /// 経過日数に対する減衰倍率を返す。
    pub fn multiplier_for_days_ago(&self, days_ago: i64) -> f64 {
        if days_ago <= 1 {
            return 1.0;
        }

        let decay_limit = self.decay_limit_days as f64;
        if (days_ago as f64) <= decay_limit {
            let progress = if decay_limit > 1.0 {
                (days_ago as f64 - 1.0) / (decay_limit - 1.0)
            } else {
                1.0
            };
            return 1.0 - progress * 0.8;
        }

        self.long_memory_multiplier
    }

    /// レコード日付と評価日から減衰倍率を返す。
    pub fn multiplier_for_record_date(
        &self,
        current_date: NaiveDate,
        record_date: NaiveDate,
    ) -> f64 {
        self.multiplier_for_days_ago((current_date - record_date).num_days())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_decay_policy_keeps_t1_at_full_weight() {
        let policy = EvidenceDecayPolicy::new(5);
        assert_eq!(policy.multiplier_for_days_ago(0), 1.0);
        assert_eq!(policy.multiplier_for_days_ago(1), 1.0);
    }

    #[test]
    fn evidence_decay_policy_linearly_decays_to_limit_floor() {
        let policy = EvidenceDecayPolicy::new(5);
        assert!((policy.multiplier_for_days_ago(2) - 0.8).abs() < 1e-10);
        assert!((policy.multiplier_for_days_ago(3) - 0.6).abs() < 1e-10);
        assert!((policy.multiplier_for_days_ago(4) - 0.4).abs() < 1e-10);
        assert!((policy.multiplier_for_days_ago(5) - 0.2).abs() < 1e-10);
    }

    #[test]
    fn evidence_decay_policy_keeps_long_memory_after_limit() {
        let policy = EvidenceDecayPolicy::new(5);
        assert!((policy.multiplier_for_days_ago(6) - 0.1).abs() < 1e-10);
        assert!((policy.multiplier_for_days_ago(61) - 0.1).abs() < 1e-10);
    }

    #[test]
    fn evidence_decay_policy_uses_configured_decay_limit() {
        let policy = EvidenceDecayPolicy::new(30);
        assert_eq!(policy.multiplier_for_days_ago(1), 1.0);
        assert!((policy.multiplier_for_days_ago(30) - 0.2).abs() < 1e-10);
        assert!((policy.multiplier_for_days_ago(31) - 0.1).abs() < 1e-10);
    }
}
