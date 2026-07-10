use crate::features::radar::domain::features::{
    calculate_relative_strength, AssetFeatures, MarketFeatures,
};
use crate::features::radar::domain::market_regime::MarketRegimeStateMachine;
use crate::features::radar::domain::rules::{ParsedRules, WatchlistEntry};
use crate::features::shared::domain::market_data::TickerHistory;

use crate::features::radar::domain::asset_state::{AssetState, AssetStateMachine};
use crate::features::radar::domain::portfolio_policy::PortfolioPolicy;

use crate::features::radar::domain::action_matrix::ActionMatrix;
use crate::features::radar::domain::breakout_detection::BreakoutEvaluator;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::trend_cohesion::TrendCohesionEvaluator;
use anyhow::Result;

pub struct Engine;

impl Engine {
    /// 一連のアセットに対して、完全なモジュール式意思決定パイプラインを実行する。
    /// 現在の特徴量およびオプションの前回セッションコンテキストに基づき、決定論的な DecisionPacket を返す。
    pub fn run_daily_pipeline<'a>(
        ticker_histories: &[(TickerHistory<'a>, &WatchlistEntry)],
        rules: &ParsedRules,
        history: &[DecisionPacket],
        evidence_history: &[crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord],
        positions: &std::collections::HashMap<String, (f64, f64)>,
    ) -> Result<DecisionPacket> {
        if ticker_histories.is_empty() {
            return Err(anyhow::anyhow!("No ticker history provided for pipeline"));
        }

        let prev_packet = history.last();

        // 0. 以前のコンテキストを抽出
        let _prev_state = prev_packet.map(|p| p.market_regime.market_state);
        let prev_age = prev_packet
            .map(|p| p.market_features.regime_age)
            .unwrap_or(0);

        // 1. 特徴量レイヤー：アセット特徴量
        let mut asset_features = Vec::new();
        for (history, entry) in ticker_histories {
            let f = AssetFeatures::compute(history, entry, rules);

            asset_features.push(f);
        }

        // 2. 市場特徴量（初期値 - prev_packet をコンテキストとして使用）
        let mut market_features =
            MarketFeatures::compute(&asset_features, prev_age, prev_packet, rules);

        // 3. 市場レジーム状態マシン（統合された遷移）
        let (market_regime, _) = MarketRegimeStateMachine::transition(
            prev_packet.map(|p| &p.market_regime),
            &mut market_features,
            prev_age,
            rules,
        );

        // 4. ポートフォリオポリシー
        let portfolio_policy = PortfolioPolicy::from_market_regime(&market_regime);

        // 5. 相対力指標メモリレイヤー (NEW V1.3)
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

        // 6. メモリ調整済みランキング（Readiness のために上位へ移動）
        let ranked_symbols =
            AssetStateMachine::rank_assets_with_memory(&asset_features, &memory_decisions);
        let current_top_tier: Vec<String> = ranked_symbols.iter().take(3).cloned().collect();

        // 7. トレンド凝集スナップショット (統合 SSOT)
        let trend_cohesion = TrendCohesionEvaluator::evaluate(
            market_features.stability_score,
            &current_top_tier,
            history,
            &rules.trend_cohesion,
        );
        let active_trend_gate_passed = trend_cohesion.gate_passed;

        let prev_trend_gate_passed = prev_packet
            .map(|p| p.trend_cohesion.gate_passed)
            .unwrap_or(false);
        let trend_gate_changed = prev_trend_gate_passed != active_trend_gate_passed;

        // 8. アセット実行状態 ＆ アクションマトリックス
        let mut asset_decisions = Vec::new();
        for f in &asset_features {
            let prev_asset_decision =
                prev_packet.and_then(|p| p.assets.iter().find(|a| a.symbol == f.symbol));

            let prev_asset_snapshot = prev_asset_decision.map(|a| &a.asset_state);

            let memory_decision = memory_decisions.get(&f.symbol);

            let asset_state_snapshot =
                AssetStateMachine::compute_state(f, rules, prev_asset_snapshot, memory_decision);

            // ストリーク計算
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

            // ウォッチリストのエントリからアセットごとの制約を取得
            let (_h, entry) = ticker_histories
                .iter()
                .find(|(h, _)| h.symbol == f.symbol)
                .unwrap();
            let trade_enabled = entry.trade_enabled.unwrap_or(true);
            let trade_amount = entry.trade_amount.unwrap_or(2000.0); // Safe default

            let state_name = format!("{:?}", asset_state_snapshot.state).to_lowercase();

            // 段階的マルチプライヤーの選択
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
                // 必要に応じて恐怖ティアのヒューリスティックを適用。現状は fear_1/2 があれば使用し、なければ状態名にフォールバック。
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

            // 決済判断の統合
            let exit_decision = crate::features::radar::domain::exit::ExitDecision::compute(
                &f.symbol,
                asset_state_snapshot.state,
                prev_asset_snapshot.map(|s| s.state),
                state_streak,
                out_of_top_tier_streak,
                market_regime.risk_overlay,
                active_trend_gate_passed,
                prev_trend_gate_passed,
            );

            // [P0-2] PositionIntent の合成（独立した用語集）
            let final_intent =
                crate::features::radar::domain::intent_synthesizer::IntentSynthesizer::synthesize(
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
                crate::features::radar::domain::position_intent::UnifiedIntentSynthesizer::synthesize(
                    final_intent,
                    &decision.exit_decision,
                    active_trend_gate_passed,
                    positions.contains_key(&decision.symbol),
                    asset_state_snapshot.state,
                )
                .intent;
            // 下流のプレゼンテーション組み立てのためにドメインファクトを記録
            decision.is_core_fact = rules.core_assets.contains(&decision.symbol);
            decision.has_position_fact = positions.contains_key(&decision.symbol);

            // 注意: DisplayContext および DisplayIntent は、エクスポート時に PresentationAssembler によって設定される。
            decision.previous_state = prev_asset_snapshot.map(|s| s.state);
            decision.state_streak = state_streak;
            decision.top_tier_streak = top_tier_streak;
            decision.out_of_top_tier_streak = out_of_top_tier_streak;
            decision.relative_strength = ticker_histories
                .iter()
                .find(|(_, candidate)| {
                    candidate.symbol
                        == rules
                            .market_state_engine
                            .market_benchmarks
                            .get(&entry.market.to_ascii_uppercase())
                            .or_else(|| rules.market_state_engine.market_benchmarks.get("US"))
                            .map(String::as_str)
                            .unwrap_or("SPY")
                })
                .and_then(|(benchmark, _)| {
                    ticker_histories
                        .iter()
                        .find(|(history, _)| history.symbol == f.symbol)
                        .and_then(|(asset, _)| calculate_relative_strength(asset, benchmark, 63))
                });

            asset_decisions.push(decision);
        }

        // 9. ランキングに基づいて並べ替え
        let mut final_decisions = Vec::with_capacity(asset_decisions.len());
        for symbol in ranked_symbols {
            if let Some(pos) = asset_decisions.iter().position(|d| d.symbol == symbol) {
                final_decisions.push(asset_decisions.remove(pos));
            }
        }
        // 残りのものを追加（通常は存在しないはず）
        final_decisions.extend(asset_decisions);

        let market_state = crate::features::radar::application::pipeline_steps::derive_market_state(
            rules,
            &market_features,
            &trend_cohesion,
            &final_decisions,
            prev_packet,
        );

        let mut packet = DecisionPacket::new(
            market_features.date,
            market_features,
            market_regime,
            Some(market_state),
            portfolio_policy,
            final_decisions,
            current_top_tier,
            trend_gate_changed,
            trend_cohesion,
            None,
            None,
        );

        crate::features::radar::application::pipeline_steps::attach_transition_context(
            &mut packet,
            prev_packet,
            &asset_features,
            rules,
            evidence_history,
        );

        Ok(packet)
    }
}
