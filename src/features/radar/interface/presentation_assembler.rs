use crate::config::{
    CreditStress, GrowthValuationImpact, LiquidityCondition, MacroGravityConfig, MacroPressure,
    ParsedRules, YieldCurveState,
};
use crate::features::radar::domain::asset_state::AssetState;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::exit::AssetExitState;
use crate::features::radar::domain::market_regime::{MarketState, RiskOverlay};
use crate::features::radar::domain::trend_cohesion::{EvidenceSourceType, EvidenceType};
use crate::features::radar::interface::display::{DisplayAdapter, DisplayContext, DisplayIntent};
use crate::features::radar::interface::presentation::{
    BreakoutDisplayStatus, BreakoutItemViewModel, BreakoutSummaryViewModel, DataAlertViewModel,
    DecisionSummaryViewModel, ExitDecisionItemViewModel, ExitDecisionSummaryViewModel,
    ExitDisplayIntent, HoldingEfficiency, HypothesisBeneficiaryViewModel,
    HypothesisCandidateViewModel, HypothesisConfidence, HypothesisEvidenceNodeViewModel,
    HypothesisFailureRiskViewModel, HypothesisLayerViewModel, MacroDisplayContext,
    MarketCyclePosition, PresentationPacket, RiskOpportunitySummaryViewModel,
    SignalSummaryViewModel, StateTransitionViewModel, TrendBreadthMode, UnmetDiffViewModel,
};
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
        let trend_breadth_mode = Self::classify_trend_breadth_mode(packet);

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
            transition_evidence: Self::build_transition_evidence(packet, rules, &dict),
            hypothesis_layer: Self::build_hypothesis_layer_from_packet(packet, &dict),
            terminal_rows: Vec::new(),
            state_code: format!("{:?}", state),
        }
    }

    fn build_transition_evidence(
        packet: &DecisionPacket,
        rules: &ParsedRules,
        dict: &DisplayDictionary,
    ) -> Option<StateTransitionViewModel> {
        let log = packet.transition_log.as_ref()?;

        let mut breakout_changes = Vec::new();
        let mut structural_breakout_symbols = HashSet::<String>::new();
        for b in &log.breakout_changes {
            if !b.status_changed {
                continue;
            }
            structural_breakout_symbols.insert(b.symbol.clone());
            breakout_changes.push(Self::format_structural_breakout_change(b, dict));
        }

        if !breakout_changes.is_empty() && packet.assets.len() > structural_breakout_symbols.len() {
            breakout_changes.push(
                dict.transition_evidence
                    .breakout_others_no_structural_change
                    .clone(),
            );
        }
        let has_structural_breakout_change = !breakout_changes.is_empty();
        let scout_continuity = if !log.trend_cohesion_gate.to {
            Some(if log.breakout_active_count >= 2 {
                dict.transition_evidence
                    .scout_continuity_multi_point
                    .clone()
            } else {
                format!(
                    "{}/{}",
                    log.scout_days_without_expansion,
                    log.scout_abort_days.max(1)
                )
            })
        } else {
            None
        };
        let scout_expansion = if !log.trend_cohesion_gate.to {
            Some(
                match log.breakout_active_count {
                    0 => dict.transition_evidence.scout_expansion_none.as_str(),
                    1 => dict.transition_evidence.scout_expansion_single.as_str(),
                    _ => dict.transition_evidence.scout_expansion_multi.as_str(),
                }
                .to_string(),
            )
        } else {
            None
        };
        let scout_reset = if !log.trend_cohesion_gate.to {
            Some(
                if log.scout_reset_triggered {
                    dict.transition_evidence.yes.as_str()
                } else {
                    dict.transition_evidence.no.as_str()
                }
                .to_string(),
            )
        } else {
            None
        };

        let trend_recognition_state = log.trend_recognition.as_ref().map(|tr| match tr.state {
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::None => dict.trend_recognition.state_none.clone(),
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::StructuralPersistence => dict.trend_recognition.state_structural_persistence.clone(),
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::EarlyLeader => dict.trend_recognition.state_early_leader.clone(),
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::LeaderConfirmedFollowersLagging => dict.trend_recognition.state_leader_confirmed_followers_lagging.clone(),
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::Broadening => dict.trend_recognition.state_broadening.clone(),
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::Mature => dict.trend_recognition.state_mature.clone(),
        });
        let trend_recognition_diffusion_score =
            log.trend_recognition.as_ref().map(|tr| tr.diffusion_score);
        let trend_recognition_conviction_score =
            log.trend_recognition.as_ref().map(|tr| tr.conviction_score);
        let trend_recognition_lag_state = log.trend_recognition.as_ref().and_then(|tr| {
            if tr.lag_state {
                Some(dict.trend_recognition.lag_alert.clone())
            } else {
                None
            }
        });
        let trend_recognition_single_asset_decay =
            log.trend_recognition
                .as_ref()
                .and_then(|tr| match tr.state {
                    crate::features::radar::domain::trend_cohesion::TrendContinuationState::Broadening
                    | crate::features::radar::domain::trend_cohesion::TrendContinuationState::Mature => None,
                    _ => Some(format!(
                        "{}/{}",
                        tr.single_asset_decay_day,
                        tr.single_asset_decay_max.max(1)
                    )),
                });
        let mut substantive_signals = Vec::new();
        let mut substantive_details = Vec::new();
        let mut price_confirmation_record_count = 0;
        let mut evidence_quality_summary = None;
        if let Some(sub) = log
            .trend_recognition
            .as_ref()
            .and_then(|tr| tr.substantive.as_ref())
        {
            let has_capex_payoff = sub.capex_payoff_signal
                || sub
                    .records
                    .iter()
                    .any(|record| record.evidence_type == EvidenceType::CapexPayoff);
            let has_earnings_validation = sub.earnings_validation
                || sub
                    .records
                    .iter()
                    .any(|record| record.evidence_type == EvidenceType::EarningsValidation);
            let has_order_visibility = sub.order_visibility
                || sub
                    .records
                    .iter()
                    .any(|record| record.evidence_type == EvidenceType::OrderVisibility);

            if has_capex_payoff {
                substantive_signals.push(dict.trend_recognition.capex_payoff.clone());
            }
            if has_earnings_validation {
                substantive_signals.push(dict.trend_recognition.earnings_validation.clone());
            }
            if has_order_visibility {
                substantive_signals.push(dict.trend_recognition.order_visibility.clone());
            }
            price_confirmation_record_count = sub
                .records
                .iter()
                .filter(|record| record.evidence_type == EvidenceType::FollowThrough)
                .count();
            evidence_quality_summary = Self::build_evidence_quality_summary(sub, dict);

            for record in &sub.records {
                let source_label = match record.source {
                    crate::features::radar::domain::trend_cohesion::EvidenceSourceType::Manual => {
                        &dict.trend_recognition.source_manual
                    }
                    crate::features::radar::domain::trend_cohesion::EvidenceSourceType::OfficialIR => {
                        &dict.trend_recognition.source_official_ir
                    }
                    crate::features::radar::domain::trend_cohesion::EvidenceSourceType::NewsMedia => {
                        &dict.trend_recognition.source_news_media
                    }
                    crate::features::radar::domain::trend_cohesion::EvidenceSourceType::PriceAction => {
                        &dict.trend_recognition.source_price_action
                    }
                };
                let symbol_part = record
                    .symbol
                    .as_ref()
                    .map(|s| format!("[{}] ", s))
                    .unwrap_or_default();
                let url_part = record
                    .source_url
                    .as_ref()
                    .map(|u| format!(" ({})", u))
                    .unwrap_or_default();
                substantive_details.push(format!(
                    "{} {}[{}] [{:?}] {} (Conf: {:.1}){}",
                    source_label,
                    symbol_part,
                    record.event_date,
                    record.evidence_type,
                    record.description,
                    record.confidence,
                    url_part
                ));
            }
        }
        let trend_breadth_mode = Self::classify_trend_breadth_mode(packet);
        let market_cycle_position = Self::classify_market_cycle_position(
            packet,
            trend_breadth_mode,
            substantive_signals.len(),
            trend_recognition_conviction_score,
        );
        let holding_efficiency = Self::classify_holding_efficiency(
            packet,
            market_cycle_position,
            substantive_signals.len(),
        );
        let risk_taxonomy =
            Self::build_risk_taxonomy(packet, log, market_cycle_position, holding_efficiency, dict);
        let structural_strength = Self::build_structural_strength(
            substantive_signals.len(),
            price_confirmation_record_count,
            trend_recognition_conviction_score,
            dict,
        );
        let strategic_context = Self::build_strategic_context(
            &substantive_signals,
            trend_recognition_conviction_score,
            log.trend_cohesion_gate.to,
            trend_breadth_mode,
            market_cycle_position,
            rules.macro_gravity.as_ref(),
            dict,
        );
        Some(StateTransitionViewModel {
            has_significant_change: log.market_state.changed
                || log.risk_overlay.changed
                || log.trend_cohesion_gate.from != log.trend_cohesion_gate.to
                || log.trend_cohesion_gate.unmet_conditions_changed
                || log.trend_cohesion_status.changed
                || log.trend_cohesion_topology.changed
                || has_structural_breakout_change
                || !substantive_signals.is_empty()
                || structural_strength.is_some()
                || !strategic_context.is_empty(),
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
            trend_unmet_diff: Self::map_unmet_diff(&log.trend_cohesion_gate, packet, dict),
            breakout_changes,
            risk_taxonomy,
            scout_continuity,
            scout_expansion,
            scout_reset,
            trend_recognition_state,
            trend_recognition_diffusion_score,
            trend_recognition_conviction_score,
            trend_recognition_lag_state,
            trend_recognition_single_asset_decay,
            structural_strength,
            evidence_quality_summary,
            substantive_signals,
            substantive_details,
            strategic_context,
            trend_breadth_mode,
            market_cycle_position,
            holding_efficiency,
        })
    }

    fn classify_trend_breadth_mode(packet: &DecisionPacket) -> TrendBreadthMode {
        if packet
            .market_regime
            .transition_audit
            .as_ref()
            .is_some_and(|audit| audit.core_breakdown)
            || packet.market_regime.risk_overlay == RiskOverlay::BROKEN
        {
            return TrendBreadthMode::StructuralDefense;
        }

        let index_trend_positive = packet.assets.iter().any(|asset| {
            asset.symbol == "SPY"
                && matches!(
                    asset.asset_state.state,
                    AssetState::OPTIMAL
                        | AssetState::CRUISE
                        | AssetState::PULLBACK
                        | AssetState::OVERHEAT
                )
        });
        let leadership_symbols = ["MSFT", "GOOG", "NVDA"];
        let leadership_count = packet
            .assets
            .iter()
            .filter(|asset| {
                leadership_symbols.contains(&asset.symbol.as_str())
                    && matches!(
                        asset.asset_state.state,
                        AssetState::OPTIMAL
                            | AssetState::CRUISE
                            | AssetState::PULLBACK
                            | AssetState::OVERHEAT
                    )
            })
            .count();
        let up_ratio = if packet.market_features.total_count > 0 {
            packet.market_features.up_count as f64 / packet.market_features.total_count as f64
        } else {
            0.0
        };
        let substantive_signal_count = packet
            .trend_recognition
            .as_ref()
            .and_then(|tr| tr.substantive.as_ref())
            .map(Self::substantive_signal_count)
            .unwrap_or(0);
        let conviction_score = packet
            .trend_recognition
            .as_ref()
            .map(|tr| tr.conviction_score)
            .unwrap_or(0.0);

        if up_ratio >= 0.60 && packet.market_features.up_count >= 4 {
            return TrendBreadthMode::BroadExpansion;
        }

        if index_trend_positive
            && leadership_count >= 2
            && substantive_signal_count >= 2
            && conviction_score >= 3.0
        {
            return TrendBreadthMode::NarrowLeadership;
        }

        TrendBreadthMode::FragileRotation
    }

    fn has_persistent_main_theme(packet: &DecisionPacket) -> bool {
        let Some(trend_recognition) = packet.trend_recognition.as_ref() else {
            return false;
        };
        let Some(substantive) = trend_recognition.substantive.as_ref() else {
            return false;
        };

        let trend_breadth_mode = Self::classify_trend_breadth_mode(packet);
        matches!(
            trend_breadth_mode,
            TrendBreadthMode::BroadExpansion | TrendBreadthMode::NarrowLeadership
        ) && Self::substantive_signal_count(substantive) >= 3
            && trend_recognition.conviction_score >= 3.0
    }

    fn map_continuity_state(streak: usize, dict: &DisplayDictionary) -> String {
        match streak {
            0 => dict.signals.continuity_none.clone(),
            1 => dict.signals.continuity_emerging.clone(),
            2 => dict.signals.continuity_building.clone(),
            _ => dict.signals.continuity_sustained.clone(),
        }
    }

    fn substantive_signal_count(
        sub: &crate::features::radar::domain::trend_cohesion::SubstantiveEvidence,
    ) -> usize {
        let has_capex_payoff = sub.capex_payoff_signal
            || sub
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::CapexPayoff);
        let has_earnings_validation = sub.earnings_validation
            || sub
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::EarningsValidation);
        let has_order_visibility = sub.order_visibility
            || sub
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::OrderVisibility);

        [
            has_capex_payoff,
            has_earnings_validation,
            has_order_visibility,
        ]
        .into_iter()
        .filter(|has_signal| *has_signal)
        .count()
    }

    fn classify_market_cycle_position(
        packet: &DecisionPacket,
        breadth_mode: TrendBreadthMode,
        substantive_signal_count: usize,
        conviction_score: Option<f64>,
    ) -> MarketCyclePosition {
        if breadth_mode == TrendBreadthMode::StructuralDefense {
            return MarketCyclePosition::DistributionWarning;
        }

        let conviction_score = conviction_score.unwrap_or(0.0);
        let overheat_count = packet
            .assets
            .iter()
            .filter(|asset| {
                asset.asset_state.state == AssetState::OVERHEAT
                    || asset.exit_decision.asset_exit_state == AssetExitState::OverheatProfitTake
            })
            .count();
        let leadership_count = Self::core_leadership_count(packet);
        let narrow_or_fragile = matches!(
            breadth_mode,
            TrendBreadthMode::NarrowLeadership | TrendBreadthMode::FragileRotation
        );

        if narrow_or_fragile
            && substantive_signal_count >= 3
            && conviction_score >= 3.0
            && (overheat_count > 0 || leadership_count >= 2)
        {
            return MarketCyclePosition::CrowdedExpectation;
        }

        if substantive_signal_count >= 3 || conviction_score >= 3.0 {
            return MarketCyclePosition::LateAcceptance;
        }

        if substantive_signal_count >= 2 || breadth_mode == TrendBreadthMode::BroadExpansion {
            return MarketCyclePosition::MidConfirmation;
        }

        if substantive_signal_count >= 1 {
            return MarketCyclePosition::EarlyFormation;
        }

        MarketCyclePosition::Unknown
    }

    fn core_leadership_count(packet: &DecisionPacket) -> usize {
        let leadership_symbols = ["MSFT", "GOOG", "NVDA"];
        packet
            .assets
            .iter()
            .filter(|asset| {
                leadership_symbols.contains(&asset.symbol.as_str())
                    && matches!(
                        asset.asset_state.state,
                        AssetState::OPTIMAL
                            | AssetState::CRUISE
                            | AssetState::PULLBACK
                            | AssetState::OVERHEAT
                    )
            })
            .count()
    }

    fn classify_holding_efficiency(
        packet: &DecisionPacket,
        market_cycle_position: MarketCyclePosition,
        substantive_signal_count: usize,
    ) -> HoldingEfficiency {
        if market_cycle_position == MarketCyclePosition::DistributionWarning {
            return HoldingEfficiency::Neutral;
        }

        let has_overheat = packet.assets.iter().any(|asset| {
            asset.asset_state.state == AssetState::OVERHEAT
                || asset.exit_decision.asset_exit_state == AssetExitState::OverheatProfitTake
        });

        if has_overheat
            && matches!(
                market_cycle_position,
                MarketCyclePosition::LateAcceptance | MarketCyclePosition::CrowdedExpectation
            )
        {
            return HoldingEfficiency::TimeCostRising;
        }

        if substantive_signal_count >= 2
            && matches!(
                market_cycle_position,
                MarketCyclePosition::EarlyFormation | MarketCyclePosition::MidConfirmation
            )
        {
            return HoldingEfficiency::Efficient;
        }

        HoldingEfficiency::Neutral
    }

    fn build_risk_taxonomy(
        packet: &DecisionPacket,
        log: &crate::features::radar::domain::transition_log::StateTransitionLog,
        market_cycle_position: MarketCyclePosition,
        holding_efficiency: HoldingEfficiency,
        dict: &DisplayDictionary,
    ) -> Vec<String> {
        let te = &dict.transition_evidence;
        let structure_risk = if packet
            .market_regime
            .transition_audit
            .as_ref()
            .is_some_and(|audit| audit.core_breakdown)
            || packet.market_regime.risk_overlay == RiskOverlay::BROKEN
        {
            &te.risk_collapse
        } else if matches!(
            packet.market_regime.risk_overlay,
            RiskOverlay::DECELERATING | RiskOverlay::DEFENSIVE
        ) {
            &te.risk_fragile
        } else {
            &te.risk_normal
        };

        let initiation_volatility = if !log.trend_cohesion_gate.to && log.breakout_active_count > 0
        {
            &te.volatility_active
        } else {
            &te.volatility_inactive
        };

        let position_risk = if packet.assets.iter().any(|asset| {
            asset.asset_state.state == AssetState::OVERHEAT
                || asset.exit_decision.asset_exit_state == AssetExitState::OverheatProfitTake
        }) {
            &te.position_risk_overheated
        } else {
            &te.position_risk_normal
        };

        let crowding_risk = Self::map_crowding_risk(market_cycle_position, dict);
        let holding_efficiency = Self::map_holding_efficiency(holding_efficiency, dict);

        vec![
            format!("{}: {}", te.market_structure_risk, structure_risk),
            format!("{}: {}", te.initiation_volatility, initiation_volatility),
            format!("{}: {}", te.position_risk, position_risk),
            format!("{}: {}", te.crowding_risk, crowding_risk),
            format!("{}: {}", te.holding_efficiency, holding_efficiency),
        ]
    }

    fn build_structural_strength(
        substantive_signal_count: usize,
        price_confirmation_record_count: usize,
        conviction_score: Option<f64>,
        dict: &DisplayDictionary,
    ) -> Option<String> {
        if substantive_signal_count == 0 && price_confirmation_record_count == 0 {
            return None;
        }

        let tr = &dict.trend_recognition;
        let label = if substantive_signal_count >= 3
            || price_confirmation_record_count >= 2
            || conviction_score.unwrap_or(0.0) >= 3.0
        {
            &tr.structural_strength_strengthening
        } else {
            &tr.structural_strength_observed
        };

        let mut parts = Vec::new();
        if substantive_signal_count > 0 {
            parts.push(format!(
                "{} {}",
                substantive_signal_count,
                Self::count_unit(
                    substantive_signal_count,
                    &tr.structural_strength_type_unit_singular,
                    &tr.structural_strength_type_unit,
                )
            ));
        }
        if price_confirmation_record_count > 0 {
            parts.push(format!(
                "{} {}",
                price_confirmation_record_count,
                Self::count_unit(
                    price_confirmation_record_count,
                    &tr.structural_strength_price_confirmation_unit_singular,
                    &tr.structural_strength_price_confirmation_unit,
                )
            ));
        }

        Some(format!("{} ({})", label, parts.join(" / ")))
    }

    fn build_evidence_quality_summary(
        sub: &crate::features::radar::domain::trend_cohesion::SubstantiveEvidence,
        dict: &DisplayDictionary,
    ) -> Option<String> {
        if sub.records.is_empty() {
            return None;
        }

        let mut high_quality = 0;
        let mut medium_quality = 0;
        let mut price_confirmation = 0;

        for record in &sub.records {
            match record.source {
                EvidenceSourceType::OfficialIR => high_quality += 1,
                EvidenceSourceType::Manual => medium_quality += 1,
                EvidenceSourceType::PriceAction => price_confirmation += 1,
                EvidenceSourceType::NewsMedia => {}
            }
        }

        let tr = &dict.trend_recognition;
        let mut parts = Vec::new();
        if high_quality > 0 {
            parts.push(format!("{} {}", tr.evidence_quality_high, high_quality));
        }
        if medium_quality > 0 {
            parts.push(format!("{} {}", tr.evidence_quality_medium, medium_quality));
        }
        if price_confirmation > 0 {
            parts.push(format!(
                "{} {}",
                tr.evidence_quality_price, price_confirmation
            ));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" / "))
        }
    }

    fn build_strategic_context(
        substantive_signals: &[String],
        conviction_score: Option<f64>,
        gate_passed: bool,
        breadth_mode: TrendBreadthMode,
        market_cycle_position: MarketCyclePosition,
        macro_gravity: Option<&MacroGravityConfig>,
        dict: &DisplayDictionary,
    ) -> Vec<String> {
        if substantive_signals.is_empty() && macro_gravity.is_none() {
            return Vec::new();
        }

        let tr = &dict.trend_recognition;
        let strengthening =
            substantive_signals.len() >= 3 || conviction_score.unwrap_or(0.0) >= 3.0;
        let direction = if strengthening {
            &tr.strategic_direction_strengthening
        } else {
            &tr.strategic_direction_observed
        };
        let continuity = if strengthening || substantive_signals.len() >= 2 {
            &tr.strategic_evidence_accumulating
        } else {
            &tr.strategic_evidence_initial
        };
        let tactical_status = if gate_passed {
            &tr.strategic_tactical_ready
        } else {
            &tr.strategic_tactical_waiting
        };
        let breadth_mode_label = match breadth_mode {
            TrendBreadthMode::BroadExpansion => &tr.trend_breadth_broad_expansion,
            TrendBreadthMode::NarrowLeadership => &tr.trend_breadth_narrow_leadership,
            TrendBreadthMode::FragileRotation => &tr.trend_breadth_fragile_rotation,
            TrendBreadthMode::StructuralDefense => &tr.trend_breadth_structural_defense,
        };

        let mut context = vec![
            format!(
                "{}: {}",
                tr.strategic_market_structure_mode, breadth_mode_label
            ),
            format!("{}: {}", tr.strategic_direction, direction),
            format!(
                "{}: {}",
                tr.strategic_cycle_position,
                Self::map_market_cycle_position(market_cycle_position, dict)
            ),
            format!(
                "{}: {}",
                tr.strategic_cycle_features,
                Self::map_market_cycle_features(market_cycle_position, dict)
            ),
            format!(
                "{}: {}",
                tr.strategic_crowding_risk,
                Self::map_crowding_risk(market_cycle_position, dict)
            ),
        ];

        if let Some(macro_gravity) =
            macro_gravity.filter(|macro_gravity| macro_gravity.enable.unwrap_or(true))
        {
            context.extend(Self::format_macro_gravity_lines(macro_gravity, dict));
        }

        if !substantive_signals.is_empty() {
            context.push(format!(
                "{}: {}",
                tr.strategic_evidence_continuity, continuity
            ));
            context.push(format!(
                "{}: {}",
                tr.strategic_evidence_coverage,
                substantive_signals.join(" / ")
            ));
        }
        context.push(format!(
            "{}: {}",
            tr.strategic_tactical_status, tactical_status
        ));
        context
    }

    fn build_hypothesis_layer_from_packet(
        packet: &DecisionPacket,
        dict: &DisplayDictionary,
    ) -> Option<HypothesisLayerViewModel> {
        let trend_recognition = packet.trend_recognition.as_ref()?;
        let substantive = trend_recognition.substantive.as_ref()?;
        let mut substantive_signals = Vec::new();
        if substantive.capex_payoff_signal
            || substantive
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::CapexPayoff)
        {
            substantive_signals.push(dict.trend_recognition.capex_payoff.clone());
        }
        if substantive.earnings_validation
            || substantive
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::EarningsValidation)
        {
            substantive_signals.push(dict.trend_recognition.earnings_validation.clone());
        }
        if substantive.order_visibility
            || substantive
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::OrderVisibility)
        {
            substantive_signals.push(dict.trend_recognition.order_visibility.clone());
        }
        let breadth_mode = Self::classify_trend_breadth_mode(packet);
        let market_cycle_position = Self::classify_market_cycle_position(
            packet,
            breadth_mode,
            substantive_signals.len(),
            Some(trend_recognition.conviction_score),
        );
        Self::build_hypothesis_layer(
            &substantive_signals,
            Some(trend_recognition.conviction_score),
            market_cycle_position,
            dict,
        )
    }

    fn build_hypothesis_layer(
        substantive_signals: &[String],
        conviction_score: Option<f64>,
        market_cycle_position: MarketCyclePosition,
        dict: &DisplayDictionary,
    ) -> Option<HypothesisLayerViewModel> {
        let has_capex = substantive_signals
            .iter()
            .any(|signal| signal == &dict.trend_recognition.capex_payoff);
        let has_earnings = substantive_signals
            .iter()
            .any(|signal| signal == &dict.trend_recognition.earnings_validation);
        let has_order = substantive_signals
            .iter()
            .any(|signal| signal == &dict.trend_recognition.order_visibility);
        let enough_reality_evidence =
            has_capex && (has_earnings || has_order) && conviction_score.unwrap_or(0.0) >= 3.0;

        if !enough_reality_evidence {
            return None;
        }

        let candidate = Self::build_profit_pool_migration_hypothesis(market_cycle_position, dict);
        if candidate.failure_risks.is_empty() {
            return None;
        }

        Some(HypothesisLayerViewModel {
            title: dict.hypothesis.title.clone(),
            notice: dict.hypothesis.notice.clone(),
            candidates: vec![candidate],
        })
    }

    fn build_profit_pool_migration_hypothesis(
        market_cycle_position: MarketCyclePosition,
        dict: &DisplayDictionary,
    ) -> HypothesisCandidateViewModel {
        let h = &dict.hypothesis;
        let (consensus_state, pricing_state) = match market_cycle_position {
            MarketCyclePosition::CrowdedExpectation => {
                (h.consensus_crowded.clone(), h.pricing_overpriced.clone())
            }
            MarketCyclePosition::LateAcceptance | MarketCyclePosition::DistributionWarning => (
                h.consensus_consensus.clone(),
                h.pricing_fully_priced.clone(),
            ),
            _ => (
                h.consensus_emerging.clone(),
                h.pricing_partially_priced.clone(),
            ),
        };
        let (narrative_saturation, reality_override_notice) = match market_cycle_position {
            MarketCyclePosition::CrowdedExpectation => (
                h.narrative_saturation_saturated.clone(),
                h.reality_override_required.clone(),
            ),
            MarketCyclePosition::LateAcceptance | MarketCyclePosition::DistributionWarning => (
                h.narrative_saturation_crowded.clone(),
                h.reality_override_required.clone(),
            ),
            _ => (
                h.narrative_saturation_developing.clone(),
                h.reality_override_watch.clone(),
            ),
        };

        HypothesisCandidateViewModel {
            title: h.title_profit_pool_migration.clone(),
            hypothesis_type: h.type_profit_pool_migration.clone(),
            summary: h.summary_profit_pool_migration.clone(),
            consensus_state,
            pricing_state,
            confidence: HypothesisConfidence::Developing,
            confidence_label: h.confidence_developing.clone(),
            time_horizon: h.horizon_medium_long.clone(),
            narrative_saturation,
            reality_override_notice,
            evidence_chain: vec![
                HypothesisEvidenceNodeViewModel {
                    label: h.evidence_capex_expansion.clone(),
                    evidence_type: "CapitalAllocationSignal".to_string(),
                    strength: h.strength_strong.clone(),
                    source_layer: h.source_evidence_record.clone(),
                    note: h.evidence_capex_expansion.clone(),
                },
                HypothesisEvidenceNodeViewModel {
                    label: h.evidence_cost_reduction.clone(),
                    evidence_type: "CostCurveShift".to_string(),
                    strength: h.strength_moderate.clone(),
                    source_layer: h.source_industry_data.clone(),
                    note: h.evidence_cost_reduction.clone(),
                },
                HypothesisEvidenceNodeViewModel {
                    label: h.evidence_pricing_power.clone(),
                    evidence_type: "PricingPower".to_string(),
                    strength: h.strength_moderate.clone(),
                    source_layer: h.source_industry_data.clone(),
                    note: h.evidence_pricing_power.clone(),
                },
                HypothesisEvidenceNodeViewModel {
                    label: h.evidence_platform_adoption.clone(),
                    evidence_type: "PlatformAdoption".to_string(),
                    strength: h.strength_moderate.clone(),
                    source_layer: h.source_evidence_record.clone(),
                    note: h.evidence_platform_adoption.clone(),
                },
                HypothesisEvidenceNodeViewModel {
                    label: h.evidence_revenue_validation.clone(),
                    evidence_type: "RevenueValidation".to_string(),
                    strength: h.strength_moderate.clone(),
                    source_layer: h.source_evidence_record.clone(),
                    note: h.evidence_revenue_validation.clone(),
                },
            ],
            candidate_beneficiaries: vec![
                HypothesisBeneficiaryViewModel {
                    symbol: "MSFT".to_string(),
                    role: h.beneficiary_primary.clone(),
                    rationale: h.beneficiary_msft_rationale.clone(),
                },
                HypothesisBeneficiaryViewModel {
                    symbol: "AMZN".to_string(),
                    role: h.beneficiary_secondary.clone(),
                    rationale: h.beneficiary_amzn_rationale.clone(),
                },
                HypothesisBeneficiaryViewModel {
                    symbol: "GOOG".to_string(),
                    role: h.beneficiary_secondary.clone(),
                    rationale: h.beneficiary_goog_rationale.clone(),
                },
            ],
            failure_risks: vec![
                HypothesisFailureRiskViewModel {
                    label: h.failure_capex_delay.clone(),
                    description: h.failure_capex_delay_desc.clone(),
                    severity: h.severity_high.clone(),
                },
                HypothesisFailureRiskViewModel {
                    label: h.failure_pricing_competition.clone(),
                    description: h.failure_pricing_competition_desc.clone(),
                    severity: h.severity_medium.clone(),
                },
                HypothesisFailureRiskViewModel {
                    label: h.failure_adoption_shortfall.clone(),
                    description: h.failure_adoption_shortfall_desc.clone(),
                    severity: h.severity_medium.clone(),
                },
                HypothesisFailureRiskViewModel {
                    label: h.failure_macro_gravity.clone(),
                    description: h.failure_macro_gravity_desc.clone(),
                    severity: h.severity_medium.clone(),
                },
            ],
            responsibility_notice: h.responsibility_notice.clone(),
        }
    }

    fn format_macro_gravity_lines(
        macro_gravity: &MacroGravityConfig,
        dict: &DisplayDictionary,
    ) -> Vec<String> {
        let tr = &dict.trend_recognition;
        let parts = [
            format!(
                "{} {}",
                tr.macro_rate_pressure,
                Self::map_macro_pressure(macro_gravity.rate_pressure, dict)
            ),
            format!(
                "{} {}",
                tr.macro_real_yield_pressure,
                Self::map_macro_pressure(macro_gravity.real_yield_pressure, dict)
            ),
            format!(
                "{} {}",
                tr.macro_credit_stress,
                Self::map_credit_stress(macro_gravity.credit_stress, dict)
            ),
            format!(
                "{} {}",
                tr.macro_growth_valuation_impact,
                Self::map_growth_valuation_impact(macro_gravity.growth_valuation_impact, dict)
            ),
            format!(
                "{} {}",
                tr.macro_liquidity,
                Self::map_liquidity_condition(macro_gravity.liquidity, dict)
            ),
            format!(
                "{} {}",
                tr.macro_yield_curve,
                Self::map_yield_curve_state(macro_gravity.yield_curve, dict)
            ),
        ];

        let mut lines = vec![format!(
            "{}: {}",
            tr.strategic_macro_gravity,
            parts.join(" / ")
        )];
        lines.push(format!(
            "{}: {}",
            tr.strategic_macro_gravity, tr.macro_boundary
        ));
        lines
    }

    fn map_macro_pressure(pressure: MacroPressure, _dict: &DisplayDictionary) -> &'static str {
        match pressure {
            MacroPressure::Falling => "FALLING",
            MacroPressure::Neutral => "NEUTRAL",
            MacroPressure::Rising => "RISING",
            MacroPressure::Tight => "TIGHT",
        }
    }

    fn map_yield_curve_state(state: YieldCurveState, _dict: &DisplayDictionary) -> &'static str {
        match state {
            YieldCurveState::Normal => "NORMAL",
            YieldCurveState::Flat => "FLAT",
            YieldCurveState::Inverted => "INVERTED",
            YieldCurveState::Steepening => "STEEPENING",
        }
    }

    fn map_credit_stress(stress: CreditStress, _dict: &DisplayDictionary) -> &'static str {
        match stress {
            CreditStress::Normal => "NORMAL",
            CreditStress::Watch => "WATCH",
            CreditStress::Stress => "STRESS",
        }
    }

    fn map_liquidity_condition(
        condition: LiquidityCondition,
        _dict: &DisplayDictionary,
    ) -> &'static str {
        match condition {
            LiquidityCondition::Loose => "LOOSE",
            LiquidityCondition::Neutral => "NEUTRAL",
            LiquidityCondition::Tight => "TIGHT",
        }
    }

    fn map_growth_valuation_impact(
        impact: GrowthValuationImpact,
        _dict: &DisplayDictionary,
    ) -> &'static str {
        match impact {
            GrowthValuationImpact::Supportive => "SUPPORTIVE",
            GrowthValuationImpact::Neutral => "NEUTRAL",
            GrowthValuationImpact::Compressing => "COMPRESSING",
        }
    }

    fn map_market_cycle_position(position: MarketCyclePosition, dict: &DisplayDictionary) -> &str {
        let tr = &dict.trend_recognition;
        match position {
            MarketCyclePosition::EarlyFormation => &tr.cycle_position_early,
            MarketCyclePosition::MidConfirmation => &tr.cycle_position_mid,
            MarketCyclePosition::LateAcceptance => &tr.cycle_position_late,
            MarketCyclePosition::CrowdedExpectation => &tr.cycle_position_crowded,
            MarketCyclePosition::DistributionWarning => &tr.cycle_position_distribution,
            MarketCyclePosition::Unknown => &tr.cycle_position_unknown,
        }
    }

    fn map_market_cycle_features(position: MarketCyclePosition, dict: &DisplayDictionary) -> &str {
        let tr = &dict.trend_recognition;
        match position {
            MarketCyclePosition::EarlyFormation => &tr.cycle_features_early,
            MarketCyclePosition::MidConfirmation => &tr.cycle_features_mid,
            MarketCyclePosition::LateAcceptance => &tr.cycle_features_late,
            MarketCyclePosition::CrowdedExpectation => &tr.cycle_features_crowded,
            MarketCyclePosition::DistributionWarning => &tr.cycle_features_distribution,
            MarketCyclePosition::Unknown => &tr.cycle_features_unknown,
        }
    }

    fn map_crowding_risk(position: MarketCyclePosition, dict: &DisplayDictionary) -> &str {
        let te = &dict.transition_evidence;
        match position {
            MarketCyclePosition::CrowdedExpectation | MarketCyclePosition::DistributionWarning => {
                &te.crowding_risk_active
            }
            MarketCyclePosition::LateAcceptance => &te.crowding_risk_watch,
            MarketCyclePosition::EarlyFormation
            | MarketCyclePosition::MidConfirmation
            | MarketCyclePosition::Unknown => &te.crowding_risk_normal,
        }
    }

    fn map_holding_efficiency(efficiency: HoldingEfficiency, dict: &DisplayDictionary) -> &str {
        let te = &dict.transition_evidence;
        match efficiency {
            HoldingEfficiency::Efficient => &te.holding_efficiency_efficient,
            HoldingEfficiency::Neutral => &te.holding_efficiency_neutral,
            HoldingEfficiency::TimeCostRising => &te.holding_efficiency_time_cost_rising,
            HoldingEfficiency::Overdiscounted => &te.holding_efficiency_overdiscounted,
        }
    }

    fn count_unit<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
        if count == 1 {
            singular
        } else {
            plural
        }
    }

    fn format_structural_breakout_change(
        b: &crate::features::radar::domain::transition_log::BreakoutTransition,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::features::radar::domain::breakout_detection::BreakoutStatus;

        let format_template = |template: &str, symbol: &str, status: &str| {
            template
                .replace("{symbol}", symbol)
                .replace("{status}", status)
        };

        if b.from_status == BreakoutStatus::NoBreakout && b.to_status != BreakoutStatus::NoBreakout
        {
            return format_template(
                &dict.transition_evidence.breakout_added,
                &b.symbol,
                &Self::map_breakout_status(b.to_status, dict),
            );
        }

        if b.from_status != BreakoutStatus::NoBreakout && b.to_status == BreakoutStatus::NoBreakout
        {
            return format_template(
                &dict.transition_evidence.breakout_removed,
                &b.symbol,
                &Self::map_breakout_status(b.from_status, dict),
            );
        }

        format!(
            "{}: {} -> {}",
            b.symbol,
            Self::map_breakout_status(b.from_status, dict),
            Self::map_breakout_status(b.to_status, dict)
        )
    }

    fn map_unmet_diff(
        diff: &crate::features::radar::domain::transition_log::GateTransition,
        packet: &DecisionPacket,
        dict: &DisplayDictionary,
    ) -> Option<UnmetDiffViewModel> {
        if diff.added.is_empty() && diff.removed.is_empty() && diff.persisting.is_empty() {
            return None;
        }

        let map_fn = |s: &str| match s {
            "StabilityThreshold" => dict.trend_cohesion.unmet.stability_threshold.replace(
                "{}",
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
                "{}",
                &format!("{:.0}", packet.trend_cohesion.rotation_quality_score),
            ),
            "WeakLeadership" => dict
                .trend_cohesion
                .unmet
                .weak_leadership
                .replace("{}", &packet.trend_cohesion.leader_count.to_string()),
            _ => s.to_string(),
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
        status: crate::features::radar::domain::breakout_detection::BreakoutStatus,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::features::radar::domain::breakout_detection::BreakoutStatus;
        match status {
            BreakoutStatus::NoBreakout => dict.breakout.no_breakout.clone(),
            BreakoutStatus::EmergingBreakout => dict.breakout.emerging_breakout.clone(),
            BreakoutStatus::ConfirmedBreakout => dict.breakout.confirmed_breakout.clone(),
        }
    }

    fn map_trend_status(
        status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::features::radar::domain::trend_cohesion::TrendCohesionStatus;
        match status {
            TrendCohesionStatus::Formed => dict.trend_cohesion.formed.clone(),
            TrendCohesionStatus::Forming => dict.trend_cohesion.forming.clone(),
            TrendCohesionStatus::Dispersed => dict.trend_cohesion.dispersed.clone(),
        }
    }

    fn map_topology(
        topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::features::radar::domain::trend_cohesion::TrendCohesionTopology;
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
        asset: &crate::features::radar::domain::action_matrix::AssetActionDecision,
        is_restrained: bool,
        is_systemic_collapse: bool,
        dict: &DisplayDictionary,
    ) -> String {
        use crate::features::radar::domain::asset_state::AssetState;
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
        rules: &crate::config::ParsedRules,
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

        let has_persistent_main_theme = Self::has_persistent_main_theme(packet);
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
