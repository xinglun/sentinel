use crate::features::radar::domain::asset_state::AssetState;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::exit::AssetExitState;
use crate::features::radar::domain::market_regime::{MarketState, RiskOverlay};
use crate::features::radar::domain::observation_timeline::derive_breadth_facts;
use crate::features::radar::domain::rules::ParsedRules;
use crate::features::radar::domain::trend_cohesion::EvidenceType;
use crate::features::radar::interface::display::{DisplayAdapter, DisplayContext, DisplayIntent};
use crate::features::radar::interface::hypothesis_read_model::{
    build_hypothesis_layer, HypothesisEvidencePresence, HypothesisReadModelInput,
};
use crate::features::radar::interface::presentation::{
    BreakoutDisplayStatus, BreakoutItemViewModel, BreakoutSummaryViewModel, DataAlertViewModel,
    DecisionSummaryViewModel, ExitDecisionItemViewModel, ExitDecisionSummaryViewModel,
    ExitDisplayIntent, HypothesisLayerViewModel, LeadershipSnapshotViewModel, MacroDisplayContext,
    PresentationPacket, RiskOpportunitySummaryViewModel, SignalSummaryViewModel, TrendBreadthMode,
};
use crate::features::radar::interface::risk_taxonomy_read_model;
use crate::features::radar::interface::strategic_context_read_model;
use crate::features::radar::interface::transition_evidence_read_model;
use crate::features::shared::interface::i18n::{get_dictionary, DisplayDictionary, Language};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

pub struct PresentationAssembler;

struct BattleboardSnapshot {
    watch_count: usize,
    hold_count: usize,
    defend_count: usize,
    opportunity_snapshot_value: String,
    risk_snapshot_value: String,
}

impl PresentationAssembler {
    /// 現在の tactical leadership が不在のとき、旧 topology と strategic context を補正する。
    pub(crate) fn reconcile_tactical_leadership_display(
        presentation: &mut PresentationPacket,
        leadership_snapshot: &LeadershipSnapshotViewModel,
        language: Language,
    ) {
        let structure =
            crate::features::radar::domain::leader_persistence::tactical_leadership_structure(
                &leadership_snapshot.primary_leader_value,
                leadership_snapshot.leader_absence_duration,
            );
        if structure != "LEADERLESS / FRAGMENTED" {
            return;
        }

        let dict = get_dictionary(language);
        presentation.decision_summary.trend_cohesion_value =
            dict.trend_cohesion.current_no_confirmed_mainline.clone();
        presentation.decision_summary.trend_topology_value =
            dict.trend_cohesion.topology_leaderless_fragmented.clone();
        presentation.decision_summary.state_tag_value =
            dict.decision.state_ignition_unconfirmed.clone();
        if let Some(evidence) = presentation.transition_evidence.as_mut() {
            strategic_context_read_model::apply_leaderless_market_structure_override(
                &mut evidence.strategic_context,
                &dict,
                language,
            );
        }
    }

    /// DecisionPacket から PresentationPacket を生成する。
    /// 入力 packet を変更しない純粋関数。
    /// AssetActionDecision を clone せず、1 回の走査で enrichment と分類を行う。
    pub fn assemble(
        packet: &DecisionPacket,
        rules: &ParsedRules,
        positions: &HashMap<String, (f64, f64)>,
        failed_symbols: Vec<String>,
        lang: Language,
    ) -> PresentationPacket {
        let dict = get_dictionary(lang);
        let top_tier = &packet.top_tier_symbols;
        let is_ready = packet.trend_cohesion.gate_passed;
        let core_assets = &rules.core_assets;
        let date_str = packet.date.format("%Y-%m-%d").to_string();
        let is_data_missing = packet.assets.is_empty() && !failed_symbols.is_empty();
        let top_tier_set: HashSet<&str> = top_tier.iter().map(String::as_str).collect();
        let core_assets_set: HashSet<&str> = core_assets.iter().map(String::as_str).collect();

        // 1. マクロ表示コンテキストを組み立てる。
        let state = packet.market_regime.market_state;
        let risk = packet.market_regime.risk_overlay;
        let trend_breadth_mode = risk_taxonomy_read_model::classify_trend_breadth_mode(packet);
        let breadth_facts = derive_breadth_facts(
            packet.market_features.up_count,
            packet.market_features.flat_count,
            packet.market_features.down_count,
            packet.market_features.total_count,
        );

        let (headline, summary, bias) = if is_data_missing {
            (
                dict.market_stages.data_missing.clone(),
                dict.market_summaries.data_missing.clone(),
                dict.market_summaries.bias_neutral.clone(),
            )
        } else if state == MarketState::DEFENSIVE
            && trend_breadth_mode == TrendBreadthMode::NarrowLeadership
        {
            (
                dict.market_stages.structural_consolidation.clone(),
                dict.market_summaries.structural_consolidation.clone(),
                dict.market_summaries.bias_ignition.clone(),
            )
        } else {
            match state {
                MarketState::ESTABLISHED | MarketState::CONFIRMED => (
                    dict.market_stages.established.clone(),
                    dict.market_summaries.established.clone(),
                    dict.market_summaries.bias_established.clone(),
                ),
                MarketState::DEFENSIVE => (
                    dict.market_stages.defensive.clone(),
                    dict.market_summaries.defensive.clone(),
                    dict.market_summaries.bias_defensive.clone(),
                ),
                MarketState::IGNITION | MarketState::NEWBORN => (
                    dict.market_stages.ignition.clone(),
                    dict.market_summaries.ignition.clone(),
                    dict.market_summaries.bias_ignition.clone(),
                ),
                _ => (
                    dict.market_stages.neutral.clone(),
                    dict.market_summaries.neutral.clone(),
                    dict.market_summaries.bias_neutral.clone(),
                ),
            }
        };

        let risk_label = match risk {
            RiskOverlay::NORMAL => dict.risks.normal.clone(),
            RiskOverlay::DECELERATING => dict.risks.mixed.clone(),
            RiskOverlay::DEFENSIVE | RiskOverlay::BROKEN => dict.risks.defensive.clone(),
        };
        let data_alert = if failed_symbols.is_empty() {
            None
        } else {
            let count = failed_symbols.len();
            let (prefix, label) = if count <= 2 {
                ("💬", dict.states.data_notice.clone())
            } else if count <= 5 {
                ("⚠️", dict.states.data_warning.clone())
            } else {
                ("🚨", dict.states.data_critical.clone())
            };

            Some(DataAlertViewModel {
                prefix: prefix.to_string(),
                label,
                message: dict.states.fetch_failed.clone(),
                symbols: failed_symbols.clone(),
            })
        };

        let flow_value = if is_data_missing {
            "N/A".to_string()
        } else {
            match packet.market_features.flow_acceleration {
                Some(flow) if flow > 0.05 => dict.states.flow_in.clone(),
                Some(flow) if flow < -0.05 => dict.states.flow_out.clone(),
                _ => dict.states.flow_stable.clone(),
            }
        };
        let continuity_state =
            Self::map_continuity_state(packet.trend_cohesion.continuity_streak, &dict);
        let inverse_potential =
            (1.0 / (1.0 + packet.market_features.potential_energy) * 50.0).clamp(0.0, 50.0);
        let trend_allocation =
            (packet.market_features.system_confidence - inverse_potential).clamp(0.0, 50.0);
        let (confidence_breakdown_label, confidence_breakdown_value) = match lang {
            Language::ZhCn => (
                "置信度构成".to_string(),
                format!(
                    "趋势配置 {:.1} + 逆势能 {:.1}",
                    trend_allocation, inverse_potential
                ),
            ),
            Language::EnUs => (
                "Confidence breakdown".to_string(),
                format!(
                    "Trend allocation {:.1} + inverse potential {:.1}",
                    trend_allocation, inverse_potential
                ),
            ),
            Language::JaJp => (
                "Confidence 内訳".to_string(),
                format!(
                    "トレンド配分 {:.1} + 逆ポテンシャル {:.1}",
                    trend_allocation, inverse_potential
                ),
            ),
        };
        let signal_summary = SignalSummaryViewModel {
            confidence_label: dict.signals.confidence.clone(),
            confidence_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!("{:.0}", packet.market_features.system_confidence)
            },
            confidence_breakdown_label,
            confidence_breakdown_value: if is_data_missing {
                "N/A".to_string()
            } else {
                confidence_breakdown_value
            },
            stability_label: dict.signals.stability.clone(),
            stability_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!("{:.1}", packet.market_features.stability_score)
            },
            cohesion_label: dict.signals.cohesion.clone(),
            cohesion_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!(
                    "{} · {} {}",
                    if packet.trend_cohesion.gate_passed {
                        dict.states.ready.clone()
                    } else {
                        dict.states.not_ready.clone()
                    },
                    dict.signals.continuity.clone(),
                    continuity_state
                )
            },
            continuity_label: dict.signals.continuity.clone(),
            continuity_value: if is_data_missing {
                "N/A".to_string()
            } else {
                continuity_state
            },
            regime_age_label: dict.signals.regime_age.clone(),
            regime_age_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!("{}d", packet.market_features.regime_age)
            },
            flow_label: dict.signals.net_flow.clone(),
            flow_value,
            breadth_label: dict.signals.universe_breadth.clone(),
            breadth_value: breadth_facts.label.clone(),
            breadth_raw_label: dict.signals.universe_breadth_raw.clone(),
            breadth_raw_value: breadth_facts
                .raw_percent
                .map(|value| format!("{value:.1}%"))
                .unwrap_or_else(|| "UNAVAILABLE".to_string()),
            breadth_counts_label: dict.signals.universe_breadth_counts.clone(),
            breadth_counts_value: format!(
                "up={} flat={} down={} total={}",
                packet.market_features.up_count,
                packet.market_features.flat_count,
                packet.market_features.down_count,
                packet.market_features.total_count
            ),
            breadth_universe_label: dict.signals.observation_universe.clone(),
            breadth_universe_value: if packet.market_features.total_count == 0 {
                "UNAVAILABLE / 0 observed".to_string()
            } else {
                format!(
                    "{:.1}% integrity / {} observed",
                    packet.market_features.universe_integrity * 100.0,
                    packet.market_features.total_count
                )
            },
            breadth_semantic_label: dict.signals.universe_breadth_label.clone(),
            breadth_semantic_value: breadth_facts.label.clone(),
            supply_phase_label: "Supply Phase".to_string(),
            supply_phase_value: String::new(),
        };
        let macro_display = MacroDisplayContext {
            headline,
            summary,
            risk_label,
            bias_label: bias,
        };

        // 2. 参照だけを使って統合分類する。
        let mut acc_refs = Vec::new();
        let mut hold_refs = Vec::new();
        let mut watch_refs = Vec::new();
        let mut defend_refs = Vec::new();

        for asset in &packet.assets {
            let context = Self::derive_display_context(
                asset,
                positions,
                &top_tier_set,
                &core_assets_set,
                is_ready,
                asset.is_core_fact,
            );
            let unified_intent = Self::derive_unified_intent(asset, &context);
            let intent = Self::derive_display_intent(unified_intent, &context);
            let item = (asset, context, intent);
            match intent {
                DisplayIntent::ADD => acc_refs.push(item),
                DisplayIntent::TRIM | DisplayIntent::EXIT => defend_refs.push(item),
                DisplayIntent::HOLD => hold_refs.push(item),
                DisplayIntent::OBSERVE => watch_refs.push(item),
            }
        }

        // 3. Decision object を複製せず、並び替えと top action 選択を行う。
        let sort_fn = |a: &(
            &crate::features::radar::domain::action_matrix::AssetActionDecision,
            DisplayContext,
            DisplayIntent,
        ),
                       b: &(
            &crate::features::radar::domain::action_matrix::AssetActionDecision,
            DisplayContext,
            DisplayIntent,
        )| {
            let change_cmp = (if b.0.action_changed { 1 } else { 0 })
                .cmp(&(if a.0.action_changed { 1 } else { 0 }));
            if change_cmp != Ordering::Equal {
                return change_cmp;
            }

            let az = a.0.z_score.unwrap_or(0.0).abs();
            let bz = b.0.z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(Ordering::Equal)
        };

        acc_refs.sort_by(sort_fn);
        hold_refs.sort_by(sort_fn);
        watch_refs.sort_by(sort_fn);
        defend_refs.sort_by(sort_fn);

        // 選択ロジック。
        let limit = if state == MarketState::DEFENSIVE {
            4
        } else {
            3
        };
        let mut selected_refs = Vec::new();

        if !is_ready {
            for r in &watch_refs {
                if selected_refs.len() >= limit {
                    break;
                }
                selected_refs.push(*r);
            }
        } else {
            if state == MarketState::DEFENSIVE {
                for r in &defend_refs {
                    if selected_refs.len() >= limit {
                        break;
                    }
                    selected_refs.push(*r);
                }
            }
            for r in &acc_refs {
                if selected_refs.len() >= limit {
                    break;
                }
                selected_refs.push(*r);
            }
            for r in &hold_refs {
                if selected_refs.len() >= limit {
                    break;
                }
                selected_refs.push(*r);
            }
            for r in &watch_refs {
                if selected_refs.len() >= limit {
                    break;
                }
                selected_refs.push(*r);
            }
            if state != MarketState::DEFENSIVE {
                for r in &defend_refs {
                    if selected_refs.len() >= limit {
                        break;
                    }
                    selected_refs.push(*r);
                }
            }
        }

        let tactical_buckets = vec![
            (
                dict.buckets.accumulate.clone(),
                "accumulate".to_string(),
                &acc_refs,
            ),
            (
                dict.buckets.holdings.clone(),
                "hold".to_string(),
                &hold_refs,
            ),
            (
                dict.buckets.watchlist.clone(),
                "watch".to_string(),
                &watch_refs,
            ),
            (
                dict.buckets.actions.clone(),
                "defend".to_string(),
                &defend_refs,
            ),
        ]
        .into_iter()
        .filter_map(|(display_name, bucket_id, refs)| {
            if refs.is_empty() {
                None
            } else {
                Some(
                    crate::features::radar::interface::display::TacticalBucketViewModel {
                        bucket_id,
                        display_name,
                        count: refs.len(),
                        items: refs
                            .iter()
                            .map(|(asset, _, _)| asset.symbol.clone())
                            .collect(),
                    },
                )
            }
        })
        .collect::<Vec<_>>();

        let mut risk_opportunities = Vec::new();
        for (asset, context, intent) in acc_refs
            .iter()
            .chain(hold_refs.iter())
            .chain(watch_refs.iter())
            .chain(defend_refs.iter())
        {
            if (*intent == DisplayIntent::ADD
                || (context.is_candidate_only && context.cohesion_ready))
                && matches!(
                    asset.asset_state.state,
                    crate::features::radar::domain::asset_state::AssetState::PULLBACK
                        | crate::features::radar::domain::asset_state::AssetState::OPTIMAL
                )
            {
                risk_opportunities.push(
                    crate::features::radar::interface::display::RiskOpportunityViewModel {
                        kind: dict.decision.opportunity.clone(),
                        symbol: asset.symbol.clone(),
                        reason: Self::derive_telegram_reason(
                            asset,
                            !is_ready,
                            Self::is_systemic_collapse(packet),
                            packet,
                            lang,
                            &dict,
                        ),
                    },
                );
            }

            if matches!(*intent, DisplayIntent::TRIM | DisplayIntent::EXIT)
                || asset.asset_state.state
                    == crate::features::radar::domain::asset_state::AssetState::OVERHEAT
            {
                risk_opportunities.push(
                    crate::features::radar::interface::display::RiskOpportunityViewModel {
                        kind: dict.decision.risk.clone(),
                        symbol: asset.symbol.clone(),
                        reason: Self::derive_telegram_reason(
                            asset,
                            !is_ready,
                            Self::is_systemic_collapse(packet),
                            packet,
                            lang,
                            &dict,
                        ),
                    },
                );
            }
        }

        let opportunity_value = risk_opportunities
            .iter()
            .find(|item| item.kind == dict.decision.opportunity)
            .map(|item| format!("{} · {}", item.symbol, item.reason))
            .unwrap_or_else(|| dict.decision.no_opportunity.clone());
        let risk_items = risk_opportunities
            .iter()
            .filter(|item| item.kind == dict.decision.risk)
            .collect::<Vec<_>>();
        let risk_value = Self::summarize_primary_risk(&risk_items, packet, &dict, lang);
        let battleboard = BattleboardSnapshot {
            watch_count: watch_refs.len(),
            hold_count: hold_refs.len(),
            defend_count: defend_refs.len(),
            opportunity_snapshot_value: opportunity_value.clone(),
            risk_snapshot_value: risk_value.clone(),
        };
        let decision_summary =
            Self::build_decision_summary(packet, is_data_missing, state, &dict, &battleboard);
        let eligible_asset_count = acc_refs
            .iter()
            .filter(|(_, context, _)| !context.has_position)
            .count();
        let final_execution_decision = Self::build_final_execution_decision(
            &decision_summary,
            state,
            lang,
            eligible_asset_count,
        );
        let (
            execution_risk_label,
            execution_risk_value,
            portfolio_risk_label,
            portfolio_risk_value,
        ) = match lang {
            Language::ZhCn => (
                "执行风险".to_string(),
                if matches!(
                    final_execution_decision.execution_window,
                    crate::features::radar::interface::presentation::ExecutionWindow::Open
                        | crate::features::radar::interface::presentation::ExecutionWindow::Limited
                ) {
                    "允许按规则执行"
                } else {
                    "暂停新增主动进攻"
                }
                .to_string(),
                "组合风险".to_string(),
                risk_value.clone(),
            ),
            Language::EnUs => (
                "Execution Risk".to_string(),
                if matches!(
                    final_execution_decision.execution_window,
                    crate::features::radar::interface::presentation::ExecutionWindow::Open
                        | crate::features::radar::interface::presentation::ExecutionWindow::Limited
                ) {
                    "Execution permitted by rules"
                } else {
                    "Pause new active entries"
                }
                .to_string(),
                "Portfolio Risk".to_string(),
                risk_value.clone(),
            ),
            Language::JaJp => (
                "執行リスク".to_string(),
                if matches!(
                    final_execution_decision.execution_window,
                    crate::features::radar::interface::presentation::ExecutionWindow::Open
                        | crate::features::radar::interface::presentation::ExecutionWindow::Limited
                ) {
                    "ルールに従う執行を許可"
                } else {
                    "新規の積極的エントリーを停止"
                }
                .to_string(),
                "ポートフォリオリスク".to_string(),
                risk_value.clone(),
            ),
        };
        let risk_opportunity_summary = RiskOpportunitySummaryViewModel {
            opportunity_label: dict.decision.opportunity.clone(),
            opportunity_value,
            risk_label: dict.decision.risk.clone(),
            risk_value,
            execution_risk_label,
            execution_risk_value,
            portfolio_risk_label,
            portfolio_risk_value,
        };
        let exit_summary = Self::build_exit_summary(
            packet,
            positions,
            &top_tier_set,
            &core_assets_set,
            is_ready,
            lang,
            &dict,
        );
        let breakout_summary = Self::build_breakout_summary(packet, rules, &dict, lang);

        let mut notices = Vec::new();
        if !is_data_missing && !is_ready {
            if matches!(state, MarketState::IGNITION | MarketState::NEWBORN) {
                notices.push(dict.reasons.ignition_notice.clone());
            } else {
                notices.push(dict.reasons.cohesion_notice.clone());
            }
        }
        // 4. Final ViewModel Conversion
        let mut top_vms = Vec::with_capacity(selected_refs.len());
        for (asset, context, intent) in selected_refs {
            let mut vm =
                DisplayAdapter::derive_top_action_view_model(asset, &context, intent, &dict);
            let reason = Self::derive_telegram_reason(
                asset,
                !is_ready,
                Self::is_systemic_collapse(packet),
                packet,
                lang,
                &dict,
            );
            if !reason.is_empty() {
                vm.diagnostic = Some(reason);
            } else if let Some(raw_reason) = asset.reasons.first() {
                vm.diagnostic = Some(raw_reason.clone());
            }
            top_vms.push(vm);
        }

        let presentation = PresentationPacket {
            date_str,
            language: lang,
            macro_display,
            decision_summary,
            final_execution_decision,
            signal_summary,
            top_actions: top_vms,
            exit_summary,
            breakout_summary,
            tactical_buckets,
            risk_opportunity_summary,
            risk_opportunities,
            notices,
            data_alert,
            transition_evidence: transition_evidence_read_model::build_transition_evidence(
                packet, rules, &dict,
            ),
            interpretation_layer: None,
            market_interpretation: None,
            leadership_snapshot: None,
            leader_persistence: None,
            current_relative_strength: {
                let value = Self::build_current_relative_strength(packet, lang);
                (!value.items.is_empty()).then_some(value)
            },
            market_change_log: None,
            hypothesis_layer: Self::build_hypothesis_layer_from_packet(packet, &dict),
            terminal_rows: Vec::new(),
            state_code: format!("{:?}", state),
        };
        presentation
    }

    fn build_current_relative_strength(
        packet: &DecisionPacket,
        language: Language,
    ) -> crate::features::radar::interface::presentation::CurrentRelativeStrengthViewModel {
        let mut items = packet
            .current_relative_strength_observations
            .iter()
            .filter(|observation| {
                let benchmark_symbol = if observation.benchmark_symbol.is_empty() {
                    "SPY"
                } else {
                    observation.benchmark_symbol.as_str()
                };
                observation.symbol != benchmark_symbol
            })
            .map(|observation| {
                let status = observation.state.as_str().to_string();
                let recovery_strength = observation.recovery_strength.as_str().to_string();
                let weakening = packet
                    .assets
                    .iter()
                    .find(|asset| asset.symbol == observation.symbol)
                    .is_some_and(|asset| {
                        matches!(
                            asset.exit_decision.asset_exit_state,
                            AssetExitState::StrengthLoss | AssetExitState::CohesionExit
                        ) || matches!(
                            asset.action,
                            crate::features::radar::domain::action_matrix::AssetAction::REDUCE
                                | crate::features::radar::domain::action_matrix::AssetAction::AVOID
                        )
                });
                let recovery_watch = weakening
                    && matches!(
                        observation.state,
                        crate::features::radar::domain::current_relative_strength::RelativeStrengthState::Improving
                    )
                    && matches!(
                        observation.recovery_strength,
                        crate::features::radar::domain::current_relative_strength::RecoveryStrength::Strong
                            | crate::features::radar::domain::current_relative_strength::RecoveryStrength::Moderate
                    );
                let weak_improvement = observation.state
                    == crate::features::radar::domain::current_relative_strength::RelativeStrengthState::Improving
                    && observation.recovery_strength
                        == crate::features::radar::domain::current_relative_strength::RecoveryStrength::Weak;
                crate::features::radar::interface::presentation::CurrentRelativeStrengthItemViewModel {
                    symbol: observation.symbol.clone(),
                    status,
                    recovery_strength,
                    relative_1d_vs_benchmark: observation.relative_1d_vs_benchmark,
                    relative_5d_vs_benchmark: observation.relative_5d_vs_benchmark,
                    price_position: observation.price_position,
                    volume_participation: observation.volume_participation,
                    conflict_code: recovery_watch.then(|| "SIGNAL_CONFLICT".to_string()),
                    recovery_watch,
                    recovery_explanation: if recovery_watch {
                        Some(match language {
                            Language::ZhCn => "长期/累计结构仍弱，但短期相对强度正在明显恢复。当前不取消既有弱势判断，停止继续使用“连续转弱”描述，进入 RECOVERY_WATCH。".to_string(),
                            Language::EnUs => "The long-term cumulative structure remains weak, while short-term relative strength is recovering. Keep the existing weakness assessment, stop describing it as continued weakening, and enter RECOVERY_WATCH.".to_string(),
                            Language::JaJp => "長期・累積構造はなお弱い一方、短期相対強度は回復しています。既存の弱勢判定は維持し、連続的な弱化という表現を止めて RECOVERY_WATCH に移行します。".to_string(),
                        })
                    } else if weak_improvement {
                        Some(match language {
                            Language::ZhCn => "相对强度出现初步改善，但尚不足以确认恢复。".to_string(),
                            Language::EnUs => "Relative strength shows an initial improvement, but it is not yet sufficient to confirm recovery.".to_string(),
                            Language::JaJp => "相対強度には初期改善が見られますが、回復を確認するにはまだ不十分です。".to_string(),
                        })
                    } else {
                        None
                    },
                }
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            right
                .relative_5d_vs_benchmark
                .partial_cmp(&left.relative_5d_vs_benchmark)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let (title, confirmed_leader, boundary) = match language {
            Language::ZhCn => (
                "当前相对强度",
                "无",
                "相对强度仅用于观察，不改变 Leader、Gate、Action Matrix 或 Position Sizing。",
            ),
            Language::EnUs => (
                "Current Relative Strength",
                "none",
                "Relative Strength is observation only and does not change Leader, Gate, Action Matrix or Position Sizing.",
            ),
            Language::JaJp => (
                "現在の相対強度",
                "なし",
                "相対強度は観測専用であり、Leader、Gate、Action Matrix、Position Sizingを変更しない。",
            ),
        };
        crate::features::radar::interface::presentation::CurrentRelativeStrengthViewModel {
            title: title.to_string(),
            confirmed_leader: confirmed_leader.to_string(),
            benchmark_symbol: packet
                .current_relative_strength_observations
                .iter()
                .find_map(|observation| {
                    (!observation.benchmark_symbol.is_empty())
                        .then_some(observation.benchmark_symbol.clone())
                })
                .unwrap_or_else(|| "SPY".to_string()),
            items,
            boundary: boundary.to_string(),
        }
    }

    fn build_final_execution_decision(
        decision: &DecisionSummaryViewModel,
        state: MarketState,
        language: Language,
        eligible_asset_count: usize,
    ) -> crate::features::radar::interface::presentation::FinalExecutionDecision {
        use crate::features::radar::interface::presentation::{
            ExecutionActionability, ExecutionWindow, FinalExecutionDecision, ParticipationMode,
        };
        if decision.is_no_trade {
            return FinalExecutionDecision {
                execution_window: ExecutionWindow::None,
                participation_mode: ParticipationMode::None,
                position_range: "0%".to_string(),
                permission_position_range: "0%".to_string(),
                eligible_asset_count,
                actionability: ExecutionActionability::CandidateOnly,
                reason: decision.hard_rule_note.clone(),
            };
        }
        let permission_position_range = decision.entry_cap_value.clone();
        let no_new_entry = eligible_asset_count == 0;
        let effective_position_range = if no_new_entry {
            "0%".to_string()
        } else {
            permission_position_range.clone()
        };
        let actionability = if no_new_entry {
            ExecutionActionability::CandidateOnly
        } else {
            ExecutionActionability::Executable
        };
        if matches!(state, MarketState::IGNITION | MarketState::NEWBORN) {
            return FinalExecutionDecision {
                execution_window: ExecutionWindow::Limited,
                participation_mode: ParticipationMode::Probe,
                position_range: effective_position_range,
                permission_position_range,
                eligible_asset_count,
                actionability,
                reason: if no_new_entry {
                    Self::no_new_entry_reason(language)
                } else {
                    match language {
                        Language::ZhCn => "有限参与窗口 / 仅 Probe".to_string(),
                        Language::EnUs => "Limited participation window / Probe Only".to_string(),
                        Language::JaJp => "限定参加ウィンドウ / Probe のみ".to_string(),
                    }
                },
            };
        }
        FinalExecutionDecision {
            execution_window: ExecutionWindow::Open,
            participation_mode: ParticipationMode::Add,
            position_range: effective_position_range,
            permission_position_range,
            eligible_asset_count,
            actionability,
            reason: if no_new_entry {
                Self::no_new_entry_reason(language)
            } else {
                decision.summary.clone()
            },
        }
    }

    fn no_new_entry_reason(language: Language) -> String {
        match language {
            Language::ZhCn => "市场权限已开放，但当前无可执行标的；实际行动为 NO_NEW_ENTRY".to_string(),
            Language::EnUs => {
                "Market permission is open, but no eligible asset is executable; effective action is NO_NEW_ENTRY".to_string()
            }
            Language::JaJp => {
                "市場参加権限は開いているが、実行可能な銘柄がないため実効アクションは NO_NEW_ENTRY".to_string()
            }
        }
    }

    fn map_continuity_state(streak: usize, dict: &DisplayDictionary) -> String {
        match streak {
            0 => dict.signals.continuity_none.clone(),
            1 => dict.signals.continuity_emerging.clone(),
            2 => dict.signals.continuity_building.clone(),
            _ => dict.signals.continuity_sustained.clone(),
        }
    }

    fn build_hypothesis_layer_from_packet(
        packet: &DecisionPacket,
        dict: &DisplayDictionary,
    ) -> Option<HypothesisLayerViewModel> {
        let trend_recognition = packet.trend_recognition.as_ref()?;
        let substantive = trend_recognition.substantive.as_ref()?;
        let mut substantive_signals = Vec::new();
        let has_capex = substantive.capex_payoff_signal
            || substantive
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::CapexPayoff);
        if has_capex {
            substantive_signals.push(dict.trend_recognition.capex_payoff.clone());
        }
        let has_earnings = substantive.earnings_validation
            || substantive
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::EarningsValidation);
        if has_earnings {
            substantive_signals.push(dict.trend_recognition.earnings_validation.clone());
        }
        let has_order = substantive.order_visibility
            || substantive
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::OrderVisibility);
        if has_order {
            substantive_signals.push(dict.trend_recognition.order_visibility.clone());
        }
        let breadth_mode = risk_taxonomy_read_model::classify_trend_breadth_mode(packet);
        let market_cycle_position = risk_taxonomy_read_model::classify_market_cycle_position(
            packet,
            breadth_mode,
            substantive_signals.len(),
            Some(trend_recognition.conviction_score),
        );
        build_hypothesis_layer(HypothesisReadModelInput {
            substantive_signals: &substantive_signals,
            substantive_records: &substantive.records,
            evidence_presence: HypothesisEvidencePresence {
                capex: has_capex,
                earnings: has_earnings,
                order: has_order,
            },
            conviction_score: Some(trend_recognition.conviction_score),
            market_cycle_position,
            as_of_date: packet.date,
            dict,
        })
    }

    fn derive_telegram_reason(
        asset: &crate::features::radar::domain::action_matrix::AssetActionDecision,
        is_restrained: bool,
        is_systemic_collapse: bool,
        _packet: &DecisionPacket,
        _language: Language,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::features::radar::domain::exit::AssetExitState;
        if asset.exit_decision.asset_exit_state != AssetExitState::None {
            return match asset.exit_decision.asset_exit_state {
                AssetExitState::DefensiveExit => {
                    if is_systemic_collapse {
                        dict.reasons.exit_systemic_collapse.clone()
                    } else {
                        dict.reasons.exit_structural_fragility.clone()
                    }
                }
                AssetExitState::StrengthLoss => dict.reasons.exit_strength_loss.clone(),
                AssetExitState::CohesionExit => dict.reasons.exit_cohesion.clone(),
                AssetExitState::OverheatProfitTake => dict.reasons.exit_overheat.clone(),
                AssetExitState::None => String::new(),
            };
        }
        match asset.asset_state.state {
            AssetState::PULLBACK => {
                if is_restrained {
                    dict.reasons.state_pullback_restrained.clone()
                } else {
                    dict.reasons.state_pullback_normal.clone()
                }
            }
            AssetState::OPTIMAL => {
                if is_restrained {
                    dict.reasons.state_optimal_restrained.clone()
                } else {
                    dict.reasons.state_optimal_normal.clone()
                }
            }
            AssetState::DEFEND => dict.reasons.state_defend.clone(),
            AssetState::OVERHEAT => dict.reasons.state_overheat.clone(),
            AssetState::CRUISE => {
                if is_restrained {
                    dict.reasons.state_cruise_restrained.clone()
                } else {
                    dict.reasons.state_cruise.clone()
                }
            }
            AssetState::CAUTION => dict.reasons.state_caution.clone(),
            AssetState::FORMING => {
                if is_restrained {
                    dict.reasons.state_forming_restrained.clone()
                } else {
                    dict.reasons.state_forming_normal.clone()
                }
            }
        }
    }

    fn derive_display_context(
        asset: &crate::features::radar::domain::action_matrix::AssetActionDecision,
        positions: &HashMap<String, (f64, f64)>,
        current_top_tier: &HashSet<&str>,
        core_assets_list: &HashSet<&str>,
        cohesion_ready: bool,
        is_core_fact: bool,
    ) -> DisplayContext {
        let has_position = asset.has_position_fact || positions.contains_key(&asset.symbol);
        let is_top_tier = current_top_tier.contains(asset.symbol.as_str());
        let is_core_rules = is_core_fact || core_assets_list.contains(asset.symbol.as_str());
        let is_core_holding = has_position && (is_core_rules || is_top_tier);
        let is_candidate_only = Self::is_candidate_eligible(asset, positions, current_top_tier);

        DisplayContext {
            has_position,
            is_core_holding,
            is_candidate_only,
            is_top_tier,
            cohesion_ready,
        }
    }

    /// 相対順位だけでは候補資格を生成せず、Action Matrix の新增資格を表示境界で確認する。
    fn is_candidate_eligible(
        asset: &crate::features::radar::domain::action_matrix::AssetActionDecision,
        positions: &HashMap<String, (f64, f64)>,
        current_top_tier: &HashSet<&str>,
    ) -> bool {
        let has_position = asset.has_position_fact || positions.contains_key(&asset.symbol);
        !has_position
            && current_top_tier.contains(asset.symbol.as_str())
            && asset.action
                == crate::features::radar::domain::action_matrix::AssetAction::ACCUMULATE
            && !matches!(
                asset.position_intent,
                crate::features::radar::domain::exit::PositionIntent::TRIM
                    | crate::features::radar::domain::exit::PositionIntent::EXIT
            )
            && !matches!(
                asset.exit_decision.position_intent,
                crate::features::radar::domain::exit::PositionIntent::TRIM
                    | crate::features::radar::domain::exit::PositionIntent::EXIT
            )
    }

    fn derive_display_intent(
        final_intent: crate::features::radar::domain::position_intent::UnifiedPositionIntent,
        context: &DisplayContext,
    ) -> DisplayIntent {
        DisplayAdapter::derive_display_intent(final_intent, context)
    }

    fn derive_unified_intent(
        asset: &crate::features::radar::domain::action_matrix::AssetActionDecision,
        context: &DisplayContext,
    ) -> crate::features::radar::domain::position_intent::UnifiedPositionIntent {
        crate::features::radar::domain::position_intent::UnifiedIntentSynthesizer::synthesize(
            asset.position_intent,
            &asset.exit_decision,
            context.cohesion_ready,
            context.has_position,
            asset.asset_state.state,
        )
        .intent
    }

    fn build_exit_summary(
        packet: &DecisionPacket,
        positions: &HashMap<String, (f64, f64)>,
        top_tier_set: &HashSet<&str>,
        core_assets_set: &HashSet<&str>,
        cohesion_ready: bool,
        language: Language,
        dict: &DisplayDictionary,
    ) -> ExitDecisionSummaryViewModel {
        let mut items = Vec::new();
        let mut signal_items = Vec::new();
        let is_systemic_collapse = Self::is_systemic_collapse(packet);

        for asset in &packet.assets {
            let context = Self::derive_display_context(
                asset,
                positions,
                top_tier_set,
                core_assets_set,
                cohesion_ready,
                asset.is_core_fact,
            );
            let unified_intent = Self::derive_unified_intent(asset, &context);

            let (intent, intent_label, reason) = match unified_intent {
                crate::features::radar::domain::position_intent::UnifiedPositionIntent::Exit => (
                    ExitDisplayIntent::Exit,
                    dict.decision.exit_intent_exit.clone(),
                    Self::defensive_position_reason(is_systemic_collapse, dict),
                ),
                crate::features::radar::domain::position_intent::UnifiedPositionIntent::Trim => {
                    let reason = match asset.exit_decision.asset_exit_state {
                        AssetExitState::StrengthLoss => {
                            dict.reasons.position_trim_strength_loss.clone()
                        }
                        AssetExitState::CohesionExit => dict.reasons.position_trim_cohesion.clone(),
                        AssetExitState::OverheatProfitTake => {
                            dict.reasons.position_trim_overheat.clone()
                        }
                        AssetExitState::DefensiveExit => {
                            Self::defensive_position_reason(is_systemic_collapse, dict)
                        }
                        AssetExitState::None => dict.reasons.position_trim_strength_loss.clone(),
                    };
                    (
                        ExitDisplayIntent::Trim,
                        dict.decision.exit_intent_trim.clone(),
                        reason,
                    )
                }
                crate::features::radar::domain::position_intent::UnifiedPositionIntent::Hold => (
                    ExitDisplayIntent::Hold,
                    dict.decision.exit_intent_hold.clone(),
                    dict.reasons.position_hold_core.clone(),
                ),
                crate::features::radar::domain::position_intent::UnifiedPositionIntent::Watch
                | crate::features::radar::domain::position_intent::UnifiedPositionIntent::Add => {
                    if matches!(
                        asset.asset_state.state,
                        crate::features::radar::domain::asset_state::AssetState::PULLBACK
                            | crate::features::radar::domain::asset_state::AssetState::CAUTION
                            | crate::features::radar::domain::asset_state::AssetState::FORMING
                    ) {
                        (
                            ExitDisplayIntent::Watch,
                            dict.decision.exit_intent_watch.clone(),
                            if asset.asset_state.state
                                == crate::features::radar::domain::asset_state::AssetState::PULLBACK
                            {
                                dict.reasons.position_watch_pullback.clone()
                            } else {
                                dict.reasons.position_watch_no_trigger.clone()
                            },
                        )
                    } else {
                        (
                            ExitDisplayIntent::Watch,
                            dict.decision.exit_intent_watch.clone(),
                            dict.reasons.position_watch_no_trigger.clone(),
                        )
                    }
                }
            };

            let item = ExitDecisionItemViewModel {
                symbol: asset.symbol.clone(),
                intent,
                intent_label,
                reason,
                action_state: Self::asset_action_code(asset.action).to_string(),
                observation_modifier: Self::is_relative_strength_recovering(packet, &asset.symbol)
                    .then(|| "RECOVERY_WATCH".to_string()),
                observation_explanation: Self::is_relative_strength_recovering(
                    packet,
                    &asset.symbol,
                )
                .then(|| Self::recovery_watch_reason(language)),
            };
            if matches!(
                item.intent,
                ExitDisplayIntent::Trim | ExitDisplayIntent::Exit
            ) {
                signal_items.push(item.clone());
            }
            if context.has_position {
                items.push(item);
            }
        }

        let sort_fn = |a: &ExitDecisionItemViewModel, b: &ExitDecisionItemViewModel| {
            let prio = |intent: ExitDisplayIntent| match intent {
                ExitDisplayIntent::Exit => 0,
                ExitDisplayIntent::Trim => 1,
                ExitDisplayIntent::Hold => 2,
                ExitDisplayIntent::Watch => 3,
            };
            prio(a.intent)
                .cmp(&prio(b.intent))
                .then_with(|| a.symbol.cmp(&b.symbol))
        };
        items.sort_by(sort_fn);
        signal_items.sort_by(sort_fn);

        ExitDecisionSummaryViewModel {
            title: dict.headers.position_handling.clone(),
            signal_title: dict.decision.reduction_signal.clone(),
            actual_action_title: dict.decision.actual_portfolio_action.clone(),
            empty_note: if items.is_empty() && signal_items.is_empty() {
                Some(
                    [
                        dict.reasons.position_none.as_str(),
                        dict.reasons.position_none_no_trigger.as_str(),
                    ]
                    .into_iter()
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n"),
                )
            } else {
                None
            },
            no_action_note: if items.is_empty() && !signal_items.is_empty() {
                Some(dict.reasons.position_no_action_no_holding.clone())
            } else {
                None
            },
            signal_items,
            items,
        }
    }

    fn is_systemic_collapse(packet: &DecisionPacket) -> bool {
        packet.market_regime.risk_overlay == RiskOverlay::BROKEN
            || packet
                .market_regime
                .transition_audit
                .as_ref()
                .is_some_and(|audit| audit.core_breakdown)
    }

    fn defensive_position_reason(is_systemic_collapse: bool, dict: &DisplayDictionary) -> String {
        if is_systemic_collapse {
            dict.reasons.position_exit_systemic_collapse.clone()
        } else {
            dict.reasons.position_exit_structural_fragility.clone()
        }
    }

    fn is_relative_strength_recovering(packet: &DecisionPacket, symbol: &str) -> bool {
        packet
            .current_relative_strength_observations
            .iter()
            .find(|observation| observation.symbol == symbol)
            .is_some_and(|observation| {
                matches!(
                    (observation.state, observation.recovery_strength),
                    (
                        crate::features::radar::domain::current_relative_strength::RelativeStrengthState::Improving,
                        crate::features::radar::domain::current_relative_strength::RecoveryStrength::Strong
                            | crate::features::radar::domain::current_relative_strength::RecoveryStrength::Moderate
                    )
                )
            })
    }

    fn recovery_watch_reason(language: Language) -> String {
        match language {
            Language::ZhCn => "长期/累计结构仍弱，但短期相对强度正在明显恢复；保持基础 Action Matrix 状态，不再描述为连续转弱。".to_string(),
            Language::EnUs => "The long-term cumulative structure remains weak, but short-term relative strength is recovering; keep the base Action Matrix state and stop describing continued weakening.".to_string(),
            Language::JaJp => "長期・累積構造はなお弱い一方、短期相対強度は回復しています。基本 Action Matrix 状態を維持し、連続的な弱化とは表現しません。".to_string(),
        }
    }

    fn asset_action_code(
        action: crate::features::radar::domain::action_matrix::AssetAction,
    ) -> &'static str {
        match action {
            crate::features::radar::domain::action_matrix::AssetAction::ACCUMULATE => "ACCUMULATE",
            crate::features::radar::domain::action_matrix::AssetAction::HOLD => "HOLD",
            crate::features::radar::domain::action_matrix::AssetAction::REDUCE => "REDUCE",
            crate::features::radar::domain::action_matrix::AssetAction::FREEZE => "FREEZE",
            crate::features::radar::domain::action_matrix::AssetAction::OBSERVE => "OBSERVE",
            crate::features::radar::domain::action_matrix::AssetAction::AVOID => "AVOID",
            crate::features::radar::domain::action_matrix::AssetAction::WAIT => "WAIT",
        }
    }

    fn build_breakout_summary(
        packet: &DecisionPacket,
        rules: &ParsedRules,
        dict: &DisplayDictionary,
        lang: Language,
    ) -> BreakoutSummaryViewModel {
        use crate::features::radar::domain::breakout_detection::{BreakoutReason, BreakoutStatus};
        let is_no_trade = !packet.trend_cohesion.gate_passed;
        let failed_risk_display_threshold = if is_no_trade {
            rules.breakout.failed_breakout_no_trade_display_threshold
        } else {
            rules.breakout.failed_breakout_display_threshold
        };

        let mut items: Vec<BreakoutItemViewModel> = packet
            .assets
            .iter()
            .filter_map(|asset| {
                let breakout = &asset.breakout;
                let has_high_failed_risk = breakout
                    .reasons
                    .contains(&BreakoutReason::FailedBreakoutRisk)
                    && breakout.failed_breakout_risk >= failed_risk_display_threshold;
                let has_signal = breakout.status != BreakoutStatus::NoBreakout
                    || breakout.reasons.contains(&BreakoutReason::OrdinaryRebound)
                    || breakout.reasons.contains(&BreakoutReason::PullbackRepair)
                    || breakout
                        .reasons
                        .contains(&BreakoutReason::FailedBreakoutRisk);
                let is_visible = if is_no_trade {
                    breakout.status != BreakoutStatus::NoBreakout || has_high_failed_risk
                } else {
                    has_signal
                };
                if !is_visible {
                    return None;
                }

                let status = match breakout.status {
                    BreakoutStatus::NoBreakout => BreakoutDisplayStatus::NoBreakout,
                    BreakoutStatus::EmergingBreakout => BreakoutDisplayStatus::EmergingBreakout,
                    BreakoutStatus::ConfirmedBreakout => BreakoutDisplayStatus::ConfirmedBreakout,
                };
                let base_status_label = match breakout.status {
                    BreakoutStatus::NoBreakout => dict.breakout.no_breakout.clone(),
                    BreakoutStatus::EmergingBreakout => dict.breakout.emerging_breakout.clone(),
                    BreakoutStatus::ConfirmedBreakout => dict.breakout.confirmed_breakout.clone(),
                };
                let status_label = Self::format_breakout_status_with_age(
                    &base_status_label,
                    breakout.status,
                    breakout.breakout_age,
                    lang,
                );
                let reason = if is_no_trade
                    && breakout.status == BreakoutStatus::NoBreakout
                    && has_high_failed_risk
                {
                    dict.breakout.failed_breakout_risk.clone()
                } else {
                    Self::localize_breakout_reason(&asset.breakout.reasons, dict)
                };

                Some(BreakoutItemViewModel {
                    symbol: asset.symbol.clone(),
                    status,
                    status_label,
                    reason,
                    strength_value: format!("{:.0}", breakout.breakout_strength),
                    quality_value: format!("{:.0}", breakout.breakout_quality),
                    consecutive_days: breakout.breakout_age,
                    failed_risk_value: if breakout.failed_breakout_risk
                        >= failed_risk_display_threshold
                    {
                        Some(format!("{:.0}", breakout.failed_breakout_risk))
                    } else {
                        None
                    },
                })
            })
            .collect();

        items.sort_by(|a, b| {
            let prio = |status: BreakoutDisplayStatus| match status {
                BreakoutDisplayStatus::ConfirmedBreakout => 0,
                BreakoutDisplayStatus::EmergingBreakout => 1,
                BreakoutDisplayStatus::NoBreakout => 2,
            };
            prio(a.status)
                .cmp(&prio(b.status))
                .then_with(|| {
                    b.quality_value
                        .parse::<u32>()
                        .unwrap_or(0)
                        .cmp(&a.quality_value.parse::<u32>().unwrap_or(0))
                })
                .then_with(|| a.symbol.cmp(&b.symbol))
        });

        BreakoutSummaryViewModel {
            title: dict.headers.breakout_detection.clone(),
            empty_note: if items.is_empty() {
                Some(dict.breakout.empty_note.clone())
            } else {
                None
            },
            items,
        }
    }

    fn format_breakout_status_with_age(
        base_label: &str,
        status: crate::features::radar::domain::breakout_detection::BreakoutStatus,
        breakout_age: usize,
        lang: Language,
    ) -> String {
        use crate::features::radar::domain::breakout_detection::BreakoutStatus;
        match status {
            BreakoutStatus::NoBreakout => base_label.to_string(),
            BreakoutStatus::EmergingBreakout | BreakoutStatus::ConfirmedBreakout => {
                let day = breakout_age.max(1);
                match lang {
                    Language::ZhCn => format!("{}（第{}天）", base_label, day),
                    Language::EnUs => format!("{} (Day {})", base_label, day),
                    Language::JaJp => format!("{}（{}日目）", base_label, day),
                }
            }
        }
    }

    fn build_decision_summary(
        packet: &DecisionPacket,
        is_data_missing: bool,
        state: MarketState,
        dict: &DisplayDictionary,
        battleboard: &BattleboardSnapshot,
    ) -> DecisionSummaryViewModel {
        let not_ready = is_data_missing || !packet.trend_cohesion.gate_passed;
        let (
            action_status_value,
            behavior_mode_value,
            exposure_value,
            entry_cap_value,
            summary,
            hard_rule_note,
            state_tag_value,
            action_tag_value,
            entry_cap_note,
        ) = if is_data_missing {
            (
                dict.decision.no_trade.clone(),
                dict.decision.no_trade_action.clone(),
                "N/A".to_string(),
                "0%".to_string(),
                dict.market_summaries.data_missing.clone(),
                dict.decision.no_trade_rule.clone(),
                dict.decision.state_data_unavailable.clone(),
                dict.decision.no_trade_action.clone(),
                Some(dict.decision.entry_cap_note.clone()),
            )
        } else if !packet.trend_cohesion.gate_passed {
            (
                dict.decision.no_trade.clone(),
                dict.decision.no_trade_action.clone(),
                "N/A".to_string(),
                "0%".to_string(),
                dict.decision.no_trade_summary.clone(),
                dict.decision.no_trade_rule.clone(),
                if matches!(state, MarketState::IGNITION | MarketState::NEWBORN) {
                    dict.decision.state_ignition_unconfirmed.clone()
                } else {
                    dict.decision.state_cohesion_blocked.clone()
                },
                dict.decision.no_trade_action.clone(),
                Some(dict.decision.entry_cap_note.clone()),
            )
        } else {
            match state {
                MarketState::IGNITION | MarketState::NEWBORN => (
                    dict.decision.probe.clone(),
                    dict.decision.probe.clone(),
                    "10-30%".to_string(),
                    "10-30%".to_string(),
                    dict.market_summaries.ignition.clone(),
                    String::new(),
                    dict.market_stages.ignition.clone(),
                    dict.decision.probe.clone(),
                    None,
                ),
                MarketState::EARLY_CONFIRMATION => (
                    dict.decision.accumulate.clone(),
                    dict.decision.accumulate.clone(),
                    "20-40%".to_string(),
                    "20-40%".to_string(),
                    dict.market_summaries.established.clone(),
                    String::new(),
                    dict.market_stages.neutral.clone(),
                    dict.decision.accumulate.clone(),
                    None,
                ),
                MarketState::ESTABLISHED | MarketState::CONFIRMED => (
                    dict.decision.trend_follow.clone(),
                    dict.decision.trend_follow.clone(),
                    "30-70%".to_string(),
                    "30-70%".to_string(),
                    dict.market_summaries.established.clone(),
                    String::new(),
                    dict.market_stages.established.clone(),
                    dict.decision.trend_follow.clone(),
                    None,
                ),
                MarketState::DEFENSIVE => (
                    dict.decision.defensive.clone(),
                    dict.decision.defensive.clone(),
                    "0-20%".to_string(),
                    "0-20%".to_string(),
                    dict.market_summaries.defensive.clone(),
                    String::new(),
                    dict.market_stages.defensive.clone(),
                    dict.decision.defensive.clone(),
                    None,
                ),
            }
        };

        let mut readiness_reasons = if is_data_missing {
            vec![dict.market_summaries.data_missing.clone()]
        } else {
            packet.market_regime.reasons.clone()
        };

        // structured unmet_conditions で表現済みの閾値理由は重複表示しない。
        // legacy module への依存を避けるため、抑制ロジックを簡略化している。
        readiness_reasons.retain(|reason| {
            let is_stability = reason.contains("稳定性")
                || reason.contains("安定性")
                || reason.to_lowercase().contains("stability")
                || reason.to_lowercase().contains("score");
            let is_continuity = reason.contains("连续性")
                || reason.contains("連続性")
                || reason.contains("持续性")
                || reason.contains("継続性")
                || reason.to_lowercase().contains("continuity")
                || reason.to_lowercase().contains("streak");

            if is_stability
                && packet.trend_cohesion.unmet_conditions.contains(
                    &crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                )
            {
                // 比較演算子と数値を含む標準的な閾値テンプレートは抑制する。
                // ただし custom prefix や複雑な技術説明を含む場合は保持する。
                let has_operator = reason.contains("<")
                    || reason.contains("＜")
                    || reason.contains("≤")
                    || reason.contains("≦")
                    || reason.contains("≥")
                    || reason.contains("⩽")
                    || reason.contains("﹤")
                    || reason.to_lowercase().contains("below")
                    || reason.to_lowercase().contains("under")
                    || reason.to_lowercase().contains("less than");
                let has_digit = reason.chars().any(|c| c.is_ascii_digit());
                let looks_like_template = has_operator
                    && has_digit
                    && !reason.contains("提示")
                    && !reason.contains("备注")
                    && !reason.contains("備考")
                    && !reason.contains("注释")
                    && !reason.contains("注釈")
                    && !reason.contains("需")
                    && !reason.contains("结合")
                    && !reason.contains("thrash")
                    && !reason.contains("<==");
                !looks_like_template
            } else if is_continuity
                && packet.trend_cohesion.unmet_conditions.contains(
                    &crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                )
            {
                let has_operator = reason.contains("<")
                    || reason.contains("＜")
                    || reason.contains("≤")
                    || reason.contains("≦")
                    || reason.contains("≥")
                    || reason.contains("⩽")
                    || reason.contains("﹤")
                    || reason.to_lowercase().contains("below")
                    || reason.to_lowercase().contains("under")
                    || reason.to_lowercase().contains("less than");
                let has_digit = reason.chars().any(|c| c.is_ascii_digit());
                let looks_like_template = has_operator
                    && has_digit
                    && !reason.contains("提示")
                    && !reason.contains("备注")
                    && !reason.contains("備考")
                    && !reason.contains("注释")
                    && !reason.contains("注釈")
                    && !reason.contains("需")
                    && !reason.contains("结合")
                    && !reason.contains("thrash")
                    && !reason.contains("<==");
                !looks_like_template
            } else {
                true
            }
        });

        let has_persistent_main_theme = risk_taxonomy_read_model::has_persistent_main_theme(packet);
        let trend_cohesion_value =
            if has_persistent_main_theme && !packet.trend_cohesion.gate_passed {
                dict.trend_cohesion.persistent_not_ready.clone()
            } else {
                match packet.trend_cohesion.status {
                crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                    dict.trend_cohesion.dispersed.clone()
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                    dict.trend_cohesion.forming.clone()
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => {
                    dict.trend_cohesion.formed.clone()
                }
            }
            };

        let trend_topology_value = if has_persistent_main_theme {
            dict.trend_cohesion.topology_core_leadership.clone()
        } else {
            match packet.trend_cohesion.topology {
                crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader => {
                    dict.trend_cohesion.topology_no_leader.clone()
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::SingleLeader => {
                    dict.trend_cohesion.topology_single_leader.clone()
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::FragmentedLeaders => {
                    dict.trend_cohesion.topology_fragmented_leaders.clone()
                }
            }
        };

        let formation_conditions: Vec<String> = vec![
            dict.trend_cohesion.conditions.stability_threshold.clone(),
            dict.trend_cohesion.conditions.continuity_threshold.clone(),
            dict.trend_cohesion.conditions.directional_cohesion.clone(),
        ];

        let unmet_conditions: Vec<String> = packet
            .trend_cohesion
            .unmet_conditions
            .iter()
            .map(|r| match r {
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold => {
                    dict.trend_cohesion.unmet.stability_threshold.replace(
                        "{}",
                        &format!("{:.1}", packet.trend_cohesion.stability_score),
                    )
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold => {
                    dict.trend_cohesion
                        .unmet
                        .continuity_threshold
                        .replace("{}", &packet.trend_cohesion.continuity_streak.to_string())
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion => {
                    dict.trend_cohesion.unmet.directional_cohesion.clone()
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::HighCandidateDispersion => {
                    dict.trend_cohesion
                        .unmet
                        .high_candidate_dispersion
                        .replace("{}", &packet.trend_cohesion.candidate_count.to_string())
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::UnstableRotation => {
                    dict.trend_cohesion.unmet.unstable_rotation.replace(
                        "{}",
                        &format!("{:.0}", packet.trend_cohesion.rotation_quality_score),
                    )
                }
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::WeakLeadership => dict
                    .trend_cohesion
                    .unmet
                    .weak_leadership
                    .replace("{}", &packet.trend_cohesion.leader_count.to_string()),
            })
            .collect();

        DecisionSummaryViewModel {
            is_no_trade: not_ready,
            section_title: dict.headers.decision_summary.clone(),
            trend_cohesion_label: dict.trend_cohesion.label.clone(),
            trend_cohesion_value,
            trend_topology_label: dict.trend_cohesion.topology_label.clone(),
            trend_topology_value,
            gate_passed: packet.trend_cohesion.gate_passed,
            formation_conditions_label: dict.trend_cohesion.formation_conditions_label.clone(),
            unmet_conditions_label: dict.trend_cohesion.unmet_conditions_label.clone(),
            formation_conditions,
            unmet_conditions,
            action_status_label: dict.decision.action_status.clone(),
            action_status_value,
            state_tag_label: dict.decision.state_tag.clone(),
            state_tag_value,
            action_tag_label: dict.decision.action_tag.clone(),
            action_tag_value,
            behavior_mode_label: dict.decision.behavior_mode.clone(),
            behavior_mode_value,
            exposure_label: dict.decision.exposure_guidance.clone(),
            exposure_value,
            entry_cap_label: dict.decision.entry_cap.clone(),
            entry_cap_value,
            entry_cap_note,
            hard_rule_note,
            summary,
            readiness_reasons_label: dict.decision.readiness_reasons.clone(),
            readiness_reasons,
            compact_stability_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!("{:.1}", packet.trend_cohesion.stability_score)
            },
            compact_continuity_value: if is_data_missing {
                "N/A".to_string()
            } else {
                packet.trend_cohesion.continuity_streak.to_string()
            },
            candidate_only_note: if not_ready && !is_data_missing {
                Some(dict.decision.candidate_only_note.clone())
            } else {
                None
            },
            market_board_label: dict.decision.market_board.clone(),
            market_board_value: format!(
                "{} {} | {} {} | {} {}",
                dict.decision.watch_count,
                battleboard.watch_count,
                dict.decision.hold_count,
                battleboard.hold_count,
                dict.decision.defend_count,
                battleboard.defend_count
            ),
            opportunity_snapshot_label: dict.decision.opportunity.clone(),
            opportunity_snapshot_value: battleboard.opportunity_snapshot_value.clone(),
            risk_snapshot_label: dict.decision.risk.clone(),
            risk_snapshot_value: battleboard.risk_snapshot_value.clone(),
        }
    }

    fn localize_breakout_reason(
        reasons: &[crate::features::radar::domain::breakout_detection::BreakoutReason],
        dict: &DisplayDictionary,
    ) -> String {
        use crate::features::radar::domain::breakout_detection::BreakoutReason;

        if reasons.contains(&BreakoutReason::StructuralBreakout) {
            dict.breakout.structural_breakout.clone()
        } else if reasons.contains(&BreakoutReason::PullbackRepair) {
            dict.breakout.pullback_repair.clone()
        } else if reasons.contains(&BreakoutReason::OrdinaryRebound) {
            dict.breakout.ordinary_rebound.clone()
        } else if reasons.contains(&BreakoutReason::FailedBreakoutRisk) {
            dict.breakout.failed_breakout_risk.clone()
        } else {
            dict.breakout.ordinary_rebound.clone()
        }
    }

    fn summarize_primary_risk(
        risk_items: &[&crate::features::radar::interface::display::RiskOpportunityViewModel],
        packet: &DecisionPacket,
        dict: &DisplayDictionary,
        language: Language,
    ) -> String {
        if risk_items.is_empty() {
            return dict.decision.no_risk.clone();
        }

        let mut grouped: HashMap<String, (usize, usize, String)> = HashMap::new();
        for (idx, item) in risk_items.iter().enumerate() {
            let reason = Self::canonical_risk_reason(item, packet, language);
            let entry = grouped.entry(reason).or_insert_with(|| {
                (
                    0,
                    idx,
                    item.symbol.clone(), // 初出銘柄を主銘柄候補として保持
                )
            });
            entry.0 += 1;
        }

        let mut best_reason = String::new();
        let mut best_count = 0usize;
        let mut best_first_idx = usize::MAX;
        let mut best_symbol = String::new();
        for (reason, (count, first_idx, symbol)) in grouped {
            let better = count > best_count
                || (count == best_count
                    && (first_idx < best_first_idx
                        || (first_idx == best_first_idx && reason < best_reason)));
            if better {
                best_reason = reason;
                best_count = count;
                best_first_idx = first_idx;
                best_symbol = symbol;
            }
        }

        let same_reason_peers = best_count.saturating_sub(1);
        if same_reason_peers == 0 {
            format!("{} · {}", best_symbol, best_reason)
        } else {
            let peer_text = dict
                .decision
                .risk_peer_suffix
                .replace("{count}", &same_reason_peers.to_string());
            format!("{} · {}{}", best_symbol, best_reason, peer_text)
        }
    }

    fn canonical_risk_reason(
        item: &crate::features::radar::interface::display::RiskOpportunityViewModel,
        packet: &DecisionPacket,
        language: Language,
    ) -> String {
        let Some(asset) = packet
            .assets
            .iter()
            .find(|asset| asset.symbol == item.symbol)
        else {
            return item.reason.clone();
        };
        if asset.exit_decision.asset_exit_state != AssetExitState::StrengthLoss {
            return item.reason.clone();
        }

        if Self::is_relative_strength_recovering(packet, &asset.symbol) {
            return match language {
                Language::ZhCn => {
                    "结构性减仓信号仍有效；短期 RS 恢复中（RECOVERY_WATCH）".to_string()
                }
                Language::EnUs => {
                    "Structural trim signal remains active; short-term RS is recovering (RECOVERY_WATCH).".to_string()
                }
                Language::JaJp => {
                    "構造的な縮小シグナルは有効です。短期相対強度は回復中です（RECOVERY_WATCH）。".to_string()
                }
            };
        }

        let leaderless = packet.top_tier_symbols.is_empty()
            || matches!(
                packet.trend_cohesion.topology,
                crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader
            );
        if leaderless {
            return match language {
                Language::ZhCn => "结构性减仓信号仍有效".to_string(),
                Language::EnUs => "Structural trim signal remains active.".to_string(),
                Language::JaJp => "構造的な縮小シグナルは有効です。".to_string(),
            };
        }

        item.reason.clone()
    }
}

#[cfg(test)]
mod breakout_projection_tests {
    use super::PresentationAssembler;
    use crate::features::radar::domain::breakout_detection::BreakoutStatus;
    use crate::features::shared::interface::i18n::Language;

    #[test]
    fn breakout_age_is_rendered_as_the_same_day_number_used_by_the_snapshot() {
        let label = PresentationAssembler::format_breakout_status_with_age(
            "Emerging",
            BreakoutStatus::EmergingBreakout,
            4,
            Language::EnUs,
        );
        assert_eq!(label, "Emerging (Day 4)");
    }
}

#[cfg(test)]
mod risk_summary_tests {
    use super::PresentationAssembler;
    use crate::features::radar::domain::action_matrix::AssetActionDecision;
    use crate::features::radar::domain::current_relative_strength::{
        CurrentRelativeStrengthObservation, RecoveryStrength, RelativeStrengthState,
    };
    use crate::features::radar::domain::decision::DecisionPacket;
    use crate::features::radar::domain::exit::{AssetExitState, ExitDecision, PositionIntent};
    use crate::features::radar::interface::display::RiskOpportunityViewModel;
    use crate::features::shared::interface::i18n::{get_dictionary, Language};

    #[test]
    fn recovery_watch_risk_summary_does_not_claim_continued_weakening() {
        let packet = DecisionPacket {
            assets: vec![AssetActionDecision {
                symbol: "TRIMME".to_string(),
                exit_decision: ExitDecision {
                    position_intent: PositionIntent::TRIM,
                    asset_exit_state: AssetExitState::StrengthLoss,
                    ..Default::default()
                },
                ..Default::default()
            }],
            current_relative_strength_observations: vec![CurrentRelativeStrengthObservation {
                symbol: "TRIMME".to_string(),
                benchmark_symbol: "SPY".to_string(),
                relative_1d_vs_benchmark: Some(1.0),
                relative_5d_vs_benchmark: Some(3.0),
                trend_slope: Some(1.0),
                price_position: Some(1.0),
                volume_participation: Some(1.0),
                state: RelativeStrengthState::Improving,
                recovery_strength: RecoveryStrength::Moderate,
                boundary: "Observation only".to_string(),
            }],
            ..Default::default()
        };
        let item = RiskOpportunityViewModel {
            kind: "风险".to_string(),
            symbol: "TRIMME".to_string(),
            reason: "📉 主线掉队: 连续转弱触发结构性减仓".to_string(),
        };

        let summary = PresentationAssembler::summarize_primary_risk(
            &[&item],
            &packet,
            &get_dictionary(Language::ZhCn),
            Language::ZhCn,
        );

        assert!(summary.contains("RECOVERY_WATCH"));
        assert!(!summary.contains("主线掉队"));
        assert!(!summary.contains("连续转弱"));
    }

    #[test]
    fn leaderless_risk_summary_does_not_claim_a_mainline() {
        let packet = DecisionPacket {
            assets: vec![AssetActionDecision {
                symbol: "TRIMME".to_string(),
                exit_decision: ExitDecision {
                    position_intent: PositionIntent::TRIM,
                    asset_exit_state: AssetExitState::StrengthLoss,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let item = RiskOpportunityViewModel {
            kind: "风险".to_string(),
            symbol: "TRIMME".to_string(),
            reason: "📉 主线掉队: 连续转弱触发结构性减仓".to_string(),
        };

        let summary = PresentationAssembler::summarize_primary_risk(
            &[&item],
            &packet,
            &get_dictionary(Language::ZhCn),
            Language::ZhCn,
        );

        assert!(!summary.contains("主线掉队"));
    }
}
