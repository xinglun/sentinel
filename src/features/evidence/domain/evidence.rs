pub use crate::features::shared::domain::evidence::{
    AutomatedEvidenceRecord, EvidenceDecayPolicy, EvidenceSourceType, EvidenceType,
};

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
