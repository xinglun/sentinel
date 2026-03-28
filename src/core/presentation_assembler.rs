use crate::config::ParsedRules;
use crate::core::decision::DecisionPacket;
use crate::core::display::{DisplayAdapter, DisplayContext, DisplayIntent};
use crate::core::exit::PositionIntent;
use crate::core::i18n::{get_dictionary, DisplayDictionary, Language};
use crate::core::market_regime::{MarketState, RiskOverlay};
use crate::core::presentation::{
    DataAlertViewModel, MacroDisplayContext, PresentationPacket, SignalSummaryViewModel,
};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

pub struct PresentationAssembler;

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
            let intent = Self::derive_display_intent(asset.position_intent, &context);
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
                    kind: "OPPORTUNITY".to_string(),
                    symbol: asset.symbol.clone(),
                    reason: Self::derive_telegram_reason(asset, !is_ready, &dict),
                });
            }

            if matches!(*intent, DisplayIntent::TRIM | DisplayIntent::EXIT)
                || asset.asset_state.state == crate::core::asset_state::AssetState::OVERHEAT
            {
                risk_opportunities.push(crate::core::display::RiskOpportunityViewModel {
                    kind: "RISK".to_string(),
                    symbol: asset.symbol.clone(),
                    reason: Self::derive_telegram_reason(asset, !is_ready, &dict),
                });
            }
        }

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
            signal_summary,
            top_actions: top_vms,
            tactical_buckets,
            risk_opportunities,
            notices,
            data_alert,
            terminal_rows: Vec::new(),
            state_code: format!("{:?}", state),
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
            AssetState::CRUISE => dict.reasons.state_cruise.clone(),
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
        final_intent: PositionIntent,
        context: &DisplayContext,
    ) -> DisplayIntent {
        DisplayAdapter::derive_display_intent(final_intent, context)
    }
}
