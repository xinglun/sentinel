use crate::features::radar::domain::breakout_detection::BreakoutStatus;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::features::AssetFeatures;
use crate::features::radar::domain::rules::ParsedRules;
use crate::features::radar::domain::trend_cohesion::{
    AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
};

/// DecisionPacket と feature signal から trend recognition 用の実体証拠を組み立てる。
pub fn assemble_substantive_evidence(
    packet: &DecisionPacket,
    asset_features: &[AssetFeatures],
    rules: &ParsedRules,
    evidence_history: &[AutomatedEvidenceRecord],
) -> SubstantiveEvidence {
    let mut substantive = SubstantiveEvidence::default();
    let current_date = packet.date;
    let mut min_days = usize::MAX;

    let evidence_retention_days = rules.market_state_engine.evidence_retention_days as i64;
    for rec in evidence_history {
        if let Ok(rec_date) = chrono::NaiveDate::parse_from_str(&rec.event_date, "%Y-%m-%d") {
            let days_ago = (current_date - rec_date).num_days();
            if (0..=evidence_retention_days).contains(&days_ago) {
                substantive.records.push(rec.clone());
                if (days_ago as usize) < min_days {
                    min_days = days_ago as usize;
                }
            }
        }
    }

    for decision in &packet.assets {
        if let Some(features) = asset_features
            .iter()
            .find(|asset| asset.symbol == decision.symbol)
        {
            let mut event_days_offset = 0;
            for signal in &features.event_signals {
                if let Some(stripped) = signal.strip_prefix("event_days:") {
                    if let Ok(days) = stripped.parse::<usize>() {
                        event_days_offset = days;
                        if days < min_days {
                            min_days = days;
                        }
                    }
                }
            }

            let record_date = current_date - chrono::Duration::days(event_days_offset as i64);
            let record_date_str = record_date.to_string();

            for signal in &features.event_signals {
                let new_record = manual_signal_record(signal, &decision.symbol, &record_date_str);
                if let Some(record) = new_record {
                    push_unique_record(&mut substantive, record);
                }
            }
        }
    }

    for decision in &packet.assets {
        let is_core = rules.core_assets.contains(&decision.symbol);
        let is_confirmed = decision.breakout.status == BreakoutStatus::ConfirmedBreakout;
        if is_core && is_confirmed && decision.breakout.breakout_age >= 3 {
            let dedupe = format!(
                "PriceAction:FollowThrough:{}:Age{}:{}",
                decision.symbol, decision.breakout.breakout_age, current_date
            );
            let record = AutomatedEvidenceRecord::new(
                EvidenceSourceType::PriceAction,
                EvidenceType::FollowThrough,
                0.9,
                format!(
                    "Automated FollowThrough: {} breakout maintained for {} days",
                    decision.symbol, decision.breakout.breakout_age
                ),
                current_date.to_string(),
                Some(decision.symbol.clone()),
                None,
                dedupe,
            );
            push_unique_record(&mut substantive, record);
            if 0 < min_days {
                min_days = 0;
            }
        }
    }

    substantive.event_days_since = if min_days == usize::MAX { 0 } else { min_days };
    substantive.aggregate();
    substantive
}

fn manual_signal_record(
    signal: &str,
    symbol: &str,
    record_date: &str,
) -> Option<AutomatedEvidenceRecord> {
    let (evidence_type, label) = match signal {
        "capex_payoff:true" => (EvidenceType::CapexPayoff, "Capex Payoff"),
        "earnings_validation:true" => (EvidenceType::EarningsValidation, "Earnings Validation"),
        "order_visibility:true" => (EvidenceType::OrderVisibility, "Order Visibility"),
        _ => return None,
    };
    let dedupe = format!("Manual:{:?}:{}:{}", evidence_type, symbol, record_date);
    Some(AutomatedEvidenceRecord::new(
        EvidenceSourceType::Manual,
        evidence_type,
        1.0,
        format!("Manual annotation: {}", label),
        record_date.to_string(),
        Some(symbol.to_string()),
        None,
        dedupe,
    ))
}

fn push_unique_record(substantive: &mut SubstantiveEvidence, record: AutomatedEvidenceRecord) {
    if !substantive
        .records
        .iter()
        .any(|existing| existing.dedupe_key() == record.dedupe_key())
    {
        substantive.records.push(record);
    }
}
