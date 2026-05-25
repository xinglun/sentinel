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
    dedupe_key: String,
}

impl AutomatedEvidenceRecord {
    /// 新しい実体的証拠レコードを作成する。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: EvidenceSourceType,
        evidence_type: EvidenceType,
        confidence: f64,
        description: String,
        event_date: String,
        symbol: Option<String>,
        source_url: Option<String>,
        dedupe_key: String,
    ) -> Self {
        Self {
            source,
            evidence_type,
            confidence,
            description,
            event_date,
            symbol,
            source_url,
            dedupe_key,
        }
    }

    /// 重複排除キーの参照を取得する。
    pub fn dedupe_key(&self) -> &str {
        &self.dedupe_key
    }

    /// 自動重複排除キーを生成して設定する。
    pub fn generate_auto_dedupe_key(&mut self) {
        self.dedupe_key = format!(
            "AUTO:{:?}:{:?}:{}:{}:{}",
            self.source,
            self.evidence_type,
            self.symbol.as_deref().unwrap_or("GLOBAL"),
            self.event_date,
            self.source_url.as_deref().unwrap_or("NO_URL")
        );
    }

    /// 重複排除キーを直接更新する（主にテストまたは手動注入用）。
    pub fn update_dedupe_key(&mut self, dedupe_key: String) {
        self.dedupe_key = dedupe_key;
    }

    /// 本番判断と監査証拠に利用可能な出自かを返す。
    pub fn is_production_eligible(&self) -> bool {
        !is_local_or_fixture_reference(&self.description)
            && !self
                .source_url
                .as_deref()
                .is_some_and(is_local_or_fixture_reference)
    }
}

fn is_local_or_fixture_reference(value: &str) -> bool {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    normalized.starts_with("file://") || normalized.contains("tests/fixtures/")
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

    #[test]
    fn fixture_or_local_evidence_is_not_production_eligible() {
        let fixture = AutomatedEvidenceRecord::new(
            EvidenceSourceType::OfficialIR,
            EvidenceType::CapexPayoff,
            0.8,
            "Detected CAPEX keywords in tests/fixtures/evidence/sample.html".to_string(),
            "2026-05-01".to_string(),
            Some("GOOG".to_string()),
            Some("file://tests/fixtures/evidence/sample.html".to_string()),
            String::new(),
        );
        let official = AutomatedEvidenceRecord::new(
            EvidenceSourceType::OfficialIR,
            EvidenceType::CapexPayoff,
            0.9,
            "Capital expenditure supported AI revenue.".to_string(),
            "2026-05-01".to_string(),
            Some("GOOG".to_string()),
            Some("https://abc.xyz/investor/earnings.html".to_string()),
            String::new(),
        );

        assert!(!fixture.is_production_eligible());
        assert!(official.is_production_eligible());
    }
}
