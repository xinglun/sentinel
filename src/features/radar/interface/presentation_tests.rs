use crate::config::{AppConfig, OutputConfig, RulesConfig, TrendConfig};
use crate::features::radar::domain::action_matrix::AssetActionDecision;
use crate::features::radar::domain::asset_state::{AssetState, AssetStateSnapshot};
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::exit::{AssetExitState, ExitDecision, PositionIntent};
use crate::features::radar::domain::rules::ParsedRules as DomainParsedRules;
use crate::features::radar::interface::presentation_assembler::PresentationAssembler;
use crate::features::radar::interface::report::{generate_refined_report, ReportRenderContext};
use crate::features::shared::interface::i18n::Language;
use crate::features::shared::interface::threshold_format::format_threshold_value;
use chrono::Utc;
use std::collections::{BTreeMap, HashMap};

fn mock_config(lang: Language) -> AppConfig {
    AppConfig {
        version: 1,
        output: OutputConfig {
            timezone: "UTC".to_string(),
            format: "markdown".to_string(),
            save_to: "/tmp".to_string(),
            weight_kind: Some("equal".to_string()),
            language: Some(lang),
            compact_transition_evidence_in_no_trade: true,
        },
        telegram: None,
        futu: None,
        finnhub: None,
        fred: None,
        trading: None,
        provider: None,
        rules: RulesConfig {
            trend: TrendConfig::default(),
            deviation_bands: BTreeMap::new(),
            actions: HashMap::new(),
            sizing_multipliers: None,
            core_assets: None,
            min_state_duration: None,
            inertia: None,
            trend_cohesion: None,
            breakout: None,
            market_state_engine: None,
            market_benchmarks: None,
        },
        watchlist: vec![],
        sec: None,
        research_attention: None,
        asset_thesis: None,
        macro_gravity: None,
        capital_absorption: None,
        capital_dynamics: None,
        gray_rhino_escalation: None,
        gray_rhino_provider_registry: None,
    }
}

fn domain_rules(config: &AppConfig) -> DomainParsedRules {
    DomainParsedRules::from(&config.get_parsed_rules())
}

fn report_context(config: &AppConfig) -> ReportRenderContext {
    let rules = config.get_parsed_rules();
    ReportRenderContext {
        compact_transition_in_no_trade: config.output.compact_transition_evidence_in_no_trade,
        compact_stability_threshold: format_threshold_value(
            rules.trend_cohesion.gate_stability_threshold,
        ),
        compact_continuity_threshold: rules.trend_cohesion.gate_continuity_threshold.to_string(),
        observation_timeline: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_threshold_mapping() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);

        // Case 1: 1 symbol failed -> Notice (💬)
        let pres1 = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec!["S1".to_string()],
            Language::ZhCn,
        );
        let rep1 = generate_refined_report(
            &report_context(&config),
            &pres1,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(rep1.markdown_body.contains("💬"));
        assert!(rep1.markdown_body.contains("提示"));

        // Case 2: 4 symbols failed -> Warning (⚠️)
        let f4 = vec!["A".into(), "B".into(), "C".into(), "D".into()];
        let pres4 = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            f4,
            Language::ZhCn,
        );
        let rep4 = generate_refined_report(
            &report_context(&config),
            &pres4,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(rep4.markdown_body.contains("⚠️"));
        assert!(rep4.markdown_body.contains("警告"));

        // Case 3: 6 symbols failed -> Critical (🚨)
        let f6 = vec![
            "1".into(),
            "2".into(),
            "3".into(),
            "4".into(),
            "5".into(),
            "6".into(),
        ];
        let pres6 = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            f6,
            Language::ZhCn,
        );
        let rep6 = generate_refined_report(
            &report_context(&config),
            &pres6,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(rep6.markdown_body.contains("🚨"));
        assert!(rep6.markdown_body.contains("严重"));
    }

    #[test]
    fn no_leader_topology_does_not_get_overridden_by_persistent_main_theme() {
        let mut packet = DecisionPacket {
            market_features: crate::features::radar::domain::features::MarketFeatures {
                up_count: 3,
                down_count: 6,
                total_count: 9,
                ..Default::default()
            },
            assets: vec!["SPY", "MSFT", "GOOG"]
                .into_iter()
                .map(|symbol| AssetActionDecision {
                    symbol: symbol.to_string(),
                    asset_state: AssetStateSnapshot {
                        symbol: symbol.to_string(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .collect(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                topology:
                    crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
                ..Default::default()
            },
            trend_recognition: Some(
                crate::features::radar::domain::trend_cohesion::TrendRecognitionEvidence {
                    conviction_score: 3.4,
                    substantive: Some(
                        crate::features::radar::domain::trend_cohesion::SubstantiveEvidence {
                            capex_payoff_signal: true,
                            earnings_validation: true,
                            order_visibility: true,
                            ..Default::default()
                        },
                    ),
                    ..Default::default()
                },
            ),
            ..Default::default()
        };
        packet.transition_log = Some(
            crate::features::radar::domain::transition_log::StateTransitionLog::compare(
                None, &packet,
            ),
        );

        let config = mock_config(Language::ZhCn);
        let presentation = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let mut presentation = presentation;
        presentation
            .transition_evidence
            .as_mut()
            .unwrap()
            .strategic_context = vec![
            "市场结构模式: 核心资产主导期".to_string(),
            "长期方向: 结构证据观察中".to_string(),
        ];
        PresentationAssembler::reconcile_tactical_leadership_display(
            &mut presentation,
            "none",
            9,
            Language::ZhCn,
        );

        assert_eq!(
            presentation.decision_summary.trend_topology_value,
            "无主线 / 分散"
        );
        let strategic_context = presentation
            .transition_evidence
            .as_ref()
            .unwrap()
            .strategic_context
            .join("\n");
        assert!(strategic_context.contains("结构整理 / 无明确主导"));
        assert!(!strategic_context.contains("核心资产主导期"));
        assert!(strategic_context.contains("长期方向: 结构证据观察中"));
    }

    #[test]
    fn test_full_failure_semantic_safety() {
        // Mock a 100% fetch failure case (same as in cli.rs)
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            ..Default::default()
        };

        // failed_symbols が設定されていることを確認する。
        let failed = vec!["BLOCKER".to_string()];
        let config = mock_config(Language::ZhCn);

        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            failed,
            Language::ZhCn,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        // High: semantic の正しさを確認する。
        // It should NOT contain default states like "扩张期" (ESTABLISHED) or "启动期" (IGNITION)
        assert!(!report.markdown_body.contains("扩张期"));
        assert!(!report.markdown_body.contains("启动期"));

        // 明示的な error semantics を含む必要がある。
        assert!(report.markdown_body.contains("数据不可用"));
        assert!(report.markdown_body.contains("无数据"));
        assert!(report.markdown_body.contains("N/A"));
        assert!(!report.markdown_body.contains("**信心指数**: 0"));
        assert_eq!(pres.signal_summary.confidence_value, "N/A");
        assert_eq!(pres.signal_summary.stability_value, "N/A");
        assert_eq!(pres.signal_summary.continuity_value, "N/A");
        assert_eq!(pres.signal_summary.regime_age_value, "N/A");
        assert_eq!(pres.signal_summary.flow_value, "N/A");
    }

    #[test]
    fn test_not_ready_state_emits_no_trade_decision_summary() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::IGNITION,
                ..Default::default()
            },
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
                gate_passed: false,
                stability_score: 1.1,
                continuity_streak: 1,
                unmet_conditions: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                ..Default::default()
            },
            market_features: crate::features::radar::domain::features::MarketFeatures {
                system_confidence: 54.0,
                stability_score: 1.1,
                regime_age: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        let config = mock_config(Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        assert!(pres
            .decision_summary
            .action_status_value
            .contains("NO TRADE"));

        assert_eq!(pres.decision_summary.entry_cap_value, "0%");
        assert_eq!(
            pres.final_execution_decision.execution_window,
            crate::features::radar::interface::presentation::ExecutionWindow::None
        );
        assert_eq!(
            pres.final_execution_decision.actionability,
            crate::features::radar::interface::presentation::ExecutionActionability::CandidateOnly
        );
        assert_eq!(pres.decision_summary.state_tag_value, "未確認始動期");
        assert_eq!(pres.decision_summary.action_tag_value, "取引禁止");
        assert_eq!(
            pres.decision_summary.hard_rule_note,
            "あらゆる能動売買はシステム規則違反となる。"
        );
        assert_eq!(
            pres.decision_summary.entry_cap_note.as_deref(),
            Some("既存保有の自然変動のみ許容し、新規建ては行わない。")
        );
        assert!(pres.decision_summary.market_board_value.contains("監視 0"));
        assert_eq!(
            pres.decision_summary.opportunity_snapshot_value,
            "明確な機会なし"
        );
        assert_eq!(
            pres.decision_summary.risk_snapshot_value,
            "目立つリスクなし"
        );
        assert!(pres.decision_summary.readiness_reasons.is_empty());
        assert!(pres
            .decision_summary
            .candidate_only_note
            .as_deref()
            .unwrap_or_default()
            .contains("候補観測"));
        assert_eq!(pres.exit_summary.title, "📉 リスク処置提案");
        assert_eq!(
            pres.exit_summary.empty_note.as_deref(),
            Some("減資または終了条件は発動していない。")
        );
    }

    #[test]
    fn ignition_ready_uses_limited_probe_final_execution_decision() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::IGNITION,
                ..Default::default()
            },
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let config = mock_config(Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        assert_eq!(
            pres.final_execution_decision.execution_window,
            crate::features::radar::interface::presentation::ExecutionWindow::Limited
        );
        assert_eq!(
            pres.final_execution_decision.participation_mode,
            crate::features::radar::interface::presentation::ParticipationMode::Probe
        );
        assert_eq!(
            pres.final_execution_decision.actionability,
            crate::features::radar::interface::presentation::ExecutionActionability::Executable
        );
        assert!(pres.final_execution_decision.reason.contains("Probe Only"));
    }

    #[test]
    fn test_trend_cohesion_unmet_conditions_are_localized() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
                gate_passed: false,
                stability_score: 7.5,
                continuity_streak: 1,
                candidate_count: 7,
                leader_count: 1,
                rotation_quality_score: 22.0,
                unmet_conditions: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::HighCandidateDispersion,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::UnstableRotation,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::WeakLeadership,
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let config = mock_config(Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );

        assert_eq!(pres.decision_summary.trend_cohesion_value, "Dispersed");
        assert_eq!(pres.decision_summary.trend_topology_value, "No Leader");
        assert!(pres
            .decision_summary
            .unmet_conditions
            .iter()
            .any(|r| r.contains("Stability score")));
        assert!(pres
            .decision_summary
            .unmet_conditions
            .iter()
            .any(|r| r.contains("Continuity streak")));
        assert!(pres
            .decision_summary
            .unmet_conditions
            .iter()
            .any(|r| r.contains("Too many candidates")));
        assert!(pres
            .decision_summary
            .unmet_conditions
            .iter()
            .any(|r| r.contains("Unstable rotation")));
        assert!(pres
            .decision_summary
            .unmet_conditions
            .iter()
            .any(|r| r.contains("Weak leadership presence")));
    }

    #[test]
    fn test_readiness_reasons_deduplicate_trend_gate_evidence() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            ..Default::default()
        };

        let config = mock_config(Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        assert_eq!(pres.decision_summary.trend_cohesion_value, "分散");
        assert_eq!(pres.decision_summary.trend_topology_value, "主導不在");
    }

    #[test]
    fn test_exit_summary_distinguishes_hold_trim_exit_and_watch() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            assets: vec![
                AssetActionDecision {
                    symbol: "EXITME".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "EXITME".into(),
                        state: AssetState::DEFEND,
                        ..Default::default()
                    },
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::EXIT,
                        asset_exit_state: AssetExitState::DefensiveExit,
                        reasons: vec![],
                    },
                    position_intent: PositionIntent::EXIT,
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TRIMME".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "TRIMME".into(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::TRIM,
                        asset_exit_state: AssetExitState::StrengthLoss,
                        reasons: vec![],
                    },
                    position_intent: PositionIntent::TRIM,
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "HOLDME".into(),
                    has_position_fact: true,
                    is_core_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "HOLDME".into(),
                        state: AssetState::OPTIMAL,
                        ..Default::default()
                    },
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::HOLD,
                        asset_exit_state: AssetExitState::CohesionExit,
                        reasons: vec![],
                    },
                    position_intent: PositionIntent::HOLD,
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "WATCHME".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "WATCHME".into(),
                        state: AssetState::PULLBACK,
                        ..Default::default()
                    },
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::HOLD,
                        asset_exit_state: AssetExitState::None,
                        reasons: vec![],
                    },
                    position_intent: PositionIntent::HOLD,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert_eq!(pres.exit_summary.items.len(), 4);
        assert!(pres
            .exit_summary
            .items
            .iter()
            .any(|i| i.symbol == "EXITME" && i.intent_label == "退出"));
        assert!(pres
            .exit_summary
            .items
            .iter()
            .any(|i| i.symbol == "TRIMME" && i.intent_label == "减仓"));
        assert!(pres
            .exit_summary
            .items
            .iter()
            .any(|i| i.symbol == "HOLDME" && i.intent_label == "持有"));
        assert!(pres
            .exit_summary
            .items
            .iter()
            .any(|i| i.symbol == "WATCHME" && i.intent_label == "观察"));
    }

    #[test]
    fn test_no_trade_candidate_reason_uses_observation_tone_for_cruise() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::IGNITION,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "MSFT".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "MSFT".into(),
                    state: AssetState::CRUISE,
                    ..Default::default()
                },
                position_intent: PositionIntent::HOLD,
                has_position_fact: false,
                ..Default::default()
            }],
            top_tier_symbols: vec!["MSFT".into()],
            ..Default::default()
        };

        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let item = pres
            .top_actions
            .iter()
            .find(|a| a.symbol == "MSFT")
            .unwrap();
        assert_eq!(item.secondary_desc, "筹备");
        assert_eq!(item.diagnostic.as_deref(), Some("结构延续中，观察持续性"));
        assert!(!item
            .diagnostic
            .as_deref()
            .unwrap_or_default()
            .contains("持有为主"));
    }

    #[test]
    fn test_exit_summary_is_localized_in_english() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "AAPL".into(),
                has_position_fact: true,
                asset_state: AssetStateSnapshot {
                    symbol: "AAPL".into(),
                    state: AssetState::PULLBACK,
                    ..Default::default()
                },
                position_intent: PositionIntent::HOLD,
                ..Default::default()
            }],
            ..Default::default()
        };

        let config = mock_config(Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );

        assert_eq!(pres.exit_summary.title, "📉 Risk Handling");
        assert!(pres
            .exit_summary
            .items
            .iter()
            .any(|i| i.symbol == "AAPL" && i.intent_label == "WATCH"));
    }

    #[test]
    fn test_zero_clone_vm_integrity() {
        // 新しい参照 sorting でも TopAction selection が正しく動くことを確認する。
        let assets = vec![
            AssetActionDecision {
                symbol: "WIN".into(),
                action_changed: true,
                z_score: Some(3.0),
                ..Default::default()
            },
            AssetActionDecision {
                symbol: "LOSE".into(),
                action_changed: false,
                z_score: Some(1.0),
                ..Default::default()
            },
        ];
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets,
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert_eq!(pres.top_actions[0].symbol, "WIN");
        assert!(pres.top_actions[0].action_changed);
    }

    #[test]
    fn test_exit_reasons_are_localized_for_participation_and_overheat() {
        let assets = vec![
            AssetActionDecision {
                symbol: "LOCKED".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "LOCKED".into(),
                    state: AssetState::OPTIMAL,
                    ..Default::default()
                },
                position_intent: PositionIntent::TRIM,
                exit_decision: ExitDecision {
                    position_intent: PositionIntent::TRIM,
                    asset_exit_state: AssetExitState::CohesionExit,
                    reasons: vec![],
                },
                reasons: vec!["raw matrix reason should not leak".to_string()],
                ..Default::default()
            },
            AssetActionDecision {
                symbol: "HOT".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "HOT".into(),
                    state: AssetState::OPTIMAL,
                    ..Default::default()
                },
                position_intent: PositionIntent::TRIM,
                exit_decision: ExitDecision {
                    position_intent: PositionIntent::TRIM,
                    asset_exit_state: AssetExitState::OverheatProfitTake,
                    reasons: vec![],
                },
                ..Default::default()
            },
        ];

        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets,
            ..Default::default()
        };
        let config = mock_config(Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        assert!(pres.top_actions.iter().any(|vm| vm.symbol == "LOCKED"
            && vm
                .diagnostic
                .as_deref()
                .is_some_and(|d| d.contains("凝集力失効")
                    && !d.contains("raw matrix reason should not leak"))));
        assert!(pres.top_actions.iter().any(|vm| vm.symbol == "HOT"
            && vm
                .diagnostic
                .as_deref()
                .is_some_and(|d| d.contains("時間コスト"))));
    }

    #[test]
    fn test_defend_and_caution_prefer_dictionary_diagnostics_over_raw_reasons() {
        let assets = vec![
            AssetActionDecision {
                symbol: "DEF".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "DEF".into(),
                    state: AssetState::DEFEND,
                    ..Default::default()
                },
                reasons: vec!["Matched band: defend (dev: -9.50)".to_string()],
                ..Default::default()
            },
            AssetActionDecision {
                symbol: "CAT".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "CAT".into(),
                    state: AssetState::CAUTION,
                    ..Default::default()
                },
                reasons: vec!["Matched band: caution (dev: -2.10)".to_string()],
                ..Default::default()
            },
        ];

        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            assets,
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert!(pres.top_actions.iter().any(|vm| vm.symbol == "DEF"
            && vm
                .diagnostic
                .as_deref()
                .is_some_and(|d| d.contains("结构转弱，避免参与") && !d.contains("Matched band"))));
        assert!(pres.top_actions.iter().any(|vm| vm.symbol == "CAT"
            && vm
                .diagnostic
                .as_deref()
                .is_some_and(|d| d.contains("信号转弱，暂不加仓") && !d.contains("Matched band"))));
    }

    #[test]
    fn test_no_trade_candidate_watchlist_excludes_defend_only_assets() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::DEFENSIVE,
                risk_overlay: crate::features::radar::domain::market_regime::RiskOverlay::DEFENSIVE,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "NVDA".into(),
                has_position_fact: true,
                asset_state: AssetStateSnapshot {
                    symbol: "NVDA".into(),
                    state: AssetState::DEFEND,
                    ..Default::default()
                },
                position_intent: PositionIntent::EXIT,
                exit_decision: ExitDecision {
                    position_intent: PositionIntent::EXIT,
                    asset_exit_state: AssetExitState::DefensiveExit,
                    reasons: vec![],
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert!(pres.decision_summary.is_no_trade);
        assert_eq!(
            pres.decision_summary.market_board_value,
            "观察 0 | 持有 0 | 收缩 1"
        );
        assert!(pres.top_actions.is_empty());
    }

    #[test]
    fn test_no_trade_candidate_watchlist_excludes_defend_only_assets_outside_defensive_state() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::IGNITION,
                risk_overlay: crate::features::radar::domain::market_regime::RiskOverlay::NORMAL,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "FIG".into(),
                has_position_fact: true,
                asset_state: AssetStateSnapshot {
                    symbol: "FIG".into(),
                    state: AssetState::DEFEND,
                    ..Default::default()
                },
                position_intent: PositionIntent::EXIT,
                exit_decision: ExitDecision {
                    position_intent: PositionIntent::EXIT,
                    asset_exit_state: AssetExitState::DefensiveExit,
                    reasons: vec![],
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert!(pres.decision_summary.is_no_trade);
        assert_eq!(
            pres.decision_summary.market_board_value,
            "观察 0 | 持有 0 | 收缩 1"
        );
        assert!(pres.top_actions.is_empty());
    }

    #[test]
    fn test_risk_snapshot_aggregates_same_reason_peers() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::DEFENSIVE,
                risk_overlay: crate::features::radar::domain::market_regime::RiskOverlay::DEFENSIVE,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "HOT".into(),
                    has_position_fact: false,
                    asset_state: AssetStateSnapshot {
                        symbol: "HOT".into(),
                        state: AssetState::OVERHEAT,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "NVDA".into(),
                        state: AssetState::DEFEND,
                        ..Default::default()
                    },
                    position_intent: PositionIntent::EXIT,
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::EXIT,
                        asset_exit_state: AssetExitState::DefensiveExit,
                        reasons: vec![],
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "SPY".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "SPY".into(),
                        state: AssetState::DEFEND,
                        ..Default::default()
                    },
                    position_intent: PositionIntent::EXIT,
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::EXIT,
                        asset_exit_state: AssetExitState::DefensiveExit,
                        reasons: vec![],
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert!(pres
            .risk_opportunity_summary
            .risk_value
            .contains("结构脆弱: 暂停主动进攻，等待扩散恢复"));
        assert!(!pres
            .risk_opportunity_summary
            .risk_value
            .contains("保命层强制退出"));
        assert!(pres
            .risk_opportunity_summary
            .risk_value
            .contains("其余1标的同类风险"));
        assert!(pres.risk_opportunity_summary.risk_value.contains("NVDA"));
        assert!(!pres.risk_opportunity_summary.risk_value.contains("HOT"));
    }

    #[test]
    fn test_risk_snapshot_uses_collapse_language_only_for_systemic_breakdown() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::DEFENSIVE,
                risk_overlay: crate::features::radar::domain::market_regime::RiskOverlay::BROKEN,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "NVDA".into(),
                has_position_fact: true,
                asset_state: AssetStateSnapshot {
                    symbol: "NVDA".into(),
                    state: AssetState::DEFEND,
                    ..Default::default()
                },
                position_intent: PositionIntent::EXIT,
                exit_decision: ExitDecision {
                    position_intent: PositionIntent::EXIT,
                    asset_exit_state: AssetExitState::DefensiveExit,
                    reasons: vec![],
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert!(pres
            .risk_opportunity_summary
            .risk_value
            .contains("系统性崩塌: 激活保命层强制退出"));
    }

    #[test]
    fn test_risk_snapshot_peer_suffix_is_localized_in_en_and_ja() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
                market_state: crate::features::radar::domain::market_regime::MarketState::DEFENSIVE,
                risk_overlay: crate::features::radar::domain::market_regime::RiskOverlay::DEFENSIVE,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "HOT".into(),
                    has_position_fact: false,
                    asset_state: AssetStateSnapshot {
                        symbol: "HOT".into(),
                        state: AssetState::OVERHEAT,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "NVDA".into(),
                        state: AssetState::DEFEND,
                        ..Default::default()
                    },
                    position_intent: PositionIntent::EXIT,
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::EXIT,
                        asset_exit_state: AssetExitState::DefensiveExit,
                        reasons: vec![],
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "SPY".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "SPY".into(),
                        state: AssetState::DEFEND,
                        ..Default::default()
                    },
                    position_intent: PositionIntent::EXIT,
                    exit_decision: ExitDecision {
                        position_intent: PositionIntent::EXIT,
                        asset_exit_state: AssetExitState::DefensiveExit,
                        reasons: vec![],
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let config_en = mock_config(Language::EnUs);
        let pres_en = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config_en),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        assert!(pres_en
            .risk_opportunity_summary
            .risk_value
            .contains("plus 1 peers with same risk"));

        let config_ja = mock_config(Language::JaJp);
        let pres_ja = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config_ja),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );
        assert!(pres_ja
            .risk_opportunity_summary
            .risk_value
            .contains("他1銘柄も同種リスク"));
    }

    #[test]
    fn test_presentation_packet_defaults_language_for_legacy_payloads() {
        let legacy = serde_json::json!({
            "date_str": "2026-03-27",
            "macro_display": {
                "headline": "扩张期",
                "summary": "扩张趋势，积极参与",
                "risk_label": "风险正常",
                "bias_label": "多头占优"
            },
            "top_actions": [],
            "terminal_rows": [],
            "state_code": "ESTABLISHED"
        });

        let packet: crate::features::radar::interface::presentation::PresentationPacket =
            serde_json::from_value(legacy).unwrap();

        assert_eq!(packet.language, Language::ZhCn);
        assert!(packet.data_alert.is_none());
    }

    #[test]
    fn test_data_alert_is_not_rendered_twice() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);

        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec!["AAPL".to_string(), "TSLA".to_string(), "NVDA".to_string()],
            Language::ZhCn,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let count = report.markdown_body.matches("获取失败").count();
        assert_eq!(count, 1, "data alert should be rendered exactly once");
    }

    #[test]
    fn test_breakout_summary_localizes_emerging_and_failed_risk() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "PLTR".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
                    breakout_strength: 72.0,
                    breakout_quality: 68.0,
                    failed_breakout_risk: 61.0,
                    reasons: vec![
                        crate::features::radar::domain::breakout_detection::BreakoutReason::StructuralBreakout,
                        crate::features::radar::domain::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                    ],
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = mock_config(Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );

        assert_eq!(pres.breakout_summary.title, "🚀 Breakout Detection");
        assert_eq!(pres.breakout_summary.items.len(), 1);
        let item = &pres.breakout_summary.items[0];
        assert_eq!(item.status_label, "Emerging Breakout (Day 1)");
        assert_eq!(item.reason, "Leadership-style breakout");
        assert_eq!(item.failed_risk_value.as_deref(), Some("61"));
    }

    #[test]
    fn test_breakout_summary_keeps_pullback_repair_distinct_from_rebound() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "TSLA".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                    breakout_strength: 34.0,
                    breakout_quality: 40.0,
                    reasons: vec![crate::features::radar::domain::breakout_detection::BreakoutReason::PullbackRepair],
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert_eq!(pres.breakout_summary.items.len(), 1);
        assert_eq!(pres.breakout_summary.items[0].status_label, "无突破");
        assert_eq!(pres.breakout_summary.items[0].reason, "回撤修复");
    }

    #[test]
    fn test_breakout_summary_surfaces_ordinary_rebound() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "QQQ".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                    breakout_strength: 28.0,
                    breakout_quality: 31.0,
                    reasons: vec![crate::features::radar::domain::breakout_detection::BreakoutReason::OrdinaryRebound],
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert_eq!(pres.breakout_summary.items.len(), 1);
        assert_eq!(pres.breakout_summary.items[0].status_label, "无突破");
        assert_eq!(pres.breakout_summary.items[0].reason, "普通反弹");
    }

    #[test]
    fn test_breakout_summary_denoises_no_trade_to_only_actionable_or_high_risk_items() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 27.0,
                        breakout_quality: 29.0,
                        reasons: vec![
                            crate::features::radar::domain::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TSLA".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 35.0,
                        breakout_quality: 42.0,
                        reasons: vec![
                            crate::features::radar::domain::breakout_detection::BreakoutReason::PullbackRepair,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
                        breakout_strength: 48.0,
                        breakout_quality: 72.0,
                        reasons: vec![
                            crate::features::radar::domain::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 31.0,
                        breakout_quality: 38.0,
                        failed_breakout_risk: 82.0,
                        reasons: vec![
                            crate::features::radar::domain::breakout_detection::BreakoutReason::OrdinaryRebound,
                            crate::features::radar::domain::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        assert_eq!(pres.breakout_summary.items.len(), 2);
        assert_eq!(pres.breakout_summary.items[0].symbol, "GOOG");
        assert_eq!(
            pres.breakout_summary.items[0].status_label,
            "突破萌芽（第1天）"
        );
        assert_eq!(pres.breakout_summary.items[1].symbol, "NVDA");
        assert_eq!(pres.breakout_summary.items[1].status_label, "无突破");
        assert_eq!(pres.breakout_summary.items[1].reason, "假突破风险");
        assert_eq!(
            pres.breakout_summary.items[1].failed_risk_value.as_deref(),
            Some("82")
        );
        assert!(!pres
            .breakout_summary
            .items
            .iter()
            .any(|item| item.symbol == "QQQ" || item.symbol == "TSLA"));
    }

    #[test]
    fn test_trend_recognition_evidence_mapping() {
        use crate::features::radar::domain::asset_state::{AssetState, AssetStateSnapshot};
        use crate::features::radar::domain::market_regime::{MarketRegimeSnapshot, RiskOverlay};
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
            TrendContinuationState, TrendRecognitionEvidence,
        };

        let mut curr = DecisionPacket::default();
        curr.market_regime = MarketRegimeSnapshot {
            risk_overlay: RiskOverlay::DEFENSIVE,
            ..Default::default()
        };
        curr.assets = vec![AssetActionDecision {
            symbol: "GOOG".to_string(),
            asset_state: AssetStateSnapshot {
                symbol: "GOOG".to_string(),
                state: AssetState::OVERHEAT,
                ..Default::default()
            },
            exit_decision: ExitDecision {
                asset_exit_state: AssetExitState::OverheatProfitTake,
                ..Default::default()
            },
            ..Default::default()
        }];
        curr.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::LeaderConfirmedFollowersLagging,
            diffusion_score: 0.45,
            conviction_score: 3.4,
            lag_state: true,
            single_asset_decay_day: 3,
            single_asset_decay_max: 5,
            substantive: Some(SubstantiveEvidence {
                capex_payoff_signal: true,
                earnings_validation: true,
                order_visibility: true,
                records: vec![
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::OfficialIR,
                        EvidenceType::CapexPayoff,
                        0.9,
                        "Official capex payoff".to_string(),
                        "2026-05-15".to_string(),
                        None,
                        None,
                        String::new(),
                    ),
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::Manual,
                        EvidenceType::EarningsValidation,
                        0.8,
                        "Curated earnings validation".to_string(),
                        "2026-05-15".to_string(),
                        None,
                        None,
                        String::new(),
                    ),
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::PriceAction,
                        EvidenceType::FollowThrough,
                        0.7,
                        "Breakout follow-through".to_string(),
                        "2026-05-15".to_string(),
                        None,
                        None,
                        String::new(),
                    ),
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::NewsMedia,
                        EvidenceType::OrderVisibility,
                        0.6,
                        "Media order visibility".to_string(),
                        "2026-05-15".to_string(),
                        None,
                        None,
                        String::new(),
                    ),
                ],
                ..Default::default()
            }),
        });

        // evidence を運ぶために transition log を計算する。
        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let transition = pres.transition_evidence.as_ref().unwrap();
        assert_eq!(
            transition.trend_recognition_state.as_deref(),
            Some("单点确立/整体滞后")
        );
        assert_eq!(transition.trend_recognition_diffusion_score, Some(0.45));
        assert_eq!(
            transition.trend_recognition_lag_state.as_deref(),
            Some("先行成立・追随迟缓")
        );
        assert_eq!(
            transition.trend_recognition_single_asset_decay.as_deref(),
            Some("3/5")
        );
        assert_eq!(
            transition.risk_taxonomy,
            vec![
                "市场结构风险: FRAGILE".to_string(),
                "启动期波动: 无".to_string(),
                "价格位置风险: OVERHEATED".to_string(),
                "拥挤风险: ACTIVE".to_string(),
                "持有效率: TIME_COST_RISING".to_string(),
            ]
        );
        assert_eq!(
            transition.market_cycle_position,
            crate::features::radar::interface::presentation::MarketCyclePosition::CrowdedExpectation
        );
        assert_eq!(
            transition.holding_efficiency,
            crate::features::radar::interface::presentation::HoldingEfficiency::TimeCostRising
        );
        assert_eq!(
            transition.structural_strength.as_deref(),
            Some("增强中 (3 类证据 / 1 条价格确认)")
        );
        assert_eq!(
            transition.evidence_quality_summary.as_deref(),
            Some("高质量 1 / 人工/二级 1 / 价格确认 1")
        );
        assert_eq!(
            transition.strategic_context,
            vec![
                "市场结构模式: 脆弱轮动期".to_string(),
                "长期方向: 长期结构趋势增强".to_string(),
                "周期位置: CROWDED_EXPECTATION".to_string(),
                "周期特征: 预期拥挤 / 核心资产集中 / 好消息钝化风险".to_string(),
                "拥挤风险: ACTIVE".to_string(),
                "证据持续性: 持续累积".to_string(),
                "证据覆盖: AI 投入产出验证 (Capex Payoff) / 业绩实质性确认 (Earnings Quality) / 订单能见度提升 (Order Visibility)".to_string(),
                "战术状态: NO TRADE，等待结构扩散".to_string(),
            ]
        );
        let hypothesis_layer = pres.hypothesis_layer.as_ref().unwrap();
        assert_eq!(
            hypothesis_layer.candidates[0].title,
            "AI 利润池可能从 GPU layer 向 cloud / platform layer 扩散"
        );
        assert_eq!(
            hypothesis_layer.candidates[0].confidence,
            crate::features::radar::interface::presentation::HypothesisConfidence::Developing
        );
        assert_eq!(hypothesis_layer.candidates[0].consensus_state, "crowded");
        assert_eq!(hypothesis_layer.candidates[0].pricing_state, "overpriced");
        assert_eq!(
            hypothesis_layer.candidates[0].narrative_saturation,
            "saturated narrative"
        );
        assert_eq!(hypothesis_layer.candidates[0].time_horizon, "LONG");
        assert_eq!(
            hypothesis_layer.candidates[0].materialization_window,
            "12-36 months"
        );
        assert!(hypothesis_layer.candidates[0]
            .tactical_isolation_notice
            .contains("不覆盖当前 NO TRADE"));
        assert!(hypothesis_layer.candidates[0]
            .reality_override_notice
            .contains("现实连续反驳"));
        assert_eq!(
            hypothesis_layer.candidates[0].reality_override_priority,
            "CRITICAL"
        );
        assert!(hypothesis_layer.candidates[0]
            .confidence_decay_notice
            .contains("必须降低假设权重"));
        assert!(!hypothesis_layer.candidates[0].failure_risks.is_empty());
        assert!(!format!("{:?}", hypothesis_layer.candidates[0].confidence).contains("Confirmed"));
    }

    #[test]
    fn test_hypothesis_layer_mapping_requires_failure_risks() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            SubstantiveEvidence, TrendContinuationState, TrendRecognitionEvidence,
        };

        let mut curr = DecisionPacket::default();
        curr.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::StructuralPersistence,
            diffusion_score: 3.4,
            conviction_score: 3.4,
            substantive: Some(SubstantiveEvidence {
                capex_payoff_signal: true,
                earnings_validation: true,
                order_visibility: true,
                ..Default::default()
            }),
            ..Default::default()
        });
        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let candidate = &pres.hypothesis_layer.as_ref().unwrap().candidates[0];
        assert!(!candidate.failure_risks.is_empty());
        assert_eq!(
            candidate.confidence,
            crate::features::radar::interface::presentation::HypothesisConfidence::Developing
        );
        assert_eq!(candidate.narrative_saturation, "crowded narrative");
        assert_eq!(candidate.materialization_window, "12-36 months");
        assert!(candidate
            .tactical_isolation_notice
            .contains("不覆盖当前 NO TRADE"));
        assert!(candidate.reality_override_notice.contains("叙事已拥挤"));
        assert_eq!(candidate.reality_override_priority, "ELEVATED");
        assert!(candidate
            .confidence_decay_notice
            .contains("必须降低假设权重"));
        assert!(!format!("{:?}", candidate.confidence).contains("Confirmed"));
    }

    #[test]
    fn test_single_asset_decay_reset_logic_simulation() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            TrendContinuationState, TrendRecognitionEvidence,
        };

        // 1. Single asset breakout persists
        let prev = DecisionPacket {
            trend_recognition: Some(TrendRecognitionEvidence {
                state: TrendContinuationState::EarlyLeader,
                diffusion_score: 0.2,
                conviction_score: 0.0,
                lag_state: false,
                single_asset_decay_day: 2,
                single_asset_decay_max: 5,
                substantive: None,
            }),
            ..Default::default()
        };

        // 2. Broadening occurs (Multiple assets), decay should reset/change
        let curr = DecisionPacket {
            trend_recognition: Some(TrendRecognitionEvidence {
                state: TrendContinuationState::Broadening,
                diffusion_score: 0.6,
                conviction_score: 0.0,
                lag_state: false,
                single_asset_decay_day: 0, // Reset
                single_asset_decay_max: 5,
                substantive: None,
            }),
            ..Default::default()
        };

        let log = StateTransitionLog::compare(Some(&prev), &curr);

        let config = mock_config(Language::JaJp);

        // assemble logic に log を注入するか、VM mapping だけを確認する。
        let mut curr_with_log = curr.clone();
        curr_with_log.transition_log = Some(log);

        let pres = PresentationAssembler::assemble(
            &curr_with_log,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        let transition = pres.transition_evidence.as_ref().unwrap();
        assert_eq!(
            transition.trend_recognition_state.as_deref(),
            Some("拡散初期")
        );
        assert_eq!(transition.trend_recognition_diffusion_score, Some(0.6));
        assert_eq!(
            transition.trend_recognition_single_asset_decay.as_deref(),
            None
        );
    }
}
