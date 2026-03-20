use crate::core::decision::DecisionPacket;
use crate::core::action_matrix::AssetAction;
use crate::core::market_regime::RiskOverlay;
use crate::core::portfolio_policy::RiskAssetsMode;
use crate::config::TradingConfig;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatedTrade {
    pub symbol: String,
    pub side: TradeSide,
    pub qty: f64,
    pub price: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}


#[derive(Debug, Clone, Serialize)]
pub struct GatedAudit {
    pub symbol: String,
    pub action: AssetAction,
    pub passed: bool,
    pub blocked_by: Option<String>,
    pub details: serde_json::Value,
}

pub struct ExecutionResult {
    pub trades: Vec<GatedTrade>,
    pub audits: Vec<GatedAudit>,
}

pub struct ExecutionGate;


impl ExecutionGate {
    /// Filters and sizes trades from a DecisionPacket based on risk and policy.
    pub fn gate_packet(
        packet: &DecisionPacket,
        trading_config: &TradingConfig,
        daily_traded: f64,
        buying_power: f64,
        current_exposure: f64,
    ) -> ExecutionResult {
        let mut gated_trades = Vec::new();
        let mut audits = Vec::new();
        
        let is_circuit_breaker_active = matches!(packet.market_regime.risk_overlay, RiskOverlay::BROKEN | RiskOverlay::DEFENSIVE);
        
        let mut current_daily_total = daily_traded;
        let mut available_power = buying_power;
        let mut running_exposure = current_exposure;
        
        let effective_limit = trading_config.max_daily_budget.unwrap_or(f64::MAX);
        let global_cap = trading_config.global_budget;

        for asset in &packet.assets {
            if !asset.trade_enabled {
                audits.push(GatedAudit {
                    symbol: asset.symbol.clone(),
                    action: asset.action,
                    passed: false,
                    blocked_by: Some("DisabledByWatchlist".to_string()),
                    details: serde_json::json!({}),
                });
                continue;
            }

            let (side, base_amount) = match asset.action {
                AssetAction::ACCUMULATE => (Some(TradeSide::Buy), asset.trade_amount), 
                AssetAction::REDUCE => (Some(TradeSide::Sell), asset.trade_amount),
                _ => (None, 0.0),
            };

            if let Some(s) = side {
                let policy_multiplier = match packet.portfolio_policy.risk_assets_mode {
                    RiskAssetsMode::AGGRESSIVE => 1.5,
                    RiskAssetsMode::NEUTRAL => 1.0,
                    RiskAssetsMode::DEFEND => 0.5,
                    RiskAssetsMode::HALT => 0.0,
                };

                let final_amount = base_amount * policy_multiplier * asset.config_multiplier;

                // Audit capture data
                let audit_details = serde_json::json!({
                    "final_amount": final_amount,
                    "available_power": available_power,
                    "running_exposure": running_exposure,
                    "daily_total": current_daily_total,
                    "effective_limit": effective_limit,
                    "global_cap": global_cap
                });

                if final_amount <= 0.0 {
                    audits.push(GatedAudit {
                        symbol: asset.symbol.clone(),
                        action: asset.action,
                        passed: false,
                        blocked_by: Some("ZeroSize".to_string()),
                        details: audit_details,
                    });
                    continue; 
                }

                if s == TradeSide::Buy && is_circuit_breaker_active {
                    audits.push(GatedAudit {
                        symbol: asset.symbol.clone(),
                        action: asset.action,
                        passed: false,
                        blocked_by: Some("CircuitBreaker".to_string()),
                        details: audit_details,
                    });
                    continue; 
                }

                if !trading_config.enabled {
                    audits.push(GatedAudit {
                        symbol: asset.symbol.clone(),
                        action: asset.action,
                        passed: false,
                        blocked_by: Some("TradingDisabled".to_string()),
                        details: audit_details,
                    });
                    continue; 
                }

                if current_daily_total + final_amount > effective_limit {
                    audits.push(GatedAudit {
                        symbol: asset.symbol.clone(),
                        action: asset.action,
                        passed: false,
                        blocked_by: Some("DailyBudget".to_string()),
                        details: audit_details,
                    });
                    continue;
                }

                if s == TradeSide::Buy && running_exposure + final_amount > global_cap {
                    audits.push(GatedAudit {
                        symbol: asset.symbol.clone(),
                        action: asset.action,
                        passed: false,
                        blocked_by: Some("GlobalExposure".to_string()),
                        details: audit_details,
                    });
                    continue; 
                }

                if s == TradeSide::Buy && final_amount > available_power {
                    audits.push(GatedAudit {
                        symbol: asset.symbol.clone(),
                        action: asset.action,
                        passed: false,
                        blocked_by: Some("BuyingPower".to_string()),
                        details: audit_details,
                    });
                    continue; 
                }

                let qty = (final_amount / asset.price).floor();
                if qty <= 0.0 { 
                    audits.push(GatedAudit {
                        symbol: asset.symbol.clone(),
                        action: asset.action,
                        passed: false,
                        blocked_by: Some("QuantityRounding".to_string()),
                        details: audit_details,
                    });
                    continue; 
                }

                audits.push(GatedAudit {
                    symbol: asset.symbol.clone(),
                    action: asset.action,
                    passed: true,
                    blocked_by: None,
                    details: audit_details,
                });

                gated_trades.push(GatedTrade {
                    symbol: asset.symbol.clone(),
                    side: s.clone(),
                    qty,
                    price: asset.price,
                    reason: format!("Action: {:?}, Policy: {:?}, Base: ${:.0}", asset.action, packet.portfolio_policy.risk_assets_mode, base_amount),
                });

                if s == TradeSide::Buy {
                    available_power -= final_amount;
                    running_exposure += final_amount;
                } else if s == TradeSide::Sell {
                    running_exposure -= final_amount;
                }
                current_daily_total += final_amount;
            }
        }

        ExecutionResult {
            trades: gated_trades,
            audits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::market_regime::MarketRegimeSnapshot;
    use crate::core::market_regime::MarketState;
    use crate::core::market_regime::LifecycleState;
    use crate::core::portfolio_policy::PortfolioPolicy;
    use crate::core::portfolio_policy::RiskAssetsMode;
    use crate::core::action_matrix::AssetActionDecision;
    use crate::core::asset_state::AssetState;

    fn mock_decision(symbol: &str, action: AssetAction, amount: f64) -> AssetActionDecision {
        AssetActionDecision {
            symbol: symbol.to_string(),
            price: 100.0,
            state: AssetState::OPTIMAL,
            action,
            reasons: vec![],
            deviation: None,
            z_score: None,
            trade_enabled: true,
            trade_amount: amount,
            config_multiplier: 1.0,
            prev_action: None,
            action_changed: false,
        }
    }

    fn mock_packet(assets: Vec<AssetActionDecision>, risk: RiskOverlay) -> DecisionPacket {
        let regime = MarketRegimeSnapshot {
            market_state: MarketState::ESTABLISHED,
            lifecycle_state: LifecycleState::ESTABLISHED,
            risk_overlay: risk,
            reasons: vec![],
        };
        DecisionPacket::new(
            chrono::Utc::now().date_naive(),
            crate::core::features::MarketFeatures::default(),
            regime,
            PortfolioPolicy { 
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
                target_exposure_min: 0.0,
                target_exposure_max: 1.0,
                allow_chase: true,
                allow_pullback_buy: true,
                allow_new_risk: true,
            },
            assets,
        )
    }

    #[test]
    fn test_gate_daily_budget_limit() {
        let config = TradingConfig { enabled: true, global_budget: 10000.0, max_daily_budget: Some(2000.0) };
        let assets = vec![
            mock_decision("A", AssetAction::ACCUMULATE, 1500.0),
            mock_decision("B", AssetAction::ACCUMULATE, 1000.0),
        ];
        let packet = mock_packet(assets, RiskOverlay::NORMAL);

        let result = ExecutionGate::gate_packet(&packet, &config, 0.0, 10000.0, 0.0);

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].symbol, "A");
        assert_eq!(result.audits[1].symbol, "B");
        assert_eq!(result.audits[1].blocked_by, Some("DailyBudget".to_string()));
    }

    #[test]
    fn test_gate_global_exposure_cap() {
        let config = TradingConfig { enabled: true, global_budget: 2000.0, max_daily_budget: None };
        let assets = vec![mock_decision("A", AssetAction::ACCUMULATE, 1500.0)];
        let packet = mock_packet(assets, RiskOverlay::NORMAL);

        let result = ExecutionGate::gate_packet(&packet, &config, 0.0, 10000.0, 1000.0);

        assert_eq!(result.trades.len(), 0);
        assert_eq!(result.audits[0].blocked_by, Some("GlobalExposure".to_string()));
    }

    #[test]
    fn test_gate_buying_power() {
        let config = TradingConfig { enabled: true, global_budget: 10000.0, max_daily_budget: None };
        let assets = vec![mock_decision("A", AssetAction::ACCUMULATE, 1500.0)];
        let packet = mock_packet(assets, RiskOverlay::NORMAL);

        let result = ExecutionGate::gate_packet(&packet, &config, 0.0, 1000.0, 0.0);

        assert_eq!(result.trades.len(), 0);
        assert_eq!(result.audits[0].blocked_by, Some("BuyingPower".to_string()));
    }

    #[test]
    fn test_gate_circuit_breaker() {
        let config = TradingConfig { enabled: true, global_budget: 10000.0, max_daily_budget: None };
        let assets = vec![
            mock_decision("A", AssetAction::ACCUMULATE, 1000.0),
            mock_decision("B", AssetAction::REDUCE, 1000.0),
        ];
        let packet = mock_packet(assets, RiskOverlay::DEFENSIVE);

        let result = ExecutionGate::gate_packet(&packet, &config, 0.0, 10000.0, 5000.0);

        // Buys are blocked, Sells are allowed
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].symbol, "B");
        assert_eq!(result.audits[0].blocked_by, Some("CircuitBreaker".to_string()));
    }

    #[test]
    fn test_gate_reduction_does_not_consume_buying_power() {
        let config = TradingConfig { enabled: true, global_budget: 10000.0, max_daily_budget: None };
        let assets = vec![mock_decision("A", AssetAction::REDUCE, 1500.0)];
        let packet = mock_packet(assets, RiskOverlay::NORMAL);

        let result = ExecutionGate::gate_packet(&packet, &config, 0.0, 1000.0, 5000.0);

        // Sells (REDUCE) shouldn't be blocked by buying power
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].symbol, "A");
        assert!(result.audits[0].passed);
    }
}
