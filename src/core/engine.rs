use crate::config::{ParsedRules, WatchlistEntry};
use crate::core::features::{AssetFeatures, MarketFeatures};
use crate::core::market_regime::MarketRegimeStateMachine;
use crate::data::yahoo_provider::TickerHistory;

use crate::core::asset_state::{AssetState, AssetStateMachine};
use crate::core::portfolio_policy::PortfolioPolicy;

use crate::core::action_matrix::ActionMatrix;
use crate::core::breakout_detection::BreakoutEvaluator;
use crate::core::decision::DecisionPacket;
use crate::core::participation::ParticipationReadiness;
use crate::core::trend_cohesion::TrendCohesionEvaluator;
use anyhow::Result;

pub struct Engine;

impl Engine {
    /// Runs the complete modular decision pipeline for a set of assets.
    /// Returns a deterministic DecisionPacket based on current features and optional previous session context.
    pub fn run_daily_pipeline<'a>(
        ticker_histories: &[(TickerHistory<'a>, &WatchlistEntry)],
        rules: &ParsedRules,
        history: &[DecisionPacket],
        positions: &std::collections::HashMap<String, (f64, f64)>,
    ) -> Result<DecisionPacket> {
        if ticker_histories.is_empty() {
            return Err(anyhow::anyhow!("No ticker history provided for pipeline"));
        }

        let prev_packet = history.last();

        // 0. Extract Previous Context
        let _prev_state = prev_packet.map(|p| p.market_regime.market_state);
        let prev_age = prev_packet
            .map(|p| p.market_features.regime_age)
            .unwrap_or(0);

        // 1. Feature Layer: Asset Features
        let mut asset_features = Vec::new();
        for (history, entry) in ticker_histories {
            let f = AssetFeatures::compute(history, entry, rules);

            asset_features.push(f);
        }

        // 2. Market Features (Initial - using prev_packet for context)
        let mut market_features =
            MarketFeatures::compute(&asset_features, prev_age, prev_packet, rules);

        // 3. Market Regime State Machine (Consolidated Transition)
        let (market_regime, _) = MarketRegimeStateMachine::transition(
            prev_packet.map(|p| &p.market_regime),
            &mut market_features,
            prev_age,
            rules,
        );

        // 4. Portfolio Policy
        let portfolio_policy = PortfolioPolicy::from_market_regime(&market_regime);

        // 5. Relative Strength Memory Layer (NEW V1.3)
        let mut memory_decisions = std::collections::HashMap::new();
        for f in &asset_features {
            let prev_asset_snapshot = prev_packet
                .and_then(|p| p.assets.iter().find(|a| a.symbol == f.symbol))
                .map(|a| &a.asset_state);

            let mem = AssetStateMachine::compute_asset_strength_memory(&f.symbol, history, rules);
            let decision = AssetStateMachine::build_asset_strength_decision(
                f,
                &mem,
                prev_asset_snapshot.map(|s| s.state),
                rules,
            );
            memory_decisions.insert(f.symbol.clone(), decision);
        }

        // 6. Memory-Adjusted Ranking (Moved up for Readiness)
        let ranked_symbols =
            AssetStateMachine::rank_assets_with_memory(&asset_features, &memory_decisions);
        let current_top_tier: Vec<String> = ranked_symbols.iter().take(3).cloned().collect();

        // 7. Participation Readiness (Legacy)
        let participation = ParticipationReadiness::compute(
            market_features.stability_score,
            &current_top_tier,
            history,
            rules.trend_cohesion.gate_stability_threshold,
            rules.trend_cohesion.gate_continuity_threshold,
        );

        // 8. Trend Cohesion Snapshot (NEW V2 Gate) moved up to supersede readiness constraints
        let trend_cohesion = TrendCohesionEvaluator::evaluate(
            market_features.stability_score,
            participation.core_tier_streak,
            &current_top_tier,
            history,
            &rules.trend_cohesion,
        );
        let active_trend_gate_passed = trend_cohesion.gate_passed;

        let prev_trend_gate_passed = prev_packet
            .map(|p| p.trend_cohesion.gate_passed)
            .unwrap_or(false);
        let trend_gate_changed = prev_trend_gate_passed != active_trend_gate_passed;

        // 6. Asset Execution State & Action Matrix
        let mut asset_decisions = Vec::new();
        for f in &asset_features {
            let prev_asset_decision =
                prev_packet.and_then(|p| p.assets.iter().find(|a| a.symbol == f.symbol));

            let prev_asset_snapshot = prev_asset_decision.map(|a| &a.asset_state);

            let memory_decision = memory_decisions.get(&f.symbol);

            let asset_state_snapshot =
                AssetStateMachine::compute_state(f, rules, prev_asset_snapshot, memory_decision);

            // Streak Calculations
            let is_in_top_tier = current_top_tier.contains(&f.symbol);
            let (state_streak, top_tier_streak, out_of_top_tier_streak) = match prev_asset_decision
            {
                Some(prev) => {
                    let s_streak = if prev.asset_state.state == asset_state_snapshot.state {
                        prev.state_streak + 1
                    } else {
                        1
                    };
                    let tt_streak = if is_in_top_tier {
                        prev.top_tier_streak + 1
                    } else {
                        0
                    };
                    let out_tt_streak = if !is_in_top_tier {
                        prev.out_of_top_tier_streak + 1
                    } else {
                        0
                    };
                    (s_streak, tt_streak, out_tt_streak)
                }
                None => (
                    1,
                    if is_in_top_tier { 1 } else { 0 },
                    if !is_in_top_tier { 1 } else { 0 },
                ),
            };

            // Get per-asset constraints from watchlist entry
            let (_h, entry) = ticker_histories
                .iter()
                .find(|(h, _)| h.symbol == f.symbol)
                .unwrap();
            let trade_enabled = entry.trade_enabled.unwrap_or(true);
            let trade_amount = entry.trade_amount.unwrap_or(2000.0); // Safe default

            let state_name = format!("{:?}", asset_state_snapshot.state).to_lowercase();

            // Tiered Multiplier Selection
            let mut action_key = state_name.clone();
            if asset_state_snapshot.state == AssetState::OVERHEAT {
                action_key = if f.z_score.unwrap_or(0.0) >= 2.5 {
                    "overheat_2".to_string()
                } else {
                    "overheat_1".to_string()
                };
            } else if asset_state_snapshot.state == AssetState::CAUTION
                || asset_state_snapshot.state == AssetState::DEFEND
            {
                // Heuristic for fear tiers if needed, for now use fear_1/2 if present or fallback to state name
                action_key = if f.z_score.unwrap_or(0.0) <= -2.0 {
                    "fear_2".to_string()
                } else if f.z_score.unwrap_or(0.0) <= -1.0 {
                    "fear_1".to_string()
                } else {
                    state_name
                };
            }

            let config_multiplier = rules
                .sizing_multipliers
                .as_ref()
                .and_then(|m: &std::collections::HashMap<String, f64>| m.get(&action_key).copied())
                .unwrap_or(1.0);

            let mut decision = ActionMatrix::decide(
                &market_regime,
                &market_features,
                active_trend_gate_passed,
                &portfolio_policy,
                &asset_state_snapshot,
                f.deviation,
                f.z_score,
                f.close,
                trade_enabled,
                trade_amount,
                config_multiplier,
            );

            // Exit Decision Integration
            let exit_decision = crate::core::exit::ExitDecision::compute(
                &f.symbol,
                asset_state_snapshot.state,
                prev_asset_snapshot.map(|s| s.state),
                state_streak,
                out_of_top_tier_streak,
                market_regime.risk_overlay,
                active_trend_gate_passed,
                prev_trend_gate_passed,
            );

            // [P0-2] Synthesize PositionIntent (Isolated Lexicon)
            let final_intent = crate::core::intent_synthesizer::IntentSynthesizer::synthesize(
                decision.action,
                &exit_decision,
                active_trend_gate_passed,
            );
            let breakout = BreakoutEvaluator::evaluate(
                f,
                &asset_state_snapshot,
                state_streak,
                top_tier_streak,
                prev_asset_snapshot.map(|s| s.state),
                prev_asset_decision.map(|a| &a.breakout),
                &rules.breakout,
            );

            decision.prev_action = prev_asset_decision.map(|a| a.action);
            decision.action_changed = decision
                .prev_action
                .map(|p| p != decision.action)
                .unwrap_or(true);
            decision.exit_decision = exit_decision;
            decision.position_intent = final_intent;
            decision.breakout = breakout;
            decision.unified_position_intent =
                crate::core::position_intent::UnifiedIntentSynthesizer::synthesize(
                    final_intent,
                    &decision.exit_decision,
                    active_trend_gate_passed,
                    positions.contains_key(&decision.symbol),
                    asset_state_snapshot.state,
                )
                .intent;
            // Record domain facts for downstream presentation assembly
            decision.is_core_fact = rules.core_assets.contains(&decision.symbol);
            decision.has_position_fact = positions.contains_key(&decision.symbol);

            // Note: DisplayContext and DisplayIntent will be populated by PresentationAssembler at export time.
            decision.previous_state = prev_asset_snapshot.map(|s| s.state);
            decision.state_streak = state_streak;
            decision.top_tier_streak = top_tier_streak;
            decision.out_of_top_tier_streak = out_of_top_tier_streak;

            asset_decisions.push(decision);
        }

        // 9. Reorder based on ranking
        let mut final_decisions = Vec::with_capacity(asset_decisions.len());
        for symbol in ranked_symbols {
            if let Some(pos) = asset_decisions.iter().position(|d| d.symbol == symbol) {
                final_decisions.push(asset_decisions.remove(pos));
            }
        }
        // Append any remaining (should be none)
        final_decisions.extend(asset_decisions);

        let current_breakouts: Vec<String> = final_decisions
            .iter()
            .filter(|d| {
                matches!(
                    d.breakout.status,
                    crate::core::breakout_detection::BreakoutStatus::EmergingBreakout
                        | crate::core::breakout_detection::BreakoutStatus::ConfirmedBreakout
                )
            })
            .map(|d| d.symbol.clone())
            .collect();

        let has_mainline = matches!(
            trend_cohesion.status,
            crate::core::trend_cohesion::TrendCohesionStatus::Formed
        );

        let market_state = crate::core::market_state::engine::DecisionEngine::process(
            &rules.market_state_engine,
            &market_features,
            has_mainline,
            &current_breakouts,
            prev_packet,
        );

        let mut packet = DecisionPacket::new(
            market_features.date,
            market_features,
            market_regime,
            Some(market_state),
            portfolio_policy,
            final_decisions,
            participation,
            current_top_tier,
            trend_gate_changed,
            trend_cohesion,
            None,
        );

        let transition_log =
            crate::core::transition_log::StateTransitionLog::compare(prev_packet, &packet);
        packet.transition_log = Some(transition_log);

        Ok(packet)
    }
}
