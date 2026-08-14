pub mod action_matrix;
pub mod asset_state;
pub mod breakout_detection;
pub(crate) mod current_relative_strength;
pub mod decision;
pub mod decision_class;
pub mod exit;
pub mod features;
pub(crate) mod hypothesis_governance_policy;
pub mod intent_synthesizer;
pub mod leader_persistence;
pub mod market_change_driver;
pub mod market_regime;
pub mod market_state;
pub mod observation_timeline;
pub mod portfolio_policy;
pub mod position_intent;
pub(crate) mod price_volume_structure;
pub mod rules;
pub mod transition_log;
pub mod trend_cohesion;

#[cfg(test)]
mod architecture_boundary_tests {
    #[test]
    fn behavior_modules_expose_explicit_internal_boundaries() {
        assert!(super::price_volume_structure::eligibility_boundary_marker());
        assert!(super::price_volume_structure::baseline_boundary_marker());
        assert!(super::price_volume_structure::classification_boundary_marker());
        assert!(super::price_volume_structure::lifecycle_boundary_marker());
        assert!(super::leader_persistence::snapshot_boundary_marker());
        assert!(super::leader_persistence::persistence_boundary_marker());
        assert!(super::leader_persistence::absence_boundary_marker());
        assert!(super::leader_persistence::transition_boundary_marker());
    }
}
