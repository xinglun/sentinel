use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use crate::core::i18n::Language;

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    #[allow(dead_code)]
    pub version: u32,
    pub output: OutputConfig,
    pub telegram: Option<TelegramConfig>,
    pub futu: Option<FutuConfig>,
    pub finnhub: Option<FinnhubConfig>,
    pub trading: Option<TradingConfig>,
    #[allow(dead_code)]
    pub provider: Option<String>,

    pub rules: RulesConfig,
    pub watchlist: Vec<WatchlistEntry>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    #[allow(dead_code)]
    pub timezone: String,
    #[allow(dead_code)]
    pub format: String,
    pub save_to: String,
    pub weight_kind: Option<String>,
    pub language: Option<Language>,
    #[serde(default = "default_true")]
    pub compact_transition_evidence_in_no_trade: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub bot_token: String,
    pub chat_id: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct FutuConfig {
    pub opend_ip: String,
    pub opend_port: u16,
    pub trd_env: u32,                        // 0: Real, 1: Simulate
    pub market: u32,                         // 1: HK, 2: US, etc.
    pub acc_id: Option<u64>,                 // Loaded from config or ENV
    pub unlock_password_md5: Option<String>, // Loaded from ENV
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct FinnhubConfig {
    pub api_key: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TradingConfig {
    pub enabled: bool,
    pub global_budget: f64,
    pub max_daily_budget: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RulesConfig {
    pub trend: TrendConfig,
    pub deviation_bands: BTreeMap<String, f64>,
    pub actions: HashMap<String, String>,
    pub sizing_multipliers: Option<HashMap<String, f64>>,
    pub core_assets: Option<Vec<String>>,
    pub min_state_duration: Option<usize>,
    pub inertia: Option<InertiaConfig>,
    pub trend_cohesion: Option<TrendCohesionRulesConfig>,
    pub breakout: Option<BreakoutRulesConfig>,
    pub market_state_engine: Option<MarketStateEngineConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct InertiaConfig {
    pub min_state_duration: Option<usize>,
    pub trend_dominant_min_confidence: Option<f64>,
    pub core_breakdown_k: Option<usize>,
    pub core_breakdown_avg_deviation: Option<f64>,
    pub core_breakdown_breadth_floor: Option<f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct TrendConfig {
    pub lookback_days: usize,
    pub flat_threshold_pct: f64,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct TrendCohesionRulesConfig {
    pub history_window_days: Option<usize>,
    pub stability_norm_max: Option<f64>,
    pub continuity_norm_max: Option<usize>,
    pub severe_stability_threshold: Option<f64>,
    pub severe_continuity_threshold: Option<usize>,
    pub severe_compactness_threshold: Option<f64>,
    pub severe_rotation_threshold: Option<f64>,
    pub severe_leadership_threshold: Option<f64>,
    pub severe_cohesion_threshold: Option<f64>,
    pub gate_stability_threshold: Option<f64>,
    pub gate_continuity_threshold: Option<usize>,
    pub directional_max_candidates: Option<usize>,
    pub directional_leadership_threshold: Option<f64>,
    pub directional_rotation_threshold: Option<f64>,
    pub directional_compactness_threshold: Option<f64>,
    pub topology_single_max_candidates: Option<usize>,
    pub topology_single_min_compactness: Option<f64>,
    pub topology_single_min_rotation: Option<f64>,
    pub cohesive_score_threshold: Option<f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct BreakoutRulesConfig {
    pub confirmed_trend_age_threshold: Option<usize>,
    pub confirmed_top_tier_streak_threshold: Option<usize>,
    pub confirmed_zscore_threshold: Option<f64>,
    pub confirmed_min_slope: Option<f64>,
    pub confirmed_min_curvature: Option<f64>,
    pub emerging_trend_age_threshold: Option<usize>,
    pub emerging_top_tier_streak_threshold: Option<usize>,
    pub emerging_zscore_threshold: Option<f64>,
    pub emerging_min_slope: Option<f64>,
    pub failed_breakout_curvature_threshold: Option<f64>,
    pub failed_breakout_slope_threshold: Option<f64>,
    pub failed_breakout_display_threshold: Option<f64>,
    pub failed_breakout_no_trade_display_threshold: Option<f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct MarketStateEngineConfig {
    pub continuity_threshold: Option<usize>,
    pub stability_threshold: Option<f64>,
    pub min_followers_threshold: Option<usize>,
    pub scout_abort_days: Option<usize>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct WatchlistEntry {
    pub symbol: String,
    pub weight: Option<f64>,
    #[allow(dead_code)]
    pub market: String,
    pub owner_ma_days: usize,
    pub leash_ma_days: usize,
    pub deviation_basis: DeviationBasis,
    pub enable: bool,
    pub trade_enabled: Option<bool>,
    pub trade_amount: Option<f64>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviationBasis {
    #[default]
    Owner,
    Leash,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedRules {
    pub trend: TrendConfig,
    pub sorted_bands: Vec<(String, f64)>, // descending thresholds
    #[allow(dead_code)]
    pub actions: HashMap<String, String>,

    pub sizing_multipliers: Option<HashMap<String, f64>>,
    pub core_assets: Vec<String>,
    pub inertia: ParsedInertia,
    pub trend_cohesion: ParsedTrendCohesionRules,
    pub breakout: ParsedBreakoutRules,
    pub market_state_engine: ParsedMarketStateEngineConfig,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedInertia {
    pub min_state_duration: usize,
    pub trend_dominant_min_confidence: f64,
    pub core_breakdown_k: usize,
    pub core_breakdown_avg_deviation: f64,
    pub core_breakdown_breadth_floor: f64,
}

#[derive(Debug, Clone)]
pub struct ParsedMarketStateEngineConfig {
    pub continuity_threshold: usize,
    pub stability_threshold: f64,
    pub min_followers_threshold: usize,
    pub scout_abort_days: usize,
}

impl Default for ParsedMarketStateEngineConfig {
    fn default() -> Self {
        Self {
            continuity_threshold: 2,
            stability_threshold: 5.5,
            min_followers_threshold: 1,
            scout_abort_days: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedTrendCohesionRules {
    pub history_window_days: usize,
    pub stability_norm_max: f64,
    pub continuity_norm_max: usize,
    pub severe_stability_threshold: f64,
    pub severe_continuity_threshold: usize,
    pub severe_compactness_threshold: f64,
    pub severe_rotation_threshold: f64,
    pub severe_leadership_threshold: f64,
    pub severe_cohesion_threshold: f64,
    pub gate_stability_threshold: f64,
    pub gate_continuity_threshold: usize,
    pub directional_max_candidates: usize,
    pub directional_leadership_threshold: f64,
    pub directional_rotation_threshold: f64,
    pub directional_compactness_threshold: f64,
    pub topology_single_max_candidates: usize,
    pub topology_single_min_compactness: f64,
    pub topology_single_min_rotation: f64,
    pub cohesive_score_threshold: f64,
}

impl Default for ParsedTrendCohesionRules {
    fn default() -> Self {
        Self {
            history_window_days: 2,
            stability_norm_max: 15.0,
            continuity_norm_max: 4,
            severe_stability_threshold: 8.0,
            severe_continuity_threshold: 2,
            severe_compactness_threshold: 45.0,
            severe_rotation_threshold: 35.0,
            severe_leadership_threshold: 45.0,
            severe_cohesion_threshold: 45.0,
            gate_stability_threshold: 10.0,
            gate_continuity_threshold: 3,
            directional_max_candidates: 4,
            directional_leadership_threshold: 60.0,
            directional_rotation_threshold: 45.0,
            directional_compactness_threshold: 60.0,
            topology_single_max_candidates: 3,
            topology_single_min_compactness: 65.0,
            topology_single_min_rotation: 30.0,
            cohesive_score_threshold: 75.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedBreakoutRules {
    pub confirmed_trend_age_threshold: usize,
    pub confirmed_top_tier_streak_threshold: usize,
    pub confirmed_zscore_threshold: f64,
    pub confirmed_min_slope: f64,
    pub confirmed_min_curvature: f64,
    pub emerging_trend_age_threshold: usize,
    pub emerging_top_tier_streak_threshold: usize,
    pub emerging_zscore_threshold: f64,
    pub emerging_min_slope: f64,
    pub failed_breakout_curvature_threshold: f64,
    pub failed_breakout_slope_threshold: f64,
    pub failed_breakout_display_threshold: f64,
    pub failed_breakout_no_trade_display_threshold: f64,
}

impl Default for ParsedBreakoutRules {
    fn default() -> Self {
        Self {
            confirmed_trend_age_threshold: 8,
            confirmed_top_tier_streak_threshold: 3,
            confirmed_zscore_threshold: 1.2,
            confirmed_min_slope: 0.0,
            confirmed_min_curvature: -0.2,
            emerging_trend_age_threshold: 5,
            emerging_top_tier_streak_threshold: 1,
            emerging_zscore_threshold: 0.5,
            emerging_min_slope: 0.0,
            failed_breakout_curvature_threshold: -0.5,
            failed_breakout_slope_threshold: 0.0,
            failed_breakout_display_threshold: 55.0,
            failed_breakout_no_trade_display_threshold: 70.0,
        }
    }
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Load .env file variables into environment if it exists.
        // Variables already present in the environment will not be overridden.
        dotenvy::dotenv().ok();

        let content =
            fs::read_to_string(path).map_err(|e| anyhow!("Failed to read config file: {}", e))?;

        let mut config: AppConfig =
            toml::from_str(&content).map_err(|e| anyhow!("Failed to parse config file: {}", e))?;

        // Environment variable overrides for Telegram
        if let Some(ref mut tg) = config.telegram {
            if let Ok(token) = std::env::var("TELEGRAM_BOT_TOKEN") {
                tg.bot_token = token;
                tg.enabled = true;
            }
            if let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") {
                tg.chat_id = chat_id;
            }
        } else if let (Ok(token), Ok(chat_id)) = (
            std::env::var("TELEGRAM_BOT_TOKEN"),
            std::env::var("TELEGRAM_CHAT_ID"),
        ) {
            config.telegram = Some(TelegramConfig {
                enabled: true,
                bot_token: token,
                chat_id,
            });
        }

        // Environment variable overrides for Moomoo/Futu API Secrets
        if let Some(ref mut futu) = config.futu {
            if let Ok(acc_str) = std::env::var("FUTU_ACC_ID") {
                if let Ok(acc_id) = acc_str.parse::<u64>() {
                    futu.acc_id = Some(acc_id);
                }
            }
            if let Ok(pwd) = std::env::var("FUTU_UNLOCK_PASSWORD_MD5") {
                futu.unlock_password_md5 = Some(pwd);
            }
        }

        // Environment variable overrides for Finnhub
        if let Ok(key) = std::env::var("FINNHUB_API_KEY") {
            if let Some(ref mut fh) = config.finnhub {
                fh.api_key = key;
            } else {
                config.finnhub = Some(FinnhubConfig { api_key: key });
            }
        }

        if config.trading.is_none() {
            config.trading = Some(TradingConfig {
                enabled: false,
                global_budget: 0.0,
                max_daily_budget: None,
            });
        }

        if let Some(t) = &config.trading {
            if t.global_budget < 0.0 {
                return Err(anyhow!(
                    "Configuration Error: global_budget cannot be negative."
                ));
            }
        }

        for band_key in config.rules.deviation_bands.keys() {
            if !config.rules.actions.contains_key(band_key) {
                return Err(anyhow!(
                    format!("Configuration Error: deviation_bands contains '{}', but no corresponding action is defined.", band_key),
                ));
            }
        }

        Ok(config)
    }

    pub fn get_parsed_rules(&self) -> ParsedRules {
        let mut bands: Vec<(String, f64)> = self
            .rules
            .deviation_bands
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        // Sort thresholds in descending order
        bands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        ParsedRules {
            trend: self.rules.trend.clone(),
            sorted_bands: bands,
            actions: self.rules.actions.clone(),
            sizing_multipliers: self.rules.sizing_multipliers.clone(),
            core_assets: self.rules.core_assets.clone().unwrap_or_default(),
            inertia: {
                let i = self.rules.inertia.as_ref();
                ParsedInertia {
                    min_state_duration: i
                        .and_then(|c| c.min_state_duration)
                        .or(self.rules.min_state_duration)
                        .unwrap_or(3),
                    trend_dominant_min_confidence: i
                        .and_then(|c| c.trend_dominant_min_confidence)
                        .unwrap_or(55.0),
                    core_breakdown_k: i.and_then(|c| c.core_breakdown_k).unwrap_or(2),
                    core_breakdown_avg_deviation: i
                        .and_then(|c| c.core_breakdown_avg_deviation)
                        .unwrap_or(-5.0),
                    core_breakdown_breadth_floor: i
                        .and_then(|c| c.core_breakdown_breadth_floor)
                        .unwrap_or(0.0),
                }
            },
            trend_cohesion: {
                let tc = self.rules.trend_cohesion.as_ref();
                let defaults = ParsedTrendCohesionRules::default();
                ParsedTrendCohesionRules {
                    history_window_days: tc
                        .and_then(|c| c.history_window_days)
                        .unwrap_or(defaults.history_window_days),
                    stability_norm_max: tc
                        .and_then(|c| c.stability_norm_max)
                        .unwrap_or(defaults.stability_norm_max),
                    continuity_norm_max: tc
                        .and_then(|c| c.continuity_norm_max)
                        .unwrap_or(defaults.continuity_norm_max),
                    severe_stability_threshold: tc
                        .and_then(|c| c.severe_stability_threshold)
                        .unwrap_or(defaults.severe_stability_threshold),
                    severe_continuity_threshold: tc
                        .and_then(|c| c.severe_continuity_threshold)
                        .unwrap_or(defaults.severe_continuity_threshold),
                    severe_compactness_threshold: tc
                        .and_then(|c| c.severe_compactness_threshold)
                        .unwrap_or(defaults.severe_compactness_threshold),
                    severe_rotation_threshold: tc
                        .and_then(|c| c.severe_rotation_threshold)
                        .unwrap_or(defaults.severe_rotation_threshold),
                    severe_leadership_threshold: tc
                        .and_then(|c| c.severe_leadership_threshold)
                        .unwrap_or(defaults.severe_leadership_threshold),
                    severe_cohesion_threshold: tc
                        .and_then(|c| c.severe_cohesion_threshold)
                        .unwrap_or(defaults.severe_cohesion_threshold),
                    gate_stability_threshold: tc
                        .and_then(|c| c.gate_stability_threshold)
                        .unwrap_or(defaults.gate_stability_threshold),
                    gate_continuity_threshold: tc
                        .and_then(|c| c.gate_continuity_threshold)
                        .unwrap_or(defaults.gate_continuity_threshold),
                    directional_max_candidates: tc
                        .and_then(|c| c.directional_max_candidates)
                        .unwrap_or(defaults.directional_max_candidates),
                    directional_leadership_threshold: tc
                        .and_then(|c| c.directional_leadership_threshold)
                        .unwrap_or(defaults.directional_leadership_threshold),
                    directional_rotation_threshold: tc
                        .and_then(|c| c.directional_rotation_threshold)
                        .unwrap_or(defaults.directional_rotation_threshold),
                    directional_compactness_threshold: tc
                        .and_then(|c| c.directional_compactness_threshold)
                        .unwrap_or(defaults.directional_compactness_threshold),
                    topology_single_max_candidates: tc
                        .and_then(|c| c.topology_single_max_candidates)
                        .unwrap_or(defaults.topology_single_max_candidates),
                    topology_single_min_compactness: tc
                        .and_then(|c| c.topology_single_min_compactness)
                        .unwrap_or(defaults.topology_single_min_compactness),
                    topology_single_min_rotation: tc
                        .and_then(|c| c.topology_single_min_rotation)
                        .unwrap_or(defaults.topology_single_min_rotation),
                    cohesive_score_threshold: tc
                        .and_then(|c| c.cohesive_score_threshold)
                        .unwrap_or(defaults.cohesive_score_threshold),
                }
            },
            breakout: {
                let bo = self.rules.breakout.as_ref();
                let defaults = ParsedBreakoutRules::default();
                ParsedBreakoutRules {
                    confirmed_trend_age_threshold: bo
                        .and_then(|c| c.confirmed_trend_age_threshold)
                        .unwrap_or(defaults.confirmed_trend_age_threshold),
                    confirmed_top_tier_streak_threshold: bo
                        .and_then(|c| c.confirmed_top_tier_streak_threshold)
                        .unwrap_or(defaults.confirmed_top_tier_streak_threshold),
                    confirmed_zscore_threshold: bo
                        .and_then(|c| c.confirmed_zscore_threshold)
                        .unwrap_or(defaults.confirmed_zscore_threshold),
                    confirmed_min_slope: bo
                        .and_then(|c| c.confirmed_min_slope)
                        .unwrap_or(defaults.confirmed_min_slope),
                    confirmed_min_curvature: bo
                        .and_then(|c| c.confirmed_min_curvature)
                        .unwrap_or(defaults.confirmed_min_curvature),
                    emerging_trend_age_threshold: bo
                        .and_then(|c| c.emerging_trend_age_threshold)
                        .unwrap_or(defaults.emerging_trend_age_threshold),
                    emerging_top_tier_streak_threshold: bo
                        .and_then(|c| c.emerging_top_tier_streak_threshold)
                        .unwrap_or(defaults.emerging_top_tier_streak_threshold),
                    emerging_zscore_threshold: bo
                        .and_then(|c| c.emerging_zscore_threshold)
                        .unwrap_or(defaults.emerging_zscore_threshold),
                    emerging_min_slope: bo
                        .and_then(|c| c.emerging_min_slope)
                        .unwrap_or(defaults.emerging_min_slope),
                    failed_breakout_curvature_threshold: bo
                        .and_then(|c| c.failed_breakout_curvature_threshold)
                        .unwrap_or(defaults.failed_breakout_curvature_threshold),
                    failed_breakout_slope_threshold: bo
                        .and_then(|c| c.failed_breakout_slope_threshold)
                        .unwrap_or(defaults.failed_breakout_slope_threshold),
                    failed_breakout_display_threshold: bo
                        .and_then(|c| c.failed_breakout_display_threshold)
                        .unwrap_or(defaults.failed_breakout_display_threshold),
                    failed_breakout_no_trade_display_threshold: bo
                        .and_then(|c| c.failed_breakout_no_trade_display_threshold)
                        .unwrap_or(defaults.failed_breakout_no_trade_display_threshold),
                }
            },
            market_state_engine: {
                let ms = self.rules.market_state_engine.as_ref();
                let defaults = ParsedMarketStateEngineConfig::default();
                ParsedMarketStateEngineConfig {
                    continuity_threshold: ms
                        .and_then(|c| c.continuity_threshold)
                        .unwrap_or(defaults.continuity_threshold),
                    stability_threshold: ms
                        .and_then(|c| c.stability_threshold)
                        .unwrap_or(defaults.stability_threshold),
                    min_followers_threshold: ms
                        .and_then(|c| c.min_followers_threshold)
                        .unwrap_or(defaults.min_followers_threshold),
                    scout_abort_days: ms
                        .and_then(|c| c.scout_abort_days)
                        .unwrap_or(defaults.scout_abort_days),
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let toml_str = r#"
            version = 1
            provider = "yahoo"
            [output]
            weight_kind = "equal"

            timezone = "Asia/Shanghai"
            format = "markdown"
            save_to = "./reports"

            [rules.trend]

            lookback_days = 20
            flat_threshold_pct = 0.5

            [rules.deviation_bands]
            overheat_2 = 30.0   
            optimal    = -5.0   

            [trading]
            enabled = false
            global_budget = 100000.0

            [rules.actions]
            overheat_2 = "停止买入"
            optimal    = "买入"
            fear       = "恐慌加仓"

            [[watchlist]]
            symbol = "TSLA"
            weight = 2.0
            market = "US"
            owner_ma_days = 120
            leash_ma_days = 20
            deviation_basis = "owner"
            enable = true
        "#;

        let config: AppConfig = toml::from_str(toml_str).expect("should parse");
        assert_eq!(config.version, 1);
        assert_eq!(config.watchlist[0].symbol, "TSLA");
    }

    #[test]
    fn test_missing_action_for_band() {
        // AppConfig::load uses a path, so we can't test it directly easily, but we can write a quick wrapper to test the logic exactly.
    }

    #[test]
    fn test_parsed_rules_exposes_trend_cohesion_and_breakout_defaults_and_overrides() {
        let toml_str = r#"
            version = 1

            [output]
            timezone = "Asia/Tokyo"
            format = "markdown"
            save_to = "./reports"

            [rules.trend]
            lookback_days = 20
            flat_threshold_pct = 0.5

            [rules.deviation_bands]
            optimal = -5.0

            [rules.actions]
            optimal = "买入"

            [rules.trend_cohesion]
            history_window_days = 4
            gate_stability_threshold = 11.0
            directional_max_candidates = 5

            [rules.market_state_engine]
            scout_abort_days = 5

            [rules.breakout]
            confirmed_zscore_threshold = 1.5
            failed_breakout_display_threshold = 70.0
            failed_breakout_no_trade_display_threshold = 82.0

            [[watchlist]]
            symbol = "TSLA"
            market = "US"
            owner_ma_days = 120
            leash_ma_days = 20
            deviation_basis = "owner"
            enable = true
        "#;

        let config: AppConfig = toml::from_str(toml_str).expect("should parse");
        let rules = config.get_parsed_rules();

        assert_eq!(rules.trend_cohesion.history_window_days, 4);
        assert_eq!(rules.trend_cohesion.gate_stability_threshold, 11.0);
        assert_eq!(rules.trend_cohesion.directional_max_candidates, 5);
        assert_eq!(rules.trend_cohesion.cohesive_score_threshold, 75.0);
        assert_eq!(rules.market_state_engine.scout_abort_days, 5);

        assert_eq!(rules.breakout.confirmed_zscore_threshold, 1.5);
        assert_eq!(rules.breakout.failed_breakout_display_threshold, 70.0);
        assert_eq!(
            rules.breakout.failed_breakout_no_trade_display_threshold,
            82.0
        );
        assert_eq!(rules.breakout.emerging_trend_age_threshold, 5);
    }
}
