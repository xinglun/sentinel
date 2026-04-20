use crate::config::ParsedRules;
use crate::core::decision::DecisionPacket;
use crate::core::display::{DisplayAdapter, DisplayContext, DisplayIntent};
use crate::core::exit::AssetExitState;
use crate::core::i18n::{get_dictionary, DisplayDictionary, Language};
use crate::core::market_regime::{MarketState, RiskOverlay};
use crate::core::participation::ParticipationReasonCode;
use crate::core::presentation::{
    BreakoutDisplayStatus, BreakoutItemViewModel, BreakoutSummaryViewModel, DataAlertViewModel,
    DecisionSummaryViewModel, ExitDecisionItemViewModel, ExitDecisionSummaryViewModel,
    ExitDisplayIntent, MacroDisplayContext, PresentationPacket, RiskOpportunitySummaryViewModel,
    SignalSummaryViewModel, StateTransitionViewModel, UnmetDiffViewModel,
};
use crate::core::threshold_format::format_threshold_value;
use std::cmp::Ordering;
use std::collections::BTreeSet;
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
    /// Generate a PresentationPacket from a DecisionPacket.
    /// Pure function: No mutation of the input packet.
    /// Optimized: Single-pass enrichment and categorization WITHOUT any cloning of AssetActionDecision.
    pub fn assemble(
        packet: &DecisionPacket,
        rules: &ParsedRules,
        positions: &HashMap<String, (f64, f64)>,
        failed_symbols: Vec<String>,
        lang: Language,
    ) -> PresentationPacket {
        let dict = get_dictionary(lang);
        let top_tier = &packet.top_tier_symbols;
        let is_ready = packet.participation.participation_ready;
        let core_assets = &rules.core_assets;
        let date_str = packet.date.format("%Y-%m-%d").to_string();
        let is_data_missing = packet.assets.is_empty() && !failed_symbols.is_empty();
        let top_tier_set: HashSet<&str> = top_tier.iter().map(String::as_str).collect();
        let core_assets_set: HashSet<&str> = core_assets.iter().map(String::as_str).collect();

        // 1. Assemble Macro Display Context
        let state = packet.market_regime.market_state;
        let risk = packet.market_regime.risk_overlay;

        let (headline, summary, bias) = if is_data_missing {
            (
                dict.market_stages.data_missing.clone(),
                dict.market_summaries.data_missing.clone(),
                dict.market_summaries.bias_neutral.clone(),
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
        let participation_value = if is_data_missing {
            "N/A".to_string()
        } else if packet.participation.participation_ready {
            format!(
                "{} · {} {}d",
                dict.states.ready, dict.signals.continuity, packet.participation.core_tier_streak
            )
        } else {
            format!(
                "{} · {} {}d",
                dict.states.not_ready,
                dict.signals.continuity,
                packet.participation.core_tier_streak
            )
        };
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
            participation_label: dict.signals.participation.clone(),
            participation_value,
            continuity_label: dict.signals.continuity.clone(),
            continuity_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!("{}d", packet.participation.core_tier_streak)
            },
            regime_age_label: dict.signals.regime_age.clone(),
            regime_age_value: if is_data_missing {
                "N/A".to_string()
            } else {
                format!("{}d", packet.market_features.regime_age)
            },
            flow_label: dict.signals.net_flow.clone(),
            flow_value,
        };
        let macro_display = MacroDisplayContext {
            headline,
            summary,
            risk_label,
            bias_label: bias,
        };

        // 2. Integrated categorization using references only.
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

        // 3. Sorting & Top Actions Selection (Still ZERO duplication of Decision objects)
        let sort_fn = |a: &(
            &crate::core::action_matrix::AssetActionDecision,
            DisplayContext,
            DisplayIntent,
        ),
                       b: &(
            &crate::core::action_matrix::AssetActionDecision,
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

        // Selection logic
        let limit = if state == MarketState::DEFENSIVE {
            4
        } else {
            3
        };
        let mut selected_refs = Vec::new();

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
                Some(crate::core::display::TacticalBucketViewModel {
                    bucket_id,
                    display_name,
                    count: refs.len(),
                    items: refs
                        .iter()
                        .map(|(asset, _, _)| asset.symbol.clone())
                        .collect(),
                })
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
                || (context.is_candidate_only && context.participation_ready))
                && matches!(
                    asset.asset_state.state,
                    crate::core::asset_state::AssetState::PULLBACK
                        | crate::core::asset_state::AssetState::OPTIMAL
                )
            {
                risk_opportunities.push(crate::core::display::RiskOpportunityViewModel {
                    kind: dict.decision.opportunity.clone(),
                    symbol: asset.symbol.clone(),
                    reason: Self::derive_telegram_reason(asset, !is_ready, &dict),
                });
            }

            if matches!(*intent, DisplayIntent::TRIM | DisplayIntent::EXIT)
                || asset.asset_state.state == crate::core::asset_state::AssetState::OVERHEAT
            {
                risk_opportunities.push(crate::core::display::RiskOpportunityViewModel {
                    kind: dict.decision.risk.clone(),
                    symbol: asset.symbol.clone(),
                    reason: Self::derive_telegram_reason(asset, !is_ready, &dict),
                });
            }
        }

        let opportunity_value = risk_opportunities
            .iter()
            .find(|item| item.kind == dict.decision.opportunity)
            .map(|item| format!("{} · {}", item.symbol, item.reason))
            .unwrap_or_else(|| dict.decision.no_opportunity.clone());
        let risk_value = risk_opportunities
            .iter()
            .find(|item| item.kind == dict.decision.risk)
            .map(|item| format!("{} · {}", item.symbol, item.reason))
            .unwrap_or_else(|| dict.decision.no_risk.clone());
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
        let decision_summary = Self::build_decision_summary(
            packet,
            rules,
            is_data_missing,
            state,
            &dict,
            &battleboard,
        );
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
                notices.push(dict.reasons.participation_notice.clone());
            }
        }
        // 4. Final ViewModel Conversion
        let mut top_vms = Vec::with_capacity(selected_refs.len());
        for (asset, context, intent) in selected_refs {
            let mut vm = DisplayAdapter::derive_top_action_view_model(asset, &dict);
            // Overwrite with locally calculated intent/context (avoids needing a clone in step 2)
            vm.primary_label = DisplayAdapter::get_label(intent, &dict);
            vm.tags = DisplayAdapter::get_primary_tag(&context, &dict)
                .into_iter()
                .collect();
            vm.indicator = match intent {
                DisplayIntent::ADD => "🟢",
                DisplayIntent::HOLD | DisplayIntent::OBSERVE => "🔵",
                DisplayIntent::TRIM => "🟠",
                DisplayIntent::EXIT => "🔴",
            }
            .to_string();

            let reason = Self::derive_telegram_reason(asset, !is_ready, &dict);
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
            transition_evidence: Self::build_transition_evidence(packet, &dict),
            terminal_rows: Vec::new(),
            state_code: format!("{:?}", state),
            market_state_rendered: packet
                .market_state
                .as_ref()
                .map(|ms| crate::core::market_state::renderer::MarketStateRenderer::render(ms)),
        }
    }

    fn build_transition_evidence(
        packet: &DecisionPacket,
        dict: &DisplayDictionary,
    ) -> Option<StateTransitionViewModel> {
        let log = packet.transition_log.as_ref()?;

        let mut breakout_changes = Vec::new();
        for b in &log.breakout_changes {
            let status_part = if b.status_changed {
                format!(
                    "{} -> {}",
                    Self::map_breakout_status(b.from_status, dict),
                    Self::map_breakout_status(b.to_status, dict)
                )
            } else {
                Self::map_breakout_status(b.to_status, dict)
            };
            let risk_part = if b.risk_changed {
                dict.transition_evidence.risk_changed_suffix.clone()
            } else {
                String::new()
            };
            breakout_changes.push(format!("{}: {}{}", b.symbol, status_part, risk_part));
        }

        Some(StateTransitionViewModel {
            has_significant_change: log.market_state.changed
                || log.risk_overlay.changed
                || log.participation_gate.from != log.participation_gate.to
                || log.participation_gate.unmet_conditions_changed
                || log.trend_cohesion_gate.from != log.trend_cohesion_gate.to
                || log.trend_cohesion_gate.unmet_conditions_changed
                || log.trend_cohesion_status.changed
                || log.trend_cohesion_topology.changed
                || !log.breakout_changes.is_empty(),
            no_trade_persists: log.no_trade_persists,
            market_state_change: if log.market_state.changed {
                Some(format!(
                    "{} -> {}",
                    Self::map_market_state(log.market_state.from, dict),
                    Self::map_market_state(log.market_state.to, dict)
                ))
            } else {
                None
            },
            risk_overlay_change: if log.risk_overlay.changed {
                Some(format!(
                    "{} -> {}",
                    Self::map_risk_overlay(log.risk_overlay.from, dict),
                    Self::map_risk_overlay(log.risk_overlay.to, dict)
                ))
            } else {
                None
            },
            participation_gate_change: if log.participation_gate.from != log.participation_gate.to {
                Some(format!(
                    "{} -> {}",
                    Self::map_gate(log.participation_gate.from, dict),
                    Self::map_gate(log.participation_gate.to, dict)
                ))
            } else {
                None
            },
            participation_gate_passed: log.participation_gate.to,
            trend_cohesion_gate_change: if log.trend_cohesion_gate.from
                != log.trend_cohesion_gate.to
            {
                Some(format!(
                    "{} -> {}",
                    Self::map_gate(log.trend_cohesion_gate.from, dict),
                    Self::map_gate(log.trend_cohesion_gate.to, dict)
                ))
            } else {
                None
            },
            trend_cohesion_gate_passed: log.trend_cohesion_gate.to,
            trend_cohesion_status_change: if log.trend_cohesion_status.changed {
                Some(format!(
                    "{} -> {}",
                    Self::map_trend_status(log.trend_cohesion_status.from, dict),
                    Self::map_trend_status(log.trend_cohesion_status.to, dict)
                ))
            } else {
                None
            },
            trend_cohesion_topology_change: if log.trend_cohesion_topology.changed {
                Some(format!(
                    "{} -> {}",
                    Self::map_topology(log.trend_cohesion_topology.from, dict),
                    Self::map_topology(log.trend_cohesion_topology.to, dict)
                ))
            } else {
                None
            },
            participation_unmet_diff: Self::map_unmet_diff(
                &log.participation_gate,
                false,
                packet,
                dict,
            ),
            trend_unmet_diff: Self::map_unmet_diff(&log.trend_cohesion_gate, true, packet, dict),
            breakout_changes,
        })
    }

    fn map_unmet_diff(
        diff: &crate::core::transition_log::GateTransition,
        is_trend: bool,
        packet: &DecisionPacket,
        dict: &DisplayDictionary,
    ) -> Option<UnmetDiffViewModel> {
        if diff.added.is_empty() && diff.removed.is_empty() && diff.persisting.is_empty() {
            return None;
        }

        let map_fn = |s: &str| {
            if is_trend {
                match s {
                    "StabilityThreshold" => dict.trend_cohesion.unmet.stability_threshold.replace(
                        "{:.1}",
                        &format!("{:.1}", packet.trend_cohesion.stability_score),
                    ),
                    "ContinuityThreshold" => dict
                        .trend_cohesion
                        .unmet
                        .continuity_threshold
                        .replace("{}", &packet.trend_cohesion.continuity_streak.to_string()),
                    "DirectionalCohesion" => dict.trend_cohesion.unmet.directional_cohesion.clone(),
                    "HighCandidateDispersion" => dict
                        .trend_cohesion
                        .unmet
                        .high_candidate_dispersion
                        .replace("{}", &packet.trend_cohesion.candidate_count.to_string()),
                    "UnstableRotation" => dict.trend_cohesion.unmet.unstable_rotation.replace(
                        "{:.0}",
                        &format!("{:.0}", packet.trend_cohesion.rotation_quality_score),
                    ),
                    "WeakLeadership" => dict
                        .trend_cohesion
                        .unmet
                        .weak_leadership
                        .replace("{}", &packet.trend_cohesion.leader_count.to_string()),
                    _ => s.to_string(),
                }
            } else {
                s.to_string()
            }
        };

        Some(UnmetDiffViewModel {
            added: diff.added.iter().map(|s| map_fn(s)).collect(),
            removed: diff.removed.iter().map(|s| map_fn(s)).collect(),
            persisting: diff.persisting.iter().map(|s| map_fn(s)).collect(),
        })
    }

    fn map_market_state(state: MarketState, dict: &DisplayDictionary) -> String {
        match state {
            MarketState::ESTABLISHED | MarketState::CONFIRMED => {
                dict.market_stages.established.clone()
            }
            MarketState::DEFENSIVE => dict.market_stages.defensive.clone(),
            MarketState::IGNITION | MarketState::NEWBORN => dict.market_stages.ignition.clone(),
            _ => dict.market_stages.neutral.clone(),
        }
    }

    fn map_risk_overlay(risk: RiskOverlay, dict: &DisplayDictionary) -> String {
        match risk {
            RiskOverlay::NORMAL => dict.risks.normal.clone(),
            RiskOverlay::DECELERATING => dict.risks.mixed.clone(),
            _ => dict.risks.defensive.clone(),
        }
    }

    fn map_breakout_status(
        status: crate::core::breakout_detection::BreakoutStatus,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::core::breakout_detection::BreakoutStatus;
        match status {
            BreakoutStatus::NoBreakout => dict.breakout.no_breakout.clone(),
            BreakoutStatus::EmergingBreakout => dict.breakout.emerging_breakout.clone(),
            BreakoutStatus::ConfirmedBreakout => dict.breakout.confirmed_breakout.clone(),
        }
    }

    fn map_trend_status(
        status: crate::core::trend_cohesion::TrendCohesionStatus,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::core::trend_cohesion::TrendCohesionStatus;
        match status {
            TrendCohesionStatus::Formed => dict.trend_cohesion.formed.clone(),
            TrendCohesionStatus::Forming => dict.trend_cohesion.forming.clone(),
            TrendCohesionStatus::Dispersed => dict.trend_cohesion.dispersed.clone(),
        }
    }

    fn map_topology(
        topology: crate::core::trend_cohesion::TrendCohesionTopology,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::core::trend_cohesion::TrendCohesionTopology;
        match topology {
            TrendCohesionTopology::NoLeader => dict.trend_cohesion.topology_no_leader.clone(),
            TrendCohesionTopology::SingleLeader => {
                dict.trend_cohesion.topology_single_leader.clone()
            }
            TrendCohesionTopology::FragmentedLeaders => {
                dict.trend_cohesion.topology_fragmented_leaders.clone()
            }
        }
    }

    fn map_gate(passed: bool, dict: &DisplayDictionary) -> String {
        if passed {
            dict.transition_evidence.gate_pass.clone()
        } else {
            dict.transition_evidence.gate_fail.clone()
        }
    }

    fn derive_telegram_reason(
        asset: &crate::core::action_matrix::AssetActionDecision,
        is_restrained: bool,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::core::asset_state::AssetState;
        use crate::core::exit::AssetExitState;
        if asset.exit_decision.asset_exit_state != AssetExitState::None {
            return match asset.exit_decision.asset_exit_state {
                AssetExitState::DefensiveExit => dict.reasons.exit_defensive.clone(),
                AssetExitState::StrengthLoss => dict.reasons.exit_strength_loss.clone(),
                AssetExitState::ParticipationExit => dict.reasons.exit_participation.clone(),
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
        participation_ready: bool,
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
            participation_ready,
        }
    }

    fn derive_display_intent(
        final_intent: crate::core::position_intent::UnifiedPositionIntent,
        context: &DisplayContext,
    ) -> DisplayIntent {
        DisplayAdapter::derive_display_intent(final_intent, context)
    }

    fn derive_unified_intent(
        asset: &crate::core::action_matrix::AssetActionDecision,
        context: &DisplayContext,
    ) -> crate::core::position_intent::UnifiedPositionIntent {
        crate::core::position_intent::UnifiedIntentSynthesizer::synthesize(
            asset.position_intent,
            &asset.exit_decision,
            context.participation_ready,
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
        participation_ready: bool,
        dict: &DisplayDictionary,
    ) -> ExitDecisionSummaryViewModel {
        let mut items = Vec::new();

        for asset in &packet.assets {
            let context = Self::derive_display_context(
                &asset.symbol,
                positions,
                top_tier_set,
                core_assets_set,
                participation_ready,
                asset.is_core_fact,
                asset.has_position_fact,
            );
            let unified_intent = Self::derive_unified_intent(asset, &context);

            if !context.has_position {
                continue;
            }

            let (intent, intent_label, reason) = match unified_intent {
                crate::core::position_intent::UnifiedPositionIntent::Exit => (
                    ExitDisplayIntent::Exit,
                    dict.decision.exit_intent_exit.clone(),
                    dict.reasons.position_exit_defensive.clone(),
                ),
                crate::core::position_intent::UnifiedPositionIntent::Trim => {
                    let reason = match asset.exit_decision.asset_exit_state {
                        AssetExitState::StrengthLoss => {
                            dict.reasons.position_trim_strength_loss.clone()
                        }
                        AssetExitState::ParticipationExit => {
                            dict.reasons.position_trim_participation.clone()
                        }
                        AssetExitState::OverheatProfitTake => {
                            dict.reasons.position_trim_overheat.clone()
                        }
                        AssetExitState::DefensiveExit => {
                            dict.reasons.position_exit_defensive.clone()
                        }
                        AssetExitState::None => dict.reasons.position_trim_strength_loss.clone(),
                    };
                    (
                        ExitDisplayIntent::Trim,
                        dict.decision.exit_intent_trim.clone(),
                        reason,
                    )
                }
                crate::core::position_intent::UnifiedPositionIntent::Hold => (
                    ExitDisplayIntent::Hold,
                    dict.decision.exit_intent_hold.clone(),
                    dict.reasons.position_hold_core.clone(),
                ),
                crate::core::position_intent::UnifiedPositionIntent::Watch
                | crate::core::position_intent::UnifiedPositionIntent::Add => {
                    if matches!(
                        asset.asset_state.state,
                        crate::core::asset_state::AssetState::PULLBACK
                            | crate::core::asset_state::AssetState::CAUTION
                            | crate::core::asset_state::AssetState::FORMING
                    ) {
                        (
                            ExitDisplayIntent::Watch,
                            dict.decision.exit_intent_watch.clone(),
                            if asset.asset_state.state
                                == crate::core::asset_state::AssetState::PULLBACK
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
                Some(format!(
                    "{}\n{}",
                    dict.reasons.position_none, dict.reasons.position_none_no_trigger
                ))
            } else {
                None
            },
            items,
        }
    }

    fn build_breakout_summary(
        packet: &DecisionPacket,
        rules: &crate::config::ParsedRules,
        dict: &DisplayDictionary,
        lang: Language,
    ) -> BreakoutSummaryViewModel {
        use crate::core::breakout_detection::{BreakoutReason, BreakoutStatus};
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
        status: crate::core::breakout_detection::BreakoutStatus,
        breakout_age: usize,
        lang: Language,
    ) -> String {
        use crate::core::breakout_detection::BreakoutStatus;
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
        rules: &ParsedRules,
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
                    dict.decision.state_participation_blocked.clone()
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

        let readiness_reasons = if is_data_missing {
            vec![dict.market_summaries.data_missing.clone()]
        } else if not_ready {
            let reason_entries: Vec<(ParticipationReasonCode, String)> =
                if packet.participation.reason_codes.is_empty() {
                    Self::infer_legacy_reason_entries(packet)
                } else {
                    let mut entries: Vec<(ParticipationReasonCode, String)> = Vec::new();
                    let code_set: BTreeSet<ParticipationReasonCode> =
                        packet.participation.reason_codes.iter().copied().collect();

                    // Preserve raw reasons as unknown evidence and avoid index binding.
                    // Suppress only known machine-template lines when equivalent
                    // structured threshold evidence already exists.
                    for reason in &packet.participation.reasons {
                        if Self::should_suppress_raw_reason_with_codes(reason, &code_set) {
                            continue;
                        }
                        entries.push((ParticipationReasonCode::Unknown, reason.clone()));
                    }

                    // Keep structured reasons from codes (deduplicated, stable order).
                    let mut seen_codes = HashSet::new();
                    for code in &packet.participation.reason_codes {
                        if seen_codes.insert(*code) {
                            entries.push((*code, String::new()));
                        }
                    }

                    entries
                };

            reason_entries
                .into_iter()
                .filter(|(code, _)| !Self::is_redundant_readiness_reason(*code, packet))
                .map(|(code, raw)| {
                    Self::localize_participation_reason(code, raw.as_str(), packet, rules, dict)
                })
                .filter(|reason| !reason.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        let trend_cohesion_value = match packet.trend_cohesion.status {
            crate::core::trend_cohesion::TrendCohesionStatus::Dispersed => {
                dict.trend_cohesion.dispersed.clone()
            }
            crate::core::trend_cohesion::TrendCohesionStatus::Forming => {
                dict.trend_cohesion.forming.clone()
            }
            crate::core::trend_cohesion::TrendCohesionStatus::Formed => {
                dict.trend_cohesion.formed.clone()
            }
        };

        let trend_topology_value = match packet.trend_cohesion.topology {
            crate::core::trend_cohesion::TrendCohesionTopology::NoLeader => {
                dict.trend_cohesion.topology_no_leader.clone()
            }
            crate::core::trend_cohesion::TrendCohesionTopology::SingleLeader => {
                dict.trend_cohesion.topology_single_leader.clone()
            }
            crate::core::trend_cohesion::TrendCohesionTopology::FragmentedLeaders => {
                dict.trend_cohesion.topology_fragmented_leaders.clone()
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
                crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold => {
                    dict.trend_cohesion.unmet.stability_threshold.replace(
                        "{:.1}",
                        &format!("{:.1}", packet.trend_cohesion.stability_score),
                    )
                }
                crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold => {
                    dict.trend_cohesion
                        .unmet
                        .continuity_threshold
                        .replace("{}", &packet.trend_cohesion.continuity_streak.to_string())
                }
                crate::core::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion => {
                    dict.trend_cohesion.unmet.directional_cohesion.clone()
                }
                crate::core::trend_cohesion::TrendCohesionGateCondition::HighCandidateDispersion => {
                    dict.trend_cohesion
                        .unmet
                        .high_candidate_dispersion
                        .replace("{}", &packet.trend_cohesion.candidate_count.to_string())
                }
                crate::core::trend_cohesion::TrendCohesionGateCondition::UnstableRotation => {
                    dict.trend_cohesion.unmet.unstable_rotation.replace(
                        "{:.0}",
                        &format!("{:.0}", packet.trend_cohesion.rotation_quality_score),
                    )
                }
                crate::core::trend_cohesion::TrendCohesionGateCondition::WeakLeadership => dict
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
                Self::canonical_continuity_streak(packet).to_string()
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

    fn localize_participation_reason(
        reason_code: ParticipationReasonCode,
        raw_reason: &str,
        packet: &DecisionPacket,
        rules: &ParsedRules,
        dict: &DisplayDictionary,
    ) -> String {
        match reason_code {
            ParticipationReasonCode::StabilityBelowThreshold => {
                let stability_now = format!("{:.1}", packet.trend_cohesion.stability_score);
                let stability_threshold =
                    format_threshold_value(rules.trend_cohesion.gate_stability_threshold);
                format!(
                    "{} {} < {}",
                    dict.signals.stability, stability_now, stability_threshold
                )
            }
            ParticipationReasonCode::CoreTierStreakBelowThreshold => {
                let streak = Self::canonical_continuity_streak(packet);
                format!(
                    "{} {}d < {}d",
                    dict.signals.continuity, streak, rules.trend_cohesion.gate_continuity_threshold
                )
            }
            ParticipationReasonCode::CoreTierSetChanged
            | ParticipationReasonCode::FirstDayOfSession => String::new(),
            ParticipationReasonCode::Unknown => raw_reason.to_string(),
        }
    }

    fn infer_legacy_reason_entries(
        packet: &DecisionPacket,
    ) -> Vec<(ParticipationReasonCode, String)> {
        let mut inferred = Vec::new();
        let has_stability_evidence = packet
            .trend_cohesion
            .unmet_conditions
            .contains(&crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold);
        let has_continuity_evidence = packet.trend_cohesion.unmet_conditions.contains(
            &crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
        );

        // Preserve legacy raw evidence by default.
        // Suppress only threshold templates that are explicitly covered by gate evidence.
        // This prevents one-sided evidence from accidentally suppressing the other reason.
        for reason in &packet.participation.reasons {
            if Self::should_suppress_raw_reason_with_gate_evidence(
                reason,
                has_stability_evidence,
                has_continuity_evidence,
            ) {
                continue;
            }
            inferred.push((ParticipationReasonCode::Unknown, reason.clone()));
        }

        // Add structured reasons only when we have explicit gate evidence.
        // Avoid relying on legacy boolean fields, which may default to false in old payloads.
        if has_stability_evidence {
            inferred.push((
                ParticipationReasonCode::StabilityBelowThreshold,
                String::new(),
            ));
        }
        if has_continuity_evidence {
            inferred.push((
                ParticipationReasonCode::CoreTierStreakBelowThreshold,
                String::new(),
            ));
        }

        inferred
    }

    fn should_suppress_raw_reason_with_codes(
        reason: &str,
        code_set: &BTreeSet<ParticipationReasonCode>,
    ) -> bool {
        (code_set.contains(&ParticipationReasonCode::StabilityBelowThreshold)
            && Self::is_legacy_stability_machine_reason(reason))
            || (code_set.contains(&ParticipationReasonCode::CoreTierStreakBelowThreshold)
                && Self::is_legacy_continuity_machine_reason(reason))
    }

    fn should_suppress_raw_reason_with_gate_evidence(
        reason: &str,
        has_stability_evidence: bool,
        has_continuity_evidence: bool,
    ) -> bool {
        (has_stability_evidence && Self::is_legacy_stability_machine_reason(reason))
            || (has_continuity_evidence && Self::is_legacy_continuity_machine_reason(reason))
    }

    fn is_legacy_stability_machine_reason(reason: &str) -> bool {
        Self::matches_legacy_english_threshold_machine_reason(reason, "Stability score (")
            || Self::matches_legacy_localized_threshold_machine_reason(reason, "稳定性不足")
            || Self::matches_legacy_localized_threshold_machine_reason(reason, "安定性不足")
    }

    fn is_legacy_continuity_machine_reason(reason: &str) -> bool {
        Self::matches_legacy_english_threshold_machine_reason(reason, "Core Tier streak (")
            || Self::matches_legacy_localized_threshold_machine_reason(reason, "连续性不足")
            || Self::matches_legacy_localized_threshold_machine_reason(reason, "連続性不足")
    }

    fn matches_legacy_english_threshold_machine_reason(reason: &str, prefix: &str) -> bool {
        let normalized: String = reason.chars().filter(|ch| !ch.is_whitespace()).collect();
        let normalized_lower = normalized.to_ascii_lowercase();
        let normalized_lower = Self::trim_trailing_sentence_punctuation(&normalized_lower);
        let prefix_lower = prefix.to_ascii_lowercase().replace(' ', "");
        if !normalized_lower.starts_with(&prefix_lower) {
            return false;
        }
        if !normalized_lower.ends_with(')') {
            return false;
        }
        let close_pos = normalized_lower.find(')').unwrap_or(0);
        if close_pos + 1 >= normalized_lower.len() {
            return false;
        }
        let current_payload = &normalized_lower[prefix_lower.len()..close_pos];
        if !Self::contains_digit(current_payload) {
            return false;
        }
        let after_current = &normalized_lower[close_pos + 1..];
        let after_current = after_current.strip_prefix(':').unwrap_or(after_current);
        let after_current = after_current.strip_prefix("is").unwrap_or(after_current);
        let Some(token_segment) = Self::strip_leading_comparator_segment(after_current) else {
            return false;
        };
        let token_segment = token_segment.strip_prefix(':').unwrap_or(token_segment);
        let has_threshold_token = Self::starts_with_ascii_word_token(token_segment, "threshold")
            || Self::starts_with_ascii_word_token(token_segment, "thresh")
            || Self::starts_with_ascii_word_token(token_segment, "thr")
            || Self::starts_with_ascii_word_token(token_segment, "limit");
        if !has_threshold_token {
            return false;
        }
        let Some((_, right_after_open)) = token_segment.split_once('(') else {
            return false;
        };
        let Some((threshold_payload, _)) = right_after_open.split_once(')') else {
            return false;
        };
        Self::contains_digit(threshold_payload)
    }

    fn matches_legacy_localized_threshold_machine_reason(reason: &str, prefix: &str) -> bool {
        let normalized: String = reason.chars().filter(|ch| !ch.is_whitespace()).collect();
        let normalized = Self::trim_trailing_sentence_punctuation(&normalized);
        let starts_with_prefix = normalized.starts_with(&(prefix.to_string() + "（"))
            || normalized.starts_with(&(prefix.to_string() + "("));
        if !starts_with_prefix || !Self::contains_threshold_operator(normalized) {
            return false;
        }
        // Require closing bracket to avoid suppressing free-form notes that merely start
        // with the same prefix and mention threshold symbols later in text.
        if !(normalized.ends_with('）') || normalized.ends_with(')')) {
            return false;
        }

        // Restrict to threshold-like payloads inside brackets.
        // Examples: "<10.0", "1d<3d", "1日≤3日".
        let after_open = normalized
            .split_once('（')
            .map(|(_, right)| right)
            .or_else(|| normalized.split_once('(').map(|(_, right)| right));
        let Some(after_open) = after_open else {
            return false;
        };
        let payload = after_open
            .rsplit_once('）')
            .map(|(left, _)| left)
            .or_else(|| after_open.rsplit_once(')').map(|(left, _)| left));
        let Some(payload) = payload else {
            return false;
        };
        if !Self::is_structured_threshold_payload(payload) {
            return false;
        }
        true
    }

    fn contains_threshold_operator(normalized_reason: &str) -> bool {
        normalized_reason.contains('<')
            || normalized_reason.contains('＜')
            || normalized_reason.contains('≤')
            || normalized_reason.contains('≦')
    }

    fn contains_digit(text: &str) -> bool {
        text.chars()
            .any(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
    }

    fn starts_with_ascii_word_token(text: &str, token: &str) -> bool {
        if !text.starts_with(token) {
            return false;
        }
        let next = text[token.len()..].chars().next();
        !next.is_some_and(|c| c.is_alphanumeric() || c == '_')
    }

    fn strip_leading_comparator_segment(text: &str) -> Option<&str> {
        if let Some(rest) = text.strip_prefix("below") {
            return Some(rest);
        }
        if let Some(rest) = text.strip_prefix("under") {
            return Some(rest);
        }
        if let Some(rest) = text.strip_prefix("lessthan") {
            return Some(rest);
        }
        let mut iter = text.char_indices();
        let (_, first) = iter.next()?;
        if Self::is_direct_less_or_equal_symbol(first) {
            let consumed = first.len_utf8();
            return Some(&text[consumed..]);
        }
        if !Self::is_strict_less_symbol(first) {
            return None;
        }
        if let Some((idx, second)) = iter.next() {
            if Self::is_equal_symbol(second) {
                return Some(&text[idx + second.len_utf8()..]);
            }
        }
        Some(&text[first.len_utf8()..])
    }

    fn trim_trailing_sentence_punctuation(input: &str) -> &str {
        input.trim_end_matches(Self::is_sentence_terminal_punctuation)
    }

    fn is_sentence_terminal_punctuation(ch: char) -> bool {
        matches!(
            ch,
            '.' | '!' | '?' | '。' | '！' | '？' | '｡' | '．' | '﹒' | '﹗' | '﹖'
        )
    }

    fn is_strict_less_symbol(ch: char) -> bool {
        matches!(ch, '<' | '＜' | '﹤')
    }

    fn is_direct_less_or_equal_symbol(ch: char) -> bool {
        matches!(ch, '≤' | '≦' | '⩽')
    }

    fn is_equal_symbol(ch: char) -> bool {
        matches!(ch, '=' | '＝' | '﹦')
    }

    fn canonical_continuity_streak(packet: &DecisionPacket) -> usize {
        // Prefer trend gate streak to keep ratio displays and reason text on one source of truth.
        // Fall back to participation streak for partially populated legacy payloads.
        let trend = packet.trend_cohesion.continuity_streak;
        if trend > 0 {
            trend
        } else {
            packet.participation.core_tier_streak
        }
    }

    fn is_structured_threshold_payload(payload: &str) -> bool {
        if !Self::contains_threshold_operator(payload) || !Self::contains_digit(payload) {
            return false;
        }
        // Keep matcher conservative: allow only concise machine-template tokens
        // so custom notes with natural language are preserved.
        payload.chars().all(|ch| {
            ch.is_ascii_digit()
                || ('０'..='９').contains(&ch)
                || matches!(
                    ch,
                    '<' | '＜' | '≤' | '≦' | '.' | '．' | '+' | '-' | 'd' | 'D' | '日' | '天'
                )
        })
    }

    fn localize_breakout_reason(
        reasons: &[crate::core::breakout_detection::BreakoutReason],
        dict: &DisplayDictionary,
    ) -> String {
        use crate::core::breakout_detection::BreakoutReason;

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

    fn is_redundant_readiness_reason(
        reason_code: ParticipationReasonCode,
        packet: &DecisionPacket,
    ) -> bool {
        let unmet_conditions = &packet.trend_cohesion.unmet_conditions;

        (reason_code == ParticipationReasonCode::StabilityBelowThreshold
            && unmet_conditions.contains(
                &crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
            ))
            || (reason_code == ParticipationReasonCode::CoreTierStreakBelowThreshold
                && unmet_conditions.contains(
                    &crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ))
    }
}
