use crate::features::backtest::application::model::{
    BacktestAssetAction, BacktestAssetSnapshot, BacktestAssetState, BacktestBreakoutStatus,
    BacktestDecisionSnapshot, BacktestTransitionAudit, BacktestTrendStatus, BacktestTrendTopology,
};
use crate::features::radar::domain::action_matrix::AssetAction;
use crate::features::radar::domain::asset_state::AssetState;
use crate::features::radar::domain::breakout_detection::{BreakoutReason, BreakoutStatus};
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::market_regime::LifecycleState;
use crate::features::radar::domain::trend_cohesion::{TrendCohesionStatus, TrendCohesionTopology};

/// Radar の DecisionPacket を Backtest bounded context の DTO に射影する。
pub(crate) fn decision_packet_to_snapshot(packet: &DecisionPacket) -> BacktestDecisionSnapshot {
    BacktestDecisionSnapshot {
        date: packet.date,
        market_state: format!("{:?}", packet.market_regime.market_state),
        trend_gate_passed: packet.trend_cohesion.gate_passed,
        trend_status: map_trend_status(packet.trend_cohesion.status),
        trend_topology: map_trend_topology(packet.trend_cohesion.topology),
        transition_audit: packet.market_regime.transition_audit.as_ref().map(|audit| {
            BacktestTransitionAudit {
                from: lifecycle_state_code(audit.from),
                to: lifecycle_state_code(audit.to),
                is_reset_blocked: audit.is_reset_blocked,
                is_downgrade_clamped: audit.is_downgrade_clamped,
                duration_locked: audit.duration_locked,
                soft_reset_applied: audit.soft_reset_applied,
                defensive_override: audit.defensive_override,
            }
        }),
        potential_energy: packet.market_features.potential_energy,
        system_confidence: packet.market_features.system_confidence,
        assets: packet
            .assets
            .iter()
            .map(|asset| BacktestAssetSnapshot {
                symbol: asset.symbol.clone(),
                action: map_asset_action(asset.action),
                deviation: asset.deviation,
                asset_state: map_asset_state(asset.asset_state.state),
                breakout_eligible: asset.breakout.breakout_eligible,
                breakout_status: map_breakout_status(asset.breakout.status),
                breakout_failed_risk: asset
                    .breakout
                    .reasons
                    .contains(&BreakoutReason::FailedBreakoutRisk),
                reasons: asset.reasons.clone(),
            })
            .collect(),
    }
}

fn map_trend_status(status: TrendCohesionStatus) -> BacktestTrendStatus {
    match status {
        TrendCohesionStatus::Dispersed => BacktestTrendStatus::Dispersed,
        TrendCohesionStatus::Forming => BacktestTrendStatus::Forming,
        TrendCohesionStatus::Formed => BacktestTrendStatus::Formed,
    }
}

fn map_trend_topology(topology: TrendCohesionTopology) -> BacktestTrendTopology {
    match topology {
        TrendCohesionTopology::NoLeader => BacktestTrendTopology::NoLeader,
        TrendCohesionTopology::SingleLeader => BacktestTrendTopology::SingleLeader,
        TrendCohesionTopology::FragmentedLeaders => BacktestTrendTopology::FragmentedLeaders,
    }
}

fn map_asset_action(action: AssetAction) -> BacktestAssetAction {
    match action {
        AssetAction::REDUCE => BacktestAssetAction::Reduce,
        AssetAction::FREEZE => BacktestAssetAction::Freeze,
        AssetAction::AVOID => BacktestAssetAction::Avoid,
        _ => BacktestAssetAction::Other,
    }
}

fn map_asset_state(state: AssetState) -> BacktestAssetState {
    match state {
        AssetState::OPTIMAL => BacktestAssetState::Optimal,
        _ => BacktestAssetState::Other,
    }
}

fn map_breakout_status(status: BreakoutStatus) -> BacktestBreakoutStatus {
    match status {
        BreakoutStatus::NoBreakout => BacktestBreakoutStatus::NoBreakout,
        BreakoutStatus::EmergingBreakout => BacktestBreakoutStatus::EmergingBreakout,
        BreakoutStatus::ConfirmedBreakout => BacktestBreakoutStatus::ConfirmedBreakout,
    }
}

fn lifecycle_state_code(state: LifecycleState) -> String {
    format!("{:?}", state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::action_matrix::{AssetAction, AssetActionDecision};
    use crate::features::radar::domain::asset_state::{AssetState, AssetStateSnapshot};
    use crate::features::radar::domain::breakout_detection::{BreakoutReason, BreakoutSnapshot};
    use crate::features::radar::domain::features::MarketFeatures;
    use crate::features::radar::domain::market_regime::{
        LifecycleState, MarketRegimeSnapshot, MarketState, MarketTransitionAudit,
    };
    use crate::features::radar::domain::trend_cohesion::{
        TrendCohesionSnapshot, TrendCohesionStatus, TrendCohesionTopology,
    };
    use chrono::NaiveDate;

    #[test]
    fn decision_packet_mapping_preserves_backtest_dto_contract() {
        let mut packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            market_features: MarketFeatures {
                potential_energy: 42.0,
                system_confidence: 63.0,
                ..Default::default()
            },
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                transition_audit: Some(MarketTransitionAudit {
                    from: LifecycleState::IGNITION,
                    to: LifecycleState::NEWBORN,
                    is_reset_blocked: true,
                    is_downgrade_clamped: true,
                    duration_locked: true,
                    soft_reset_applied: true,
                    defensive_override: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            trend_cohesion: TrendCohesionSnapshot {
                status: TrendCohesionStatus::Forming,
                topology: TrendCohesionTopology::SingleLeader,
                gate_passed: true,
                ..Default::default()
            },
            ..Default::default()
        };
        packet.assets.push(AssetActionDecision {
            symbol: "NVDA".to_string(),
            action: AssetAction::REDUCE,
            deviation: Some(1.25),
            asset_state: AssetStateSnapshot {
                symbol: "NVDA".to_string(),
                state: AssetState::OPTIMAL,
                ..Default::default()
            },
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                breakout_eligible: true,
                reasons: vec![BreakoutReason::FailedBreakoutRisk],
                ..Default::default()
            },
            reasons: vec!["risk".to_string()],
            ..Default::default()
        });

        let snapshot = decision_packet_to_snapshot(&packet);

        assert_eq!(snapshot.date, packet.date);
        assert_eq!(snapshot.market_state, "DEFENSIVE");
        assert!(snapshot.trend_gate_passed);
        assert_eq!(snapshot.trend_status, BacktestTrendStatus::Forming);
        assert_eq!(snapshot.trend_topology, BacktestTrendTopology::SingleLeader);
        assert_eq!(snapshot.potential_energy, 42.0);
        assert_eq!(snapshot.system_confidence, 63.0);
        let audit = snapshot.transition_audit.unwrap();
        assert_eq!(audit.from, "IGNITION");
        assert_eq!(audit.to, "NEWBORN");
        assert!(audit.is_reset_blocked);
        assert!(audit.is_downgrade_clamped);
        assert!(audit.duration_locked);
        assert!(audit.soft_reset_applied);
        assert!(audit.defensive_override);
        let asset = &snapshot.assets[0];
        assert_eq!(asset.symbol, "NVDA");
        assert_eq!(asset.action, BacktestAssetAction::Reduce);
        assert_eq!(asset.deviation, Some(1.25));
        assert_eq!(asset.asset_state, BacktestAssetState::Optimal);
        assert!(asset.breakout_eligible);
        assert_eq!(
            asset.breakout_status,
            BacktestBreakoutStatus::EmergingBreakout
        );
        assert!(asset.breakout_failed_risk);
        assert_eq!(asset.reasons, vec!["risk".to_string()]);
    }
}
