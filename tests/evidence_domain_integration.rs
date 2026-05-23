use stock_sentinel::core::trend_cohesion::{
    AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType,
};
use stock_sentinel::domain::evidence::EvidenceDecayPolicy;

#[test]
fn core_reexport_preserves_evidence_record_schema() {
    let record = AutomatedEvidenceRecord {
        source: EvidenceSourceType::OfficialIR,
        evidence_type: EvidenceType::EarningsValidation,
        confidence: 0.9,
        description: "official filing".to_string(),
        event_date: "2026-05-24".to_string(),
        symbol: Some("MSFT".to_string()),
        source_url: Some("https://example.com/filing".to_string()),
        dedupe_key: "MSFT:2026-05-24:earnings".to_string(),
    };

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
