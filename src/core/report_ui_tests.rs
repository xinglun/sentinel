use crate::config::{AppConfig, OutputConfig, RulesConfig, TrendCohesionRulesConfig, TrendConfig};
use crate::core::action_matrix::AssetActionDecision;
use crate::core::asset_state::{AssetState, AssetStateSnapshot};
use crate::core::decision::DecisionPacket;
use crate::core::exit::PositionIntent;
use crate::core::market_regime::{MarketRegimeSnapshot, MarketState, RiskOverlay};
use crate::core::presentation_assembler::PresentationAssembler;
use crate::core::report::generate_refined_report;
use chrono::{NaiveDate, Utc};
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
            compact_transition_evidence_in_no_trade: true,
        },
        telegram: None,
        futu: None,
        finnhub: None,
        sec: None,
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
    use crate::core::i18n::{get_dictionary, Language};
    use crate::core::transition_log::StateTransitionLog;
    use crate::core::trend_cohesion::{TrendCohesionGateCondition, TrendCohesionSnapshot};
    use std::fs;
    use std::path::PathBuf;

    fn no_trade_snapshot_packet() -> DecisionPacket {
        DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status: crate::core::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::core::trend_cohesion::TrendCohesionTopology::NoLeader,
                stability_score: 1.1,
                continuity_streak: 1,
                unmet_conditions: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion,
                ],
                ..Default::default()
            },
            market_features: crate::core::features::MarketFeatures {
                system_confidence: 52.0,
                stability_score: 1.1,
                regime_age: 1,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "GOOG".into(),
                    state: AssetState::FORMING,
                    ..Default::default()
                },
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::EmergingBreakout,
                    breakout_strength: 62.0,
                    breakout_quality: 100.0,
                    reasons: vec![
                        crate::core::breakout_detection::BreakoutReason::StructuralBreakout,
                    ],
                    ..Default::default()
                },
                position_intent: PositionIntent::HOLD,
                has_position_fact: false,
                ..Default::default()
            }],
            top_tier_symbols: vec!["GOOG".into()],
            ..Default::default()
        }
    }

    fn build_no_trade_report(
        language: crate::core::i18n::Language,
    ) -> crate::core::report::ReportResult {
        let config = mock_config_with_language(language);
        let pres = PresentationAssembler::assemble(
            &no_trade_snapshot_packet(),
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            language,
        );
        generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap()
    }

    fn build_no_trade_transition_report(
        language: crate::core::i18n::Language,
    ) -> crate::core::report::ReportResult {
        let curr = no_trade_transition_order_packet();
        let config = mock_config_with_language(language);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            language,
        );
        generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap()
    }

    fn no_trade_transition_order_packet() -> DecisionPacket {
        let prev = no_trade_snapshot_packet();
        let mut curr = no_trade_snapshot_packet();
        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));
        curr
    }

    fn no_trade_transition_reason_diff_packet() -> DecisionPacket {
        let prev = DecisionPacket {
            trend_cohesion: TrendCohesionSnapshot {
                unmet_conditions: vec![
                    TrendCohesionGateCondition::StabilityThreshold,
                    TrendCohesionGateCondition::ContinuityThreshold,
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: TrendCohesionSnapshot {
                unmet_conditions: vec![
                    TrendCohesionGateCondition::ContinuityThreshold,
                    TrendCohesionGateCondition::DirectionalCohesion,
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));
        curr
    }

    fn assert_no_trade_html_execution_order(language: Language, html: &str) {
        let dict = get_dictionary(language);
        let decision_marker = format!("<b>{}</b>", dict.decision.no_trade);
        let breakout_marker = format!("<b>{}</b>", dict.headers.breakout_detection);
        let watch_marker = format!("<b>{}</b>", dict.decision.candidate_watchlist);
        let transition_marker = format!("<b>{}</b>", dict.transition_evidence.title);

        let decision_idx = html.find(&decision_marker).unwrap();
        let breakout_idx = html.find(&breakout_marker).unwrap();
        let watch_idx = html.find(&watch_marker).unwrap();
        let transition_idx = html.find(&transition_marker).unwrap();
        assert!(decision_idx < breakout_idx);
        assert!(breakout_idx < watch_idx);
        assert!(watch_idx < transition_idx);
    }

    fn snapshot_path(file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/core/snapshots")
            .join(file_name)
    }

    fn assert_snapshot(file_name: &str, actual: &str) {
        let path = snapshot_path(file_name);
        let normalized_actual = actual.replace("\r\n", "\n");
        let update = std::env::var("UPDATE_SNAPSHOTS").as_deref() == Ok("1");

        if update {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, normalized_actual).unwrap();
            return;
        }

        let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "snapshot file missing: {} (run with UPDATE_SNAPSHOTS=1 once)",
                path.display()
            )
        });
        assert_eq!(expected.replace("\r\n", "\n"), normalized_actual);
    }

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
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status: crate::core::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::core::trend_cohesion::TrendCohesionTopology::NoLeader,
                stability_score: 1.1,
                continuity_streak: 1,
                unmet_conditions: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion,
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
    fn test_compact_no_trade_reasons_do_not_render_na_ratios_on_data_missing() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "AAPL".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec!["AAPL".to_string()],
            crate::core::i18n::Language::ZhCn,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(!report.markdown_body.contains("N/A/10"));
        assert!(!report.markdown_body.contains("N/A/3"));
        assert!(!report.telegram_html_body.contains("N/A/10"));
        assert!(!report.telegram_html_body.contains("N/A/3"));
    }

    #[test]
    fn test_compact_no_trade_reasons_use_configured_thresholds() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status: crate::core::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::core::trend_cohesion::TrendCohesionTopology::NoLeader,
                stability_score: 7.5,
                continuity_streak: 1,
                unmet_conditions: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::core::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion,
                ],
                ..Default::default()
            },
            market_features: crate::core::features::MarketFeatures {
                stability_score: 7.5,
                regime_age: 1,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "AAPL".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        config.rules.trend_cohesion = Some(TrendCohesionRulesConfig {
            gate_stability_threshold: Some(11.0),
            gate_continuity_threshold: Some(4),
            ..Default::default()
        });

        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            crate::core::i18n::Language::ZhCn,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        assert!(report.markdown_body.contains("稳定性 7.5/11"));
        assert!(report.markdown_body.contains("连续性 1/4"));
        assert!(report.telegram_html_body.contains("稳定性 7.5/11"));
        assert!(report.telegram_html_body.contains("连续性 1/4"));
    }

    #[test]
    fn test_legacy_threshold_template_matcher_table_driven() {
        struct Case {
            reason: &'static str,
            unmet: Vec<crate::core::trend_cohesion::TrendCohesionGateCondition>,
            should_suppress: bool,
        }

        // Compatibility matcher matrix (input -> expected display):
        // - Canonical threshold templates should be suppressed when matching evidence exists.
        // - Symbol variants (< / ＜ / ≤ / ≦) should behave consistently.
        // - Non-template or prefixed free text should never be over-suppressed.
        let cases = vec![
            Case {
                reason: "稳定性不足（< 10.0）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "稳定性不足（＜10.0）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "稳定性不足（≤10.0）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "安定性不足(≦10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "连续性不足（1d < 3d）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "連続性不足（1日 ≤ 3日）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= threshold (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "STABILITY SCORE(8.0)<=THRESHOLD(10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= THR (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= threshold (10.0).",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= threshold (10.0)!",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) ＜ threshold (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) ＜ threshold (10.0)。",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) ＜＝ threshold (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <＝ threshold (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "稳定性不足（<10.0）。",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "連続性不足（1日≤3日）！",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "备注：稳定性不足但先观察，不立即处理",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "连续性不足（等待确认）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "盘中提示：稳定性不足（<10.0）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "稳定性不足但需结合成交量<均值判断",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "稳定性不足（需结合成交量变化）并结合<均值判断",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "稳定性不足（备注<阈值，先观察）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "連続性不足（出来高≤基準なので様子見）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "連続性不足（注記≤閾値で監視）",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (comment: < avg volume) keep watching",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (note) <= threshold (watch)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (8.0) <= thrash (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (8.0) = threshold (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (8.0) <== threshold (10.0)",
                unmet: vec![
                    crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
        ];

        for case in cases {
            let packet = DecisionPacket {
                date: Utc::now().date_naive(),
                market_regime: MarketRegimeSnapshot {
                    market_state: MarketState::IGNITION,
                    risk_overlay: RiskOverlay::NORMAL,
                    reasons: vec![case.reason.to_string()],
                    ..Default::default()
                },
                trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                    gate_passed: false,
                    status: crate::core::trend_cohesion::TrendCohesionStatus::Dispersed,
                    topology: crate::core::trend_cohesion::TrendCohesionTopology::NoLeader,
                    stability_score: 8.0,
                    continuity_streak: 1,
                    unmet_conditions: case.unmet,
                    ..Default::default()
                },
                assets: vec![AssetActionDecision {
                    symbol: "AAPL".into(),
                    ..Default::default()
                }],
                ..Default::default()
            };

            let mut config = mock_config_with_language(crate::core::i18n::Language::ZhCn);
            config.output.compact_transition_evidence_in_no_trade = false;
            let pres = PresentationAssembler::assemble(
                &packet,
                &config.get_parsed_rules(),
                &HashMap::new(),
                vec![],
                crate::core::i18n::Language::ZhCn,
            );
            let report =
                generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new())
                    .unwrap();

            if case.should_suppress {
                assert!(
                    !report.markdown_body.contains(case.reason),
                    "expected suppression for reason: {}",
                    case.reason
                );
            } else {
                assert!(
                    report.markdown_body.contains(case.reason),
                    "expected preserve for reason: {}",
                    case.reason
                );
            }
        }
    }

    fn report_markdown_contains_reason(
        reason: &str,
        unmet: Vec<crate::core::trend_cohesion::TrendCohesionGateCondition>,
        language: crate::core::i18n::Language,
    ) -> bool {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                reasons: vec![reason.to_string()],
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status: crate::core::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::core::trend_cohesion::TrendCohesionTopology::NoLeader,
                stability_score: 8.0,
                continuity_streak: 1,
                unmet_conditions: unmet,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "AAPL".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut config = mock_config_with_language(language);
        config.output.compact_transition_evidence_in_no_trade = false;
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            language,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();
        report.markdown_body.contains(reason)
    }

    #[test]
    fn test_legacy_threshold_matcher_property_like_valid_templates_suppress() {
        let comparators = [
            "<",
            "<=",
            "＜",
            "＜=",
            "＜＝",
            "<＝",
            "≤",
            "≦",
            "below",
            "under",
            "less than",
        ];
        let tokens = ["threshold", "thresh", "thr", "limit"];
        let punctuations = ["", ".", "!", "。", "！", "？"];
        for comparator in comparators {
            for token in tokens {
                for punct in punctuations {
                    let reason = format!(
                        "Stability score (8.0) {} {} (10.0){}",
                        comparator, token, punct
                    );
                    assert!(
                        !report_markdown_contains_reason(
                            &reason,
                            vec![
                                crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                            ],
                            crate::core::i18n::Language::ZhCn,
                        ),
                        "expected suppression for generated english template: {}",
                        reason
                    );
                }
            }
        }

        let localized_cases = vec![
            (
                "稳定性不足",
                "<10.0",
                crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
            ),
            (
                "安定性不足",
                "<10.0",
                crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
            ),
            (
                "连续性不足",
                "1d<3d",
                crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
            ),
            (
                "連続性不足",
                "1日≤3日",
                crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
            ),
        ];
        for (prefix, payload, gate) in localized_cases {
            for punct in punctuations {
                let reason = format!("{}（{}）{}", prefix, payload, punct);
                assert!(
                    !report_markdown_contains_reason(
                        &reason,
                        vec![gate],
                        crate::core::i18n::Language::ZhCn,
                    ),
                    "expected suppression for generated localized template: {}",
                    reason
                );
            }
        }
    }

    #[test]
    fn test_legacy_threshold_matcher_property_like_custom_or_invalid_templates_preserved() {
        let invalid_english = vec![
            "Stability score (8.0) = threshold (10.0)",
            "Stability score (8.0) == threshold (10.0)",
            "Stability score (8.0) <== threshold (10.0)",
            "Stability score (8.0) ‹ threshold (10.0)",
            "Stability score (8.0) ❮ threshold (10.0)",
            "Stability score (8.0) > threshold (10.0)",
            "Stability score (8.0) ＞ threshold (10.0)",
            "Stability score (8.0) <= threshold注释 (10.0)",
            "Stability score (8.0) <= thr備考 (10.0)",
            "Stability score (8.0) <= thrash (10.0)",
            "Stability score (note) <= threshold (watch)",
        ];
        for reason in invalid_english {
            assert!(
                report_markdown_contains_reason(
                    reason,
                    vec![
                        crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    ],
                    crate::core::i18n::Language::ZhCn,
                ),
                "expected preserve for invalid/custom english reason: {}",
                reason
            );
        }

        let custom_localized = vec![
            "稳定性不足（备注<阈值，先观察）",
            "連続性不足（注記≤閾値で監視）",
            "稳定性不足（等待确认）",
        ];
        for reason in custom_localized {
            assert!(
                report_markdown_contains_reason(
                    reason,
                    vec![
                        crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                        crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    ],
                    crate::core::i18n::Language::ZhCn,
                ),
                "expected preserve for custom localized reason: {}",
                reason
            );
        }
    }

    #[test]
    fn test_legacy_threshold_matcher_property_like_multilang_smoke() {
        let suppress_reason = "Stability score (8.0) < threshold (10.0)";
        let preserve_reason = "Stability score (note) < threshold (watch)";
        let unmet =
            vec![crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold];

        for language in [
            crate::core::i18n::Language::EnUs,
            crate::core::i18n::Language::JaJp,
        ] {
            assert!(
                !report_markdown_contains_reason(suppress_reason, unmet.clone(), language),
                "expected suppression in language {:?}",
                language
            );
            assert!(
                report_markdown_contains_reason(preserve_reason, unmet.clone(), language),
                "expected preservation in language {:?}",
                language
            );
        }
    }

    #[test]
    fn test_legacy_threshold_matcher_fuzz_like_randomized_variants() {
        fn next_u64(state: &mut u64) -> u64 {
            // xorshift64* (deterministic, fast, no extra deps)
            *state ^= *state >> 12;
            *state ^= *state << 25;
            *state ^= *state >> 27;
            state.wrapping_mul(0x2545F4914F6CDD1D)
        }
        fn pick<'a>(state: &mut u64, items: &'a [&'a str]) -> &'a str {
            let idx = (next_u64(state) as usize) % items.len();
            items[idx]
        }

        let comparators_valid = [
            "<",
            "<=",
            "<＝",
            "＜",
            "＜=",
            "＜＝",
            "≤",
            "≦",
            "⩽",
            "﹤",
            "below",
            "under",
            "less than",
        ];
        let tokens_valid = ["threshold", "thresh", "thr", "limit"];
        let suffixes = ["", ".", "!", "?", "。", "！", "？", "｡", "﹒", "﹗", "﹖"];
        let spaces = ["", " ", "\t", "\n", "\u{3000}"];

        let mut seed = 0xC0FFEE_u64;
        let families = vec![
            (
                "Stability score",
                "(8.0)",
                "(10.0)",
                vec![crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold],
            ),
            (
                "Core Tier streak",
                "(1)",
                "(3)",
                vec![crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold],
            ),
        ];
        for (prefix, lhs, rhs, unmet) in families {
            for _ in 0..120 {
                let cmp = pick(&mut seed, &comparators_valid);
                let tok = pick(&mut seed, &tokens_valid);
                let suf = pick(&mut seed, &suffixes);
                let s1 = pick(&mut seed, &spaces);
                let s2 = pick(&mut seed, &spaces);
                let s3 = pick(&mut seed, &spaces);
                let s4 = pick(&mut seed, &spaces);
                let s5 = pick(&mut seed, &spaces);

                let reason = format!(
                    "{prefix}{s1}{lhs}{s2}{cmp}{s3}{tok}{s4}{rhs}{s5}{suf}",
                    prefix = prefix,
                    lhs = lhs,
                    rhs = rhs,
                    s1 = s1,
                    s2 = s2,
                    cmp = cmp,
                    s3 = s3,
                    tok = tok,
                    s4 = s4,
                    s5 = s5,
                    suf = suf
                );
                assert!(
                    !report_markdown_contains_reason(
                        &reason,
                        unmet.clone(),
                        crate::core::i18n::Language::ZhCn,
                    ),
                    "expected suppression for randomized template: {}",
                    reason
                );
            }
        }

        let comparators_invalid = ["=", "==", "<==", ">", "＞", ">=", "＞＝"];
        let tokens_invalid = [
            "thrash",
            "watch",
            "comment",
            "memo",
            "threshold注释",
            "thr備考",
        ];
        let invalid_families = vec![
            (
                "Stability score",
                "(8.0)",
                "(10.0)",
                vec![crate::core::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold],
            ),
            (
                "Core Tier streak",
                "(1)",
                "(3)",
                vec![crate::core::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold],
            ),
        ];
        for (prefix, lhs, rhs, unmet) in invalid_families {
            for _ in 0..90 {
                let cmp = pick(&mut seed, &comparators_invalid);
                let tok = pick(&mut seed, &tokens_invalid);
                let suf = pick(&mut seed, &suffixes);
                let reason = format!("{prefix} {lhs} {cmp} {tok} {rhs}{suf}");
                assert!(
                    report_markdown_contains_reason(
                        &reason,
                        unmet.clone(),
                        crate::core::i18n::Language::ZhCn,
                    ),
                    "expected preserve for randomized non-template: {}",
                    reason
                );
            }
        }
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
                status: crate::core::trend_cohesion::TrendCohesionStatus::Dispersed,
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
            },            market_features: crate::core::features::MarketFeatures {
                stability_score: 7.5,
                regime_age: 1,
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

        assert!(card.contains("未就绪原因"));
        assert!(card.contains("主线结构"));
        assert!(card.contains("无主线"));
        let parsed_rules = config.get_parsed_rules();
        let stability_threshold = if (parsed_rules.trend_cohesion.gate_stability_threshold
            - parsed_rules.trend_cohesion.gate_stability_threshold.round())
        .abs()
            < f64::EPSILON
        {
            format!(
                "{:.0}",
                parsed_rules.trend_cohesion.gate_stability_threshold
            )
        } else {
            parsed_rules
                .trend_cohesion
                .gate_stability_threshold
                .to_string()
        };
        assert!(card.contains(&format!("稳定性 7.5/{}", stability_threshold)));
        assert!(card.contains(&format!(
            "连续性 1/{}",
            parsed_rules.trend_cohesion.gate_continuity_threshold
        )));
        assert!(!card.contains("主线形成条件"));
        assert!(!card.contains("当前未满足项"));
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
    fn test_no_trade_optimal_candidate_uses_candidate_qualified_label() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "SPY".into(),
                asset_state: AssetStateSnapshot {
                    symbol: "SPY".into(),
                    state: AssetState::OPTIMAL,
                    ..Default::default()
                },
                position_intent: PositionIntent::ADD,
                has_position_fact: false,
                ..Default::default()
            }],
            top_tier_symbols: vec!["SPY".into()],
            ..Default::default()
        };

        let config_zh = mock_config_with_language(crate::core::i18n::Language::ZhCn);
        let pres_zh = PresentationAssembler::assemble(
            &packet,
            &config_zh.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            crate::core::i18n::Language::ZhCn,
        );
        let report_zh =
            generate_refined_report(&config_zh, &pres_zh, 0.0, &HashMap::new(), &HashMap::new())
                .unwrap();
        assert!(report_zh
            .telegram_html_body
            .contains("SPY · 最优 (候选标的)"));

        let config_en = mock_config_with_language(crate::core::i18n::Language::EnUs);
        let pres_en = PresentationAssembler::assemble(
            &packet,
            &config_en.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            crate::core::i18n::Language::EnUs,
        );
        let report_en =
            generate_refined_report(&config_en, &pres_en, 0.0, &HashMap::new(), &HashMap::new())
                .unwrap();
        assert!(report_en
            .telegram_html_body
            .contains("SPY · Optimal (Candidate)"));

        let config_ja = mock_config_with_language(crate::core::i18n::Language::JaJp);
        let pres_ja = PresentationAssembler::assemble(
            &packet,
            &config_ja.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            crate::core::i18n::Language::JaJp,
        );
        let report_ja =
            generate_refined_report(&config_ja, &pres_ja, 0.0, &HashMap::new(), &HashMap::new())
                .unwrap();
        assert!(report_ja.telegram_html_body.contains("SPY · 最適 (候補)"));
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
        assert!(report.markdown_body.contains("GOOG · 突破萌芽（第1天）"));
        assert!(report.markdown_body.contains("NVDA · 无突破"));
        assert!(report.markdown_body.contains("假突破风险"));
        assert!(report.markdown_body.contains("失败风险 82"));
        assert!(!report.markdown_body.contains("QQQ · 无突破"));
        assert!(!report.markdown_body.contains("TSLA · 无突破"));
        assert!(!report.markdown_body.contains("普通反弹"));
        assert!(!report.markdown_body.contains("回撤修复"));
        assert!(report.telegram_html_body.contains("GOOG · 突破萌芽"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · 突破萌芽（第1天）"));
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
        assert!(report
            .markdown_body
            .contains("GOOG · Emerging Breakout (Day 1)"));
        assert!(report.markdown_body.contains("NVDA · No Breakout"));
        assert!(report.markdown_body.contains("Failure Risk"));
        assert!(report.markdown_body.contains("Failure Risk 82"));
        assert!(!report.markdown_body.contains("QQQ · No Breakout"));
        assert!(!report.markdown_body.contains("Ordinary rebound"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · Emerging Breakout"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · Emerging Breakout (Day 1)"));
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
        assert!(report.markdown_body.contains("GOOG · 突破初動（1日目）"));
        assert!(report.markdown_body.contains("NVDA · 突破未成立"));
        assert!(report.markdown_body.contains("失敗リスク 82"));
        assert!(!report.markdown_body.contains("QQQ · 突破未成立"));
        assert!(!report.markdown_body.contains("通常反発"));
        assert!(report.telegram_html_body.contains("GOOG · 突破初動"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · 突破初動（1日目）"));
        assert!(report.telegram_html_body.contains("NVDA · 突破未成立"));
        assert!(report.telegram_html_body.contains("失敗リスク 82"));
        assert!(!report.telegram_html_body.contains("QQQ · 突破未成立"));
    }
    #[test]
    fn test_transition_evidence_rendering_zh_cn() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;

        let prev = DecisionPacket {
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                risk_overlay: RiskOverlay::BROKEN,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // Compute transition log
        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let md = report.archival_markdown;
        // Verify localized section title
        assert!(md.contains("🔄 状态转移证据"));
        // Verify localized state change (Ignition -> 启动期, Defensive -> 保命期)
        assert!(md.contains("保命期 -> 启动期"));
        assert!(md.contains("结构防御 -> 风险正常"));
        assert!(md.contains("未达标 -> 达标"));
        // Ensure NO debug enum strings are present
        assert!(!md.contains("DEFENSIVE"));
        assert!(!md.contains("IGNITION"));
    }

    #[test]
    fn test_trend_recognition_report_rendering_zh_cn() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;
        use crate::core::trend_cohesion::{
            AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
            TrendContinuationState, TrendRecognitionEvidence,
        };

        let mut curr = DecisionPacket::default();
        curr.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::LeaderConfirmedFollowersLagging,
            diffusion_score: 0.45,
            conviction_score: 0.0,
            lag_state: true,
            single_asset_decay_day: 3,
            single_asset_decay_max: 5,
            substantive: Some(SubstantiveEvidence {
                records: vec![AutomatedEvidenceRecord {
                    source: EvidenceSourceType::OfficialIR,
                    evidence_type: EvidenceType::EarningsValidation,
                    confidence: 0.95,
                    description: "Earnings beat expectations by 15%".to_string(),
                    event_date: "2026-04-22".to_string(),
                    symbol: Some("GOOG".to_string()),
                    source_url: Some("https://example.com/ir/goog".to_string()),
                    dedupe_key: "test:goog:earnings:2026-04-22".to_string(),
                }],
                earnings_validation: true,
                ..Default::default()
            }),
        });

        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let md = report.archival_markdown;
        assert!(md.contains("🎯 趋势特征识别"));
        assert!(md.contains("进展阶段: 单点确立/整体滞后"));
        assert!(md.contains("扩散度: 0.45"));
        assert!(md.contains("滞后预警: 先行成立・追随迟缓"));
        assert!(md.contains("单极突破衰减: 3/5"));
        assert!(md.contains("[2026-04-22] [EarningsValidation]"));
        assert!(md.contains("Earnings beat expectations by 15%"));
        assert!(md.contains("https://example.com/ir/goog"));

        let html = report.telegram_html_body;
        assert!(html.contains("🎯 趋势特征识别"));
        assert!(html.contains("<i>进展阶段: 单点确立/整体滞后</i>"));
        assert!(html.contains("实质性证据: 业绩实质性确认"));
        assert!(!html.contains("[2026-04-22] [EarningsValidation]"));
        assert!(!html.contains("https://example.com/ir/goog"));
        // Verify that the transition evidence block is rendered even if it's just trend recognition
        assert!(md.contains("🔄 状态转移证据"));
    }

    #[test]
    fn test_trend_recognition_report_rendering_ja_jp() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;
        use crate::core::trend_cohesion::{TrendContinuationState, TrendRecognitionEvidence};

        let mut curr = DecisionPacket::default();
        curr.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::Broadening,
            diffusion_score: 0.65,
            conviction_score: 0.0,
            lag_state: false,
            single_asset_decay_day: 0,
            single_asset_decay_max: 5,
            substantive: None,
        });

        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        let config = mock_config_with_language(Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let md = report.archival_markdown;
        assert!(md.contains("🎯 トレンド特徴認識"));
        assert!(md.contains("進行段階: 拡散初期"));
        assert!(md.contains("ブレイクアウト拡散度: 0.65"));
        assert!(!md.contains("追随遅延")); // lag_state is false, should not appear in simplified output if logic holds
        assert!(md.contains("単独突破の連続日数: 0/5"));
        // Verify that the transition evidence block is rendered even if it's just trend recognition
        assert!(md.contains("🔄 状態遷移エビデンス"));
    }

    #[test]
    fn test_trend_recognition_substantive_details_render_in_en_and_ja() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;
        use crate::core::trend_cohesion::{
            AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
            TrendContinuationState, TrendRecognitionEvidence,
        };

        let mut curr = DecisionPacket::default();
        curr.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::Broadening,
            diffusion_score: 0.65,
            conviction_score: 0.95,
            lag_state: false,
            single_asset_decay_day: 0,
            single_asset_decay_max: 5,
            substantive: Some(SubstantiveEvidence {
                records: vec![AutomatedEvidenceRecord {
                    source: EvidenceSourceType::OfficialIR,
                    evidence_type: EvidenceType::EarningsValidation,
                    confidence: 0.95,
                    description: "Earnings beat expectations by 15%".to_string(),
                    event_date: "2026-04-22".to_string(),
                    symbol: Some("GOOG".to_string()),
                    source_url: Some("https://example.com/ir/goog".to_string()),
                    dedupe_key: "test:goog:earnings:2026-04-22".to_string(),
                }],
                earnings_validation: true,
                ..Default::default()
            }),
        });
        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        for language in [Language::EnUs, Language::JaJp] {
            let config = mock_config_with_language(language);
            let pres = PresentationAssembler::assemble(
                &curr,
                &config.get_parsed_rules(),
                &HashMap::new(),
                vec![],
                language,
            );
            let report =
                generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new())
                    .unwrap();

            assert!(report.telegram_html_body.contains("Earnings Quality"));
            assert!(!report
                .telegram_html_body
                .contains("[2026-04-22] [EarningsValidation]"));
            assert!(!report
                .telegram_html_body
                .contains("Earnings beat expectations by 15%"));
            assert!(!report
                .telegram_html_body
                .contains("https://example.com/ir/goog"));
            assert!(report
                .archival_markdown
                .contains("[2026-04-22] [EarningsValidation]"));
            assert!(report
                .archival_markdown
                .contains("Earnings beat expectations by 15%"));
            assert!(report
                .archival_markdown
                .contains("https://example.com/ir/goog"));
        }
    }

    #[test]
    fn test_no_trade_persistence_explanation_en_us() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;

        let prev = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            ..Default::default()
        };

        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let md = report.archival_markdown;
        assert!(md.contains("🔄 State Transition Evidence"));
        // Verify persistent explanation
        assert!(md.contains("NO TRADE Persists"));
    }

    #[test]
    fn test_transition_evidence_rendering_ja_jp() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;

        let prev = DecisionPacket {
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                risk_overlay: RiskOverlay::BROKEN,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                status: crate::core::trend_cohesion::TrendCohesionStatus::Dispersed,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                status: crate::core::trend_cohesion::TrendCohesionStatus::Forming,
                ..Default::default()
            },
            ..Default::default()
        };

        // Compute transition log
        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let md = report.archival_markdown;
        // Verify localized section title
        assert!(md.contains("🔄 状態遷移エビデンス"));
        // Verify localized state change
        // defensive -> ignition maps to 守備期 -> 始動期
        assert!(md.contains("守備期 -> 始動期"));
        assert!(md.contains("分散 -> 形成中"));
        // Verify topology change is rendered
        assert!(
            md.contains("主導不在 -> 形成中")
                || md.contains("主線構造の変化")
                || md.contains("主導不在")
        );
    }

    #[test]
    fn test_transition_evidence_breakout_changes_focus_on_structural_deltas() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;

        let prev = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "U".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::EmergingBreakout,
                        failed_breakout_risk: 61.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 82.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "U".into(),
                    breakout: crate::core::breakout_detection::BreakoutSnapshot {
                        status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 73.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();
        let md = report.archival_markdown;

        assert!(md.contains("**关键变化**"));
        assert!(md.contains("GOOG：新增突破萌芽"));
        assert!(md.contains("其余资产：无结构变化"));
    }

    #[test]
    fn test_transition_evidence_not_shown_for_breakout_risk_only_change() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;

        let prev = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "NVDA".into(),
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                    failed_breakout_risk: 10.0,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "NVDA".into(),
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                    failed_breakout_risk: 82.0,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();
        let md = report.archival_markdown;
        assert!(md.contains("🔄 状态转移证据"));
        assert!(!md.contains("关键变化"));
        assert!(!md.contains("NVDA："));
    }

    #[test]
    fn test_transition_evidence_renders_scout_status_only_in_transition_block() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;

        let prev = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::EmergingBreakout,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();
        let html = report.telegram_html_body;

        assert!(html.contains("<b>🔄 状态转移证据</b>"));
        assert!(html.contains("侦察状态"));
        assert!(html.contains("breakout 连续性: 1/3"));
        assert!(html.contains("扩散: 无（单资产）"));
        assert!(html.contains("reset: 否"));
    }

    #[test]
    fn test_transition_evidence_renders_scout_status_in_en_and_ja() {
        use crate::core::i18n::Language;
        use crate::core::transition_log::StateTransitionLog;

        let prev = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::NoBreakout,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::core::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::core::breakout_detection::BreakoutSnapshot {
                    status: crate::core::breakout_detection::BreakoutStatus::EmergingBreakout,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config_en = mock_config_with_language(Language::EnUs);
        let pres_en = PresentationAssembler::assemble(
            &curr,
            &config_en.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        let report_en =
            generate_refined_report(&config_en, &pres_en, 0.0, &HashMap::new(), &HashMap::new())
                .unwrap();
        let html_en = report_en.telegram_html_body;
        assert!(html_en.contains("<b>🔄 State Transition Evidence</b>"));
        assert!(html_en.contains("Scout Status"));
        assert!(html_en.contains("breakout continuity: 1/3"));
        assert!(html_en.contains("expansion: none (single asset)"));
        assert!(html_en.contains("reset: no"));

        let config_ja = mock_config_with_language(Language::JaJp);
        let pres_ja = PresentationAssembler::assemble(
            &curr,
            &config_ja.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );
        let report_ja =
            generate_refined_report(&config_ja, &pres_ja, 0.0, &HashMap::new(), &HashMap::new())
                .unwrap();
        let html_ja = report_ja.telegram_html_body;
        assert!(html_ja.contains("<b>🔄 状態遷移エビデンス</b>"));
        assert!(html_ja.contains("偵察状態"));
        assert!(html_ja.contains("breakout 連続性: 1/3"));
        assert!(html_ja.contains("拡散: なし（単一資産）"));
        assert!(html_ja.contains("reset: なし"));
    }

    #[test]
    fn test_audit_grade_reason_diff_rendering() {
        let language = Language::ZhCn;
        let curr = no_trade_transition_reason_diff_packet();
        let config = mock_config_with_language(language);
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            language,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let md_compact = report.markdown_body.clone();
        let html_compact = report.telegram_html_body.clone();
        let md = report.archival_markdown;

        // Cohesion Gate Checks (Chinese)
        assert!(md.contains("新增阻碍"));
        assert!(md.contains("已消除阻碍"));
        assert!(md.contains("持续阻碍"));

        // Trend Gate Checks (Chinese)
        // ContinuityThreshold is persisting -> 核心资产持续性不足
        // StabilityThreshold is resolved -> 市场稳定性不足
        // DirectionalCohesion is added -> 主导方向分散或领导者缺失
        assert!(md.contains("新增阻碍: 主导方向分散或领导者缺失"));
        assert!(md.contains("已消除阻碍: 市场稳定度不足"));
        assert!(md.contains("持续阻碍: 核心资产持续性不足"));
        assert!(!md.contains("{:.0}"));
        assert!(!md.contains("当前: {} 天"));
        assert!(!md.contains("当前: {} 只"));
        assert!(!md.contains("领涨: {} 只"));

        // User-facing markdown should keep transition evidence compact under NO TRADE.
        assert!(md_compact.contains("🔄 状态转移证据"));
        assert!(!md_compact.contains("新增阻碍因素: C"));
        assert!(md_compact.contains("未就绪原因"));
        let parsed_rules = config.get_parsed_rules();
        let stability_threshold = if (parsed_rules.trend_cohesion.gate_stability_threshold
            - parsed_rules.trend_cohesion.gate_stability_threshold.round())
        .abs()
            < f64::EPSILON
        {
            format!(
                "{:.0}",
                parsed_rules.trend_cohesion.gate_stability_threshold
            )
        } else {
            parsed_rules
                .trend_cohesion
                .gate_stability_threshold
                .to_string()
        };
        assert!(md_compact.contains(&format!("/{}", stability_threshold)));
        assert!(md_compact.contains(&format!(
            "/{}",
            parsed_rules.trend_cohesion.gate_continuity_threshold
        )));
        let decision_idx = md_compact.find("### 禁止动作（NO TRADE）").unwrap();
        let breakout_idx = md_compact.find("### 🚀 突破识别").unwrap();
        let watch_idx = md_compact.find("### 👀 候选观察名单").unwrap();
        let transition_idx = md_compact.find("### 🔄 状态转移证据").unwrap();
        assert!(decision_idx < breakout_idx);
        assert!(breakout_idx < watch_idx);
        assert!(watch_idx < transition_idx);

        // Telegram HTML should preserve the same execution-first ordering.
        assert_no_trade_html_execution_order(Language::ZhCn, &html_compact);
    }

    #[test]
    fn test_transition_evidence_can_be_expanded_in_no_trade_via_config() {
        let curr = no_trade_transition_reason_diff_packet();
        let mut config = mock_config_with_language(Language::ZhCn);
        config.output.compact_transition_evidence_in_no_trade = false;
        let pres = PresentationAssembler::assemble(
            &curr,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );
        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();

        let md = report.markdown_body;
        assert!(md.contains("新增阻碍: 主导方向分散或领导者缺失"));
    }

    #[test]
    fn test_no_trade_snapshot_zh_cn_markdown() {
        let report = build_no_trade_report(crate::core::i18n::Language::ZhCn);
        assert_snapshot("no_trade_zh_cn.md", &report.markdown_body);
    }

    #[test]
    fn test_no_trade_snapshot_zh_cn_html() {
        let report = build_no_trade_report(crate::core::i18n::Language::ZhCn);
        assert_snapshot("no_trade_zh_cn.html.txt", &report.telegram_html_body);
    }

    #[test]
    fn test_no_trade_snapshot_breakout_age_displays_day_one_in_zh_cn() {
        let report = build_no_trade_report(crate::core::i18n::Language::ZhCn);
        assert!(report.markdown_body.contains("GOOG · 突破萌芽（第1天）"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · 突破萌芽（第1天）"));
    }

    #[test]
    fn test_no_trade_snapshot_breakout_age_displays_day_one_in_en_us() {
        let report = build_no_trade_report(crate::core::i18n::Language::EnUs);
        assert!(report
            .markdown_body
            .contains("GOOG · Emerging Breakout (Day 1)"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · Emerging Breakout (Day 1)"));
    }

    #[test]
    fn test_no_trade_snapshot_breakout_age_displays_day_one_in_ja_jp() {
        let report = build_no_trade_report(crate::core::i18n::Language::JaJp);
        assert!(report.markdown_body.contains("GOOG · 突破初動（1日目）"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · 突破初動（1日目）"));
    }

    #[test]
    fn test_no_trade_html_execution_order_in_en_us() {
        let report = build_no_trade_transition_report(Language::EnUs);
        assert_no_trade_html_execution_order(Language::EnUs, &report.telegram_html_body);
    }

    #[test]
    fn test_no_trade_html_execution_order_in_ja_jp() {
        let report = build_no_trade_transition_report(Language::JaJp);
        assert_no_trade_html_execution_order(Language::JaJp, &report.telegram_html_body);
    }

    #[test]
    fn test_no_trade_snapshot_en_us_markdown() {
        let report = build_no_trade_report(crate::core::i18n::Language::EnUs);
        assert_snapshot("no_trade_en_us.md", &report.markdown_body);
    }

    #[test]
    fn test_no_trade_snapshot_en_us_html() {
        let report = build_no_trade_report(crate::core::i18n::Language::EnUs);
        assert_snapshot("no_trade_en_us.html.txt", &report.telegram_html_body);
    }

    #[test]
    fn test_no_trade_snapshot_ja_jp_markdown() {
        let report = build_no_trade_report(crate::core::i18n::Language::JaJp);
        assert_snapshot("no_trade_ja_jp.md", &report.markdown_body);
    }

    #[test]
    fn test_no_trade_snapshot_ja_jp_html() {
        let report = build_no_trade_report(crate::core::i18n::Language::JaJp);
        assert_snapshot("no_trade_ja_jp.html.txt", &report.telegram_html_body);
    }

    #[test]
    fn test_ssot_uniqueness_and_consistency() {
        let packet = no_trade_snapshot_packet();
        let config = mock_config_with_language(Language::ZhCn);
        let lang = Language::ZhCn;
        let pres = PresentationAssembler::assemble(
            &packet,
            &config.get_parsed_rules(),
            &HashMap::new(),
            vec![],
            lang,
        );

        let report =
            generate_refined_report(&config, &pres, 0.0, &HashMap::new(), &HashMap::new()).unwrap();
        let body = report.markdown_body;

        // 1. Ensure exactly one "市场状态" marker (not duplicated by dual engines)
        let market_state_marker = "市场状态";
        assert_eq!(
            body.matches(market_state_marker).count(),
            1,
            "Market state should only be reported once (SSOT violation)"
        );

        // 2. Ensure NO Japanese headers in a Chinese report (Language Consistency)
        assert!(
            !body.contains("マーケット状态サマリー"),
            "Japanese headers found in Chinese report"
        );

        // 3. Ensure unified threshold format (current/threshold)
        // From no_trade_snapshot_packet: stability=1.1, continuity_streak=1.
        // Default rules: stability_threshold=10.0, continuity_threshold=3.
        assert!(
            body.contains("1.1/10"),
            "Stability threshold should use unified (current/threshold) format"
        );
        assert!(
            body.contains("1/3"),
            "Continuity threshold should use unified (current/threshold) format"
        );

        // 4. Ensure no redundant "Stability score (1.1) below threshold (10.0)"-style English text
        // if it's being correctly localized/localized to the new format.
        assert!(
            !body.contains("Stability score (1.1) below threshold (10.0)"),
            "Legacy English threshold message should be suppressed or localized"
        );
    }
}
