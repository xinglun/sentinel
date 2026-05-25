use crate::features::radar::application::evidence_assembly::assemble_substantive_evidence;
use crate::features::radar::domain::breakout_detection::BreakoutStatus;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::features::AssetFeatures;
use crate::features::radar::domain::rules::ParsedRules;
use crate::features::radar::domain::transition_log::StateTransitionLog;
use crate::features::radar::domain::trend_cohesion::{
    AutomatedEvidenceRecord, SubstantiveEvidence, TrendRecognitionEvidence,
};

/// Trend recognition の evidence 計算と packet / transition log への反映を担当する。
pub fn attach_trend_recognition(
    packet: &mut DecisionPacket,
    transition_log: &mut StateTransitionLog,
    asset_features: &[AssetFeatures],
    rules: &ParsedRules,
    evidence_history: &[AutomatedEvidenceRecord],
) {
    let (confirmed_count, emerging_count) = breakout_signal_counts(packet);
    let substantive =
        assemble_substantive_evidence(packet, asset_features, rules, evidence_history);
    let substantive = if substantive != SubstantiveEvidence::default() {
        Some(substantive)
    } else {
        None
    };

    let evidence = TrendRecognitionEvidence::compute(
        confirmed_count,
        emerging_count,
        transition_log.scout_days_without_expansion,
        transition_log.scout_abort_days,
        substantive,
        packet.date,
        &rules.market_state_engine,
    );

    packet.trend_recognition = Some(evidence.clone());
    transition_log.trend_recognition = Some(evidence);
}

fn breakout_signal_counts(packet: &DecisionPacket) -> (usize, usize) {
    let mut confirmed_count = 0;
    let mut emerging_count = 0;
    for decision in &packet.assets {
        match decision.breakout.status {
            BreakoutStatus::ConfirmedBreakout => confirmed_count += 1,
            BreakoutStatus::EmergingBreakout => emerging_count += 1,
            _ => {}
        }
    }
    (confirmed_count, emerging_count)
}
