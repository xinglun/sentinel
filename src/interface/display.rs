use std::cmp::Ordering;

use crate::core::position_intent::UnifiedPositionIntent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct DisplayContext {
    #[serde(default)]
    pub has_position: bool,
    #[serde(default)]
    pub is_core_holding: bool,
    #[serde(default)]
    pub is_candidate_only: bool,
    #[serde(default)]
    pub is_top_tier: bool,
    #[serde(default)]
    pub cohesion_ready: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum DisplayIntent {
    ADD,
    #[default]
    HOLD,
    OBSERVE,
    TRIM,
    EXIT,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopActionViewModel {
    pub symbol: String,
    pub indicator: String,
    pub primary_label: String,
    pub tags: Vec<String>,
    pub secondary_desc: String,
    pub diagnostic: Option<String>,
    pub action_changed: bool,
    pub is_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalBucketViewModel {
    pub bucket_id: String,
    pub display_name: String,
    pub count: usize,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskOpportunityViewModel {
    pub kind: String, // "RISK" or "OPPORTUNITY"
    pub symbol: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
pub struct DisplayAssetRef<'a> {
    pub asset: &'a crate::core::action_matrix::AssetActionDecision,
    pub context: DisplayContext,
    pub intent: DisplayIntent,
}

/// Optimized container using references to avoid redundant cloning during categorization.
pub struct DisplayBuckets<'a> {
    pub accumulate: Vec<DisplayAssetRef<'a>>,
    pub hold: Vec<DisplayAssetRef<'a>>,
    pub watch: Vec<DisplayAssetRef<'a>>,
    pub defend: Vec<DisplayAssetRef<'a>>,
}

pub struct DisplayAdapter;

impl DisplayAdapter {
    /// 基于执行意向（PositionIntent）和结构化展示上下文推导展示语义
    pub fn derive_display_intent(
        pos_intent: UnifiedPositionIntent,
        context: &DisplayContext,
    ) -> DisplayIntent {
        match pos_intent {
            UnifiedPositionIntent::Add => DisplayIntent::ADD,
            UnifiedPositionIntent::Trim => DisplayIntent::TRIM,
            UnifiedPositionIntent::Exit => DisplayIntent::EXIT,
            UnifiedPositionIntent::Watch => DisplayIntent::OBSERVE,
            UnifiedPositionIntent::Hold => {
                if context.is_candidate_only {
                    DisplayIntent::OBSERVE
                } else if context.has_position {
                    DisplayIntent::HOLD
                } else {
                    DisplayIntent::OBSERVE
                }
            }
        }
    }

    pub fn derive_risk_opportunity_view_models(
        items: &[DisplayAssetRef<'_>],
        dict: &crate::interface::i18n::DisplayDictionary,
    ) -> Vec<RiskOpportunityViewModel> {
        let mut vms = Vec::new();
        for item in items {
            let asset = item.asset;
            if (item.intent == DisplayIntent::ADD
                || (item.context.is_candidate_only && item.context.cohesion_ready))
                && matches!(
                    asset.asset_state.state,
                    crate::core::asset_state::AssetState::PULLBACK
                        | crate::core::asset_state::AssetState::OPTIMAL
                )
            {
                vms.push(RiskOpportunityViewModel {
                    kind: "OPPORTUNITY".to_string(),
                    symbol: asset.symbol.clone(),
                    reason: format!("触发 {:?}", asset.asset_state.state),
                });
            }

            if matches!(item.intent, DisplayIntent::EXIT | DisplayIntent::TRIM)
                || asset.asset_state.state == crate::core::asset_state::AssetState::OVERHEAT
            {
                let state_label = match asset.asset_state.state {
                    crate::core::asset_state::AssetState::PULLBACK => &dict.asset_states.pullback,
                    crate::core::asset_state::AssetState::OPTIMAL => &dict.asset_states.optimal,
                    crate::core::asset_state::AssetState::OVERHEAT => &dict.asset_states.overheat,
                    crate::core::asset_state::AssetState::CRUISE => &dict.asset_states.cruise,
                    crate::core::asset_state::AssetState::CAUTION => &dict.asset_states.caution,
                    crate::core::asset_state::AssetState::DEFEND => &dict.asset_states.defend,
                    _ => &dict.asset_states.forming,
                };

                vms.push(RiskOpportunityViewModel {
                    kind: "RISK".to_string(),
                    symbol: asset.symbol.clone(),
                    reason: if item.intent == DisplayIntent::EXIT
                        || item.intent == DisplayIntent::TRIM
                    {
                        format!("触发 {}", Self::get_label(item.intent, dict))
                    } else {
                        format!("过度 {}", state_label)
                    },
                });
            }
        }
        vms
    }

    pub fn get_primary_tag(
        ctx: &DisplayContext,
        dict: &crate::interface::i18n::DisplayDictionary,
    ) -> Option<String> {
        if ctx.is_candidate_only && !ctx.cohesion_ready {
            Some(dict.asset_tags.blocked.clone())
        } else if ctx.is_core_holding {
            Some(dict.asset_tags.core.clone())
        } else if ctx.is_candidate_only {
            Some(dict.asset_tags.candidate.clone())
        } else {
            None
        }
    }

    /// Categorize assets into buckets using references (O(1) cloning per bucket entry).
    pub fn categorize_refs<'a>(
        items: impl IntoIterator<Item = DisplayAssetRef<'a>>,
    ) -> DisplayBuckets<'a> {
        let mut buckets = DisplayBuckets {
            accumulate: Vec::new(),
            hold: Vec::new(),
            watch: Vec::new(),
            defend: Vec::new(),
        };

        for item in items {
            match item.intent {
                DisplayIntent::ADD => buckets.accumulate.push(item),
                DisplayIntent::TRIM | DisplayIntent::EXIT => buckets.defend.push(item),
                DisplayIntent::HOLD => buckets.hold.push(item),
                DisplayIntent::OBSERVE => buckets.watch.push(item),
            }
        }

        let sort_fn = |a: &DisplayAssetRef<'_>, b: &DisplayAssetRef<'_>| {
            let change_cmp = (if b.asset.action_changed { 1 } else { 0 })
                .cmp(&(if a.asset.action_changed { 1 } else { 0 }));
            if change_cmp != Ordering::Equal {
                return change_cmp;
            }

            let az = a.asset.z_score.unwrap_or(0.0).abs();
            let bz = b.asset.z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(Ordering::Equal)
        };

        buckets.accumulate.sort_by(sort_fn);
        buckets.hold.sort_by(sort_fn);
        buckets.watch.sort_by(sort_fn);
        buckets.defend.sort_by(sort_fn);

        buckets
    }

    pub fn derive_top_action_view_model(
        asset: &crate::core::action_matrix::AssetActionDecision,
        context: &DisplayContext,
        intent: DisplayIntent,
        dict: &crate::interface::i18n::DisplayDictionary,
    ) -> TopActionViewModel {
        let indicator = match intent {
            DisplayIntent::ADD => "🟢",
            DisplayIntent::HOLD | DisplayIntent::OBSERVE => "🔵",
            DisplayIntent::TRIM => "🟠",
            DisplayIntent::EXIT => "🔴",
        };

        TopActionViewModel {
            symbol: asset.symbol.clone(),
            indicator: indicator.to_string(),
            primary_label: Self::get_label(intent, dict),
            tags: Self::get_primary_tag(context, dict).into_iter().collect(),
            secondary_desc: match asset.asset_state.state {
                crate::core::asset_state::AssetState::PULLBACK => &dict.asset_states.pullback,
                crate::core::asset_state::AssetState::OPTIMAL => &dict.asset_states.optimal,
                crate::core::asset_state::AssetState::OVERHEAT => &dict.asset_states.overheat,
                _ => &dict.asset_states.forming,
            }
            .to_string(),
            diagnostic: None,
            action_changed: asset.action_changed,
            is_new: asset.prev_action.is_none(),
        }
    }

    pub fn get_label(
        intent: DisplayIntent,
        dict: &crate::interface::i18n::DisplayDictionary,
    ) -> String {
        match intent {
            DisplayIntent::ADD => dict.actions.accumulate.clone(),
            DisplayIntent::HOLD => dict.actions.hold.clone(),
            DisplayIntent::OBSERVE => dict.actions.observe.clone(),
            DisplayIntent::TRIM => dict.actions.trim.clone(),
            DisplayIntent::EXIT => dict.actions.exit.clone(),
        }
    }
}
