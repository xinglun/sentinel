use crate::core::execution_gate::TradeSide;
use crate::core::ledger::{Ledger, TradeRecord};
use crate::trade::trader::{OrderSide, OrderType, PlaceOrderRequest, TradeExecutor};
use anyhow::Result;
use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TraderAgent {
    executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>,
    ledger: Arc<Ledger>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct TradeExecutionAudit {
    pub symbol: String,
    pub side: String,
    pub qty: f64,
    pub price: f64,
    pub success: bool,
    pub order_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct ExecutionSummary {
    pub audits: Vec<TradeExecutionAudit>,
    pub status: Result<()>,
}

impl TraderAgent {
    pub fn new(executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>, ledger: Arc<Ledger>) -> Self {
        Self { executor, ledger }
    }

    pub async fn execute_signals(
        &self,
        gated_trades: Vec<crate::core::execution_gate::GatedTrade>,
    ) -> Result<ExecutionSummary> {
        let mut audits = Vec::new();
        if gated_trades.is_empty() {
            println!("ℹ️  TraderAgent: No trades to execute (filtered or no signals).");
            return Ok(ExecutionSummary {
                audits,
                status: Ok(()),
            });
        }

        let mut errors = Vec::new();
        let mut first = true;

        for trade in gated_trades {
            // 2. Rate Limiting: 1s buffer between orders to comply with Moomoo limits (15/30s)
            if !first {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            first = false;

            // Check if we've already acted on this symbol today for the same side to avoid double-trading
            let side_str_upper = format!("{:?}", trade.side).to_uppercase();
            if self.ledger.has_acted_today(&trade.symbol, &side_str_upper) {
                continue;
            }

            println!(
                "🛰️  TraderAgent: Dispatching gated trade for {} ({} units @ ${:.2}). Reason: {}",
                trade.symbol, trade.qty, trade.price, trade.reason
            );

            let order_side = match trade.side {
                TradeSide::Buy => OrderSide::Buy,
                TradeSide::Sell => OrderSide::Sell,
            };

            let req = PlaceOrderRequest {
                symbol: trade.symbol.clone(),
                side: order_side.clone(),
                order_type: OrderType::Normal,
                qty: trade.qty,
                price: Some(trade.price),
            };

            let mut audit = TradeExecutionAudit {
                symbol: trade.symbol.clone(),
                side: format!("{:?}", trade.side),
                qty: trade.qty,
                price: trade.price,
                success: false,
                order_id: None,
                error: None,
            };

            let exec = self.executor.lock().await;
            match exec.place_order(req).await {
                Ok(res) => {
                    println!(
                        "✅ [Trader - SUCCESS] Order placed for {}. Order ID: {}",
                        trade.symbol, res.order_id
                    );
                    audit.success = true;
                    audit.order_id = Some(res.order_id);

                    let side_str = if trade.side == TradeSide::Buy {
                        "BUY"
                    } else {
                        "SELL"
                    };
                    let _ = self.ledger.record_trade(TradeRecord {
                        date: Local::now().date_naive(),
                        timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        symbol: trade.symbol.clone(),
                        side: side_str.to_string(),
                        qty: trade.qty,
                        price: trade.price,
                        signal: format!("{:?}", trade.side),
                    });
                }
                Err(e) => {
                    println!(
                        "❌ [Trader - FAILED] Failed to complete trade for {}: {}",
                        trade.symbol, e
                    );
                    audit.error = Some(e.to_string());
                    errors.push(format!("{}: {}", trade.symbol, e));
                }
            }
            audits.push(audit);
        }

        let status = if !errors.is_empty() {
            Err(anyhow::anyhow!(
                "Partial trade execution failure: {}",
                errors.join("; ")
            ))
        } else {
            Ok(())
        };

        Ok(ExecutionSummary { audits, status })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::action_matrix::{AssetAction, AssetActionDecision};
    use crate::core::asset_state::AssetState;
    use crate::core::market_regime::{
        LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
    };
    use crate::core::portfolio_policy::{PortfolioPolicy, RiskAssetsMode};

    use crate::config::AppConfig;
    use crate::core::features::MarketFeatures;
    use crate::trade::trader::MockTradeExecutor;
    use std::sync::atomic::Ordering;

    use tempfile::tempdir;

    #[tokio::test]
    async fn test_trader_agent_dispatch() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();

        let config_str = r#"
            version = 1
            provider = "yahoo"
            [output]
            timezone = "UTC"
            format = "json"
            save_to = "./test"

            [rules.trend]


            lookback_days = 20
            flat_threshold_pct = 0.5
            [rules.deviation_bands]
            optimal = 5.0
            [rules.actions]
            optimal = "BUY"
            [trading]
            enabled = true
            global_budget = 10000.0
            max_daily_budget = 1000000.0

            [[watchlist]]
            symbol = "AAPL"
            market = "US"
            owner_ma_days = 20
            leash_ma_days = 10
            deviation_basis = "owner"
            enable = true
        "#;

        let config: AppConfig = toml::from_str(config_str).unwrap();
        let config_arc = Arc::new(config);
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));
        let ledger = Arc::new(Ledger::new(save_dir.clone()));

        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        let market = MarketRegimeSnapshot {
            market_state: MarketState::NEWBORN,
            lifecycle_state: LifecycleState::NEWBORN,
            risk_overlay: RiskOverlay::NORMAL,
            reasons: vec![],
        };
        let mut policy = PortfolioPolicy::from_market_regime(&market);
        policy.risk_assets_mode = RiskAssetsMode::NEUTRAL; // Force NEUTRAL for test stability

        let assets = vec![AssetActionDecision {
            symbol: "AAPL".to_string(),
            price: 150.0,
            state: AssetState::OPTIMAL,
            action: AssetAction::ACCUMULATE,
            reasons: vec![],
            deviation: Some(10.0),
            z_score: Some(1.0),
            trade_enabled: true,
            trade_amount: 5000.0,
            config_multiplier: 1.0,
            prev_action: None,
            action_changed: false,
        }];

        use crate::core::decision::DecisionPacket;
        use crate::core::execution_gate::ExecutionGate;

        let market_features = MarketFeatures::default();
        let packet = DecisionPacket::new(
            chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            market_features,
            market,
            policy,
            assets,
        );

        let trading_config = config_arc.trading.as_ref().unwrap();
        let execution_result =
            ExecutionGate::gate_packet(&packet, trading_config, 0.0, 100000.0, 0.0);

        let summary = agent
            .execute_signals(execution_result.trades)
            .await
            .expect("Preflight should not fail");
        summary.status.expect("Execution should not fail");

        let count = mock_exec
            .lock()
            .await
            .placed_orders_count
            .load(Ordering::SeqCst);
        assert!(
            count >= 1,
            "Should have placed at least 1 order, got {}",
            count
        );
    }
}
