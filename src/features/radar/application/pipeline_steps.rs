use crate::features::radar::application::trend_recognition_service::attach_trend_recognition;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::features::AssetFeatures;
use crate::features::radar::domain::rules::ParsedRules;
use crate::features::radar::domain::transition_log::StateTransitionLog;
use crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord;

/// packet に transition log と trend recognition を付与する pipeline step。
pub fn attach_transition_context(
    packet: &mut DecisionPacket,
    prev_packet: Option<&DecisionPacket>,
    asset_features: &[AssetFeatures],
    rules: &ParsedRules,
    evidence_history: &[AutomatedEvidenceRecord],
) {
    let mut transition_log = StateTransitionLog::compare_with_rules(prev_packet, packet, rules);
    attach_trend_recognition(
        packet,
        &mut transition_log,
        asset_features,
        rules,
        evidence_history,
    );
    packet.transition_log = Some(transition_log);
}
