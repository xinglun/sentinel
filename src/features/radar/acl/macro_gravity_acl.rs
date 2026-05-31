use crate::config;
use crate::features::radar::domain::rules::{
    CreditStress, GrowthValuationImpact, LiquidityCondition, MacroGravitySnapshot, MacroPressure,
    YieldCurveState,
};

impl From<config::MacroPressure> for MacroPressure {
    fn from(value: config::MacroPressure) -> Self {
        match value {
            config::MacroPressure::Falling => Self::Falling,
            config::MacroPressure::Neutral => Self::Neutral,
            config::MacroPressure::Rising => Self::Rising,
            config::MacroPressure::Tight => Self::Tight,
        }
    }
}

impl From<config::YieldCurveState> for YieldCurveState {
    fn from(value: config::YieldCurveState) -> Self {
        match value {
            config::YieldCurveState::Normal => Self::Normal,
            config::YieldCurveState::Flat => Self::Flat,
            config::YieldCurveState::Inverted => Self::Inverted,
            config::YieldCurveState::Steepening => Self::Steepening,
        }
    }
}

impl From<config::CreditStress> for CreditStress {
    fn from(value: config::CreditStress) -> Self {
        match value {
            config::CreditStress::Normal => Self::Normal,
            config::CreditStress::Watch => Self::Watch,
            config::CreditStress::Stress => Self::Stress,
        }
    }
}

impl From<config::LiquidityCondition> for LiquidityCondition {
    fn from(value: config::LiquidityCondition) -> Self {
        match value {
            config::LiquidityCondition::Loose => Self::Loose,
            config::LiquidityCondition::Neutral => Self::Neutral,
            config::LiquidityCondition::Tight => Self::Tight,
        }
    }
}

impl From<config::GrowthValuationImpact> for GrowthValuationImpact {
    fn from(value: config::GrowthValuationImpact) -> Self {
        match value {
            config::GrowthValuationImpact::Supportive => Self::Supportive,
            config::GrowthValuationImpact::Neutral => Self::Neutral,
            config::GrowthValuationImpact::Compressing => Self::Compressing,
        }
    }
}

impl From<&config::MacroGravityConfig> for MacroGravitySnapshot {
    fn from(value: &config::MacroGravityConfig) -> Self {
        Self {
            rate_pressure: value.rate_pressure.into(),
            real_yield_pressure: value.real_yield_pressure.into(),
            yield_curve: value.yield_curve.into(),
            credit_stress: value.credit_stress.into(),
            liquidity: value.liquidity.into(),
            growth_valuation_impact: value.growth_valuation_impact.into(),
            note: value.note.clone(),
            enabled: value.enable.unwrap_or(true),
        }
    }
}
