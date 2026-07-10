use crate::features::radar::domain::asset_state::AssetState;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::exit::AssetExitState;
use crate::features::radar::domain::market_regime::{MarketState, RiskOverlay};
use crate::features::radar::domain::rules::ParsedRules;
use crate::features::radar::domain::trend_cohesion::EvidenceType;
use crate::features::radar::interface::display::{DisplayAdapter, DisplayContext, DisplayIntent};
use crate::features::radar::interface::hypothesis_read_model::{
    build_hypothesis_layer, HypothesisEvidencePresence, HypothesisReadModelInput,
};
use crate::features::radar::interface::presentation::{
    BreakoutDisplayStatus, BreakoutItemViewModel, BreakoutSummaryViewModel, DataAlertViewModel,
    DecisionSummaryViewModel, ExitDecisionItemViewModel, ExitDecisionSummaryViewModel,
    ExitDisplayIntent, HypothesisLayerViewModel, MacroDisplayContext, PresentationPacket,
    RiskOpportunitySummaryViewModel, SignalSummaryViewModel, TrendBreadthMode,
};
use crate::features::radar::interface::risk_taxonomy_read_model;
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
        let signal_summary = SignalSummaryViewModel {
            confidence_label: dict.signals.confidence.clone(),
            confidence_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!("{:.0}", packet.market_features.system_confidence)
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
            breadth_label: "Breadth".to_string(),
            breadth_value: match trend_breadth_mode {
                TrendBreadthMode::BroadExpansion => "Broad Participation".to_string(),
                TrendBreadthMode::NarrowLeadership => "Very Narrow".to_string(),
                TrendBreadthMode::FragileRotation => "Narrow".to_string(),
                TrendBreadthMode::StructuralDefense => "Narrow".to_string(),
            },
            breadth_semantic_label: "Breadth Label".to_string(),
            breadth_semantic_value: match trend_breadth_mode {
                TrendBreadthMode::BroadExpansion => "Broad Participation".to_string(),
                TrendBreadthMode::NarrowLeadership => "Very Narrow".to_string(),
                TrendBreadthMode::FragileRotation => "Healthy Expansion".to_string(),
                TrendBreadthMode::StructuralDefense => "Narrow".to_string(),
            },
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
                &asset.symbol,
                positions,
                &top_tier_set,
                &core_assets_set,
                is_ready,
                asset.is_core_fact,
                asset.has_position_fact,
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
        let risk_value = Self::summarize_primary_risk(&risk_items, &dict);
        let risk_opportunity_summary = RiskOpportunitySummaryViewModel {
            opportunity_label: dict.decision.opportunity.clone(),
            opportunity_value,
            risk_label: dict.decision.risk.clone(),
            risk_value,
        };
        let battleboard = BattleboardSnapshot {
            watch_count: watch_refs.len(),
            hold_count: hold_refs.len(),
            defend_count: defend_refs.len(),
            opportunity_snapshot_value: risk_opportunity_summary.opportunity_value.clone(),
            risk_snapshot_value: risk_opportunity_summary.risk_value.clone(),
        };
        let decision_summary =
            Self::build_decision_summary(packet, is_data_missing, state, &dict, &battleboard);
        let exit_summary = Self::build_exit_summary(
            packet,
            positions,
            &top_tier_set,
            &core_assets_set,
            is_ready,
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
                &dict,
            );
            if !reason.is_empty() {
                vm.diagnostic = Some(reason);
            } else if let Some(raw_reason) = asset.reasons.first() {
                vm.diagnostic = Some(raw_reason.clone());
            }
            top_vms.push(vm);
        }

        PresentationPacket {
            date_str,
            language: lang,
            macro_display,
            decision_summary,
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
            market_change_log: None,
            hypothesis_layer: Self::build_hypothesis_layer_from_packet(packet, &dict),
            terminal_rows: Vec::new(),
            state_code: format!("{:?}", state),
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
        symbol: &str,
        positions: &HashMap<String, (f64, f64)>,
        current_top_tier: &HashSet<&str>,
        core_assets_list: &HashSet<&str>,
        cohesion_ready: bool,
        is_core_fact: bool,
        has_position_fact: bool,
    ) -> DisplayContext {
        let has_position = has_position_fact || positions.contains_key(symbol);
        let is_top_tier = current_top_tier.contains(symbol);
        let is_core_rules = is_core_fact || core_assets_list.contains(symbol);
        let is_core_holding = has_position && (is_core_rules || is_top_tier);
        let is_candidate_only = !has_position && is_top_tier;

        DisplayContext {
            has_position,
            is_core_holding,
            is_candidate_only,
            is_top_tier,
            cohesion_ready,
        }
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
        dict: &DisplayDictionary,
    ) -> ExitDecisionSummaryViewModel {
        let mut items = Vec::new();
        let is_systemic_collapse = Self::is_systemic_collapse(packet);

        for asset in &packet.assets {
            let context = Self::derive_display_context(
                &asset.symbol,
                positions,
                top_tier_set,
                core_assets_set,
                cohesion_ready,
                asset.is_core_fact,
                asset.has_position_fact,
            );
            let unified_intent = Self::derive_unified_intent(asset, &context);

            if !context.has_position {
                continue;
            }

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

            items.push(ExitDecisionItemViewModel {
                symbol: asset.symbol.clone(),
                intent,
                intent_label,
                reason,
            });
        }

        items.sort_by(|a, b| {
            let prio = |intent: ExitDisplayIntent| match intent {
                ExitDisplayIntent::Exit => 0,
                ExitDisplayIntent::Trim => 1,
                ExitDisplayIntent::Hold => 2,
                ExitDisplayIntent::Watch => 3,
            };
            prio(a.intent)
                .cmp(&prio(b.intent))
                .then_with(|| a.symbol.cmp(&b.symbol))
        });

        ExitDecisionSummaryViewModel {
            title: dict.headers.position_handling.clone(),
            empty_note: if items.is_empty() {
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
        dict: &DisplayDictionary,
    ) -> String {
        if risk_items.is_empty() {
            return dict.decision.no_risk.clone();
        }

        let mut grouped: HashMap<String, (usize, usize, String)> = HashMap::new();
        for (idx, item) in risk_items.iter().enumerate() {
            let entry = grouped.entry(item.reason.clone()).or_insert_with(|| {
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
}
