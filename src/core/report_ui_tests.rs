use crate::config::{AppConfig, OutputConfig, RulesConfig, TrendConfig};
use crate::core::action_matrix::AssetActionDecision;
use crate::core::asset_state::{AssetState, AssetStateSnapshot};
use crate::core::decision::DecisionPacket;
use crate::core::exit::PositionIntent;
use crate::core::market_regime::{MarketRegimeSnapshot, MarketState, RiskOverlay};
use crate::core::presentation_assembler::PresentationAssembler;
use crate::core::report::generate_refined_report;
use chrono::Utc;
use std::collections::{BTreeMap, HashMap};

fn mock_config() -> AppConfig {
    AppConfig {
        version: 1,
        output: OutputConfig {
            timezone: "UTC".to_string(),
            format: "markdown".to_string(),
            save_to: "/tmp".to_string(),
            weight_kind: Some("equal".to_string()),
            language: Some(crate::core::i18n::Language::ZhCn),
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
            trend_cohesion: None,
            breakout: None,
        },
        watchlist: vec![],
    }
}

fn mock_config_with_language(language: crate::core::i18n::Language) -> AppConfig {
    let mut config = mock_config();
    config.output.language = Some(language);
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_v3_ui_established() {
        let assets = vec![AssetActionDecision {
            symbol: "NVDA".into(),
            asset_state: AssetStateSnapshot {
                symbol: "NVDA".into(),
                state: AssetState::OPTIMAL,
                ..Default::default()
            },
            position_intent: PositionIntent::ADD,
            price: 100.0,
            ..Default::default()
        }];

        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::ESTABLISHED,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            participation: crate::core::participation::ParticipationReadiness {
                participation_ready: true,
                core_tier_streak: 3,
                stability_ready: true,
                core_tier_streak_ready: true,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets,
            ..Default::default()
        };

        let config = mock_config();
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let prices: HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );

        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &prices)
            .unwrap()
            .markdown_body;

        // Verify updated layout (report.rs 100% Logic De-bloat version)
        assert!(card.contains("市场状态"));
        assert!(card.contains("扩张期"));
        assert!(card.contains("NVDA"));
        assert!(card.contains("🟢"));
        assert!(card.contains("决策结论"));
        assert!(card.contains("监控信号"));
        assert!(card.contains("战术分区"));
    }

    #[test]
    fn test_telegram_html_body_uses_html_not_markdown_markers() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
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
            ..Default::default()
        };

        let config = mock_config();
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let result =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(result.telegram_html_body.contains("<b>🌍 市场摘要</b>"));
        assert!(result.telegram_html_body.contains("<b>🚫 决策结论</b>"));
        assert!(!result.telegram_html_body.contains("## 🌍 市场摘要"));
        assert!(!result.telegram_html_body.contains("**🚫 决策结论**"));
    }

    #[test]
    fn test_telegram_v3_ui_defensive() {
        let assets = vec![AssetActionDecision {
            symbol: "SPY".into(),
            asset_state: AssetStateSnapshot {
                symbol: "SPY".into(),
                state: AssetState::DEFEND,
                ..Default::default()
            },
            position_intent: PositionIntent::EXIT,
            price: 400.0,
            ..Default::default()
        }];

        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                risk_overlay: RiskOverlay::DEFENSIVE,
                ..Default::default()
            },
            participation: crate::core::participation::ParticipationReadiness {
                participation_ready: true,
                stability_ready: true,
                core_tier_streak_ready: true,
                core_tier_streak: 3,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets,
            ..Default::default()
        };

        let config = mock_config();
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let prices: HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );

        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &prices)
            .unwrap()
            .markdown_body;

        assert!(card.contains("保命期"));
        assert!(card.contains("SPY"));
        assert!(card.contains("全決済") || card.contains("退出"));
        assert!(card.contains("🔴"));
    }

    #[test]
    fn test_telegram_v3_ui_data_error() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::ESTABLISHED,
                ..Default::default()
            },
            ..Default::default()
        };

        let config = mock_config();
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let failed_symbols = vec!["AAPL".to_string(), "TSLA".to_string()];

        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            failed_symbols,
            lang,
        );
        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new())
            .unwrap()
            .markdown_body;

        // Verify new 3-level alert format: 💬 提示: 获取失败 (AAPL, TSLA)
        assert!(card.contains("💬"));
        assert!(card.contains("提示"));
        assert!(card.contains("获取失败"));
        assert!(card.contains("AAPL, TSLA"));
    }

    #[test]
    fn test_trend_cohesion_gate_renders_extended_unmet_conditions() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                status: crate::core::trend_cohesion::TrendCohesionStatus::NotFormed,
                topology: crate::core::trend_cohesion::TrendCohesionTopology::NoLeader,
                gate_passed: false,
                stability_score: 7.5,
                continuity_streak: 1,
                candidate_count: 7,
                leader_count: 1,
                rotation_quality_score: 22.0,
                unmet_conditions: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::HighCandidateDispersion,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::UnstableRotation,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::WeakLeadership,
                ],
                ..Default::default()
            },
            participation: crate::core::participation::ParticipationReadiness {
                participation_ready: false,
                reasons: vec![
                    "Stability score (7.5) below threshold (10.0)".to_string(),
                    "Core Tier streak (1) below threshold (3)".to_string(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let config = mock_config();
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new())
            .unwrap()
            .markdown_body;

        assert!(card.contains("主线形成条件"));
        assert!(card.contains("当前未满足项"));
        assert!(card.contains("主线结构"));
        assert!(card.contains("无主线"));
        assert!(card.contains("候选池过于发散"));
        assert!(card.contains("主线轮动不稳定"));
        assert!(card.contains("持续领涨强度不足"));
        assert_eq!(card.matches("稳定性不足").count(), 1);
        assert_eq!(card.matches("连续性不足").count(), 1);
    }

    #[test]
    fn test_report_respects_configured_japanese_language() {
        let assets = vec![AssetActionDecision {
            symbol: "NVDA".into(),
            asset_state: AssetStateSnapshot {
                symbol: "NVDA".into(),
                state: AssetState::OPTIMAL,
                ..Default::default()
            },
            position_intent: PositionIntent::ADD,
            price: 100.0,
            ..Default::default()
        }];

        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::ESTABLISHED,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            participation: crate::core::participation::ParticipationReadiness {
                participation_ready: true,
                stability_ready: true,
                core_tier_streak_ready: true,
                core_tier_streak: 3,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets,
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let prices: HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );

        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &prices)
            .unwrap()
            .markdown_body;

        assert!(card.contains("市場サマリー"));
        assert!(card.contains("行動判断"));
        assert!(card.contains("主要アクション"));
        assert!(card.contains("強気"));
        assert!(card.contains("監視シグナル"));
        assert!(!card.contains("市场摘要"));
    }

    #[test]
    fn test_no_trade_report_renders_full_battleboard_sections() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
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
            assets: vec![AssetActionDecision {
                symbol: "NVDA".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "NVDA".into(),
                    state: AssetState::FORMING,
                    ..Default::default()
                },
                position_intent: PositionIntent::HOLD,
                has_position_fact: false,
                ..Default::default()
            }],
            top_tier_symbols: vec!["NVDA".into()],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new())
            .unwrap()
            .markdown_body;

        assert!(card.contains("市場サマリー"));
        assert!(card.contains("行動判断"));
        assert!(card.contains("取引禁止（NO TRADE）"));
        assert!(card.contains("候補観測リスト"));
        assert!(card.contains("構造形成段階"));
        assert!(card.contains("監視シグナル"));
        assert!(card.contains("戦術的区分"));
        assert!(card.contains("リスクと機会"));
        assert!(card.contains("戦況総覧"));
        assert!(card.contains("以下は候補観測のみ"));
        assert!(card.contains("あらゆる能動売買はシステム規則違反となる。"));
        assert!(!card.contains("1. 🔵 **NVDA** - 観測"));
        assert!(card.contains("- NVDA · 形成中"));
        assert!(!card.contains("1. **NVDA**"));
        assert!(card.contains("### 取引禁止（NO TRADE）"));
        assert!(card.contains("状態：未確認始動期"));
        assert!(card.contains("行動：取引禁止"));
        assert!(card.contains("新規建て上限 · 0%"));
        assert!(!card.contains("0-10%"));
        assert!(!card.contains("推奨ポジション"));
        assert!(!card.contains("買い増し"));
        assert!(!card.contains("買入"));
        assert!(!card.contains("建玉"));
        assert!(card.contains("> 信頼指数 "));
        assert!(card.contains("サイクル期間 "));

        let decision_idx = card.find("行動判断").unwrap();
        let actions_idx = card.find("候補観測リスト").unwrap();
        assert!(decision_idx < actions_idx);
    }

    #[test]
    fn test_telegram_output_omits_trailing_rule_and_no_trade_watchlist_stays_observational() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
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

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(!report.telegram_html_body.trim_end().ends_with("---"));
        assert!(!report.markdown_body.trim_end().ends_with("---"));
        assert!(report.markdown_body.contains("结构延续中，观察持续性"));
        assert!(!report.markdown_body.contains("持有为主"));
    }

    #[test]
    fn test_no_trade_pullback_reason_avoids_core_priority_hint() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
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
            assets: vec![AssetActionDecision {
                symbol: "TSLA".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "TSLA".into(),
                    state: AssetState::PULLBACK,
                    ..Default::default()
                },
                position_intent: PositionIntent::HOLD,
                has_position_fact: false,
                ..Default::default()
            }],
            top_tier_symbols: vec!["TSLA".into()],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("回撤结构，观察强度"));
        assert!(!report.markdown_body.contains("核心回撤"));
    }

    #[test]
    fn test_exit_summary_renders_separately_from_no_trade() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
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
            assets: vec![
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    has_position_fact: true,
                    is_core_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "NVDA".into(),
                        state: AssetState::OPTIMAL,
                        ..Default::default()
                    },
                    position_intent: PositionIntent::HOLD,
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "FIG".into(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "FIG".into(),
                        state: AssetState::DEFEND,
                        ..Default::default()
                    },
                    exit_decision: crate::core::exit::ExitDecision {
                        position_intent: PositionIntent::EXIT,
                        asset_exit_state: crate::core::exit::AssetExitState::DefensiveExit,
                        reasons: vec![],
                    },
                    position_intent: PositionIntent::EXIT,
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TSLA".into(),
                    asset_state: AssetStateSnapshot {
                        symbol: "TSLA".into(),
                        state: AssetState::FORMING,
                        ..Default::default()
                    },
                    position_intent: PositionIntent::HOLD,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new())
            .unwrap()
            .markdown_body;

        assert!(card.contains("### 📉 持仓处理建议"));
        assert!(card.contains("- NVDA · 持有"));
        assert!(card.contains("- FIG · 退出"));
        assert!(!card.contains("- NVDA · 卖出"));
        let decision_idx = card.find("### 禁止动作（NO TRADE）").unwrap();
        let exit_idx = card.find("### 📉 持仓处理建议").unwrap();
        let watch_idx = card.find("### 👀 候选观察名单").unwrap();
        assert!(decision_idx < exit_idx);
        assert!(exit_idx < watch_idx);
    }

    #[test]
    fn test_exit_summary_empty_state_keeps_decision_tone() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            participation: crate::core::participation::ParticipationReadiness {
                participation_ready: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new())
            .unwrap()
            .markdown_body;

        assert!(card.contains("### 📉 持仓处理建议"));
        assert!(card.contains("> 当前无持仓，无需处理。"));
        assert!(card.contains("> 未触发任何退出条件。"));
    }

    #[test]
    fn test_breakout_section_renders_evidence_without_buy_language() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "PLTR".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::ConfirmedBreakout,
                        breakout_strength: 81.0,
                        breakout_quality: 77.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TSLA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 33.0,
                        breakout_quality: 39.0,
                        failed_breakout_risk: 66.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                            crate::core::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("### 🚀 突破识别"));
        assert!(report.markdown_body.contains("- PLTR · 结构突破"));
        assert!(report.markdown_body.contains("主动领涨突破"));
        assert!(report.markdown_body.contains("失败风险 66"));
        assert!(!report.markdown_body.contains("BUY"));
        assert!(!report.markdown_body.contains("加仓"));
        assert!(report.telegram_html_body.contains("<b>🚀 突破识别</b>"));
    }

    #[test]
    fn test_breakout_section_renders_ordinary_rebound_as_visible_evidence() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "QQQ".into(),
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                    breakout_strength: 27.0,
                    breakout_quality: 29.0,
                    reasons: vec![crate::core::breakout_detection::BreakoutReason::OrdinaryRebound],
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("### 🚀 突破识别"));
        assert!(report.markdown_body.contains("- QQQ · 无突破"));
        assert!(report.markdown_body.contains("普通反弹"));
        assert!(report.telegram_html_body.contains("<b>🚀 突破识别</b>"));
        assert!(report.telegram_html_body.contains("QQQ · 无突破"));
        assert!(report.telegram_html_body.contains("普通反弹"));
    }

    #[test]
    fn test_breakout_section_renders_all_categories_in_english_final_report() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 24.0,
                        breakout_quality: 28.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TSLA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 35.0,
                        breakout_quality: 42.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::PullbackRepair,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "PLTR".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::ConfirmedBreakout,
                        breakout_strength: 81.0,
                        breakout_quality: 77.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 31.0,
                        breakout_quality: 38.0,
                        failed_breakout_risk: 66.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                            crate::core::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::EnUs);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("### 🚀 Breakout Detection"));
        assert!(report.markdown_body.contains("QQQ · No Breakout"));
        assert!(report.markdown_body.contains("Ordinary rebound"));
        assert!(report.markdown_body.contains("TSLA · No Breakout"));
        assert!(report.markdown_body.contains("Pullback repair"));
        assert!(report.markdown_body.contains("PLTR · Confirmed Breakout"));
        assert!(report.markdown_body.contains("Leadership-style breakout"));
        assert!(report.markdown_body.contains("NVDA · No Breakout"));
        assert!(report.markdown_body.contains("Failure Risk 66"));
        assert!(report
            .telegram_html_body
            .contains("<b>🚀 Breakout Detection</b>"));
        assert!(report.telegram_html_body.contains("QQQ · No Breakout"));
        assert!(report.telegram_html_body.contains("Ordinary rebound"));
        assert!(report.telegram_html_body.contains("Pullback repair"));
        assert!(report
            .telegram_html_body
            .contains("PLTR · Confirmed Breakout"));
        assert!(report
            .telegram_html_body
            .contains("Leadership-style breakout"));
        assert!(report.telegram_html_body.contains("Failure Risk 66"));
    }

    #[test]
    fn test_breakout_section_renders_all_categories_in_japanese_final_report() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 24.0,
                        breakout_quality: 28.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TSLA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 35.0,
                        breakout_quality: 42.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::PullbackRepair,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "PLTR".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::ConfirmedBreakout,
                        breakout_strength: 81.0,
                        breakout_quality: 77.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 31.0,
                        breakout_quality: 38.0,
                        failed_breakout_risk: 66.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                            crate::core::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("### 🚀 突破認識"));
        assert!(report.markdown_body.contains("QQQ · 突破未成立"));
        assert!(report.markdown_body.contains("通常反発"));
        assert!(report.markdown_body.contains("TSLA · 突破未成立"));
        assert!(report.markdown_body.contains("押し目修復"));
        assert!(report.markdown_body.contains("PLTR · 構造的突破"));
        assert!(report.markdown_body.contains("主導突破"));
        assert!(report.markdown_body.contains("NVDA · 突破未成立"));
        assert!(report.markdown_body.contains("失敗リスク 66"));
        assert!(report.telegram_html_body.contains("<b>🚀 突破認識</b>"));
        assert!(report.telegram_html_body.contains("QQQ · 突破未成立"));
        assert!(report.telegram_html_body.contains("通常反発"));
        assert!(report.telegram_html_body.contains("押し目修復"));
        assert!(report.telegram_html_body.contains("PLTR · 構造的突破"));
        assert!(report.telegram_html_body.contains("主導突破"));
        assert!(report.telegram_html_body.contains("失敗リスク 66"));
    }

    #[test]
    fn test_breakout_section_denoises_no_trade_in_chinese() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 27.0,
                        breakout_quality: 29.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TSLA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 35.0,
                        breakout_quality: 42.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::PullbackRepair,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::EmergingBreakout,
                        breakout_strength: 47.0,
                        breakout_quality: 88.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 31.0,
                        breakout_quality: 38.0,
                        failed_breakout_risk: 82.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                            crate::core::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("### 🚀 突破识别"));
        assert!(report.markdown_body.contains("GOOG · 突破萌芽"));
        assert!(report.markdown_body.contains("NVDA · 无突破"));
        assert!(report.markdown_body.contains("假突破风险"));
        assert!(report.markdown_body.contains("失败风险 82"));
        assert!(!report.markdown_body.contains("QQQ · 无突破"));
        assert!(!report.markdown_body.contains("TSLA · 无突破"));
        assert!(!report.markdown_body.contains("普通反弹"));
        assert!(!report.markdown_body.contains("回撤修复"));
        assert!(report.telegram_html_body.contains("GOOG · 突破萌芽"));
        assert!(report.telegram_html_body.contains("NVDA · 无突破"));
        assert!(report.telegram_html_body.contains("假突破风险"));
        assert!(report.telegram_html_body.contains("失败风险 82"));
        assert!(!report.telegram_html_body.contains("QQQ · 无突破"));
        assert!(!report.telegram_html_body.contains("TSLA · 无突破"));
    }

    #[test]
    fn test_breakout_section_denoises_no_trade_in_english() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::EmergingBreakout,
                        breakout_strength: 47.0,
                        breakout_quality: 88.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 82.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                            crate::core::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::EnUs);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("GOOG · Emerging Breakout"));
        assert!(report.markdown_body.contains("NVDA · No Breakout"));
        assert!(report.markdown_body.contains("Failure Risk"));
        assert!(report.markdown_body.contains("Failure Risk 82"));
        assert!(!report.markdown_body.contains("QQQ · No Breakout"));
        assert!(!report.markdown_body.contains("Ordinary rebound"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · Emerging Breakout"));
        assert!(report.telegram_html_body.contains("NVDA · No Breakout"));
        assert!(report.telegram_html_body.contains("Failure Risk 82"));
        assert!(!report.telegram_html_body.contains("QQQ · No Breakout"));
    }

    #[test]
    fn test_breakout_section_denoises_no_trade_in_japanese() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::EmergingBreakout,
                        breakout_strength: 47.0,
                        breakout_quality: 88.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 82.0,
                        reasons: vec![
                            crate::core::breakout_detection::BreakoutReason::OrdinaryRebound,
                            crate::core::breakout_detection::BreakoutReason::FailedBreakoutRisk,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("GOOG · 突破初動"));
        assert!(report.markdown_body.contains("NVDA · 突破未成立"));
        assert!(report.markdown_body.contains("失敗リスク 82"));
        assert!(!report.markdown_body.contains("QQQ · 突破未成立"));
        assert!(!report.markdown_body.contains("通常反発"));
        assert!(report.telegram_html_body.contains("GOOG · 突破初動"));
        assert!(report.telegram_html_body.contains("NVDA · 突破未成立"));
        assert!(report.telegram_html_body.contains("失敗リスク 82"));
        assert!(!report.telegram_html_body.contains("QQQ · 突破未成立"));
    }
}
