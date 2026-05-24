use stock_sentinel::features::evidence::domain::evidence::EvidenceDecayPolicy;
use stock_sentinel::features::radar::application::policy::trend_cohesion::{
    AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType,
};

#[test]
fn core_reexport_preserves_evidence_record_schema() {
    let record = AutomatedEvidenceRecord::new(
        EvidenceSourceType::OfficialIR,
        EvidenceType::EarningsValidation,
        0.9,
        "official filing".to_string(),
        "2026-05-24".to_string(),
        Some("MSFT".to_string()),
        Some("https://example.com/filing".to_string()),
        "MSFT:2026-05-24:earnings".to_string(),
    );

    let json = serde_json::to_string(&record).expect("record should serialize");
    assert!(json.contains("EarningsValidation"));
    assert!(json.contains("OfficialIR"));

    let roundtrip: AutomatedEvidenceRecord =
        serde_json::from_str(&json).expect("record should deserialize");
    assert_eq!(roundtrip, record);
}

#[test]
fn domain_decay_policy_matches_substantive_evidence_contract() {
    let policy = EvidenceDecayPolicy::new(5);
    let observed = [
        policy.multiplier_for_days_ago(1),
        policy.multiplier_for_days_ago(2),
        policy.multiplier_for_days_ago(3),
        policy.multiplier_for_days_ago(4),
        policy.multiplier_for_days_ago(5),
        policy.multiplier_for_days_ago(6),
    ];
    assert_eq!(
        observed,
        [1.0, 0.8, 0.6, 0.3999999999999999, 0.19999999999999996, 0.1]
    );
}
