use serde::{Deserialize, Serialize};

/// 取引システム全体が参照するグローバルな市場状態（総合判定結果）。
#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketState {
    #[default]
    IGNITION,
    NEWBORN,
    EARLY_CONFIRMATION,
    ESTABLISHED,
    CONFIRMED,
    DEFENSIVE,
}

/// 市場トレンドのライフサイクル進行状態。
#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum LifecycleState {
    #[default]
    NONE,
    IGNITION,
    NEWBORN,
    EARLY_CONFIRMATION,
    ESTABLISHED,
    CONFIRMED,
}

/// リスク状況を示すオーバーレイ判定。
#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum RiskOverlay {
    #[default]
    NORMAL,
    DECELERATING,
    DEFENSIVE,
    BROKEN,
}

/// 特定時点における市場レジーム（体制）のスナップショット。
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketRegimeSnapshot {
    pub market_state: MarketState,
    pub lifecycle_state: LifecycleState,
    pub risk_overlay: RiskOverlay,
    pub reasons: Vec<String>,
    pub low_stability_streak: usize,
    pub duration_in_state: usize,
    pub transition_audit: Option<MarketTransitionAudit>,
}

/// レジーム遷移の判定プロセスに関する監査ログ。
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketTransitionAudit {
    pub from: LifecycleState,
    pub to: LifecycleState,
    pub is_reset_blocked: bool,
    pub is_downgrade_clamped: bool,
    pub core_breakdown: bool,
    pub duration_locked: bool,
    pub trend_dominant: bool,
    pub reset_gate_passed: bool,
    pub indicator_cap: LifecycleState,
    pub soft_reset_applied: bool,
    pub defensive_override: bool,
}
