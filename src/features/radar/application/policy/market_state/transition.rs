use super::models::{ActionStatus, LifecycleState, StateTransition};
use crate::config::ParsedMarketStateEngineRules;
use crate::features::radar::application::policy::features::MarketFeatures;

pub struct StateTransitionManager {
    config: ParsedMarketStateEngineRules,
}

impl StateTransitionManager {
    pub fn new(config: ParsedMarketStateEngineRules) -> Self {
        Self { config }
    }

    pub fn evaluate(
        &self,
        current_state: &LifecycleState,
        features: &MarketFeatures,
        has_mainline: bool,
        follower_count: usize,
    ) -> (LifecycleState, ActionStatus, Vec<StateTransition>) {
        let stability = features.stability_score;
        let continuity = features.regime_age;

        let mut reasons = Vec::new();

        if continuity < self.config.continuity_threshold {
            reasons.push(format!(
                "連続性不足（{} < {}）",
                continuity, self.config.continuity_threshold
            ));
        }
        if stability < self.config.stability_threshold {
            reasons.push(format!(
                "安定性不足（{:.1} < {:.1}）",
                stability, self.config.stability_threshold
            ));
        }
        if !has_mainline {
            reasons.push("主線未形成".to_string());
        }
        if follower_count < self.config.min_followers_threshold {
            reasons.push(format!(
                "フォロワー不足（{} < {}）",
                follower_count, self.config.min_followers_threshold
            ));
        }

        let is_ready = reasons.is_empty();

        let mut transitions = Vec::new();
        let next_state = match current_state {
            LifecycleState::Startup => {
                if is_ready {
                    transitions.push(StateTransition {
                        from: LifecycleState::Startup,
                        to: LifecycleState::Ready,
                        reason: "定量的な条件をすべて満たしました".to_string(),
                    });
                    LifecycleState::Ready
                } else if continuity > 0 && stability > 0.0 {
                    transitions.push(StateTransition {
                        from: LifecycleState::Startup,
                        to: LifecycleState::Transition,
                        reason: "観察を開始しました".to_string(),
                    });
                    LifecycleState::Transition
                } else {
                    LifecycleState::Startup
                }
            }
            LifecycleState::Transition => {
                if is_ready {
                    transitions.push(StateTransition {
                        from: LifecycleState::Transition,
                        to: LifecycleState::Ready,
                        reason: "定量的な条件をすべて満たしました".to_string(),
                    });
                    LifecycleState::Ready
                } else if continuity == 0 {
                    transitions.push(StateTransition {
                        from: LifecycleState::Transition,
                        to: LifecycleState::Startup,
                        reason: "連続性が途切れました".to_string(),
                    });
                    LifecycleState::Startup
                } else {
                    LifecycleState::Transition
                }
            }
            LifecycleState::Ready => {
                if !is_ready {
                    transitions.push(StateTransition {
                        from: LifecycleState::Ready,
                        to: LifecycleState::Transition,
                        reason: "条件を満たさなくなったため、観察状態に戻ります".to_string(),
                    });
                    LifecycleState::Transition
                } else {
                    LifecycleState::Ready
                }
            }
        };

        // 注意: 遷移状態（Transition）であっても、準備完了（Ready）でなければ取引をブロックする。
        let action_status = if is_ready {
            ActionStatus::Participate
        } else {
            ActionStatus::NoTrade(reasons)
        };

        (next_state, action_status, transitions)
    }
}
