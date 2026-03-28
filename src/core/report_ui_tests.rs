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
        assert!(card.contains("监控信号"));
        assert!(card.contains("战术分区"));
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
        assert!(card.contains("主要アクション"));
        assert!(card.contains("強気"));
        assert!(card.contains("監視シグナル"));
        assert!(!card.contains("市场摘要"));
    }
}
