/// Hypothesis Layer 向けの反叙事 governance 派生 policy。
/// 表示専用 metadata を生成し、Gate / execution / trading state は変更しない。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisMarketCyclePhase {
    EarlyFormation,
    MidConfirmation,
    LateAcceptance,
    CrowdedExpectation,
    DistributionWarning,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisConsensusKey {
    Emerging,
    Consensus,
    Crowded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisPricingKey {
    PartiallyPriced,
    FullyPriced,
    Overpriced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisNarrativeSaturationKey {
    Developing,
    Crowded,
    Saturated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisRealityOverridePriorityKey {
    Watch,
    Elevated,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisConfidenceDecayKey {
    Watch,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisTimeHorizonKey {
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HypothesisGovernanceDerivation {
    pub consensus: HypothesisConsensusKey,
    pub pricing: HypothesisPricingKey,
    pub narrative_saturation: HypothesisNarrativeSaturationKey,
    pub reality_override_priority: HypothesisRealityOverridePriorityKey,
    pub confidence_decay: HypothesisConfidenceDecayKey,
    pub time_horizon: HypothesisTimeHorizonKey,
    pub materialization_window: &'static str,
}

pub const HYPOTHESIS_LONG_MATERIALIZATION_WINDOW: &str = "12-36 months";

pub fn derive_hypothesis_governance(
    phase: HypothesisMarketCyclePhase,
) -> HypothesisGovernanceDerivation {
    let (consensus, pricing, narrative_saturation, reality_override_priority, confidence_decay) =
        match phase {
            HypothesisMarketCyclePhase::CrowdedExpectation => (
                HypothesisConsensusKey::Crowded,
                HypothesisPricingKey::Overpriced,
                HypothesisNarrativeSaturationKey::Saturated,
                HypothesisRealityOverridePriorityKey::Critical,
                HypothesisConfidenceDecayKey::Required,
            ),
            HypothesisMarketCyclePhase::LateAcceptance
            | HypothesisMarketCyclePhase::DistributionWarning => (
                HypothesisConsensusKey::Consensus,
                HypothesisPricingKey::FullyPriced,
                HypothesisNarrativeSaturationKey::Crowded,
                HypothesisRealityOverridePriorityKey::Elevated,
                HypothesisConfidenceDecayKey::Required,
            ),
            _ => (
                HypothesisConsensusKey::Emerging,
                HypothesisPricingKey::PartiallyPriced,
                HypothesisNarrativeSaturationKey::Developing,
                HypothesisRealityOverridePriorityKey::Watch,
                HypothesisConfidenceDecayKey::Watch,
            ),
        };

    HypothesisGovernanceDerivation {
        consensus,
        pricing,
        narrative_saturation,
        reality_override_priority,
        confidence_decay,
        time_horizon: HypothesisTimeHorizonKey::Long,
        materialization_window: HYPOTHESIS_LONG_MATERIALIZATION_WINDOW,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crowded_expectation_requires_critical_reality_override() {
        let derived = derive_hypothesis_governance(HypothesisMarketCyclePhase::CrowdedExpectation);

        assert_eq!(derived.consensus, HypothesisConsensusKey::Crowded);
        assert_eq!(
            derived.reality_override_priority,
            HypothesisRealityOverridePriorityKey::Critical
        );
        assert_eq!(derived.time_horizon, HypothesisTimeHorizonKey::Long);
        assert_eq!(
            derived.materialization_window,
            HYPOTHESIS_LONG_MATERIALIZATION_WINDOW
        );
    }

    #[test]
    fn late_acceptance_uses_elevated_reality_override() {
        let derived = derive_hypothesis_governance(HypothesisMarketCyclePhase::LateAcceptance);

        assert_eq!(
            derived.reality_override_priority,
            HypothesisRealityOverridePriorityKey::Elevated
        );
        assert_eq!(
            derived.narrative_saturation,
            HypothesisNarrativeSaturationKey::Crowded
        );
    }
}
