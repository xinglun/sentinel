use crate::application::provider::TickerHistory;
use crate::config::{ParsedRules, WatchlistEntry};
use crate::core::features::{AssetFeatures, MarketFeatures};
use crate::core::market_regime::MarketRegimeStateMachine;

use crate::core::asset_state::{AssetState, AssetStateMachine};
use crate::core::portfolio_policy::PortfolioPolicy;

use crate::core::action_matrix::ActionMatrix;
use crate::core::breakout_detection::BreakoutEvaluator;
use crate::core::decision::DecisionPacket;
use crate::core::trend_cohesion::{
    AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, TrendCohesionEvaluator,
};
use anyhow::Result;

pub struct Engine;

impl Engine {
    /// 一連のアセットに対して、完全なモジュール式意思決定パイプラインを実行する。
    /// 現在の特徴量およびオプションの前回セッションコンテキストに基づき、決定論的な DecisionPacket を返す。
    pub fn run_daily_pipeline<'a>(
        ticker_histories: &[(TickerHistory<'a>, &WatchlistEntry)],
        rules: &ParsedRules,
        history: &[DecisionPacket],
        evidence_history: &[crate::core::trend_cohesion::AutomatedEvidenceRecord],
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

            // [P0-2] PositionIntent の合成（独立した用語集）
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
            // 下流のプレゼンテーション組み立てのためにドメインファクトを記録
            decision.is_core_fact = rules.core_assets.contains(&decision.symbol);
            decision.has_position_fact = positions.contains_key(&decision.symbol);

            // 注意: DisplayContext および DisplayIntent は、エクスポート時に PresentationAssembler によって設定される。
            decision.previous_state = prev_asset_snapshot.map(|s| s.state);
            decision.state_streak = state_streak;
            decision.top_tier_streak = top_tier_streak;
            decision.out_of_top_tier_streak = out_of_top_tier_streak;

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
            current_top_tier,
            trend_gate_changed,
            trend_cohesion,
            None,
            None,
        );

        let mut transition_log =
            crate::core::transition_log::StateTransitionLog::compare_with_rules(
                prev_packet,
                &packet,
                rules,
            );

        let mut confirmed_count = 0;
        let mut emerging_count = 0;
        for d in &packet.assets {
            match d.breakout.status {
                crate::core::breakout_detection::BreakoutStatus::ConfirmedBreakout => {
                    confirmed_count += 1
                }
                crate::core::breakout_detection::BreakoutStatus::EmergingBreakout => {
                    emerging_count += 1
                }
                _ => {}
            }
        }

        // 実体的な証拠（Substantive Evidence）の集計
        let mut substantive = crate::core::trend_cohesion::SubstantiveEvidence::default();
        let current_date = packet.date;
        let mut min_days = usize::MAX;

        // 1. 歴史的な証拠レコードの読み込み
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

        // 2. タグからの証拠抽出
        for d in &packet.assets {
            if let Some(f) = asset_features.iter().find(|af| af.symbol == d.symbol) {
                // 2. タグからの証拠抽出
                let mut event_days_offset = 0;
                for signal in &f.event_signals {
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

                for signal in &f.event_signals {
                    let mut new_record = None;
                    match signal.as_str() {
                        "capex_payoff:true" => {
                            new_record = Some(AutomatedEvidenceRecord {
                                source: EvidenceSourceType::Manual,
                                evidence_type: EvidenceType::CapexPayoff,
                                confidence: 1.0,
                                description: "Manual annotation: Capex Payoff".to_string(),
                                event_date: record_date_str.clone(),
                                symbol: Some(d.symbol.clone()),
                                source_url: None,
                                dedupe_key: format!(
                                    "Manual:CapexPayoff:{}:{}",
                                    d.symbol, record_date_str
                                ),
                            });
                        }
                        "earnings_validation:true" => {
                            new_record = Some(AutomatedEvidenceRecord {
                                source: EvidenceSourceType::Manual,
                                evidence_type: EvidenceType::EarningsValidation,
                                confidence: 1.0,
                                description: "Manual annotation: Earnings Validation".to_string(),
                                event_date: record_date_str.clone(),
                                symbol: Some(d.symbol.clone()),
                                source_url: None,
                                dedupe_key: format!(
                                    "Manual:EarningsValidation:{}:{}",
                                    d.symbol, record_date_str
                                ),
                            });
                        }
                        "order_visibility:true" => {
                            new_record = Some(AutomatedEvidenceRecord {
                                source: EvidenceSourceType::Manual,
                                evidence_type: EvidenceType::OrderVisibility,
                                confidence: 1.0,
                                description: "Manual annotation: Order Visibility".to_string(),
                                event_date: record_date_str.clone(),
                                symbol: Some(d.symbol.clone()),
                                source_url: None,
                                dedupe_key: format!(
                                    "Manual:OrderVisibility:{}:{}",
                                    d.symbol, record_date_str
                                ),
                            });
                        }
                        _ => {}
                    }

                    if let Some(rec) = new_record {
                        if !substantive
                            .records
                            .iter()
                            .any(|r| r.dedupe_key == rec.dedupe_key)
                        {
                            substantive.records.push(rec);
                        }
                    }
                }
            }
        }

        // 3. 自動価格追随（FollowThrough）の抽出
        for d in &packet.assets {
            let is_core = rules.core_assets.contains(&d.symbol);
            let is_confirmed = d.breakout.status
                == crate::core::breakout_detection::BreakoutStatus::ConfirmedBreakout;

            if is_core && is_confirmed && d.breakout.breakout_age >= 3 {
                let rec = AutomatedEvidenceRecord {
                    source: EvidenceSourceType::PriceAction,
                    evidence_type: EvidenceType::FollowThrough,
                    confidence: 0.9,
                    description: format!(
                        "Automated FollowThrough: {} breakout maintained for {} days",
                        d.symbol, d.breakout.breakout_age
                    ),
                    event_date: current_date.to_string(),
                    symbol: Some(d.symbol.clone()),
                    source_url: None,
                    dedupe_key: format!(
                        "PriceAction:FollowThrough:{}:Age{}:{}",
                        d.symbol, d.breakout.breakout_age, current_date
                    ),
                };

                if !substantive
                    .records
                    .iter()
                    .any(|r| r.dedupe_key == rec.dedupe_key)
                {
                    substantive.records.push(rec);
                    if 0 < min_days {
                        min_days = 0;
                    }
                }
            }
        }

        substantive.event_days_since = if min_days == usize::MAX { 0 } else { min_days };
        substantive.aggregate();

        let evidence = crate::core::trend_cohesion::TrendRecognitionEvidence::compute(
            confirmed_count,
            emerging_count,
            transition_log.scout_days_without_expansion,
            transition_log.scout_abort_days,
            if substantive != crate::core::trend_cohesion::SubstantiveEvidence::default() {
                Some(substantive)
            } else {
                None
            },
            current_date,
            &rules.market_state_engine,
        );

        packet.trend_recognition = Some(evidence.clone());
        transition_log.trend_recognition = Some(evidence);

        packet.transition_log = Some(transition_log);

        Ok(packet)
    }
}
