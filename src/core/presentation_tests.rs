use crate::config::{AppConfig, OutputConfig, RulesConfig, TrendConfig};
use crate::core::action_matrix::AssetActionDecision;
use crate::core::asset_state::{AssetState, AssetStateSnapshot};
use crate::core::decision::DecisionPacket;
use crate::core::exit::{AssetExitState, ExitDecision, PositionIntent};
use crate::core::i18n::Language;
use crate::core::presentation_assembler::PresentationAssembler;
use crate::core::report::generate_refined_report;
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
        },
        telegram: None,
        futu: None,
        finnhub: None,
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
        },
        watchlist: vec![],
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
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec!["S1".to_string()],
            Language::ZhCn,
        );
        let rep1 = generate_refined_report(&config, &pres1, 0.0, &HashMap::new(), &HashMap::new())
            .unwrap();
        assert!(rep1.markdown_body.contains("💬"));
        assert!(rep1.markdown_body.contains("提示"));

        // Case 2: 4 symbols failed -> Warning (⚠️)
        let f4 = vec!["A".into(), "B".into(), "C".into(), "D".into()];
        let pres4 = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            f4,
            Language::ZhCn,
        );
        let rep4 = generate_refined_report(&config, &pres4, 0.0, &HashMap::new(), &HashMap::new())
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
            &config.get_parsed_rules(),
            &HashMap::new(),
            f6,
            Language::ZhCn,
        );
        let rep6 = generate_refined_report(&config, &pres6, 0.0, &HashMap::new(), &HashMap::new())
            .unwrap();
        assert!(rep6.markdown_body.contains("🚨"));
        assert!(rep6.markdown_body.contains("严重"));
    }

    #[test]
    fn test_full_failure_semantic_safety() {
        // Mock a 100% fetch failure case (same as in cli.rs)
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            ..Default::default()
        };

        // Ensure failed_symbols is populated
        let failed = vec!["BLOCKER".to_string()];
        let config = mock_config(Language::ZhCn);

        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            failed,
            Language::ZhCn,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        // High: Semantic correctness check
        // It should NOT contain default states like "扩张期" (ESTABLISHED) or "启动期" (IGNITION)
        assert!(!report.markdown_body.contains("扩张期"));
        assert!(!report.markdown_body.contains("启动期"));

        // It SHOULD contain the explicit error semantics
        assert!(report.markdown_body.contains("数据不可用"));
        assert!(report.markdown_body.contains("无数据"));
        assert!(report.markdown_body.contains("N/A"));
        assert!(!report.markdown_body.contains("**信心指数**: 0"));
        assert_eq!(pres.signal_summary.confidence_value, "N/A");
        assert_eq!(pres.signal_summary.stability_value, "N/A");
        assert_eq!(pres.signal_summary.participation_value, "N/A");
        assert_eq!(pres.signal_summary.continuity_value, "N/A");
        assert_eq!(pres.signal_summary.regime_age_value, "N/A");
        assert_eq!(pres.signal_summary.flow_value, "N/A");
    }

    #[test]
    fn test_not_ready_state_emits_no_trade_decision_summary() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: crate::core::market_regime::MarketRegimeSnapshot {
                market_state: crate::core::market_regime::MarketState::IGNITION,
                ..Default::default()
            },
            participation: crate::core::participation::ParticipationReadiness {
                participation_ready: false,
                core_tier_streak: 1,
                reasons: vec![
                    "Stability score (1.1) below threshold (10.0)".to_string(),
                    "Core Tier streak (1) below threshold (3)".to_string(),
                ],
                ..Default::default()
            },
            market_features: crate::core::features::MarketFeatures {
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
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        assert!(pres
            .decision_summary
            .action_status_value
            .contains("NO TRADE"));
        assert_eq!(pres.decision_summary.exposure_value, "0-10%");
        assert!(pres.decision_summary.market_board_value.contains("監視 0"));
        assert_eq!(
            pres.decision_summary.opportunity_snapshot_value,
            "明確な機会なし"
        );
        assert_eq!(
            pres.decision_summary.risk_snapshot_value,
            "目立つリスクなし"
        );
        assert!(pres
            .decision_summary
            .readiness_reasons
            .iter()
            .any(|r| r.contains("安定性")));
        assert!(pres
            .decision_summary
            .readiness_reasons
            .iter()
            .any(|r| r.contains("継続性")));
    }

    #[test]
    fn test_zero_clone_vm_integrity() {
        // Verify TopAction selection still works correctly with new reference sorting
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
            assets,
            ..Default::default()
        };
        let config = mock_config(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
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
                    asset_exit_state: AssetExitState::ParticipationExit,
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
            assets,
            ..Default::default()
        };
        let config = mock_config(Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        assert!(pres.top_actions.iter().any(|vm| vm.symbol == "LOCKED"
            && vm
                .diagnostic
                .as_deref()
                .is_some_and(|d| d.contains("参加機会終了")
                    && !d.contains("raw matrix reason should not leak"))));
        assert!(pres.top_actions.iter().any(|vm| vm.symbol == "HOT"
            && vm
                .diagnostic
                .as_deref()
                .is_some_and(|d| d.contains("利益確定売り"))));
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
            &config.get_parsed_rules(),
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

        let packet: crate::core::presentation::PresentationPacket =
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
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec!["AAPL".to_string(), "TSLA".to_string(), "NVDA".to_string()],
            Language::ZhCn,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let count = report.markdown_body.matches("获取失败").count();
        assert_eq!(count, 1, "data alert should be rendered exactly once");
    }
}
