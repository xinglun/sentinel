use std::collections::HashMap;

use crate::features::radar::application::execution_gate::{
    ExecutionGate, ExecutionResult, TradingLimits,
};
use crate::features::radar::application::radar::{
    build_account_snapshot, build_data_quality_log, build_portfolio_snapshot,
    build_state_machine_summary, AccountSnapshot, AccountSnapshotInput, DataAcquisitionSummary,
    DataQualityLog, PortfolioSnapshot,
};
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord;
use crate::features::shared::application::run_status::StateMachineSummary;
use crate::features::shared::domain::market_regime::MarketState;

/// Radar の交付前に Application が確定する入力値。
pub struct RadarDeliveryInput<'a> {
    pub packet: &'a DecisionPacket,
    pub trading_limits: TradingLimits,
    pub daily_traded: f64,
    pub realized_pl: f64,
    pub positions: &'a HashMap<String, (f64, f64)>,
    pub failed_symbols: &'a [String],
    pub data_acquisition: DataAcquisitionSummary,
    pub previous_market_state: Option<MarketState>,
    pub should_persist_history: bool,
    pub timestamp: &'a str,
}

/// Infrastructure へ引き渡す Radar run の application-level delivery plan。
pub struct RadarDeliveryPlan {
    pub execution_result: ExecutionResult,
    pub decision_state: String,
    pub new_position_limit: f64,
    pub current_exposure: f64,
    pub buying_power: f64,
    pub portfolio_snapshot: PortfolioSnapshot,
    pub account_snapshot: AccountSnapshot,
    pub data_quality_log: DataQualityLog,
    pub state_machine: StateMachineSummary,
    pub prices: HashMap<String, f64>,
    pub substantive_records: Vec<AutomatedEvidenceRecord>,
}

pub fn derive_new_position_limit(limits: &TradingLimits) -> f64 {
    if limits.enabled {
        limits
            .max_daily_budget
            .unwrap_or(limits.global_budget)
            .max(0.0)
    } else {
        0.0
    }
}

/// DecisionPacket を永続化・配信可能な payload へ変換する application service。
pub struct RadarDeliveryPlanner;

impl RadarDeliveryPlanner {
    /// execution policy と監査 payload を一度だけ計算し、Interface の判断分岐を排除する。
    pub fn plan(input: RadarDeliveryInput<'_>) -> RadarDeliveryPlan {
        let current_exposure: f64 = input
            .positions
            .values()
            .map(|(quantity, average_price)| quantity * average_price)
            .sum();
        let buying_power = (input.trading_limits.global_budget - current_exposure).max(0.0);
        let execution_result = ExecutionGate::gate_packet(
            input.packet,
            &input.trading_limits,
            input.daily_traded,
            buying_power,
            current_exposure,
        );
        let new_position_limit = derive_new_position_limit(&input.trading_limits);
        let decision_state = if new_position_limit > 0.0 && !execution_result.trades.is_empty() {
            "TRADE_ALLOWED"
        } else {
            "NO_TRADE"
        };
        let date = input.packet.date.to_string();

        let substantive_records = input
            .packet
            .trend_recognition
            .as_ref()
            .and_then(|recognition| recognition.substantive.as_ref())
            .map(|substantive| substantive.records.clone())
            .unwrap_or_default();

        RadarDeliveryPlan {
            execution_result,
            decision_state: decision_state.to_string(),
            new_position_limit,
            current_exposure,
            buying_power,
            portfolio_snapshot: build_portfolio_snapshot(
                &date,
                input.realized_pl,
                current_exposure,
                input.positions,
            ),
            account_snapshot: build_account_snapshot(AccountSnapshotInput {
                date: &date,
                global_budget: input.trading_limits.global_budget,
                max_daily_budget: input.trading_limits.max_daily_budget,
                daily_traded: input.daily_traded,
                buying_power,
                current_exposure,
                realized_pl: input.realized_pl,
                failed_fetch_count: input.failed_symbols.len(),
            }),
            data_quality_log: build_data_quality_log(
                input.timestamp,
                &date,
                input.data_acquisition,
                input.failed_symbols,
            ),
            state_machine: build_state_machine_summary(
                input.previous_market_state,
                input.packet.market_regime.market_state,
                input.packet.market_regime.transition_audit.as_ref(),
                input.should_persist_history,
            ),
            prices: input
                .packet
                .assets
                .iter()
                .map(|asset| (asset.symbol.clone(), asset.price))
                .collect(),
            substantive_records,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::derive_new_position_limit;
    use crate::features::radar::application::execution_gate::TradingLimits;

    #[test]
    fn new_position_limit_uses_daily_budget_or_global_budget_and_zero_when_disabled() {
        assert_eq!(
            derive_new_position_limit(&TradingLimits {
                enabled: true,
                global_budget: 100.0,
                max_daily_budget: None,
            }),
            100.0
        );
        assert_eq!(
            derive_new_position_limit(&TradingLimits {
                enabled: true,
                global_budget: 100.0,
                max_daily_budget: Some(40.0),
            }),
            40.0
        );
        assert_eq!(
            derive_new_position_limit(&TradingLimits {
                enabled: false,
                global_budget: 100.0,
                max_daily_budget: Some(40.0),
            }),
            0.0
        );
    }
}
