use crate::features::radar::application::trend_recognition_service::attach_trend_recognition;
use crate::features::radar::domain::action_matrix::AssetActionDecision;
use crate::features::radar::domain::breakout_detection::BreakoutStatus;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::features::{AssetFeatures, MarketFeatures};
use crate::features::radar::domain::market_state::models::MarketStateOutput;
use crate::features::radar::domain::rules::ParsedRules;
use crate::features::radar::domain::transition_log::StateTransitionLog;
use crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord;
use crate::features::radar::domain::trend_cohesion::{TrendCohesionSnapshot, TrendCohesionStatus};

/// market state engine に渡す context を組み立てる pipeline step。
pub fn derive_market_state(
    rules: &ParsedRules,
    market_features: &MarketFeatures,
    trend_cohesion: &TrendCohesionSnapshot,
    final_decisions: &[AssetActionDecision],
    prev_packet: Option<&DecisionPacket>,
) -> MarketStateOutput {
    let current_breakouts: Vec<String> = final_decisions
        .iter()
        .filter(|decision| {
            matches!(
                decision.breakout.status,
                BreakoutStatus::EmergingBreakout | BreakoutStatus::ConfirmedBreakout
            )
        })
        .map(|decision| decision.symbol.clone())
        .collect();
    let has_mainline = matches!(trend_cohesion.status, TrendCohesionStatus::Formed);

    crate::features::radar::domain::market_state::engine::DecisionEngine::process(
        &rules.market_state_engine,
        market_features,
        has_mainline,
        &current_breakouts,
        prev_packet,
    )
}

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
