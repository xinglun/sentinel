use crate::config::{AppConfig, OutputConfig, RulesConfig, TrendCohesionRulesConfig, TrendConfig};
use crate::features::radar::domain::action_matrix::AssetActionDecision;
use crate::features::radar::domain::asset_state::{AssetState, AssetStateSnapshot};
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::exit::{AssetExitState, ExitDecision, PositionIntent};
use crate::features::radar::domain::market_regime::{
    MarketRegimeSnapshot, MarketState, RiskOverlay,
};
use crate::features::radar::domain::rules::ParsedRules as DomainParsedRules;
use crate::features::radar::interface::presentation_assembler::PresentationAssembler;
use crate::features::radar::interface::report::{generate_refined_report, ReportRenderContext};
use crate::features::shared::interface::threshold_format::format_threshold_value;
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
            language: Some(crate::features::shared::interface::i18n::Language::ZhCn),
            compact_transition_evidence_in_no_trade: true,
        },
        telegram: None,
        futu: None,
        finnhub: None,
        fred: None,
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
            market_benchmarks: None,
        },
        watchlist: vec![],
        research_attention: None,
        asset_thesis: None,
        macro_gravity: None,
        capital_absorption: None,
        capital_dynamics: None,
        gray_rhino_escalation: None,
        gray_rhino_provider_registry: None,
    }
}

fn mock_config_with_language(
    language: crate::features::shared::interface::i18n::Language,
) -> AppConfig {
    let mut config = mock_config();
    config.output.language = Some(language);
    config
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
    use crate::features::radar::domain::transition_log::StateTransitionLog;
    use crate::features::radar::domain::trend_cohesion::{
        SubstantiveEvidence, TrendContinuationState, TrendRecognitionEvidence,
    };
    use crate::features::radar::domain::trend_cohesion::{
        TrendCohesionGateCondition, TrendCohesionSnapshot,
    };
    use crate::features::radar::interface::display::{
        RiskOpportunityViewModel, TopActionViewModel,
    };
    use crate::features::radar::interface::interpretation_read_model::{
        build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
        InterpretationNarrativeSignal,
    };
    use crate::features::radar::interface::presentation::{
        ExitDecisionItemViewModel, ExitDecisionSummaryViewModel, ExitDisplayIntent,
        InterpretationExpectationQuality, InterpretationGravityDataQuality,
        InterpretationGravityDataQualityReason, InterpretationTrendState,
    };
    use crate::features::radar::interface::signal_context_event_read_model::{
        build_signal_context_event_read_model, SignalContextEventReadModel,
        SignalContextEventReadModelInput,
    };
    use crate::features::research::interface::expectation_report_builder::build_expectation_layer_fixture_snapshot;
    use crate::features::research::interface::macro_event_calendar_adapter::MacroEventCalendarReadModel;
    use crate::features::research::interface::macro_event_observation::{
        FutureCalendarKind, FutureCalendarObservation, MacroEventImportance,
        MacroEventInformationContent, MacroEventLifecycle, MacroEventObservation,
        MacroEventSourceHealth, MacroEventSurpriseState, MacroEventType,
    };
    use crate::features::shared::interface::i18n::{get_dictionary, Language};
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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
                stability_score: 1.1,
                continuity_streak: 1,
                unmet_conditions: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion,
                ],
                ..Default::default()
            },
            market_features: crate::features::radar::domain::features::MarketFeatures {
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
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
                    breakout_strength: 62.0,
                    breakout_quality: 100.0,
                    reasons: vec![
                        crate::features::radar::domain::breakout_detection::BreakoutReason::StructuralBreakout,
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
        language: crate::features::shared::interface::i18n::Language,
    ) -> crate::features::radar::interface::report::ReportResult {
        let config = mock_config_with_language(language);
        let pres = PresentationAssembler::assemble(
            &no_trade_snapshot_packet(),
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            language,
        );
        generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap()
    }

    fn build_no_trade_transition_report(
        language: crate::features::shared::interface::i18n::Language,
    ) -> crate::features::radar::interface::report::ReportResult {
        let curr = no_trade_transition_order_packet();
        let config = mock_config_with_language(language);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            language,
        );
        generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap()
    }

    #[test]
    fn no_trade_report_explains_confidence_and_separates_execution_from_portfolio_risk() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::EnUs);

        assert!(report.markdown_body.contains("Confidence breakdown"));
        assert!(report.markdown_body.contains("Trend allocation"));
        assert!(report.markdown_body.contains("inverse potential"));
        assert!(report.markdown_body.contains("Execution Risk"));
        assert!(report.markdown_body.contains("Pause new active entries"));
        assert!(report.markdown_body.contains("Portfolio Risk"));
        assert!(report.telegram_html_body.contains("Confidence breakdown"));
        assert!(report.telegram_html_body.contains("Execution Risk"));
        assert!(report.telegram_html_body.contains("Portfolio Risk"));
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
            .join("src/features/radar/interface/snapshots")
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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
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
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let prices: HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );

        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &prices,
        )
        .unwrap()
        .markdown_body;

        // 更新後 layout（report.rs logic de-bloat 版）を確認する。
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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
                stability_score: 1.1,
                continuity_streak: 1,
                unmet_conditions: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion,
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let config = mock_config();
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let result = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
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
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let prices: HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );

        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &prices,
        )
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
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let failed_symbols = vec!["AAPL".to_string(), "TSLA".to_string()];

        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            failed_symbols,
            lang,
        );
        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap()
        .markdown_body;

        // 新しい 3 段階 alert format を確認する。
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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "AAPL".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec!["AAPL".to_string()],
            crate::features::shared::interface::i18n::Language::ZhCn,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert!(!report.markdown_body.contains("N/A/10"));
        assert!(!report.markdown_body.contains("N/A/3"));
        assert!(!report.telegram_html_body.contains("N/A/10"));
        assert!(!report.telegram_html_body.contains("N/A/3"));
    }

    #[test]
    fn test_compact_no_trade_continuity_uses_status_not_ratio() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
                stability_score: 7.5,
                continuity_streak: 5,
                unmet_conditions: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::DirectionalCohesion,
                ],
                ..Default::default()
            },
            market_features: crate::features::radar::domain::features::MarketFeatures {
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

        let mut config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        config.rules.trend_cohesion = Some(TrendCohesionRulesConfig {
            gate_stability_threshold: Some(11.0),
            gate_continuity_threshold: Some(4),
            ..Default::default()
        });

        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            crate::features::shared::interface::i18n::Language::ZhCn,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert!(report.markdown_body.contains("稳定性 7.5/11"));
        assert!(report.markdown_body.contains("连续性 sustained"));
        assert!(!report.markdown_body.contains("连续性 5/4"));
        assert!(report.telegram_html_body.contains("稳定性 7.5/11"));
        assert!(report.telegram_html_body.contains("连续性 sustained"));
        assert!(!report.telegram_html_body.contains("连续性 5/4"));
    }

    #[test]
    fn test_legacy_threshold_template_matcher_table_driven() {
        struct Case {
            reason: &'static str,
            unmet: Vec<crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition>,
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
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "稳定性不足（＜10.0）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "稳定性不足（≤10.0）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "安定性不足(≦10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "连续性不足（1d < 3d）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "連続性不足（1日 ≤ 3日）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= threshold (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "STABILITY SCORE(8.0)<=THRESHOLD(10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= THR (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= threshold (10.0).",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <= threshold (10.0)!",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) ＜ threshold (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) ＜ threshold (10.0)。",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) ＜＝ threshold (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "Stability score (8.0) <＝ threshold (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "稳定性不足（<10.0）。",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "連続性不足（1日≤3日）！",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: true,
            },
            Case {
                reason: "备注：稳定性不足但先观察，不立即处理",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "连续性不足（等待确认）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "盘中提示：稳定性不足（<10.0）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "稳定性不足但需结合成交量<均值判断",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "稳定性不足（需结合成交量变化）并结合<均值判断",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "稳定性不足（备注<阈值，先观察）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "連続性不足（出来高≤基準なので様子見）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "連続性不足（注記≤閾値で監視）",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (comment: < avg volume) keep watching",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (note) <= threshold (watch)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (8.0) <= thrash (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (8.0) = threshold (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                ],
                should_suppress: false,
            },
            Case {
                reason: "Stability score (8.0) <== threshold (10.0)",
                unmet: vec![
                    crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
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
                trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                    gate_passed: false,
                    status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
                    topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
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

            let mut config =
                mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
            config.output.compact_transition_evidence_in_no_trade = false;
            let pres = PresentationAssembler::assemble(
                &packet,
                &domain_rules(&config),
                &HashMap::new(),
                vec![],
                crate::features::shared::interface::i18n::Language::ZhCn,
            );
            let report = generate_refined_report(
                &report_context(&config),
                &pres,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
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
        unmet: Vec<crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition>,
        language: crate::features::shared::interface::i18n::Language,
    ) -> bool {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                reasons: vec![reason.to_string()],
                ..Default::default()
            },
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                status:
                    crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
                topology:
                    crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader,
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
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            language,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
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
                                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                            ],
                            crate::features::shared::interface::i18n::Language::ZhCn,
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
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
            ),
            (
                "安定性不足",
                "<10.0",
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
            ),
            (
                "连续性不足",
                "1d<3d",
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
            ),
            (
                "連続性不足",
                "1日≤3日",
                crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
            ),
        ];
        for (prefix, payload, gate) in localized_cases {
            for punct in punctuations {
                let reason = format!("{}（{}）{}", prefix, payload, punct);
                assert!(
                    !report_markdown_contains_reason(
                        &reason,
                        vec![gate],
                        crate::features::shared::interface::i18n::Language::ZhCn,
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
                        crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                    ],
                    crate::features::shared::interface::i18n::Language::ZhCn,
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
                        crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold,
                        crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold,
                    ],
                    crate::features::shared::interface::i18n::Language::ZhCn,
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
            vec![crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold];

        for language in [
            crate::features::shared::interface::i18n::Language::EnUs,
            crate::features::shared::interface::i18n::Language::JaJp,
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
                vec![crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold],
            ),
            (
                "Core Tier streak",
                "(1)",
                "(3)",
                vec![crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold],
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
                        crate::features::shared::interface::i18n::Language::ZhCn,
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
                vec![crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::StabilityThreshold],
            ),
            (
                "Core Tier streak",
                "(1)",
                "(3)",
                vec![crate::features::radar::domain::trend_cohesion::TrendCohesionGateCondition::ContinuityThreshold],
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
                        crate::features::shared::interface::i18n::Language::ZhCn,
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
            },            market_features: crate::features::radar::domain::features::MarketFeatures {
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
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
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
        assert!(card.contains("连续性 emerging"));
        assert!(!card.contains("主线形成条件"));
        assert!(!card.contains("当前未满足项"));
    }

    #[test]
    fn test_persistent_main_theme_displays_existing_theme_without_gate_permission() {
        use crate::features::radar::domain::trend_cohesion::{
            SubstantiveEvidence, TrendCohesionSnapshot, TrendCohesionStatus, TrendCohesionTopology,
            TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;

        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::IGNITION,
                risk_overlay: RiskOverlay::NORMAL,
                ..Default::default()
            },
            trend_cohesion: TrendCohesionSnapshot {
                gate_passed: false,
                status: TrendCohesionStatus::Dispersed,
                topology: TrendCohesionTopology::NoLeader,
                stability_score: 7.1,
                continuity_streak: 4,
                ..Default::default()
            },
            market_features: crate::features::radar::domain::features::MarketFeatures {
                up_count: 2,
                down_count: 7,
                total_count: 9,
                stability_score: 7.1,
                system_confidence: 54.0,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "SPY".to_string(),
                    asset_state: AssetStateSnapshot {
                        symbol: "SPY".to_string(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "MSFT".to_string(),
                    asset_state: AssetStateSnapshot {
                        symbol: "MSFT".to_string(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".to_string(),
                    asset_state: AssetStateSnapshot {
                        symbol: "GOOG".to_string(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            trend_recognition: Some(TrendRecognitionEvidence {
                state: TrendContinuationState::StructuralPersistence,
                diffusion_score: 3.40,
                conviction_score: 3.40,
                lag_state: false,
                single_asset_decay_day: 0,
                single_asset_decay_max: 3,
                substantive: Some(SubstantiveEvidence {
                    capex_payoff_signal: true,
                    earnings_validation: true,
                    order_visibility: true,
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };

        for (language, mainline, topology, forbidden) in [
            (
                Language::ZhCn,
                "主线状态：主线存在（战术未许可）",
                "主线结构：核心资产主导",
                "主线状态：主线未形成",
            ),
            (
                Language::EnUs,
                "Trend Cohesion：Main Theme Present (tactical permission not ready)",
                "Trend Topology：Core Asset Leadership",
                "Trend Cohesion：Dispersed",
            ),
            (
                Language::JaJp,
                "主線状態：主線存在（戦術未許可）",
                "主線構造：コア資産主導",
                "主線状態：分散",
            ),
        ] {
            let config = mock_config_with_language(language);
            let pres = PresentationAssembler::assemble(
                &packet,
                &domain_rules(&config),
                &HashMap::new(),
                vec![],
                language,
            );
            assert!(!pres.decision_summary.gate_passed);

            let report = generate_refined_report(
                &report_context(&config),
                &pres,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
            assert!(report.markdown_body.contains(mainline));
            assert!(report.markdown_body.contains(topology));
            assert!(!report.markdown_body.contains(forbidden));
            assert!(report.markdown_body.contains("NO TRADE"));
        }
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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets,
            ..Default::default()
        };

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let prices: HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );

        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &prices,
        )
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
            market_features: crate::features::radar::domain::features::MarketFeatures {
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
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
        assert!(card.contains("行動：候補のみ / 実行ウィンドウなし"));
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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

        let config_zh =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres_zh = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config_zh),
            &HashMap::new(),
            vec![],
            crate::features::shared::interface::i18n::Language::ZhCn,
        );
        let report_zh = generate_refined_report(
            &report_context(&config_zh),
            &pres_zh,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(report_zh
            .telegram_html_body
            .contains("SPY · 最优 (候选标的)"));

        let config_en =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::EnUs);
        let pres_en = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config_en),
            &HashMap::new(),
            vec![],
            crate::features::shared::interface::i18n::Language::EnUs,
        );
        let report_en = generate_refined_report(
            &report_context(&config_en),
            &pres_en,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(report_en
            .telegram_html_body
            .contains("SPY · Optimal (Candidate)"));

        let config_ja =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::JaJp);
        let pres_ja = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config_ja),
            &HashMap::new(),
            vec![],
            crate::features::shared::interface::i18n::Language::JaJp,
        );
        let report_ja = generate_refined_report(
            &report_context(&config_ja),
            &pres_ja,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
                    exit_decision: crate::features::radar::domain::exit::ExitDecision {
                        position_intent: PositionIntent::EXIT,
                        asset_exit_state:
                            crate::features::radar::domain::exit::AssetExitState::DefensiveExit,
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap()
        .markdown_body;

        assert!(card.contains("### 📉 风险处置建议"));
        assert!(card.contains("- NVDA · 持有"));
        assert!(card.contains("- FIG · 退出"));
        assert!(!card.contains("- NVDA · 卖出"));
        let decision_idx = card.find("### 禁止动作（NO TRADE）").unwrap();
        let exit_idx = card.find("### 📉 风险处置建议").unwrap();
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let card = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap()
        .markdown_body;

        assert!(card.contains("### 📉 风险处置建议"));
        assert!(card.contains("> 未触发减仓或退出条件。"));
        assert!(!card.contains("当前无持仓"));
        assert!(!card.contains("未触发任何退出条件。"));
    }

    #[test]
    fn test_breakout_section_renders_evidence_without_buy_language() {
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "PLTR".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::ConfirmedBreakout,
                        breakout_strength: 81.0,
                        breakout_quality: 77.0,
                        reasons: vec![
                            crate::features::radar::domain::breakout_detection::BreakoutReason::StructuralBreakout,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "TSLA".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 33.0,
                        breakout_quality: 39.0,
                        failed_breakout_risk: 66.0,
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "QQQ".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                    breakout_strength: 27.0,
                    breakout_quality: 29.0,
                    reasons: vec![crate::features::radar::domain::breakout_detection::BreakoutReason::OrdinaryRebound],
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 24.0,
                        breakout_quality: 28.0,
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
                    symbol: "PLTR".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::ConfirmedBreakout,
                        breakout_strength: 81.0,
                        breakout_quality: 77.0,
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
                        failed_breakout_risk: 66.0,
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::EnUs);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        breakout_strength: 24.0,
                        breakout_quality: 28.0,
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
                    symbol: "PLTR".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::ConfirmedBreakout,
                        breakout_strength: 81.0,
                        breakout_quality: 77.0,
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
                        failed_breakout_risk: 66.0,
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
                        breakout_strength: 47.0,
                        breakout_quality: 88.0,
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::ZhCn);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        reasons: vec![
                            crate::features::radar::domain::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
                        breakout_strength: 47.0,
                        breakout_quality: 88.0,
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::EnUs);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "QQQ".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        reasons: vec![
                            crate::features::radar::domain::breakout_detection::BreakoutReason::OrdinaryRebound,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
                        breakout_strength: 47.0,
                        breakout_quality: 88.0,
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

        let config =
            mock_config_with_language(crate::features::shared::interface::i18n::Language::JaJp);
        let lang = config
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

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
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // transition log を計算する。
        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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

        let md = report.archival_markdown;
        // localize された section title を確認する。
        assert!(md.contains("🔄 状态转移证据"));
        // localize された state change を確認する。
        assert!(md.contains("保命期 -> 启动期"));
        assert!(md.contains("结构防御 -> 风险正常"));
        assert!(md.contains("未达标 -> 达标"));
        // debug enum string が出力されていないことを確認する。
        assert!(!md.contains("DEFENSIVE"));
        assert!(!md.contains("IGNITION"));
    }

    #[test]
    fn test_trend_recognition_report_rendering_zh_cn() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
            TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;

        let mut curr = DecisionPacket::default();
        curr.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::LeaderConfirmedFollowersLagging,
            diffusion_score: 0.45,
            conviction_score: 0.0,
            lag_state: true,
            single_asset_decay_day: 3,
            single_asset_decay_max: 5,
            substantive: Some(SubstantiveEvidence {
                records: vec![
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::OfficialIR,
                        EvidenceType::EarningsValidation,
                        0.95,
                        "Earnings beat expectations by 15%".to_string(),
                        "2026-04-22".to_string(),
                        Some("GOOG".to_string()),
                        Some("https://example.com/ir/goog".to_string()),
                        "test:goog:earnings:2026-04-22".to_string(),
                    ),
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::PriceAction,
                        EvidenceType::FollowThrough,
                        0.80,
                        "Breakout follow-through persisted".to_string(),
                        "2026-04-23".to_string(),
                        Some("GOOG".to_string()),
                        None,
                        "test:goog:follow-through:2026-04-23".to_string(),
                    ),
                ],
                ..Default::default()
            }),
        });
        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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

        let md = report.archival_markdown;
        assert!(md.contains("🧭 战略背景"));
        assert!(md.contains("市场结构模式: 脆弱轮动期"));
        assert!(md.contains("长期方向: 结构证据观察中"));
        assert!(md.contains("战术状态: NO TRADE，等待结构扩散"));
        assert!(md.contains("🎯 趋势特征识别"));
        assert!(md.contains("进展阶段: 单点确立/整体滞后"));
        assert!(md.contains("趋势扩散分: 0.45"));
        assert!(md.contains("滞后预警: 先行成立・追随迟缓"));
        assert!(md.contains("单极突破衰减: 3/5"));
        assert!(md.contains("[2026-04-22] [EarningsValidation]"));
        assert!(md.contains("Earnings beat expectations by 15%"));
        assert!(md.contains("https://example.com/ir/goog"));
        assert!(md.contains("[2026-04-23] [FollowThrough]"));
        assert!(md.contains("Breakout follow-through persisted"));

        let html = report.telegram_html_body;
        assert!(html.contains("🧭 战略背景"));
        assert!(html.contains("<i>市场结构模式: 脆弱轮动期</i>"));
        assert!(html.contains("<i>长期方向: 结构证据观察中</i>"));
        assert!(html.contains("<i>战术状态: NO TRADE，等待结构扩散</i>"));
        assert!(html.contains("🎯 趋势特征识别"));
        assert!(html.contains("<i>进展阶段: 单点确立/整体滞后</i>"));
        assert!(html.contains("实质性证据: 业绩实质性确认"));
        assert!(html.contains("结构强度: 已观察 (1 类证据 / 1 条价格确认)"));
        assert!(html.contains("证据质量: 高质量 1 / 价格确认 1"));
        assert!(!html.contains("媒体噪音"));
        assert!(!html.contains("Media Noise"));
        assert!(!html.contains("メディアノイズ"));
        assert!(!html.contains("条记录"));
        assert!(!html.contains("FollowThrough"));
        assert!(!html.contains("[2026-04-22] [EarningsValidation]"));
        assert!(!html.contains("https://example.com/ir/goog"));
        // trend recognition だけでも transition evidence block が描画されることを確認する。
        assert!(md.contains("🔄 状态转移证据"));
    }

    #[test]
    fn test_trend_recognition_report_rendering_ja_jp() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;

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
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let md = report.archival_markdown;
        assert!(md.contains("🎯 トレンド特徴認識"));
        assert!(md.contains("進行段階: 拡散初期"));
        assert!(md.contains("トレンド拡散スコア: 0.65"));
        assert!(!md.contains("追随遅延")); // lag_state is false, should not appear in simplified output if logic holds
        assert!(!md.contains("単独突破の連続日数: 0/5"));
        // trend recognition だけでも transition evidence block が描画されることを確認する。
        assert!(md.contains("🔄 状態遷移エビデンス"));
    }

    #[test]
    fn test_trend_recognition_substantive_details_render_in_en_and_ja() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
            TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;

        let mut curr = DecisionPacket::default();
        curr.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::Broadening,
            diffusion_score: 0.65,
            conviction_score: 0.95,
            lag_state: false,
            single_asset_decay_day: 0,
            single_asset_decay_max: 5,
            substantive: Some(SubstantiveEvidence {
                records: vec![
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::OfficialIR,
                        EvidenceType::EarningsValidation,
                        0.95,
                        "Earnings beat expectations by 15%".to_string(),
                        "2026-04-22".to_string(),
                        Some("GOOG".to_string()),
                        Some("https://example.com/ir/goog".to_string()),
                        "test:goog:earnings:2026-04-22".to_string(),
                    ),
                    AutomatedEvidenceRecord::new(
                        EvidenceSourceType::PriceAction,
                        EvidenceType::FollowThrough,
                        0.80,
                        "Breakout follow-through persisted".to_string(),
                        "2026-04-23".to_string(),
                        Some("GOOG".to_string()),
                        None,
                        "test:goog:follow-through:2026-04-23".to_string(),
                    ),
                ],
                earnings_validation: true,
                ..Default::default()
            }),
        });
        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        for language in [Language::EnUs, Language::JaJp] {
            let mut config = mock_config_with_language(language);
            config.macro_gravity = Some(crate::config::MacroGravityConfig {
                rate_pressure: crate::config::MacroPressure::Rising,
                real_yield_pressure: crate::config::MacroPressure::Tight,
                yield_curve: crate::config::YieldCurveState::Flat,
                credit_stress: crate::config::CreditStress::Normal,
                liquidity: crate::config::LiquidityCondition::Neutral,
                growth_valuation_impact: crate::config::GrowthValuationImpact::Compressing,
                note: Some("Discount-rate gravity is a context layer only.".to_string()),
                enable: Some(true),
            });
            let pres = PresentationAssembler::assemble(
                &curr,
                &domain_rules(&config),
                &HashMap::new(),
                vec![],
                language,
            );
            let report = generate_refined_report(
                &report_context(&config),
                &pres,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            assert!(!report
                .telegram_html_body
                .contains("Discount-rate gravity is a context layer only."));
            assert!(!report
                .archival_markdown
                .contains("Discount-rate gravity is a context layer only."));
            assert!(report.telegram_html_body.contains("Earnings Quality"));
            match language {
                Language::EnUs => {
                    assert!(report.telegram_html_body.contains("Strategic Context"));
                    assert!(report
                        .telegram_html_body
                        .contains("Market Structure Mode: Fragile Rotation"));
                    assert!(report
                        .telegram_html_body
                        .contains("Long-Term Direction: Structural evidence under observation"));
                    assert!(report
                        .telegram_html_body
                        .contains("Tactical Status: NO TRADE, waiting for structural diffusion"));
                    assert!(report.telegram_html_body.contains("Risk Taxonomy"));
                    assert!(report
                        .telegram_html_body
                        .contains("Market Structure Risk: NORMAL"));
                    assert!(report.telegram_html_body.contains("Structural Strength"));
                    assert!(report
                        .telegram_html_body
                        .contains("Observed (1 evidence type / 1 price confirmation)"));
                    assert!(report
                        .telegram_html_body
                        .contains("Evidence Quality: High Quality 1 / Price Confirmation 1"));
                    assert!(!report.telegram_html_body.contains("Media Noise"));
                    assert!(!report.telegram_html_body.contains("1 record"));
                }
                Language::JaJp => {
                    assert!(report.telegram_html_body.contains("戦略文脈"));
                    assert!(report
                        .telegram_html_body
                        .contains("市場構造モード: 脆弱ローテーション期"));
                    assert!(report
                        .telegram_html_body
                        .contains("長期方向: 構造証拠を観測中"));
                    assert!(report
                        .telegram_html_body
                        .contains("戦術状態: NO TRADE、構造拡散待ち"));
                    assert!(report.telegram_html_body.contains("リスク分類"));
                    assert!(report.telegram_html_body.contains("市場構造リスク: NORMAL"));
                    assert!(report.telegram_html_body.contains("構造強度"));
                    assert!(report
                        .telegram_html_body
                        .contains("観測済み (1 種類の証拠 / 1 件の価格確認)"));
                    assert!(report
                        .telegram_html_body
                        .contains("証拠品質: 高品質 1 / 価格確認 1"));
                    assert!(!report.telegram_html_body.contains("メディアノイズ"));
                    assert!(!report.telegram_html_body.contains("1 件の記録"));
                }
                Language::ZhCn => unreachable!(),
            }
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
            match language {
                Language::EnUs => {
                    assert!(report.archival_markdown.contains("Strategic Context"));
                    assert!(report
                        .archival_markdown
                        .contains("Market Structure Mode: Fragile Rotation"));
                    assert!(report
                        .archival_markdown
                        .contains("Evidence Coverage: Earnings Quality Confirmed"));
                    assert!(report.archival_markdown.contains("Risk Taxonomy"));
                    assert!(report
                        .archival_markdown
                        .contains("Market Structure Risk: NORMAL"));
                    assert!(report.archival_markdown.contains("Structural Strength"));
                    assert!(report
                        .archival_markdown
                        .contains("Observed (1 evidence type / 1 price confirmation)"));
                    assert!(report
                        .archival_markdown
                        .contains("Evidence Quality: High Quality 1 / Price Confirmation 1"));
                    assert!(!report.archival_markdown.contains("Media Noise"));
                }
                Language::JaJp => {
                    assert!(report.archival_markdown.contains("戦略文脈"));
                    assert!(report
                        .archival_markdown
                        .contains("市場構造モード: 脆弱ローテーション期"));
                    assert!(report
                        .archival_markdown
                        .contains("証拠カバレッジ: 業績の実質的裏付け"));
                    assert!(report.archival_markdown.contains("リスク分類"));
                    assert!(report.archival_markdown.contains("市場構造リスク: NORMAL"));
                    assert!(report.archival_markdown.contains("構造強度"));
                    assert!(report
                        .archival_markdown
                        .contains("観測済み (1 種類の証拠 / 1 件の価格確認)"));
                    assert!(report
                        .archival_markdown
                        .contains("証拠品質: 高品質 1 / 価格確認 1"));
                    assert!(!report.archival_markdown.contains("メディアノイズ"));
                }
                Language::ZhCn => unreachable!(),
            }
            assert!(report
                .archival_markdown
                .contains("Earnings beat expectations by 15%"));
            assert!(report
                .archival_markdown
                .contains("https://example.com/ir/goog"));
        }
    }

    #[test]
    fn test_strategic_layer_no_trade_boundary_is_stable_across_languages() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            SubstantiveEvidence, TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;

        fn strategic_section<'a>(body: &'a str, title: &str) -> &'a str {
            let start = body.find(title).expect("strategic section title");
            let tail = &body[start..];
            let end = tail.find("🎯").unwrap_or(tail.len());
            &tail[..end]
        }

        let forbidden_execution_terms = [
            "BUY",
            "ADD",
            "ENTRY",
            "Open Position",
            "Increase Exposure",
            "买入",
            "加仓",
            "开仓",
            "追击",
            "購入",
            "買い",
            "買増",
            "エントリー",
            "追い",
        ];

        for (language, title, cycle_line, crowding_line, tactical_line, state_line) in [
            (
                Language::ZhCn,
                "🧭 战略背景",
                "周期位置: LATE_ACCEPTANCE",
                "拥挤风险: WATCH",
                "战术状态: NO TRADE，等待结构扩散",
                "进展阶段: 结构延续/战术冷却",
            ),
            (
                Language::EnUs,
                "🧭 Strategic Context",
                "Cycle Position: LATE_ACCEPTANCE",
                "Crowding Risk: WATCH",
                "Tactical Status: NO TRADE, waiting for structural diffusion",
                "Continuation State: Structural Persistence / Tactical Cooldown",
            ),
            (
                Language::JaJp,
                "🧭 戦略文脈",
                "サイクル位置: LATE_ACCEPTANCE",
                "混雑リスク: WATCH",
                "戦術状態: NO TRADE、構造拡散待ち",
                "進行段階: 構造持続 / 戦術冷却",
            ),
        ] {
            let mut curr = DecisionPacket::default();
            curr.trend_recognition = Some(TrendRecognitionEvidence {
                state: TrendContinuationState::StructuralPersistence,
                diffusion_score: 3.90,
                conviction_score: 3.40,
                lag_state: false,
                single_asset_decay_day: 1,
                single_asset_decay_max: 3,
                substantive: Some(SubstantiveEvidence {
                    capex_payoff_signal: true,
                    earnings_validation: true,
                    order_visibility: true,
                    ..Default::default()
                }),
            });
            curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

            let mut config = mock_config_with_language(language);
            config.macro_gravity = Some(crate::config::MacroGravityConfig {
                rate_pressure: crate::config::MacroPressure::Rising,
                real_yield_pressure: crate::config::MacroPressure::Tight,
                yield_curve: crate::config::YieldCurveState::Flat,
                credit_stress: crate::config::CreditStress::Normal,
                liquidity: crate::config::LiquidityCondition::Neutral,
                growth_valuation_impact: crate::config::GrowthValuationImpact::Compressing,
                note: Some("Discount-rate gravity is a context layer only.".to_string()),
                enable: Some(true),
            });
            let pres = PresentationAssembler::assemble(
                &curr,
                &domain_rules(&config),
                &HashMap::new(),
                vec![],
                language,
            );
            let report = generate_refined_report(
                &report_context(&config),
                &pres,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            let markdown_section = strategic_section(&report.archival_markdown, title);
            let telegram_section = strategic_section(&report.telegram_html_body, title);
            assert!(markdown_section.contains(cycle_line));
            assert!(telegram_section.contains(cycle_line));
            assert!(markdown_section.contains(crowding_line));
            assert!(telegram_section.contains(crowding_line));
            assert!(markdown_section.contains("RISING"));
            assert!(telegram_section.contains("RISING"));
            assert!(markdown_section.contains("COMPRESSING"));
            assert!(telegram_section.contains("COMPRESSING"));
            assert!(!markdown_section.contains("Discount-rate gravity is a context layer only."));
            assert!(!telegram_section.contains("Discount-rate gravity is a context layer only."));
            assert!(markdown_section.contains(tactical_line));
            assert!(telegram_section.contains(tactical_line));
            assert!(report.archival_markdown.contains(state_line));
            assert!(report.telegram_html_body.contains(state_line));

            for term in forbidden_execution_terms {
                assert!(
                    !markdown_section.contains(term),
                    "Strategic Layer markdown must not include execution term: {term}"
                );
                assert!(
                    !telegram_section.contains(term),
                    "Strategic Layer telegram must not include execution term: {term}"
                );
            }
        }
    }

    #[test]
    fn test_hypothesis_layer_renders_speculative_notice_without_gate_change() {
        use crate::features::radar::domain::trend_cohesion::{
            AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
            TrendCohesionSnapshot, TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;
        use chrono::NaiveDate;

        let curr = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
            trend_cohesion: TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            trend_recognition: Some(TrendRecognitionEvidence {
                state: TrendContinuationState::StructuralPersistence,
                diffusion_score: 3.4,
                conviction_score: 3.4,
                substantive: Some(SubstantiveEvidence {
                    capex_payoff_signal: true,
                    earnings_validation: true,
                    order_visibility: true,
                    records: vec![
                        AutomatedEvidenceRecord::new(
                            EvidenceSourceType::OfficialIR,
                            EvidenceType::CapexPayoff,
                            0.9,
                            "capex payoff".to_string(),
                            "2026-05-01".to_string(),
                            Some("MSFT".to_string()),
                            Some("https://example.com/capex".to_string()),
                            "capex".to_string(),
                        ),
                        AutomatedEvidenceRecord::new(
                            EvidenceSourceType::OfficialIR,
                            EvidenceType::EarningsValidation,
                            0.9,
                            "earnings quality".to_string(),
                            "2026-05-20".to_string(),
                            Some("MSFT".to_string()),
                            Some("https://example.com/earnings".to_string()),
                            "earnings".to_string(),
                        ),
                        AutomatedEvidenceRecord::new(
                            EvidenceSourceType::OfficialIR,
                            EvidenceType::OrderVisibility,
                            0.9,
                            "order visibility".to_string(),
                            "2026-05-25".to_string(),
                            Some("MSFT".to_string()),
                            Some("https://example.com/order".to_string()),
                            "order".to_string(),
                        ),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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

        assert!(report.archival_markdown.contains("禁止动作（NO TRADE）"));
        assert!(report.archival_markdown.contains("新开仓上限 · 0%"));
        assert!(report.telegram_html_body.contains("未来地图"));
        assert!(report.telegram_html_body.contains("推测参考"));
        assert!(report.telegram_html_body.contains("不属于当前事实"));
        assert!(report.telegram_html_body.contains("不生成交易信号"));
        assert!(report.telegram_html_body.contains("失败路径"));
        assert!(report.telegram_html_body.contains("兑现窗口"));
        assert!(report.telegram_html_body.contains("战术隔离"));
        assert!(report.telegram_html_body.contains("12-36 months"));
        assert!(report.telegram_html_body.contains("叙事饱和"));
        assert!(report.telegram_html_body.contains("现实覆盖"));
        assert!(report.telegram_html_body.contains("现实覆盖优先级"));
        assert!(report.telegram_html_body.contains("置信衰减"));
        assert!(report.telegram_html_body.contains("假设年龄: 30 天"));
        assert!(report.telegram_html_body.contains("命题验证: 3/5"));
        assert!(report.telegram_html_body.contains("✓ CapEx 持续投入"));
        assert!(report.telegram_html_body.contains("✓ 订单或需求能见度提升"));
        assert!(report
            .telegram_html_body
            .contains("✗ 平台 / workflow 付费入口"));
        assert!(report.telegram_html_body.contains("MSFT"));
        assert!(report
            .telegram_html_body
            .contains("AI 利润池可能从 GPU layer 向 cloud / platform layer 扩散"));
        assert!(report.telegram_html_body.contains("摘要: GPU 需求强度"));
        assert!(
            pres.transition_evidence.is_none(),
            "Hypothesis Layer must not depend on transition_log"
        );
    }

    #[test]
    fn test_hypothesis_layer_is_separated_from_reality_sections() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            SubstantiveEvidence, TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;

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

        let config = mock_config_with_language(Language::ZhCn);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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
        let hypothesis_start = report
            .telegram_html_body
            .find("未来地图")
            .expect("hypothesis section");
        let reality_text = &report.telegram_html_body[..hypothesis_start];

        for term in ["未来地图", "AI 利润池可能", "潜在受益者"] {
            assert!(
                !reality_text.contains(term),
                "Reality section must not include hypothesis-only term: {term}"
            );
        }
        let transition_start = report
            .telegram_html_body
            .find("状态转移证据")
            .expect("transition section");
        let transition_text = &report.telegram_html_body[transition_start..hypothesis_start];
        assert!(
            !transition_text.contains("未来地图"),
            "Hypothesis Layer must not be nested in transition evidence"
        );
    }

    #[test]
    fn test_interpretation_layer_renders_before_hypothesis_with_read_only_boundary() {
        use crate::features::radar::domain::trend_cohesion::{
            SubstantiveEvidence, TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::presentation::{
            InterpretationExpectationQuality, InterpretationGravityDataQuality,
            InterpretationGravityDataQualityReason, InterpretationTrendState,
        };
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let config = mock_config_with_language(Language::EnUs);
        let mut packet = DecisionPacket {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            ..Default::default()
        };
        packet.trend_recognition = Some(TrendRecognitionEvidence {
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
        let mut pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        let subjects = vec!["TSLA".to_string(), "GOOG".to_string(), "NVDA".to_string()];
        let dict = get_dictionary(Language::EnUs);
        pres.interpretation_layer = Some(build_interpretation_layer_view_model(
            InterpretationLayerReadModelInput {
                as_of_date: packet.date,
                subjects: &subjects,
                signal: InterpretationNarrativeSignal {
                    trend_state: InterpretationTrendState::Stable,
                    trend_available: true,
                    expectation_quality: InterpretationExpectationQuality::High,
                    expectation_quality_reason:
                        crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    gravity_data_quality: InterpretationGravityDataQuality::Ready,
                    gravity_data_quality_reason:
                        InterpretationGravityDataQualityReason::ConsensusUnavailable,
                    gravity_status: Some(
                        crate::features::research::domain::valuation_gravity::GravityStatus::Fair,
                    ),
                    supply_pressure: false,
                    supply_available: true,
                    flow_acceleration: Some(0.0),
                    gray_rhino_escalated: false,
                },
                future_context: Default::default(),
                decision_summary: Some(&crate::features::radar::interface::presentation::DecisionSummaryViewModel {
                    is_no_trade: true,
                    summary: "NO TRADE".to_string(),
                    readiness_reasons_label: "Readiness Reasons".to_string(),
                    readiness_reasons: vec![
                        "突破连续性不足".to_string(),
                        "扩散范围有限".to_string(),
                        "核心资产尚未形成一致攻击".to_string(),
                    ],
                    ..Default::default()
                }),
                language: Language::EnUs,
                dict: &dict,
            },
        ));

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let interpretation_start = report
            .telegram_html_body
            .find("Interpretation Layer")
            .expect("interpretation section");
        let hypothesis_start = report
            .telegram_html_body
            .find("Future Map / Hypothesis Layer")
            .expect("hypothesis section");

        assert!(interpretation_start < hypothesis_start);
        assert!(report
            .telegram_html_body
            .contains("Observation Layer read models"));
        assert!(report
            .telegram_html_body
            .contains("Current decision weight: 0%"));
        assert!(report.telegram_html_body.contains("Signal Context"));
        assert!(report.telegram_html_body.contains("Information Content"));
        assert!(report.telegram_html_body.contains("Primary Context"));
        assert!(report.telegram_html_body.contains("Context Quality"));
        assert!(report.telegram_html_body.contains("Lifecycle: UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("Expected: UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("Actual: UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("Surprise: UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("Reason: UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("Context Coverage"));
        assert!(report
            .telegram_html_body
            .contains("Scheduled Macro: UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("Corporate: UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("Overall: UNAVAILABLE"));
        assert!(!report.telegram_html_body.contains("No major event today"));
        assert!(report
            .telegram_html_body
            .contains("Quarter-end Rebalancing"));
        assert!(report.telegram_html_body.contains("UNAVAILABLE"));
        assert!(report.telegram_html_body.contains("HIGH"));
        assert!(report
            .telegram_html_body
            .contains("Expectation Quality: HIGH"));
        assert!(report
            .telegram_html_body
            .contains("Expectation Quality Reason"));
        assert!(report
            .telegram_html_body
            .contains("Market consensus available"));
        assert!(report
            .telegram_html_body
            .contains("Gravity Data Quality: READY"));
        assert!(report
            .telegram_html_body
            .contains("Gravity Data Quality Reason"));
        assert!(report.telegram_html_body.contains("Consensus unavailable"));
        assert!(report.telegram_html_body.contains("TSLA, GOOG, NVDA"));
        assert!(report.telegram_html_body.contains("Narrative Components"));
        assert!(report.telegram_html_body.contains("Trend"));
        assert!(report.telegram_html_body.contains("Expectation"));
        assert!(report.telegram_html_body.contains("Supply"));
        assert!(report.telegram_html_body.contains("Gravity"));
        assert!(report.telegram_html_body.contains("Flow"));
        assert!(report.telegram_html_body.contains("Interpretation"));
        assert!(report.telegram_html_body.contains("Decision Explanation"));
        assert!(report
            .telegram_html_body
            .contains("See Market Interpretation for the main narrative."));
        assert!(!report
            .telegram_html_body
            .contains("Price structure remains stable"));
        assert!(report
            .telegram_html_body
            .contains("does not enter the domain decision pipeline"));
        assert!(report
            .archival_markdown
            .contains("does not generate trade signals"));
        let vm = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            as_of_date: packet.date,
            subjects: &subjects,
            signal: InterpretationNarrativeSignal {
                trend_state: InterpretationTrendState::Stable,
                trend_available: true,
                expectation_quality: InterpretationExpectationQuality::High,
                expectation_quality_reason:
                    crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                gravity_data_quality: InterpretationGravityDataQuality::Ready,
                gravity_data_quality_reason:
                    InterpretationGravityDataQualityReason::ConsensusUnavailable,
                gravity_status: Some(
                    crate::features::research::domain::valuation_gravity::GravityStatus::Fair
                ),
                supply_pressure: false,
                supply_available: true,
                flow_acceleration: Some(0.0),
                    gray_rhino_escalated: false,
            },
            future_context: Default::default(),
            decision_summary: Some(&crate::features::radar::interface::presentation::DecisionSummaryViewModel {
                is_no_trade: true,
                summary: "NO TRADE".to_string(),
                readiness_reasons_label: "Readiness Reasons".to_string(),
                readiness_reasons: vec![
                    "突破连续性不足".to_string(),
                    "扩散范围有限".to_string(),
                    "核心资产尚未形成一致攻击".to_string(),
                ],
                ..Default::default()
            }),
            language: Language::EnUs,
            dict: &dict,
        });
        assert_eq!(vm.signal_context_information_content_value, "UNAVAILABLE");
        assert_eq!(
            vm.signal_context_primary_context_value,
            "Quarter-end Rebalancing"
        );
        assert_eq!(vm.signal_context_quality_value, "UNAVAILABLE");
        assert!(vm
            .signal_context_interpretation_value
            .contains("Available source coverage is incomplete"));
        assert_eq!(
            vm.interpretation_value,
            "Signal context unavailable today. See Market Interpretation for the main narrative."
        );
    }

    #[test]
    fn test_interpretation_layer_renders_todays_explanation() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::presentation::{
            InterpretationExpectationQuality, InterpretationGravityDataQuality,
            InterpretationGravityDataQualityReason, InterpretationTrendState,
        };
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let config = mock_config_with_language(Language::EnUs);
        let packet = DecisionPacket {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            ..Default::default()
        };
        let mut pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        let subjects = vec!["TSLA".to_string()];
        let dict = get_dictionary(Language::EnUs);
        pres.interpretation_layer = Some(build_interpretation_layer_view_model(
            InterpretationLayerReadModelInput {
                as_of_date: packet.date,
                subjects: &subjects,
                signal: InterpretationNarrativeSignal {
                    trend_state: InterpretationTrendState::Stable,
                    trend_available: true,
                    expectation_quality: InterpretationExpectationQuality::High,
                    expectation_quality_reason:
                        crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    gravity_data_quality: InterpretationGravityDataQuality::Unavailable,
                    gravity_data_quality_reason:
                        InterpretationGravityDataQualityReason::ConsensusUnavailable,
                    gravity_status: None,
                    supply_pressure: true,
                    supply_available: true,
                    flow_acceleration: Some(0.1), // clear flow acceleration (Secondary)
                    gray_rhino_escalated: true, // escalated (Secondary)
                },
                future_context: Default::default(),
                decision_summary: None,
                language: Language::EnUs,
                dict: &dict,
            },
        ));

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        // Markdown レンダリング検証
        assert!(report.archival_markdown.contains("Today's Explanation"));
        assert!(report
            .archival_markdown
            .contains("See Market Interpretation for the main narrative."));
        assert!(!report.archival_markdown.contains("Secondary Drivers:"));
        assert!(!report
            .archival_markdown
            .contains("Supply pressure, secondary."));
        assert!(!report.archival_markdown.contains("Flow secondary."));
        assert!(!report.archival_markdown.contains("Gray Rhino secondary."));
        assert!(!report.archival_markdown.contains("Ignored Today:"));
        assert!(!report.archival_markdown.contains("Gravity unavailable."));

        // HTML レンダリング検証
        assert!(report.telegram_html_body.contains("Today's Explanation"));
        assert!(report
            .telegram_html_body
            .contains("See Market Interpretation for the main narrative."));
        assert!(!report.telegram_html_body.contains("Secondary Drivers:"));
        assert!(!report.telegram_html_body.contains("Ignored Today:"));
    }

    #[test]
    fn interpretation_layer_renders_future_read_model_signal_contexts() {
        let config = mock_config_with_language(Language::EnUs);
        let mut packet = DecisionPacket {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            ..Default::default()
        };
        packet.trend_recognition = Some(TrendRecognitionEvidence {
            state: TrendContinuationState::StructuralPersistence,
            diffusion_score: 2.8,
            conviction_score: 2.8,
            substantive: Some(SubstantiveEvidence {
                capex_payoff_signal: true,
                earnings_validation: true,
                order_visibility: true,
                ..Default::default()
            }),
            ..Default::default()
        });

        let subjects = vec!["TSLA".to_string()];
        let dict = get_dictionary(Language::EnUs);
        let expectation_snapshot = build_expectation_layer_fixture_snapshot();

        let build_view_model = |future_context: SignalContextEventReadModel| {
            build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
                as_of_date: packet.date,
                subjects: &subjects,
                signal: InterpretationNarrativeSignal {
                    trend_state: InterpretationTrendState::Stable,
                    trend_available: true,
                    expectation_quality: InterpretationExpectationQuality::High,
                    expectation_quality_reason:
                        crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    gravity_data_quality: InterpretationGravityDataQuality::Ready,
                    gravity_data_quality_reason:
                        InterpretationGravityDataQualityReason::ConsensusUnavailable,
                    gravity_status: Some(
                        crate::features::research::domain::valuation_gravity::GravityStatus::Fair,
                    ),
                    supply_pressure: false,
                    supply_available: true,
                    flow_acceleration: Some(0.0),
                    gray_rhino_escalated: false,
                },
                future_context,
                decision_summary: Some(
                    &crate::features::radar::interface::presentation::DecisionSummaryViewModel {
                        is_no_trade: true,
                        summary: "NO TRADE".to_string(),
                        readiness_reasons_label: "Readiness Reasons".to_string(),
                        readiness_reasons: vec![
                            "突破连续性不足".to_string(),
                            "扩散范围有限".to_string(),
                            "核心资产尚未形成一致攻击".to_string(),
                        ],
                        ..Default::default()
                    },
                ),
                language: Language::EnUs,
                dict: &dict,
            })
        };

        let pre_earnings_context =
            build_signal_context_event_read_model(SignalContextEventReadModelInput {
                as_of_date: expectation_snapshot.as_of_date,
                expectation_snapshot: Some(&expectation_snapshot),
                future_calendar: None,
            });
        let pre_earnings_vm = build_view_model(pre_earnings_context);
        let mut pre_earnings_pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        pre_earnings_pres.interpretation_layer = Some(pre_earnings_vm.clone());
        let pre_earnings_report = generate_refined_report(
            &report_context(&config),
            &pre_earnings_pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(pre_earnings_report
            .telegram_html_body
            .contains("Pre-Earnings Waiting"));
        assert_eq!(
            pre_earnings_vm.signal_context_information_content_value,
            "MEDIUM"
        );
        assert_eq!(pre_earnings_vm.signal_context_quality_value, "LOW");
        assert!(pre_earnings_vm
            .signal_context_interpretation_value
            .contains("waiting"));
        assert!(!pre_earnings_vm
            .signal_context_interpretation_value
            .to_lowercase()
            .contains("buy"));
        assert!(!pre_earnings_vm
            .signal_context_interpretation_value
            .to_lowercase()
            .contains("sell"));
        assert!(!pre_earnings_vm
            .signal_context_interpretation_value
            .to_lowercase()
            .contains("ready"));
        assert!(!pre_earnings_vm
            .signal_context_interpretation_value
            .to_lowercase()
            .contains("execute"));
        assert!(!pre_earnings_vm
            .signal_context_interpretation_value
            .to_lowercase()
            .contains("position sizing"));

        assert!(!pre_earnings_report
            .telegram_html_body
            .contains("Macro Event"));
        assert!(!pre_earnings_report
            .telegram_html_body
            .contains("macro information"));
    }

    fn macro_event_observation(event_date: chrono::NaiveDate) -> MacroEventObservation {
        MacroEventObservation {
            event_id: "cpi-2026-06-18".to_string(),
            as_of_date: event_date,
            event_date,
            event_time: Some("08:30".to_string()),
            timezone: "America/New_York".to_string(),
            country: "US".to_string(),
            event_type: MacroEventType::Cpi,
            event_name: "CPI Release".to_string(),
            source: "BLS".to_string(),
            source_url: "https://www.bls.gov/schedule/news_release/cpi.htm".to_string(),
            importance: MacroEventImportance::Critical,
            lifecycle: MacroEventLifecycle::Upcoming,
            expected_value: Some("2.9%".to_string()),
            actual_value: None,
            previous_value: Some("2.8%".to_string()),
            unit: Some("%".to_string()),
            surprise_state: MacroEventSurpriseState::NotAvailable,
            information_content: MacroEventInformationContent::High,
            source_health: MacroEventSourceHealth::Succeeded,
            observed_at: event_date,
        }
    }

    fn future_calendar_fact(
        as_of_date: chrono::NaiveDate,
        kind: FutureCalendarKind,
        event_date: chrono::NaiveDate,
        event_name: &str,
    ) -> FutureCalendarObservation {
        FutureCalendarObservation {
            kind,
            event_id: format!("fact-{:?}-{}", kind, event_date),
            as_of_date,
            event_date,
            event_time: Some("08:30".to_string()),
            timezone: "America/New_York".to_string(),
            country: "US".to_string(),
            event_type: MacroEventType::Gdp,
            event_name: event_name.to_string(),
            source: "Official Calendar".to_string(),
            source_url: "https://example.com/calendar".to_string(),
            importance: MacroEventImportance::High,
            lifecycle: MacroEventLifecycle::Upcoming,
            expected_value: None,
            actual_value: None,
            previous_value: None,
            unit: None,
            surprise_state: MacroEventSurpriseState::NotAvailable,
            information_content: MacroEventInformationContent::Low,
            source_health: MacroEventSourceHealth::Succeeded,
            observed_at: event_date,
        }
    }

    #[test]
    fn interpretation_layer_renders_macro_event_without_trade_language() {
        let config = mock_config_with_language(Language::EnUs);
        let packet = DecisionPacket {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            ..Default::default()
        };
        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string()];
        let observation = macro_event_observation(packet.date);
        let calendar = MacroEventCalendarReadModel::from_observations(
            packet.date,
            "inline".to_string(),
            vec![observation],
        );
        let future_context =
            build_signal_context_event_read_model(SignalContextEventReadModelInput {
                as_of_date: packet.date,
                expectation_snapshot: None,
                future_calendar: Some(&calendar),
            });
        let mut pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        pres.interpretation_layer = Some(build_interpretation_layer_view_model(
            InterpretationLayerReadModelInput {
                as_of_date: packet.date,
                subjects: &subjects,
                signal: InterpretationNarrativeSignal {
                    trend_state: InterpretationTrendState::Stable,
                    trend_available: true,
                    expectation_quality: InterpretationExpectationQuality::High,
                    expectation_quality_reason:
                        crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    gravity_data_quality: InterpretationGravityDataQuality::Ready,
                    gravity_data_quality_reason:
                        InterpretationGravityDataQualityReason::ConsensusUnavailable,
                    gravity_status: Some(
                        crate::features::research::domain::valuation_gravity::GravityStatus::Fair,
                    ),
                    supply_pressure: false,
                    supply_available: true,
                    flow_acceleration: Some(0.0),
                    gray_rhino_escalated: false,
                },
                future_context,
                decision_summary: Some(
                    &crate::features::radar::interface::presentation::DecisionSummaryViewModel {
                        is_no_trade: true,
                        summary: "NO TRADE".to_string(),
                        readiness_reasons_label: "Readiness Reasons".to_string(),
                        readiness_reasons: vec![
                            "突破连续性不足".to_string(),
                            "扩散范围有限".to_string(),
                        ],
                        ..Default::default()
                    },
                ),
                language: Language::EnUs,
                dict: &dict,
            },
        ));

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(report.telegram_html_body.contains("Macro Event"));
        assert!(report.telegram_html_body.contains("Information Content"));
        assert!(report.telegram_html_body.contains("HIGH"));
        assert!(report.telegram_html_body.contains("Event Fact"));
        assert!(report
            .telegram_html_body
            .contains("CPI Release / 2026-06-18 / BLS"));
        assert!(!report.telegram_html_body.contains("Source Diagnostics"));
        let signal_context = pres.interpretation_layer.unwrap();
        assert_eq!(
            signal_context.signal_context_primary_context_value,
            "Macro Event: CPI Release"
        );
        assert_eq!(
            signal_context.signal_context_information_content_value,
            "HIGH"
        );
        assert_eq!(
            signal_context.signal_context_event_fact_value,
            "CPI Release / 2026-06-18 / BLS"
        );
        assert_eq!(signal_context.signal_context_lifecycle_value, "UPCOMING");
        assert_eq!(signal_context.signal_context_expected_value, "2.9%");
        assert_eq!(signal_context.signal_context_actual_value, "UNAVAILABLE");
        assert_eq!(signal_context.signal_context_surprise_value, "NotAvailable");
        assert!(signal_context
            .signal_context_source_diagnostics_value
            .is_empty());
        assert!(signal_context
            .signal_context_interpretation_value
            .to_lowercase()
            .contains("macro information"));
        let lower = signal_context
            .signal_context_interpretation_value
            .to_lowercase();
        assert!(!lower.contains("buy"));
        assert!(!lower.contains("sell"));
        assert!(!lower.contains("ready"));
        assert!(!lower.contains("execute"));
        assert!(!lower.contains("position sizing"));
    }

    #[test]
    fn interpretation_layer_renders_future_calendar_contexts_without_trade_language() {
        let config = mock_config_with_language(Language::EnUs);
        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string()];

        for (packet_date, fact) in [
            (
                chrono::NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
                future_calendar_fact(
                    chrono::NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
                    FutureCalendarKind::IndexReconstitution,
                    chrono::NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
                    "Index Reconstitution",
                ),
            ),
            (chrono::NaiveDate::from_ymd_opt(2026, 6, 29).unwrap(), {
                let mut fact = future_calendar_fact(
                    chrono::NaiveDate::from_ymd_opt(2026, 6, 29).unwrap(),
                    FutureCalendarKind::MajorEventWaiting,
                    chrono::NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
                    "Major Event Waiting",
                );
                fact.information_content = MacroEventInformationContent::High;
                fact
            }),
            (
                chrono::NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
                future_calendar_fact(
                    chrono::NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
                    FutureCalendarKind::EtfRebalance,
                    chrono::NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
                    "ETF Rebalance",
                ),
            ),
            (
                chrono::NaiveDate::from_ymd_opt(2026, 12, 24).unwrap(),
                future_calendar_fact(
                    chrono::NaiveDate::from_ymd_opt(2026, 12, 24).unwrap(),
                    FutureCalendarKind::HolidayLiquidity,
                    chrono::NaiveDate::from_ymd_opt(2026, 12, 24).unwrap(),
                    "Holiday Liquidity",
                ),
            ),
        ] {
            let expected_event_fact = format!(
                "{} / {} / Official Calendar",
                fact.event_name, fact.event_date
            );
            let expected_event_name = fact.event_name.clone();
            let packet = DecisionPacket {
                date: packet_date,
                ..Default::default()
            };
            let calendar = MacroEventCalendarReadModel::from_observations(
                packet_date,
                "inline".to_string(),
                vec![fact],
            );
            let future_context =
                build_signal_context_event_read_model(SignalContextEventReadModelInput {
                    as_of_date: packet.date,
                    expectation_snapshot: None,
                    future_calendar: Some(&calendar),
                });
            let mut pres = PresentationAssembler::assemble(
                &packet,
                &domain_rules(&config),
                &HashMap::new(),
                vec![],
                Language::EnUs,
            );
            pres.interpretation_layer = Some(build_interpretation_layer_view_model(
                InterpretationLayerReadModelInput {
                    as_of_date: packet.date,
                    subjects: &subjects,
                    signal: InterpretationNarrativeSignal {
                        trend_state: InterpretationTrendState::Stable,
                        trend_available: true,
                        expectation_quality: InterpretationExpectationQuality::High,
                        expectation_quality_reason:
                            crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                        gravity_data_quality: InterpretationGravityDataQuality::Ready,
                        gravity_data_quality_reason:
                            InterpretationGravityDataQualityReason::ConsensusUnavailable,
                        gravity_status: Some(
                            crate::features::research::domain::valuation_gravity::GravityStatus::Fair,
                        ),
                        supply_pressure: false,
                        supply_available: true,
                        flow_acceleration: Some(0.0),
                    gray_rhino_escalated: false,
                    },
                    future_context,
                    decision_summary: None,
                    language: Language::EnUs,
                    dict: &dict,
                },
            ));

            let report = generate_refined_report(
                &report_context(&config),
                &pres,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();
            assert_eq!(
                pres.interpretation_layer
                    .as_ref()
                    .unwrap()
                    .signal_context_event_fact_value,
                expected_event_fact
            );
            let body_lower = report.telegram_html_body.to_lowercase();
            assert!(report.telegram_html_body.contains("Interpretation Layer"));
            assert!(report.telegram_html_body.contains("Event Fact"));
            assert!(!report.telegram_html_body.contains("Source Diagnostics"));
            assert!(report.telegram_html_body.contains(&expected_event_name));
            assert!(body_lower.contains("low"));
            assert!(!body_lower.contains("buy"));
            assert!(!body_lower.contains("sell"));
        }
    }

    #[test]
    fn interpretation_layer_renders_source_diagnostics_only_for_degraded_sources() {
        let config = mock_config_with_language(Language::EnUs);
        let packet = DecisionPacket {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            ..Default::default()
        };
        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string()];
        let observation = future_calendar_fact(
            packet.date,
            FutureCalendarKind::IndexReconstitution,
            packet.date,
            "Index Reconstitution",
        );
        let calendar = MacroEventCalendarReadModel::from_observations_with_stats(
            packet.date,
            "official-calendar-connector".to_string(),
            vec![observation],
            2,
            1,
            1,
            Some("fetch failed".to_string()),
        );
        let future_context =
            build_signal_context_event_read_model(SignalContextEventReadModelInput {
                as_of_date: packet.date,
                expectation_snapshot: None,
                future_calendar: Some(&calendar),
            });
        let mut pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        pres.interpretation_layer = Some(build_interpretation_layer_view_model(
            InterpretationLayerReadModelInput {
                as_of_date: packet.date,
                subjects: &subjects,
                signal: InterpretationNarrativeSignal {
                    trend_state: InterpretationTrendState::Stable,
                    trend_available: true,
                    expectation_quality: InterpretationExpectationQuality::High,
                    expectation_quality_reason:
                        crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    gravity_data_quality: InterpretationGravityDataQuality::Ready,
                    gravity_data_quality_reason:
                        InterpretationGravityDataQualityReason::ConsensusUnavailable,
                    gravity_status: Some(
                        crate::features::research::domain::valuation_gravity::GravityStatus::Fair,
                    ),
                    supply_pressure: false,
                    supply_available: true,
                    flow_acceleration: Some(0.0),
                    gray_rhino_escalated: false,
                },
                future_context,
                decision_summary: None,
                language: Language::EnUs,
                dict: &dict,
            },
        ));

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(
            pres.interpretation_layer
                .as_ref()
                .unwrap()
                .signal_context_source_diagnostics_value,
            "Today: Index Reconstitution"
        );
        assert!(pres
            .interpretation_layer
            .as_ref()
            .unwrap()
            .signal_context_source_diagnostics_appendix_value
            .contains("Today: Index Reconstitution"));
        assert!(pres
            .interpretation_layer
            .as_ref()
            .unwrap()
            .signal_context_source_diagnostics_appendix_value
            .contains("Official calendar source health: PARTIAL"));
        assert!(report.telegram_html_body.contains("Source Diagnostics"));
        assert!(report
            .telegram_html_body
            .contains("Today: Index Reconstitution"));
        assert!(!report.telegram_html_body.contains("fetch failed"));
        assert!(report.archival_markdown.contains("PARTIAL"));
    }

    #[test]
    fn market_interpretation_report_section_is_observation_only() {
        let config = mock_config_with_language(Language::EnUs);
        let packet = DecisionPacket::default();
        let interpretation_layer =
            crate::features::radar::interface::presentation::InterpretationLayerViewModel {
                signal_context_information_content_value: "LOW".to_string(),
                signal_context_primary_context_value: "ETF Rebalance".to_string(),
                signal_context_quality_value: "MEDIUM".to_string(),
                signal_context_event_fact_value: "rebalance".to_string(),
                signal_context_source_diagnostics_value: "calendar".to_string(),
                signal_context_interpretation_value: "repositioning".to_string(),
                trend_confidence_value: "HIGH".to_string(),
                supply_confidence_value: "MEDIUM".to_string(),
                expectation_confidence_value: "NONE".to_string(),
                gravity_confidence_value: "NONE".to_string(),
                flow_confidence_value: "LOW".to_string(),
                interpretation_quality_value: "MEDIUM".to_string(),
                ..Default::default()
            };
        let pres_for_builder =
            crate::features::radar::interface::presentation::PresentationPacket {
                top_actions: vec![
                    TopActionViewModel {
                        symbol: "SPY".to_string(),
                        ..Default::default()
                    },
                    TopActionViewModel {
                        symbol: "GOOG".to_string(),
                        ..Default::default()
                    },
                    TopActionViewModel {
                        symbol: "U".to_string(),
                        ..Default::default()
                    },
                ],
                exit_summary: ExitDecisionSummaryViewModel {
                    items: vec![ExitDecisionItemViewModel {
                        symbol: "NVDA".to_string(),
                        intent: ExitDisplayIntent::Trim,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                risk_opportunities: vec![RiskOpportunityViewModel {
                    kind: "RISK".to_string(),
                    symbol: "PLTR".to_string(),
                    reason: "rotation".to_string(),
                }],
                interpretation_layer: Some(interpretation_layer.clone()),
                ..Default::default()
            };
        let market_interpretation =
            crate::features::radar::interface::market_interpretation_read_model::build_market_interpretation_view_model(
                &packet,
                &pres_for_builder,
                &crate::features::radar::interface::market_interpretation_read_model::build_leadership_snapshot_view_model(&pres_for_builder, Language::EnUs),
                Language::EnUs,
            )
            .unwrap();
        let pres = crate::features::radar::interface::presentation::PresentationPacket {
            interpretation_layer: Some(interpretation_layer),
            market_interpretation: Some(market_interpretation),
            ..Default::default()
        };

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert!(report.markdown_body.contains("Market Interpretation Layer"));
        assert!(report.markdown_body.contains("dayType: exceptional"));
        assert!(report
            .markdown_body
            .contains("rotationType: index_rotation"));
        assert!(report.markdown_body.contains("observationOnly: true"));
        assert!(report
            .telegram_html_body
            .contains("Market Interpretation Layer"));
        assert!(report
            .telegram_html_body
            .contains("Current decision weight: 0%"));
        assert!(!report.markdown_body.contains("BUY"));
        assert!(!report.markdown_body.contains("SELL"));
        assert!(!report.telegram_html_body.contains("BUY"));
        assert!(!report.telegram_html_body.contains("SELL"));
        assert_eq!(packet.market_regime.market_state, MarketState::IGNITION);
    }

    #[test]
    fn leader_persistence_report_section_is_observation_only() {
        let config = mock_config_with_language(Language::EnUs);
        let leader_persistence =
            crate::features::radar::interface::presentation::LeaderPersistenceViewModel {
                title: "Leader Persistence".to_string(),
                primary_leader_label: "Composite Leader".to_string(),
                primary_leader_value: "GOOG".to_string(),
                persistence_label: "Leader Persistence".to_string(),
                persistence_value: "5 days".to_string(),
                persistence_days: 5,
                leader_absence_duration: 0,
                observed_days_label: "Observed Leadership Days in Lookback".to_string(),
                observed_days_value: "5 days".to_string(),
                breakout_continuity_label: "Breakout Continuity".to_string(),
                breakout_continuity_value: "5 days".to_string(),
                history_coverage_label: "History Coverage".to_string(),
                history_coverage_value: "PARTIAL".to_string(),
                first_observed_at_value: Some("2026-07-01".to_string()),
                previous_leader_value: Some("MSFT".to_string()),
                previous_snapshot_leader_value: Some("MSFT".to_string()),
                last_confirmed_leader_value: Some("MSFT".to_string()),
                leader_absence_since_value: None,
                tactical_leadership_structure_value: "CORE_ASSET_LED".to_string(),
                history_note: Some("Leadership history unavailable before feature activation.".to_string()),
                leadership_score_label: "Leadership Score".to_string(),
                leadership_score_value: "82.4".to_string(),
                leadership_score: 82.4,
                leader_state_label: "Leader State".to_string(),
                leader_state_value: "ESTABLISHED".to_string(),
                change_from_yesterday_label: "Change from Yesterday".to_string(),
                change_from_yesterday_value: "+1 day, score stable".to_string(),
                persistence_change_days: 1,
                score_change: 0.0,
                switch_history_label: "Switch History".to_string(),
                switch_history_values: vec!["2026-07-02: MSFT -> GOOG".to_string()],
                boundary: "Boundary: observation only; this block does not change Decision, Gate, Execution, Trader, or Position Sizing.".to_string(),
            };
        let pres = crate::features::radar::interface::presentation::PresentationPacket {
            leader_persistence: Some(leader_persistence),
            ..Default::default()
        };

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert!(report.markdown_body.contains("Leader Persistence"));
        assert!(report.markdown_body.contains("Composite Leader: GOOG"));
        assert!(!report.markdown_body.contains("Primary Leader: GOOG"));
        assert!(report.markdown_body.contains("Leadership Score: 82.4"));
        assert!(report
            .markdown_body
            .contains("Change from Yesterday: +1 day, score stable"));
        assert!(report.markdown_body.contains("Switch History"));
        assert!(report.markdown_body.contains("2026-07-02: MSFT -> GOOG"));
        assert!(report.telegram_html_body.contains("Leader Persistence"));
    }

    #[test]
    fn absent_leader_renderer_omits_leader_only_metrics() {
        let config = mock_config_with_language(Language::EnUs);
        let leader_persistence =
            crate::features::radar::interface::presentation::LeaderPersistenceViewModel {
                title: "Leader Persistence".to_string(),
                primary_leader_label: "Composite Leader".to_string(),
                primary_leader_value: "none".to_string(),
                persistence_label: "Leader Persistence".to_string(),
                persistence_value: "0 days".to_string(),
                persistence_days: 0,
                leader_absence_duration: 3,
                observed_days_label: "Observed Leadership Days in Lookback".to_string(),
                observed_days_value: "0 days".to_string(),
                breakout_continuity_label: "Breakout Continuity".to_string(),
                breakout_continuity_value: "UNAVAILABLE".to_string(),
                history_coverage_label: "History Coverage".to_string(),
                history_coverage_value: "PARTIAL".to_string(),
                first_observed_at_value: None,
                previous_leader_value: Some("GOOG".to_string()),
                previous_snapshot_leader_value: Some("GOOG".to_string()),
                last_confirmed_leader_value: Some("GOOG".to_string()),
                leader_absence_since_value: Some("2026-07-01".to_string()),
                tactical_leadership_structure_value: "LEADERLESS / FRAGMENTED".to_string(),
                history_note: Some("History coverage is partial.".to_string()),
                leadership_score_label: "Leadership Score".to_string(),
                leadership_score_value: "0.0".to_string(),
                leadership_score: 0.0,
                leader_state_label: "Leader State".to_string(),
                leader_state_value: "ABSENT".to_string(),
                change_from_yesterday_label: "Change from Yesterday".to_string(),
                change_from_yesterday_value: "GOOG -> none".to_string(),
                persistence_change_days: 0,
                score_change: 0.0,
                switch_history_label: "Switch History".to_string(),
                switch_history_values: vec![],
                boundary: "Boundary: observation only".to_string(),
            };
        let pres = crate::features::radar::interface::presentation::PresentationPacket {
            leader_persistence: Some(leader_persistence),
            ..Default::default()
        };

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        for body in [&report.markdown_body, &report.telegram_html_body] {
            assert!(body.contains("Leader State: ABSENT"));
            assert!(body.contains("Leader Absence Duration: 3 trading days"));
            assert!(body.contains("Previous Snapshot Leader: GOOG"));
            assert!(body.contains("Last Confirmed Leader: GOOG"));
            assert!(body.contains("Leader Absence Since: 2026-07-01"));
            assert!(!body.contains("Previous Leader: GOOG"));
            assert!(body.contains("Last Transition: GOOG -> none"));
            assert!(body.contains("History Coverage: PARTIAL"));
            assert!(!body.contains("Leader Persistence: 0 days"));
            assert!(!body.contains("Observed Leadership Days in Lookback"));
            assert!(!body.contains("Leadership Score: 0.0"));
            assert!(!body.contains("Change from Yesterday"));
        }
    }

    #[test]
    fn relative_strength_renderer_is_plain_markdown_in_all_delivery_bodies() {
        let config = mock_config_with_language(Language::EnUs);
        let pres = crate::features::radar::interface::presentation::PresentationPacket {
            current_relative_strength: Some(
                crate::features::radar::interface::presentation::CurrentRelativeStrengthViewModel {
                    title: "Current Relative Strength".to_string(),
                    confirmed_leader: "none".to_string(),
                    items: vec![crate::features::radar::interface::presentation::CurrentRelativeStrengthItemViewModel {
                        symbol: "NVDA".to_string(),
                        status: "IMPROVING".to_string(),
                        relative_1d_vs_benchmark: Some(1.2),
                        relative_5d_vs_benchmark: Some(4.5),
                        price_position: None,
                        volume_participation: None,
                        conflict_code: Some("SIGNAL_CONFLICT".to_string()),
                        recovery_watch: true,
                        recovery_explanation: Some(
                            "长期/累计结构仍弱，但短期相对强度正在明显恢复。".to_string(),
                        ),
                    }],
                    boundary: "Observation only".to_string(),
                },
            ),
            ..Default::default()
        };
        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert!(report
            .markdown_body
            .contains("### Current Relative Strength"));
        assert!(!report.markdown_body.contains("<h3>"));
        assert!(!report.markdown_body.contains("<li>"));
        assert!(report
            .telegram_html_body
            .contains("### Current Relative Strength"));
        assert!(report.telegram_html_body.contains("  - 确认 Leader"));
        assert!(!report
            .telegram_html_body
            .contains("<h3>Current Relative Strength</h3>"));
        assert!(!report.telegram_html_body.contains("<li>确认 Leader"));
        assert!(report
            .archival_markdown
            .contains("### Current Relative Strength"));
        assert!(!report.archival_markdown.contains("<h3>"));
        assert!(!report.archival_markdown.contains("<li>"));
        for body in [&report.markdown_body, &report.telegram_html_body] {
            assert!(body.contains("状态冲突: SIGNAL_CONFLICT"));
            assert!(body.contains("Recovery Watch: RECOVERY_WATCH"));
            assert!(body.contains("长期/累计结构仍弱，但短期相对强度正在明显恢复。"));
        }
    }

    #[test]
    fn leaderless_reconciliation_reaches_all_report_bodies_without_erasing_long_term_context() {
        let config = mock_config_with_language(Language::ZhCn);
        let mut pres = crate::features::radar::interface::presentation::PresentationPacket {
            decision_summary:
                crate::features::radar::interface::presentation::DecisionSummaryViewModel {
                    is_no_trade: true,
                    trend_topology_label: "主线结构".to_string(),
                    trend_topology_value: "核心资产主导".to_string(),
                    ..Default::default()
                },
            transition_evidence: Some(
                crate::features::radar::interface::presentation::StateTransitionViewModel {
                    strategic_context: vec![
                        "市场结构模式: 核心资产主导期".to_string(),
                        "长期方向: 结构证据观察中".to_string(),
                    ],
                    ..Default::default()
                },
            ),
            ..Default::default()
        };

        PresentationAssembler::reconcile_tactical_leadership_display(
            &mut pres,
            "none",
            9,
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

        for body in [
            &report.markdown_body,
            &report.telegram_html_body,
            &report.archival_markdown,
        ] {
            assert!(body.contains("主线结构：无主线"));
            assert!(body.contains("市场结构模式: 结构整理 / 无明确主导"));
            assert!(body.contains("长期方向: 结构证据观察中"));
            assert!(!body.contains("核心资产主导"));
        }
    }

    #[test]
    fn interpretation_report_renders_full_tactical_distribution() {
        let config = mock_config_with_language(Language::ZhCn);
        let packet = DecisionPacket::default();
        let mut pres = crate::features::radar::interface::presentation::PresentationPacket {
            interpretation_layer: Some(Default::default()),
            tactical_buckets: vec![
                crate::features::radar::interface::display::TacticalBucketViewModel {
                    bucket_id: "watch".to_string(),
                    display_name: "观察".to_string(),
                    count: 1,
                    items: vec!["SPCX".to_string()],
                },
                crate::features::radar::interface::display::TacticalBucketViewModel {
                    bucket_id: "hold".to_string(),
                    display_name: "持有".to_string(),
                    count: 0,
                    items: vec![],
                },
                crate::features::radar::interface::display::TacticalBucketViewModel {
                    bucket_id: "defend".to_string(),
                    display_name: "收缩".to_string(),
                    count: 9,
                    items: vec!["A".to_string(); 9],
                },
            ],
            ..Default::default()
        };
        let leadership_snapshot = crate::features::radar::interface::market_interpretation_read_model::build_leadership_snapshot_view_model_from_components(
            vec![],
            vec![],
            vec![],
            false,
            Language::ZhCn,
        );
        pres.market_interpretation = crate::features::radar::interface::market_interpretation_read_model::build_market_interpretation_view_model(
            &packet,
            &pres,
            &leadership_snapshot,
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

        for body in [
            &report.markdown_body,
            &report.telegram_html_body,
            &report.archival_markdown,
        ] {
            assert!(body.contains("动作分布：观察 1 / 持有 0 / 收缩 9。"));
        }
    }

    #[test]
    fn improving_relative_strength_is_reported_as_signal_conflict_without_changing_action() {
        let config = mock_config_with_language(Language::ZhCn);
        let asset = AssetActionDecision {
            symbol: "SPCX".to_string(),
            action: crate::features::radar::domain::action_matrix::AssetAction::REDUCE,
            exit_decision: ExitDecision {
                position_intent: PositionIntent::TRIM,
                asset_exit_state: AssetExitState::StrengthLoss,
                reasons: vec![],
            },
            ..Default::default()
        };
        let original_action = asset.action;
        let packet = DecisionPacket {
            assets: vec![asset.clone()],
            current_relative_strength_observations: vec![
                crate::features::radar::domain::current_relative_strength::CurrentRelativeStrengthObservation {
                    symbol: "SPCX".to_string(),
                    relative_1d_vs_benchmark: Some(6.34),
                    relative_5d_vs_benchmark: Some(6.87),
                    price_position: None,
                    volume_participation: None,
                    status: crate::features::radar::domain::current_relative_strength::CurrentRelativeStrengthStatus::Improving,
                    boundary: "Observation only".to_string(),
                },
            ],
            ..Default::default()
        };
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );
        let item = pres
            .current_relative_strength
            .as_ref()
            .and_then(|strength| strength.items.first())
            .expect("SPCX current relative strength item");

        assert_eq!(item.conflict_code.as_deref(), Some("SIGNAL_CONFLICT"));
        assert!(item.recovery_watch);
        assert!(item
            .recovery_explanation
            .as_deref()
            .is_some_and(|value| value.contains("RECOVERY_WATCH")));
        assert_eq!(asset.action, original_action);
    }

    #[test]
    fn market_interpretation_conflict_suppresses_leadership_lists() {
        let config = mock_config_with_language(Language::EnUs);
        let packet = DecisionPacket {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            market_features: crate::features::radar::domain::features::MarketFeatures {
                flow_acceleration: Some(0.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["GOOG".to_string()];
        let interpretation_layer = build_interpretation_layer_view_model(
            InterpretationLayerReadModelInput {
                as_of_date: packet.date,
                subjects: &subjects,
                signal: InterpretationNarrativeSignal {
                    trend_available: true,
                    trend_state: InterpretationTrendState::Stable,
                    expectation_quality: InterpretationExpectationQuality::High,
                    expectation_quality_reason:
                        crate::features::radar::interface::presentation::InterpretationExpectationQualityReason::MarketConsensusAvailable,
                    gravity_data_quality: InterpretationGravityDataQuality::Ready,
                    gravity_data_quality_reason:
                        InterpretationGravityDataQualityReason::ConsensusUnavailable,
                    gravity_status: Some(
                        crate::features::research::domain::valuation_gravity::GravityStatus::Fair,
                    ),
                    supply_available: true,
                    supply_pressure: false,
                    flow_acceleration: Some(0.0),
                    gray_rhino_escalated: false,
                },
                future_context: SignalContextEventReadModel::default(),
                decision_summary: None,
                language: Language::EnUs,
                dict: &dict,
            },
        );

        let mut pres = crate::features::radar::interface::presentation::PresentationPacket {
            date_str: "2026-06-18".to_string(),
            language: Language::EnUs,
            macro_display: Default::default(),
            decision_summary: Default::default(),
            final_execution_decision: Default::default(),
            signal_summary: Default::default(),
            top_actions: vec![TopActionViewModel {
                symbol: "GOOG".to_string(),
                ..Default::default()
            }],
            exit_summary: ExitDecisionSummaryViewModel {
                items: vec![ExitDecisionItemViewModel {
                    symbol: "GOOG".to_string(),
                    intent: ExitDisplayIntent::Exit,
                    ..Default::default()
                }],
                ..Default::default()
            },
            breakout_summary: Default::default(),
            tactical_buckets: vec![],
            risk_opportunity_summary: Default::default(),
            risk_opportunities: vec![],
            notices: vec![],
            data_alert: None,
            transition_evidence: Some(crate::features::radar::interface::presentation::StateTransitionViewModel {
                trend_breadth_mode:
                    crate::features::radar::interface::presentation::TrendBreadthMode::NarrowLeadership,
                market_cycle_position:
                    crate::features::radar::interface::presentation::MarketCyclePosition::CrowdedExpectation,
                ..Default::default()
            }),
            interpretation_layer: Some(interpretation_layer.clone()),
            leadership_snapshot: None,
            leader_persistence: None,
            current_relative_strength: None,
            market_change_log: None,
            market_interpretation: None,
            hypothesis_layer: None,
            terminal_rows: vec![],
            state_code: String::new(),
        };

        let market_interpretation =
            crate::features::radar::interface::market_interpretation_read_model::build_market_interpretation_view_model(
                &packet,
                &pres,
                &crate::features::radar::interface::market_interpretation_read_model::build_leadership_snapshot_view_model(&pres, Language::EnUs),
                Language::EnUs,
            )
            .expect("market interpretation should be available");
        assert_eq!(
            market_interpretation.leadership_classification_value,
            "MEDIUM"
        );
        pres.market_interpretation = Some(market_interpretation);

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert!(!report.markdown_body.contains("primary: [GOOG]"));
        assert!(!report.markdown_body.contains("supporting: [GOOG]"));
        assert!(!report.markdown_body.contains("weakening: [GOOG]"));
    }

    #[test]
    fn test_quarter_end_and_official_source_unavailable_is_unavailable() {
        use crate::features::radar::interface::interpretation_read_model::InterpretationNarrativeSignal;
        use crate::features::radar::interface::presentation::{
            SignalContextInformationContent, SignalContextPrimaryContext,
        };
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::radar::interface::signal_context_read_model::{
            build_signal_context_assessment, SignalContextReadModelInput,
        };
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::Language;

        let as_of_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(); // Quarter end
        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Unavailable,
            ..Default::default()
        };

        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date,
            signal: InterpretationNarrativeSignal::default(),
            future_context,
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::QuarterEndRebalancing
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Unknown
        );
    }

    #[test]
    fn test_month_end_and_official_source_unavailable_is_unavailable() {
        use crate::features::radar::interface::interpretation_read_model::InterpretationNarrativeSignal;
        use crate::features::radar::interface::presentation::{
            SignalContextInformationContent, SignalContextPrimaryContext,
        };
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::radar::interface::signal_context_read_model::{
            build_signal_context_assessment, SignalContextReadModelInput,
        };
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::Language;

        let as_of_date = chrono::NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(); // Month end
        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Unavailable,
            ..Default::default()
        };

        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date,
            signal: InterpretationNarrativeSignal::default(),
            future_context,
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::MonthEndRebalancing
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Unknown
        );
    }

    #[test]
    fn test_none_primary_context_and_loaded_context_is_unavailable_without_full_scan() {
        use crate::features::radar::interface::interpretation_read_model::InterpretationNarrativeSignal;
        use crate::features::radar::interface::presentation::{
            SignalContextInformationContent, SignalContextPrimaryContext,
        };
        use crate::features::radar::interface::signal_context_event_read_model::{
            SignalContextEventReadModel, SignalContextEventSlot,
        };
        use crate::features::radar::interface::signal_context_read_model::{
            build_signal_context_assessment, SignalContextReadModelInput,
        };
        use crate::features::shared::interface::i18n::Language;

        let as_of_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let future_context = SignalContextEventReadModel {
            index_reconstitution: SignalContextEventSlot::Loaded(None),
            ..Default::default()
        };

        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date,
            signal: InterpretationNarrativeSignal::default(),
            future_context,
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Unknown
        );
    }

    #[test]
    fn test_none_primary_context_and_no_loaded_context_is_unknown() {
        use crate::features::radar::interface::interpretation_read_model::InterpretationNarrativeSignal;
        use crate::features::radar::interface::presentation::{
            SignalContextInformationContent, SignalContextPrimaryContext,
        };
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::radar::interface::signal_context_read_model::{
            build_signal_context_assessment, SignalContextReadModelInput,
        };
        use crate::features::shared::interface::i18n::Language;

        let as_of_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let future_context = SignalContextEventReadModel::default();

        let assessment = build_signal_context_assessment(SignalContextReadModelInput {
            as_of_date,
            signal: InterpretationNarrativeSignal::default(),
            future_context,
            language: Language::EnUs,
        });

        assert_eq!(
            assessment.primary_context,
            SignalContextPrimaryContext::None
        );
        assert_eq!(
            assessment.information_content,
            SignalContextInformationContent::Unknown
        );
    }

    #[test]
    fn test_expectation_unavailable_and_source_unavailable_shows_source_unavailable() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::presentation::InterpretationExpectationQuality;
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let signal = InterpretationNarrativeSignal {
            expectation_quality: InterpretationExpectationQuality::Unavailable,
            gray_rhino_escalated: false,
            ..Default::default()
        };

        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Unavailable,
            ..Default::default()
        };

        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string()];

        let view_model = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            subjects: &subjects,
            signal,
            future_context,
            decision_summary: None,
            language: Language::EnUs,
            dict: &dict,
        });

        assert!(view_model
            .expectation_next_observation_value
            .contains("Official source is currently unavailable"));
    }

    #[test]
    fn test_pending_event_shows_waiting_for_official_release() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::presentation::{
            InterpretationExpectationQuality, SignalContextQuality,
        };
        use crate::features::radar::interface::signal_context_event_read_model::{
            SignalContextEventReadModel, SignalContextEventSlot, SignalContextEvidence,
            SignalContextEvidenceSource,
        };
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let signal = InterpretationNarrativeSignal {
            expectation_quality: InterpretationExpectationQuality::High,
            gray_rhino_escalated: false,
            ..Default::default()
        };

        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Succeeded,
            pre_earnings_waiting: SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                detected: true,
                quality: SignalContextQuality::High,
                source: SignalContextEvidenceSource::Calendar,
                summary: "Earnings".to_string(),
            })),
            ..Default::default()
        };

        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string()];

        let view_model = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            subjects: &subjects,
            signal,
            future_context,
            decision_summary: None,
            language: Language::EnUs,
            dict: &dict,
        });

        assert!(
            view_model
                .expectation_next_observation_value
                .contains("Waiting")
                || view_model
                    .expectation_next_observation_value
                    .contains("No high-information events")
                || view_model
                    .expectation_next_observation_value
                    .contains("official calendar update")
        );
    }

    #[test]
    fn test_quarter_end_expectation_state_is_low_even_with_unavailable_source() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Unavailable,
            ..Default::default()
        };

        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string()];

        let view_model = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(), // Quarter end
            subjects: &subjects,
            signal: InterpretationNarrativeSignal::default(),
            future_context,
            decision_summary: None,
            language: Language::EnUs,
            dict: &dict,
        });

        assert_eq!(view_model.expectation_lifecycle_value, "UNAVAILABLE");
        assert!(view_model
            .expectation_next_observation_value
            .contains("Official source is currently unavailable"));
    }

    #[test]
    fn test_none_primary_context_and_source_succeeded_expectation_state_is_unknown() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Succeeded,
            ..Default::default()
        };

        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string()];

        let view_model = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            subjects: &subjects,
            signal: InterpretationNarrativeSignal::default(),
            future_context,
            decision_summary: None,
            language: Language::EnUs,
            dict: &dict,
        });

        assert_eq!(view_model.expectation_lifecycle_value, "UNKNOWN");
        assert!(view_model
            .expectation_next_observation_value
            .contains("Unable to confirm if events exist"));
    }

    #[test]
    fn test_expectation_observing_japanese_typo_is_fixed() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::presentation::SignalContextQuality;
        use crate::features::radar::interface::signal_context_event_read_model::{
            SignalContextEventReadModel, SignalContextEventSlot, SignalContextEvidence,
            SignalContextEvidenceSource,
        };
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let signal = InterpretationNarrativeSignal::default();

        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Succeeded,
            macro_event: SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                detected: true,
                quality: SignalContextQuality::High,
                source: SignalContextEvidenceSource::Calendar,
                summary: "CPI Release".to_string(),
            })),
            ..Default::default()
        };

        let dict = get_dictionary(Language::JaJp);
        let subjects = vec!["TSLA".to_string()];

        let view_model = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            subjects: &subjects,
            signal,
            future_context,
            decision_summary: None,
            language: Language::JaJp,
            dict: &dict,
        });

        assert_eq!(view_model.expectation_lifecycle_value, "OBSERVING");
        assert!(view_model
            .expectation_next_observation_value
            .contains("予想の修正"));
        assert!(!view_model
            .expectation_next_observation_value
            .contains("期待 of の修正"));
    }

    #[test]
    fn test_interpretation_gravity_unavailable_degradation() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::presentation::{
            InterpretationExpectationQuality, InterpretationGravityDataQuality,
        };
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let signal = InterpretationNarrativeSignal {
            gravity_data_quality: InterpretationGravityDataQuality::Unavailable,
            trend_available: true,
            expectation_quality: InterpretationExpectationQuality::High,
            supply_available: true,
            flow_acceleration: Some(0.1),
            gray_rhino_escalated: false,
            ..Default::default()
        };

        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Succeeded,
            ..Default::default()
        };

        for language in [Language::EnUs, Language::ZhCn, Language::JaJp] {
            let dict = get_dictionary(language);
            let view_model =
                build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
                    as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
                    subjects: &["TSLA".to_string()],
                    signal,
                    future_context: future_context.clone(),
                    decision_summary: None,
                    language,
                    dict: &dict,
                });

            // 1. Quality must reflect the degradation (should be MEDIUM, not HIGH)
            assert_eq!(view_model.interpretation_quality_value, "MEDIUM");

            // 2. The summary must stay compact and hand off the human narrative to Market Interpretation
            let main_text = &view_model.interpretation_value;

            match language {
                Language::EnUs => {
                    assert!(main_text.contains("See Market Interpretation for the main narrative."));
                    assert!(!main_text.contains("trend continuation"));
                    assert!(!main_text.contains("valuation"));
                }
                Language::ZhCn => {
                    assert!(main_text.contains("主叙事见 Market Interpretation。"));
                    assert!(!main_text.contains("趋势"));
                    assert!(!main_text.contains("估值"));
                }
                Language::JaJp => {
                    assert!(main_text.contains("主叙事は Market Interpretation を参照。"));
                    assert!(!main_text.contains("トレンド"));
                    assert!(!main_text.contains("バリュエーション"));
                }
            }
        }
    }

    #[test]
    fn test_interpretation_gravity_partial_degradation() {
        use crate::features::radar::interface::interpretation_read_model::{
            build_interpretation_layer_view_model, InterpretationLayerReadModelInput,
            InterpretationNarrativeSignal,
        };
        use crate::features::radar::interface::presentation::{
            InterpretationExpectationQuality, InterpretationGravityDataQuality,
        };
        use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
        use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
        use crate::features::shared::interface::i18n::{get_dictionary, Language};

        let signal = InterpretationNarrativeSignal {
            gravity_data_quality: InterpretationGravityDataQuality::Partial,
            trend_available: true,
            expectation_quality: InterpretationExpectationQuality::High,
            supply_available: true,
            flow_acceleration: Some(0.1),
            gray_rhino_escalated: false,
            ..Default::default()
        };

        let future_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Succeeded,
            ..Default::default()
        };

        for language in [Language::EnUs, Language::ZhCn, Language::JaJp] {
            let dict = get_dictionary(language);
            let view_model =
                build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
                    as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
                    subjects: &["TSLA".to_string()],
                    signal,
                    future_context: future_context.clone(),
                    decision_summary: None,
                    language,
                    dict: &dict,
                });

            let main_text = &view_model.interpretation_value;

            match language {
                Language::EnUs => {
                    assert!(main_text.contains("See Market Interpretation for the main narrative."));
                    assert!(!main_text.contains("trend continuation"));
                    assert!(!main_text.contains("valuation"));
                }
                Language::ZhCn => {
                    assert!(main_text.contains("主叙事见 Market Interpretation。"));
                    assert!(!main_text.contains("趋势"));
                    assert!(!main_text.contains("估值"));
                }
                Language::JaJp => {
                    assert!(main_text.contains("主叙事は Market Interpretation を参照。"));
                    assert!(!main_text.contains("トレンド"));
                    assert!(!main_text.contains("バリュエーション"));
                }
            }
        }
    }

    #[test]
    fn test_hypothesis_without_failure_risks_is_not_rendered() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            SubstantiveEvidence, TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;

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

        let config = mock_config_with_language(Language::ZhCn);
        let mut pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::ZhCn,
        );
        pres.hypothesis_layer.as_mut().unwrap().candidates[0]
            .failure_risks
            .clear();

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert!(!report.telegram_html_body.contains("未来地图"));
        assert!(!report.archival_markdown.contains("未来地图"));
    }

    #[test]
    fn test_hypothesis_layer_renders_in_en_and_ja() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
            TrendContinuationState, TrendRecognitionEvidence,
        };
        use crate::features::shared::interface::i18n::Language;
        use chrono::NaiveDate;

        for (
            language,
            title,
            notice,
            beneficiary_label,
            summary_label,
            age_label,
            validation_label,
        ) in [
            (
                Language::EnUs,
                "Future Map / Hypothesis Layer",
                "not current facts",
                "Potential Beneficiaries",
                "Summary: GPU demand",
                "Hypothesis Age: 30 days",
                "Thesis Validation: 3/5",
            ),
            (
                Language::JaJp,
                "未来地図 / Hypothesis Layer",
                "現在の事実ではなく",
                "潜在的受益者",
                "要約: GPU 需要",
                "仮説年齢: 30 日",
                "命題検証: 3/5",
            ),
        ] {
            let mut curr = DecisionPacket::default();
            curr.date = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
            curr.trend_recognition = Some(TrendRecognitionEvidence {
                state: TrendContinuationState::StructuralPersistence,
                diffusion_score: 3.4,
                conviction_score: 3.4,
                substantive: Some(SubstantiveEvidence {
                    capex_payoff_signal: true,
                    earnings_validation: true,
                    order_visibility: true,
                    records: vec![
                        AutomatedEvidenceRecord::new(
                            EvidenceSourceType::OfficialIR,
                            EvidenceType::CapexPayoff,
                            0.9,
                            "capex payoff".to_string(),
                            "2026-05-01".to_string(),
                            Some("MSFT".to_string()),
                            Some("https://example.com/capex".to_string()),
                            "capex".to_string(),
                        ),
                        AutomatedEvidenceRecord::new(
                            EvidenceSourceType::OfficialIR,
                            EvidenceType::EarningsValidation,
                            0.9,
                            "earnings quality".to_string(),
                            "2026-05-20".to_string(),
                            Some("MSFT".to_string()),
                            Some("https://example.com/earnings".to_string()),
                            "earnings".to_string(),
                        ),
                        AutomatedEvidenceRecord::new(
                            EvidenceSourceType::OfficialIR,
                            EvidenceType::OrderVisibility,
                            0.9,
                            "order visibility".to_string(),
                            "2026-05-25".to_string(),
                            Some("MSFT".to_string()),
                            Some("https://example.com/order".to_string()),
                            "order".to_string(),
                        ),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            });
            curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

            let config = mock_config_with_language(language);
            let pres = PresentationAssembler::assemble(
                &curr,
                &domain_rules(&config),
                &HashMap::new(),
                vec![],
                language,
            );
            let report = generate_refined_report(
                &report_context(&config),
                &pres,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            assert!(report.telegram_html_body.contains(title));
            assert!(report.telegram_html_body.contains(notice));
            assert!(report.telegram_html_body.contains(beneficiary_label));
            assert!(report.telegram_html_body.contains(summary_label));
            assert!(report.telegram_html_body.contains(age_label));
            assert!(report.telegram_html_body.contains(validation_label));
            assert!(
                report.telegram_html_body.contains("Failure")
                    || report.telegram_html_body.contains("失敗")
            );
        }
    }

    #[test]
    fn test_narrow_leadership_is_displayed_as_structural_consolidation_without_gate_change() {
        use crate::features::radar::domain::asset_state::{AssetState, AssetStateSnapshot};
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::radar::domain::trend_cohesion::{
            SubstantiveEvidence, TrendCohesionSnapshot, TrendContinuationState,
            TrendRecognitionEvidence,
        };
        use crate::features::radar::interface::presentation::{
            HoldingEfficiency, MarketCyclePosition, TrendBreadthMode,
        };
        use crate::features::shared::interface::i18n::Language;

        let mut curr = DecisionPacket {
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                risk_overlay: RiskOverlay::DEFENSIVE,
                ..Default::default()
            },
            trend_cohesion: TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            market_features: crate::features::radar::domain::features::MarketFeatures {
                up_count: 3,
                down_count: 6,
                total_count: 9,
                system_confidence: 49.0,
                stability_score: 0.8,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "SPY".to_string(),
                    asset_state: AssetStateSnapshot {
                        symbol: "SPY".to_string(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "MSFT".to_string(),
                    asset_state: AssetStateSnapshot {
                        symbol: "MSFT".to_string(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "GOOG".to_string(),
                    asset_state: AssetStateSnapshot {
                        symbol: "GOOG".to_string(),
                        state: AssetState::CRUISE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".to_string(),
                    has_position_fact: true,
                    asset_state: AssetStateSnapshot {
                        symbol: "NVDA".to_string(),
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
            trend_recognition: Some(TrendRecognitionEvidence {
                state: TrendContinuationState::EarlyLeader,
                diffusion_score: 3.90,
                conviction_score: 3.40,
                lag_state: false,
                single_asset_decay_day: 1,
                single_asset_decay_max: 3,
                substantive: Some(SubstantiveEvidence {
                    capex_payoff_signal: true,
                    earnings_validation: true,
                    order_visibility: true,
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };
        curr.transition_log = Some(StateTransitionLog::compare(None, &curr));

        for (
            language,
            expected_regime,
            forbidden_regime,
            expected_mode,
            expected_cycle,
            expected_crowding,
            expected_holding_efficiency,
            expected_tactical,
            expected_fragility,
        ) in [
            (
                Language::ZhCn,
                "**市场状态**: 结构整理期",
                "**市场状态**: 保命期",
                "市场结构模式: 核心资产主导期",
                "周期位置: CROWDED_EXPECTATION",
                "拥挤风险: ACTIVE",
                "持有效率: NEUTRAL",
                "战术状态: NO TRADE，等待结构扩散",
                "结构脆弱: 暂停主动进攻，等待扩散恢复",
            ),
            (
                Language::EnUs,
                "**Market State**: Structural Consolidation",
                "**Market State**: Protect",
                "Market Structure Mode: Narrow Leadership",
                "Cycle Position: CROWDED_EXPECTATION",
                "Crowding Risk: ACTIVE",
                "Holding Efficiency: NEUTRAL",
                "Tactical Status: NO TRADE, waiting for structural diffusion",
                "Structural Fragility: pause active offense and wait for diffusion recovery",
            ),
            (
                Language::JaJp,
                "**市場状態**: 構造整理期",
                "**市場状態**: 守備期",
                "市場構造モード: コア資産主導期",
                "サイクル位置: CROWDED_EXPECTATION",
                "混雑リスク: ACTIVE",
                "保有效率: NEUTRAL",
                "戦術状態: NO TRADE、構造拡散待ち",
                "構造脆弱：能動的な攻勢を停止し、拡散回復を待つ",
            ),
        ] {
            let config = mock_config_with_language(language);
            let pres = PresentationAssembler::assemble(
                &curr,
                &domain_rules(&config),
                &HashMap::new(),
                vec![],
                language,
            );
            let transition = pres.transition_evidence.as_ref().unwrap();
            assert_eq!(
                transition.trend_breadth_mode,
                TrendBreadthMode::NarrowLeadership
            );
            assert_eq!(
                transition.market_cycle_position,
                MarketCyclePosition::CrowdedExpectation
            );
            assert_eq!(transition.holding_efficiency, HoldingEfficiency::Neutral);
            assert!(!transition.trend_cohesion_gate_passed);

            let report = generate_refined_report(
                &report_context(&config),
                &pres,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            assert!(report.archival_markdown.contains(expected_regime));
            assert!(!report.archival_markdown.contains(forbidden_regime));
            assert!(report.archival_markdown.contains(expected_mode));
            assert!(report.archival_markdown.contains(expected_cycle));
            assert!(report.archival_markdown.contains(expected_crowding));
            assert!(report
                .archival_markdown
                .contains(expected_holding_efficiency));
            assert!(report.archival_markdown.contains(expected_tactical));
            assert!(report.archival_markdown.contains(expected_fragility));
            assert!(!report.archival_markdown.contains("保命层强制退出"));
            assert!(!report.archival_markdown.contains("Safety layer activated"));
            assert!(!report.archival_markdown.contains("安全層を発動"));
        }
    }

    #[test]
    fn test_no_trade_persistence_explanation_en_us() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

        let prev = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            ..Default::default()
        };

        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::EnUs);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let md = report.archival_markdown;
        assert!(md.contains("🔄 State Transition Evidence"));
        // 永続化される explanation を確認する。
        assert!(md.contains("NO TRADE Persists"));
    }

    #[test]
    fn test_transition_evidence_rendering_ja_jp() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

        let prev = DecisionPacket {
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                risk_overlay: RiskOverlay::BROKEN,
                ..Default::default()
            },
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                status:
                    crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed,
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
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                status:
                    crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming,
                ..Default::default()
            },
            ..Default::default()
        };

        // transition log を計算する。
        curr.transition_log = Some(StateTransitionLog::compare(Some(&prev), &curr));

        let config = mock_config_with_language(Language::JaJp);
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let md = report.archival_markdown;
        // localize された section title を確認する。
        assert!(md.contains("🔄 状態遷移エビデンス"));
        // localize された state change を確認する。
        // defensive -> ignition maps to 守備期 -> 始動期
        assert!(md.contains("守備期 -> 始動期"));
        assert!(md.contains("分散 -> 形成中"));
        // topology change が描画されることを確認する。
        assert!(
            md.contains("主導不在 -> 形成中")
                || md.contains("主線構造の変化")
                || md.contains("主導不在")
        );
    }

    #[test]
    fn test_transition_evidence_breakout_changes_focus_on_structural_deltas() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

        let prev = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "U".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "GOOG".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
                        failed_breakout_risk: 61.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        failed_breakout_risk: 82.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "U".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
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
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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
        let md = report.archival_markdown;

        assert!(md.contains("**关键变化**"));
        assert!(md.contains("GOOG：新增突破萌芽"));
        assert!(md.contains("其余资产：无结构变化"));
    }

    #[test]
    fn test_transition_evidence_not_shown_for_breakout_risk_only_change() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

        let prev = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "NVDA".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                    failed_breakout_risk: 10.0,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "NVDA".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
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
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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
        let md = report.archival_markdown;
        assert!(md.contains("🔄 状态转移证据"));
        assert!(!md.contains("关键变化"));
        assert!(!md.contains("NVDA："));
    }

    #[test]
    fn test_transition_evidence_renders_scout_status_only_in_transition_block() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

        let prev = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
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
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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
        let html = report.telegram_html_body;

        assert!(html.contains("<b>🔄 状态转移证据</b>"));
        assert!(html.contains("侦察状态"));
        assert!(html.contains("breakout 连续性: 1/3"));
        assert!(html.contains("扩散: 无（单资产）"));
        assert!(html.contains("reset: 否"));
    }

    #[test]
    fn test_scout_multi_point_expansion_does_not_render_zero_ratio() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

        let prev = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "U".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![
                AssetActionDecision {
                    symbol: "NVDA".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AssetActionDecision {
                    symbol: "U".into(),
                    breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                        status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
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
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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
        let html = report.telegram_html_body;

        assert!(html.contains("侦察状态"));
        assert!(html.contains("breakout 连续性: multi-point"));
        assert!(html.contains("扩散: 多点"));
        assert!(!html.contains("breakout 连续性: 0/3"));
    }

    #[test]
    fn test_transition_evidence_renders_scout_status_in_en_and_ja() {
        use crate::features::radar::domain::transition_log::StateTransitionLog;
        use crate::features::shared::interface::i18n::Language;

        let prev = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut curr = DecisionPacket {
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: false,
                ..Default::default()
            },
            assets: vec![AssetActionDecision {
                symbol: "GOOG".into(),
                breakout: crate::features::radar::domain::breakout_detection::BreakoutSnapshot {
                    status: crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout,
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
            &domain_rules(&config_en),
            &HashMap::new(),
            vec![],
            Language::EnUs,
        );
        let report_en = generate_refined_report(
            &report_context(&config_en),
            &pres_en,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
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
            &domain_rules(&config_ja),
            &HashMap::new(),
            vec![],
            Language::JaJp,
        );
        let report_ja = generate_refined_report(
            &report_context(&config_ja),
            &pres_ja,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
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
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            language,
        );

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        let md_compact = report.markdown_body.clone();
        let html_compact = report.telegram_html_body.clone();
        let md = report.archival_markdown;

        // Cohesion Gate check（中国語）。
        assert!(md.contains("新增阻碍"));
        assert!(md.contains("已消除阻碍"));
        assert!(md.contains("持续阻碍"));

        // Trend Gate check（中国語）。
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

        // user-facing markdown では NO TRADE 時の transition evidence を compact に保つ。
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

        // Telegram HTML でも同じ execution-first ordering を維持する。
        assert_no_trade_html_execution_order(Language::ZhCn, &html_compact);
    }

    #[test]
    fn test_transition_evidence_can_be_expanded_in_no_trade_via_config() {
        let curr = no_trade_transition_reason_diff_packet();
        let mut config = mock_config_with_language(Language::ZhCn);
        config.output.compact_transition_evidence_in_no_trade = false;
        let pres = PresentationAssembler::assemble(
            &curr,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
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

        let md = report.markdown_body;
        assert!(md.contains("新增阻碍: 主导方向分散或领导者缺失"));
    }

    #[test]
    fn test_no_trade_snapshot_zh_cn_markdown() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::ZhCn);
        assert_snapshot("no_trade_zh_cn.md", &report.markdown_body);
    }

    #[test]
    fn test_no_trade_snapshot_zh_cn_html() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::ZhCn);
        assert_snapshot("no_trade_zh_cn.html.txt", &report.telegram_html_body);
    }

    #[test]
    fn test_no_trade_snapshot_breakout_age_displays_day_one_in_zh_cn() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::ZhCn);
        assert!(report.markdown_body.contains("GOOG · 突破萌芽（第1天）"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · 突破萌芽（第1天）"));
    }

    #[test]
    fn test_no_trade_snapshot_breakout_age_displays_day_one_in_en_us() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::EnUs);
        assert!(report
            .markdown_body
            .contains("GOOG · Emerging Breakout (Day 1)"));
        assert!(report
            .telegram_html_body
            .contains("GOOG · Emerging Breakout (Day 1)"));
    }

    #[test]
    fn test_no_trade_snapshot_breakout_age_displays_day_one_in_ja_jp() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::JaJp);
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
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::EnUs);
        assert_snapshot("no_trade_en_us.md", &report.markdown_body);
    }

    #[test]
    fn test_no_trade_snapshot_en_us_html() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::EnUs);
        assert_snapshot("no_trade_en_us.html.txt", &report.telegram_html_body);
    }

    #[test]
    fn test_no_trade_snapshot_ja_jp_markdown() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::JaJp);
        assert_snapshot("no_trade_ja_jp.md", &report.markdown_body);
    }

    #[test]
    fn test_no_trade_snapshot_ja_jp_html() {
        let report =
            build_no_trade_report(crate::features::shared::interface::i18n::Language::JaJp);
        assert_snapshot("no_trade_ja_jp.html.txt", &report.telegram_html_body);
    }

    #[test]
    fn test_ssot_uniqueness_and_consistency() {
        let packet = no_trade_snapshot_packet();
        let config = mock_config_with_language(Language::ZhCn);
        let lang = Language::ZhCn;
        let pres = PresentationAssembler::assemble(
            &packet,
            &domain_rules(&config),
            &HashMap::new(),
            vec![],
            lang,
        );

        let report = generate_refined_report(
            &report_context(&config),
            &pres,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        let body = report.markdown_body;

        // 1. 「市场状态」marker が dual engine で重複しないことを確認する。
        let market_state_marker = "市场状态";
        assert_eq!(
            body.matches(market_state_marker).count(),
            1,
            "Market state should only be reported once (SSOT violation)"
        );

        // 2. 中国語 report に日本語 header が混入しないことを確認する。
        assert!(
            !body.contains("マーケット状态サマリー"),
            "Japanese headers found in Chinese report"
        );

        // 3. stability は threshold format を維持し、continuity は state label を使うことを確認する。
        // From no_trade_snapshot_packet: stability=1.1, continuity_streak=1.
        assert!(
            body.contains("1.1/10"),
            "Stability threshold should use unified (current/threshold) format"
        );
        assert!(
            body.contains("连续性 emerging"),
            "Continuity should use state labels instead of unbounded ratios"
        );

        // 4. redundant な英語 threshold 文が残らないことを確認する。
        // 正しく localize され、新 format に揃っていることを確認する。
        assert!(
            !body.contains("Stability score (1.1) below threshold (10.0)"),
            "Legacy English threshold message should be suppressed or localized"
        );
    }

    #[test]
    fn main_report_renders_observation_timeline_summary() {
        let config = mock_config_with_language(Language::ZhCn);
        let mut context = report_context(&config);
        context.observation_timeline = Some(
            crate::features::radar::domain::observation_timeline::ObservationTimeline {
                history_coverage:
                    crate::features::radar::domain::observation_timeline::HistoryCoverage::Partial,
                entries: vec![
                    crate::features::radar::domain::observation_timeline::ObservationTimelineEntry {
                        date: NaiveDate::from_ymd_opt(2026, 7, 13).unwrap(),
                        primary_leader: "SPY".to_string(),
                        secondary_leaders: vec![],
                        breadth_score: Some(50.0),
                        breadth_raw_percent: Some(70.0),
                        concentration_score: 50.0,
                        rotation_score: 50.0,
                        confidence_index: 50.0,
                        market_state: "RANGE".to_string(),
                        supply_phase: "WATCH".to_string(),
                        risk_state: "NORMAL".to_string(),
                        day_type: "NORMAL".to_string(),
                        ..Default::default()
                    },
                    crate::features::radar::domain::observation_timeline::ObservationTimelineEntry {
                        date: NaiveDate::from_ymd_opt(2026, 7, 14).unwrap(),
                        primary_leader: "MSFT".to_string(),
                        secondary_leaders: vec![],
                        breadth_score: Some(60.0),
                        breadth_raw_percent: Some(80.0),
                        concentration_score: 55.0,
                        rotation_score: 45.0,
                        confidence_index: 65.0,
                        market_state: "TREND".to_string(),
                        supply_phase: "WATCH".to_string(),
                        risk_state: "NORMAL".to_string(),
                        day_type: "NORMAL".to_string(),
                        ..Default::default()
                    },
                ],
                summary: "STRUCTURAL_CHANGE".to_string(),
            },
        );
        for (language, title, leader_label, breadth_label, confidence_label, supply_label) in [
            (
                Language::ZhCn,
                "市场演化观察",
                "主导者序列",
                "市场广度原始值序列",
                "置信度序列",
                "供给阶段序列",
            ),
            (
                Language::EnUs,
                "Observation Timeline",
                "Leader sequence",
                "Breadth Raw sequence",
                "Confidence sequence",
                "Supply sequence",
            ),
            (
                Language::JaJp,
                "市場進化観測",
                "主導銘柄の推移",
                "市場広度Rawの推移",
                "確信度の推移",
                "供給局面の推移",
            ),
        ] {
            let presentation =
                crate::features::radar::interface::presentation::PresentationPacket {
                    language,
                    ..Default::default()
                };
            let report = generate_refined_report(
                &context,
                &presentation,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            assert!(report.markdown_body.contains(title));
            assert!(report.markdown_body.contains("PARTIAL"));
            assert!(report.markdown_body.contains(leader_label));
            assert!(report.markdown_body.contains(breadth_label));
            assert!(
                report
                    .markdown_body
                    .contains("Breadth Classification Score sequence")
                    || report.markdown_body.contains("广度分类分数序列")
                    || report.markdown_body.contains("市場広度分類スコアの推移")
            );
            assert!(report.markdown_body.contains(confidence_label));
            assert!(report.markdown_body.contains(supply_label));
            assert!(report.markdown_body.contains("SPY → MSFT"));
            assert!(report.markdown_body.contains("UNAVAILABLE → UNAVAILABLE"));
            assert!(!report.markdown_body.contains("WATCH"));
            assert!(!report.markdown_body.contains("2026-07-14"));
            match language {
                Language::ZhCn => {
                    assert!(report.markdown_body.contains("7日趋势结论暂不生成"));
                    assert!(!report
                        .markdown_body
                        .contains("过去 7 个交易日未出现结构性变化"));
                }
                Language::EnUs => assert!(report
                    .markdown_body
                    .contains("The 7-day trend conclusion is not generated")),
                Language::JaJp => assert!(report
                    .markdown_body
                    .contains("7日間のトレンド結論は生成しません")),
            }
            assert!(!report.markdown_body.contains("Leader 序列"));
            assert!(!report.markdown_body.contains("Breadth 序列"));
            assert!(!report.markdown_body.contains("Confidence 序列"));
            assert!(!report.markdown_body.contains("Supply 序列"));
            assert!(!report.markdown_body.contains("Leader 推移"));
            assert!(!report.markdown_body.contains("Breadth 推移"));
            assert!(!report.markdown_body.contains("Confidence 推移"));
            assert!(!report.markdown_body.contains("Supply 推移"));
            if language != Language::ZhCn {
                assert!(!report.markdown_body.contains("过去 7 个交易日"));
            }
        }
    }

    #[test]
    fn market_change_log_never_renders_raw_breadth_as_a_classification_in_any_language() {
        for language in [Language::ZhCn, Language::EnUs, Language::JaJp] {
            let config = mock_config_with_language(language);
            let presentation =
                crate::features::radar::interface::presentation::PresentationPacket {
                    language,
                    market_change_log: Some(
                        crate::features::radar::interface::presentation::MarketChangeLogViewModel {
                            baseline_status: "AVAILABLE".to_string(),
                            change_status: "DETERMINED".to_string(),
                            title: "Market Change Log".to_string(),
                            breadth_label: "Breadth".to_string(),
                            breadth_value: "Very Narrow".to_string(),
                            summary_values: vec!["Breadth remains Very Narrow.".to_string()],
                            ..Default::default()
                        },
                    ),
                    ..Default::default()
                };

            let report = generate_refined_report(
                &report_context(&config),
                &presentation,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            assert!(report
                .markdown_body
                .contains("Breadth remains Very Narrow."));
            assert!(!report
                .markdown_body
                .contains("Breadth shifted from 35.0 to Very Narrow."));
        }
    }

    #[test]
    fn report_renders_signal_context_lifecycle_and_observation_facts() {
        use crate::features::radar::interface::presentation::{
            InterpretationLayerViewModel, PresentationPacket,
        };
        use crate::features::shared::interface::i18n::Language;

        let config = mock_config_with_language(Language::EnUs);
        let presentation = PresentationPacket {
            language: Language::EnUs,
            interpretation_layer: Some(InterpretationLayerViewModel {
                signal_context_lifecycle_label: "Lifecycle".to_string(),
                signal_context_lifecycle_value: "RELEASED".to_string(),
                signal_context_expected_label: "Expected".to_string(),
                signal_context_expected_value: "2.9%".to_string(),
                signal_context_actual_label: "Actual".to_string(),
                signal_context_actual_value: "3.1%".to_string(),
                signal_context_surprise_label: "Surprise".to_string(),
                signal_context_surprise_value: "+0.2%".to_string(),
                signal_context_reason_label: "Reason".to_string(),
                signal_context_reason_value: "EVENT_DATA_UNAVAILABLE".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let report = generate_refined_report(
            &report_context(&config),
            &presentation,
            0.0,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert!(report.markdown_body.contains("Lifecycle: RELEASED"));
        assert!(report.markdown_body.contains("Expected: 2.9%"));
        assert!(report.markdown_body.contains("Actual: 3.1%"));
        assert!(report.markdown_body.contains("Surprise: +0.2%"));
        assert!(report
            .markdown_body
            .contains("Reason: EVENT_DATA_UNAVAILABLE"));
    }

    #[test]
    fn report_localizes_signal_context_fact_labels_in_markdown_and_html() {
        use crate::features::radar::interface::presentation::{
            InterpretationLayerViewModel, PresentationPacket,
        };
        use crate::features::shared::interface::i18n::Language;

        for (language, labels) in [
            (
                Language::ZhCn,
                ["生命周期", "预期", "实际", "意外值", "原因"],
            ),
            (
                Language::EnUs,
                ["Lifecycle", "Expected", "Actual", "Surprise", "Reason"],
            ),
            (
                Language::JaJp,
                ["ライフサイクル", "予想", "実績", "サプライズ", "理由"],
            ),
        ] {
            let config = mock_config_with_language(language);
            let presentation = PresentationPacket {
                language,
                interpretation_layer: Some(InterpretationLayerViewModel {
                    signal_context_lifecycle_value: "RELEASED".to_string(),
                    signal_context_expected_value: "2.9%".to_string(),
                    signal_context_actual_value: "3.1%".to_string(),
                    signal_context_surprise_value: "+0.2%".to_string(),
                    signal_context_reason_value: "EVENT_DATA_UNAVAILABLE".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let report = generate_refined_report(
                &report_context(&config),
                &presentation,
                0.0,
                &HashMap::new(),
                &HashMap::new(),
            )
            .unwrap();

            for label in labels {
                assert!(report.markdown_body.contains(label));
                assert!(report.telegram_html_body.contains(label));
            }
            assert!(report.markdown_body.contains("RELEASED"));
            assert!(report.telegram_html_body.contains("EVENT_DATA_UNAVAILABLE"));
        }
    }
}
