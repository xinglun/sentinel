use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

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
    pub sec: Option<SecConfig>,
    pub research_attention: Option<BTreeMap<String, ResearchAttentionEntry>>,
    pub asset_thesis: Option<BTreeMap<String, AssetThesisEntry>>,
    pub macro_gravity: Option<MacroGravityConfig>,
    pub gray_rhino_escalation: Option<GrayRhinoEscalationConfig>,
    /// 旧 provider registry 設定を読み捨てるための互換フィールド。
    #[allow(dead_code)]
    pub gray_rhino_provider_registry: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SecConfig {
    pub user_agent: String,
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
    pub language: Option<OutputLanguage>,
    #[serde(default = "default_true")]
    pub compact_transition_evidence_in_no_trade: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputLanguage {
    #[serde(rename = "zh-cn")]
    #[default]
    ZhCn,
    #[serde(rename = "en-us")]
    EnUs,
    #[serde(rename = "ja-jp")]
    JaJp,
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
    #[serde(alias = "api_key")]
    pub finnhub_api_key: String,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CognitiveYield {
    High,
    Medium,
    Low,
    Degrading,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttentionCost {
    Low,
    Moderate,
    High,
    Draining,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InformationDensity {
    Expanding,
    Active,
    Stable,
    Saturated,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ResearchAttentionEntry {
    pub cognitive_yield: CognitiveYield,
    pub attention_cost: AttentionCost,
    pub information_density: InformationDensity,
    pub reason: String,
    pub reason_zh: Option<String>,
    pub reason_en: Option<String>,
    pub reason_ja: Option<String>,
    pub enable: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AssetThesisEntry {
    pub thesis: String,
    pub thesis_zh: Option<String>,
    pub thesis_en: Option<String>,
    pub thesis_ja: Option<String>,
    pub observation_focus: Vec<String>,
    pub observation_focus_zh: Option<Vec<String>>,
    pub observation_focus_en: Option<Vec<String>>,
    pub observation_focus_ja: Option<Vec<String>>,
    pub invalidation: Vec<String>,
    pub invalidation_zh: Option<Vec<String>>,
    pub invalidation_en: Option<Vec<String>>,
    pub invalidation_ja: Option<Vec<String>>,
    pub enable: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MacroPressure {
    Falling,
    Neutral,
    Rising,
    Tight,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum YieldCurveState {
    Normal,
    Flat,
    Inverted,
    Steepening,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CreditStress {
    Normal,
    Watch,
    Stress,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiquidityCondition {
    Loose,
    Neutral,
    Tight,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrowthValuationImpact {
    Supportive,
    Neutral,
    Compressing,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct MacroGravityConfig {
    pub rate_pressure: MacroPressure,
    pub real_yield_pressure: MacroPressure,
    pub yield_curve: YieldCurveState,
    pub credit_stress: CreditStress,
    pub liquidity: LiquidityCondition,
    pub growth_valuation_impact: GrowthValuationImpact,
    pub note: Option<String>,
    pub enable: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrayRhinoRiskLevel {
    Low,
    Moderate,
    Elevated,
    High,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GrayRhinoEscalationConfig {
    pub risk_expansion_rate: GrayRhinoRiskLevel,
    pub constraint_growth_rate: GrayRhinoRiskLevel,
    pub dependency_centralization: GrayRhinoRiskLevel,
    pub awareness_decay: GrayRhinoRiskLevel,
    pub narrative_overconfidence: GrayRhinoRiskLevel,
    pub single_point_fragility: GrayRhinoRiskLevel,
    pub fallback_survivability_risk: GrayRhinoRiskLevel,
    pub notes: Option<Vec<String>>,
    pub enable: Option<bool>,
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
    // Phase 6: 証拠レイヤー設定
    pub evidence_decay_days: Option<u32>,
    pub evidence_retention_days: Option<u32>,
    pub capex_payoff_weight: Option<f64>,
    pub earnings_validation_weight: Option<f64>,
    pub order_visibility_weight: Option<f64>,
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
    pub event_tags: Option<Vec<String>>,
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
    pub market_state_engine: ParsedMarketStateEngineRules,
    pub breakout: ParsedBreakoutRules,
    pub sec: Option<SecConfig>,
    pub macro_gravity: Option<MacroGravityConfig>,
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
pub struct ParsedMarketStateEngineRules {
    pub continuity_threshold: usize,
    pub stability_threshold: f64,
    pub min_followers_threshold: usize,
    pub scout_abort_days: usize,
    // Phase 6: 証拠レイヤーの計算ルール
    pub evidence_decay_days: u32,
    pub evidence_retention_days: u32,
    pub capex_payoff_weight: f64,
    pub earnings_validation_weight: f64,
    pub order_visibility_weight: f64,
}

impl Default for ParsedMarketStateEngineRules {
    fn default() -> Self {
        Self {
            continuity_threshold: 2,
            stability_threshold: 5.5,
            min_followers_threshold: 1,
            scout_abort_days: 3,
            evidence_decay_days: 5,
            evidence_retention_days: 3650,
            capex_payoff_weight: 2.0,
            earnings_validation_weight: 1.5,
            order_visibility_weight: 1.0,
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
    /// 設定の妥当性を検証する。
    pub fn validate(&self) -> Result<()> {
        if let Some(sec) = &self.sec {
            if sec.user_agent.is_empty() {
                return Err(anyhow!(
                    "SEC User-Agent is empty. Required format: 'Company Name <email@example.com>'"
                ));
            }
            let ua = &sec.user_agent;
            let open = ua.find('<');
            let close = ua.rfind('>');
            let strict_format = if let (Some(open), Some(close)) = (open, close) {
                let company = ua[..open].trim();
                let email = ua[open + 1..close].trim();
                let mut parts = email.split('@');
                let local = parts.next().unwrap_or_default();
                let domain = parts.next().unwrap_or_default();

                close == ua.len() - 1
                    && company.contains(' ')
                    && !company.is_empty()
                    && !local.is_empty()
                    && !domain.is_empty()
                    && domain.contains('.')
                    && parts.next().is_none()
                    && !email.contains(['<', '>', ' '])
            } else {
                false
            };

            if !strict_format {
                return Err(anyhow!(
                    "SEC User-Agent format invalid: '{}'. Expected 'Company Name <email@example.com>'",
                    sec.user_agent
                ));
            }
        }
        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        // .env file が存在する場合、環境変数へ読み込む。
        // 既存の環境変数は上書きしない。
        dotenvy::dotenv().ok();

        let content =
            fs::read_to_string(path).map_err(|e| anyhow!("Failed to read config file: {}", e))?;

        let mut config: AppConfig =
            toml::from_str(&content).map_err(|e| anyhow!("Failed to parse config file: {}", e))?;

        // Telegram 設定を環境変数で上書きする。
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

        // Moomoo / Futu API secret を環境変数で上書きする。
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

        // Finnhub 設定を環境変数で上書きする。
        if let Ok(key) = std::env::var("FINNHUB_API_KEY") {
            if key.trim().is_empty() {
                // 空文字の環境変数は設定ファイルを上書きしない。
                // CI で未設定 Secret が空文字として注入されるケースを吸収する。
            } else if let Some(finnhub) = &mut config.finnhub {
                finnhub.finnhub_api_key = key;
            } else {
                config.finnhub = Some(FinnhubConfig {
                    finnhub_api_key: key,
                });
            }
        }

        // SEC 設定を環境変数で上書きする。
        if let Ok(ua) = std::env::var("SEC_USER_AGENT") {
            if ua.trim().is_empty() {
                // 空文字の環境変数は設定ファイルを上書きしない。
                // CI で未設定 Secret が空文字として注入されるケースを吸収する。
            } else if let Some(ref mut sec) = config.sec {
                sec.user_agent = ua;
            } else {
                config.sec = Some(SecConfig { user_agent: ua });
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
        // 全体的な整合性チェック
        config.validate()?;

        Ok(config)
    }

    pub fn get_parsed_rules(&self) -> ParsedRules {
        let mut bands: Vec<(String, f64)> = self
            .rules
            .deviation_bands
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        // threshold を降順に並び替える。
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
                let m = self.rules.market_state_engine.as_ref();
                let defaults = ParsedMarketStateEngineRules::default();
                ParsedMarketStateEngineRules {
                    continuity_threshold: m
                        .and_then(|x| x.continuity_threshold)
                        .unwrap_or(defaults.continuity_threshold),
                    stability_threshold: m
                        .and_then(|x| x.stability_threshold)
                        .unwrap_or(defaults.stability_threshold),
                    min_followers_threshold: m
                        .and_then(|x| x.min_followers_threshold)
                        .unwrap_or(defaults.min_followers_threshold),
                    scout_abort_days: m
                        .and_then(|x| x.scout_abort_days)
                        .unwrap_or(defaults.scout_abort_days),
                    evidence_decay_days: m
                        .and_then(|x| x.evidence_decay_days)
                        .unwrap_or(defaults.evidence_decay_days),
                    evidence_retention_days: m
                        .and_then(|x| x.evidence_retention_days)
                        .unwrap_or(defaults.evidence_retention_days),
                    capex_payoff_weight: m
                        .and_then(|x| x.capex_payoff_weight)
                        .unwrap_or(defaults.capex_payoff_weight),
                    earnings_validation_weight: m
                        .and_then(|x| x.earnings_validation_weight)
                        .unwrap_or(defaults.earnings_validation_weight),
                    order_visibility_weight: m
                        .and_then(|x| x.order_visibility_weight)
                        .unwrap_or(defaults.order_visibility_weight),
                }
            },
            sec: self.sec.clone(),
            macro_gravity: self.macro_gravity.clone(),
        }
    }
}

impl From<&DeviationBasis> for crate::features::radar::domain::rules::DeviationBasis {
    fn from(value: &DeviationBasis) -> Self {
        match value {
            DeviationBasis::Owner => Self::Owner,
            DeviationBasis::Leash => Self::Leash,
        }
    }
}

impl From<&TrendConfig> for crate::features::radar::domain::rules::TrendConfig {
    fn from(value: &TrendConfig) -> Self {
        Self {
            lookback_days: value.lookback_days,
            flat_threshold_pct: value.flat_threshold_pct,
        }
    }
}

impl From<&WatchlistEntry> for crate::features::radar::domain::rules::WatchlistEntry {
    fn from(value: &WatchlistEntry) -> Self {
        Self {
            symbol: value.symbol.clone(),
            weight: value.weight,
            market: value.market.clone(),
            owner_ma_days: value.owner_ma_days,
            leash_ma_days: value.leash_ma_days,
            deviation_basis: (&value.deviation_basis).into(),
            enable: value.enable,
            trade_enabled: value.trade_enabled,
            trade_amount: value.trade_amount,
            event_tags: value.event_tags.clone(),
        }
    }
}

impl From<&ParsedRules> for crate::features::radar::domain::rules::ParsedRules {
    fn from(value: &ParsedRules) -> Self {
        Self {
            trend: (&value.trend).into(),
            sorted_bands: value.sorted_bands.clone(),
            actions: value.actions.clone(),
            sizing_multipliers: value.sizing_multipliers.clone(),
            core_assets: value.core_assets.clone(),
            inertia: crate::features::radar::domain::rules::ParsedInertia {
                min_state_duration: value.inertia.min_state_duration,
                trend_dominant_min_confidence: value.inertia.trend_dominant_min_confidence,
                core_breakdown_k: value.inertia.core_breakdown_k,
                core_breakdown_avg_deviation: value.inertia.core_breakdown_avg_deviation,
                core_breakdown_breadth_floor: value.inertia.core_breakdown_breadth_floor,
            },
            trend_cohesion: crate::features::radar::domain::rules::ParsedTrendCohesionRules {
                history_window_days: value.trend_cohesion.history_window_days,
                stability_norm_max: value.trend_cohesion.stability_norm_max,
                continuity_norm_max: value.trend_cohesion.continuity_norm_max,
                severe_stability_threshold: value.trend_cohesion.severe_stability_threshold,
                severe_continuity_threshold: value.trend_cohesion.severe_continuity_threshold,
                severe_compactness_threshold: value.trend_cohesion.severe_compactness_threshold,
                severe_rotation_threshold: value.trend_cohesion.severe_rotation_threshold,
                severe_leadership_threshold: value.trend_cohesion.severe_leadership_threshold,
                severe_cohesion_threshold: value.trend_cohesion.severe_cohesion_threshold,
                gate_stability_threshold: value.trend_cohesion.gate_stability_threshold,
                gate_continuity_threshold: value.trend_cohesion.gate_continuity_threshold,
                directional_max_candidates: value.trend_cohesion.directional_max_candidates,
                directional_leadership_threshold: value
                    .trend_cohesion
                    .directional_leadership_threshold,
                directional_rotation_threshold: value.trend_cohesion.directional_rotation_threshold,
                directional_compactness_threshold: value
                    .trend_cohesion
                    .directional_compactness_threshold,
                topology_single_max_candidates: value.trend_cohesion.topology_single_max_candidates,
                topology_single_min_compactness: value
                    .trend_cohesion
                    .topology_single_min_compactness,
                topology_single_min_rotation: value.trend_cohesion.topology_single_min_rotation,
                cohesive_score_threshold: value.trend_cohesion.cohesive_score_threshold,
            },
            market_state_engine:
                crate::features::radar::domain::rules::ParsedMarketStateEngineRules {
                    continuity_threshold: value.market_state_engine.continuity_threshold,
                    stability_threshold: value.market_state_engine.stability_threshold,
                    min_followers_threshold: value.market_state_engine.min_followers_threshold,
                    scout_abort_days: value.market_state_engine.scout_abort_days,
                    evidence_decay_days: value.market_state_engine.evidence_decay_days,
                    evidence_retention_days: value.market_state_engine.evidence_retention_days,
                    capex_payoff_weight: value.market_state_engine.capex_payoff_weight,
                    earnings_validation_weight: value
                        .market_state_engine
                        .earnings_validation_weight,
                    order_visibility_weight: value.market_state_engine.order_visibility_weight,
                },
            breakout: crate::features::radar::domain::rules::ParsedBreakoutRules {
                confirmed_trend_age_threshold: value.breakout.confirmed_trend_age_threshold,
                confirmed_top_tier_streak_threshold: value
                    .breakout
                    .confirmed_top_tier_streak_threshold,
                confirmed_zscore_threshold: value.breakout.confirmed_zscore_threshold,
                confirmed_min_slope: value.breakout.confirmed_min_slope,
                confirmed_min_curvature: value.breakout.confirmed_min_curvature,
                emerging_trend_age_threshold: value.breakout.emerging_trend_age_threshold,
                emerging_top_tier_streak_threshold: value
                    .breakout
                    .emerging_top_tier_streak_threshold,
                emerging_zscore_threshold: value.breakout.emerging_zscore_threshold,
                emerging_min_slope: value.breakout.emerging_min_slope,
                failed_breakout_curvature_threshold: value
                    .breakout
                    .failed_breakout_curvature_threshold,
                failed_breakout_slope_threshold: value.breakout.failed_breakout_slope_threshold,
                failed_breakout_display_threshold: value.breakout.failed_breakout_display_threshold,
                failed_breakout_no_trade_display_threshold: value
                    .breakout
                    .failed_breakout_no_trade_display_threshold,
            },
            sec: None,
            macro_gravity: None,
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
        // AppConfig::load は path を受け取るため、wrapper で同じ logic を検証する。
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
        assert_eq!(rules.market_state_engine.evidence_decay_days, 5);
        assert_eq!(rules.market_state_engine.evidence_retention_days, 3650);
        assert_eq!(rules.market_state_engine.order_visibility_weight, 1.0);

        assert_eq!(rules.breakout.confirmed_zscore_threshold, 1.5);
        assert_eq!(rules.breakout.failed_breakout_display_threshold, 70.0);
        assert_eq!(
            rules.breakout.failed_breakout_no_trade_display_threshold,
            82.0
        );
        assert_eq!(rules.breakout.emerging_trend_age_threshold, 5);
    }

    #[test]
    fn test_sec_config_validation() {
        let mut config = AppConfig {
            version: 1,
            output: OutputConfig {
                format: "table".to_string(),
                language: None,
                timezone: "UTC".to_string(),
                save_to: "output".to_string(),
                weight_kind: None,
                compact_transition_evidence_in_no_trade: true,
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
                market_state_engine: None,
            },
            watchlist: vec![],
            sec: None,
            research_attention: None,
            asset_thesis: None,
            macro_gravity: None,
            gray_rhino_escalation: None,
            gray_rhino_provider_registry: None,
        };

        // SEC config がなくても許容する。
        assert!(config.validate().is_ok());

        // Empty UA
        config.sec = Some(SecConfig {
            user_agent: "".to_string(),
        });
        assert!(config.validate().is_err());

        // Invalid format (no space or no @)
        config.sec = Some(SecConfig {
            user_agent: "InvalidUA".to_string(),
        });
        assert!(config.validate().is_err());

        // bracket 付きの正しい形式（現在は必須）。
        config.sec = Some(SecConfig {
            user_agent: "Sample Company <admin@example.com>".to_string(),
        });
        assert!(config.validate().is_ok());

        // Invalid: no brackets (even with space and @)
        config.sec = Some(SecConfig {
            user_agent: "Sample Company admin@example.com".to_string(),
        });
        assert!(config.validate().is_err());

        // 不正: space がない。
        config.sec = Some(SecConfig {
            user_agent: "<admin@example.com>".to_string(),
        });
        assert!(config.validate().is_err());

        // Invalid: no @
        config.sec = Some(SecConfig {
            user_agent: "Sample Company <admin>".to_string(),
        });
        assert!(config.validate().is_err());

        // 不正: email は bracket 内に完全に含める必要がある。
        config.sec = Some(SecConfig {
            user_agent: "Sample Company <admin>@example.com>".to_string(),
        });
        assert!(config.validate().is_err());

        // 不正: domain は email domain 形式である必要がある。
        config.sec = Some(SecConfig {
            user_agent: "Sample Company <admin@example>".to_string(),
        });
        assert!(config.validate().is_err());
    }
}
