use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
pub struct TrendConfig {
    pub lookback_days: usize,
    pub flat_threshold_pct: f64,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviationBasis {
    #[default]
    Owner,
    Leash,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
pub struct WatchlistEntry {
    pub symbol: String,
    pub weight: Option<f64>,
    pub market: String,
    pub owner_ma_days: usize,
    pub leash_ma_days: usize,
    pub deviation_basis: DeviationBasis,
    pub enable: bool,
    pub trade_enabled: Option<bool>,
    pub trade_amount: Option<f64>,
    pub event_tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedRules {
    pub trend: TrendConfig,
    pub sorted_bands: Vec<(String, f64)>,
    pub actions: HashMap<String, String>,
    pub sizing_multipliers: Option<HashMap<String, f64>>,
    pub core_assets: Vec<String>,
    pub inertia: ParsedInertia,
    pub trend_cohesion: ParsedTrendCohesionRules,
    pub market_state_engine: ParsedMarketStateEngineRules,
    pub breakout: ParsedBreakoutRules,
    pub sec: Option<()>,
    pub macro_gravity: Option<MacroGravitySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroGravitySnapshot {
    pub rate_pressure: MacroPressure,
    pub real_yield_pressure: MacroPressure,
    pub yield_curve: YieldCurveState,
    pub credit_stress: CreditStress,
    pub liquidity: LiquidityCondition,
    pub growth_valuation_impact: GrowthValuationImpact,
    pub note: Option<String>,
    pub enabled: bool,
}

impl Default for MacroGravitySnapshot {
    fn default() -> Self {
        Self {
            rate_pressure: MacroPressure::Neutral,
            real_yield_pressure: MacroPressure::Neutral,
            yield_curve: YieldCurveState::Normal,
            credit_stress: CreditStress::Normal,
            liquidity: LiquidityCondition::Neutral,
            growth_valuation_impact: GrowthValuationImpact::Neutral,
            note: None,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroPressure {
    Falling,
    Neutral,
    Rising,
    Tight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YieldCurveState {
    Normal,
    Flat,
    Inverted,
    Steepening,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditStress {
    Normal,
    Watch,
    Stress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityCondition {
    Loose,
    Neutral,
    Tight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrowthValuationImpact {
    Supportive,
    Neutral,
    Compressing,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedInertia {
    pub min_state_duration: usize,
    pub trend_dominant_min_confidence: f64,
    pub core_breakdown_k: usize,
    pub core_breakdown_avg_deviation: f64,
    pub core_breakdown_breadth_floor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMarketStateEngineRules {
    pub continuity_threshold: usize,
    pub stability_threshold: f64,
    pub min_followers_threshold: usize,
    pub scout_abort_days: usize,
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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
