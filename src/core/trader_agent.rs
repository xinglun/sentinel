use crate::core::execution_gate::TradeSide;
use crate::core::ledger::{Ledger, TradeRecord};
use crate::domain::reconciliation::{PositionMismatch, ReconciliationReport};
use crate::trade::trader::{OrderSide, OrderType, PlaceOrderRequest, TradeExecutor};
use anyhow::Result;
use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TraderAgent {
    executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>,
    ledger: Arc<Ledger>,
    poll_interval: std::time::Duration,
    max_poll_attempts: usize,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct TradeExecutionAudit {
    pub symbol: String,
    pub side: String,
    pub qty_requested: f64,
    pub qty_filled: f64,
    pub price: f64,
    pub success: bool,
    pub order_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub failure_reason: crate::trade::trader::OrderFailureReason,
}

#[derive(Debug)]
pub struct ExecutionSummary {
    pub audits: Vec<TradeExecutionAudit>,
    pub status: Result<()>,
}

impl TraderAgent {
    pub fn new(executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>, ledger: Arc<Ledger>) -> Self {
        Self {
            executor,
            ledger,
            poll_interval: std::time::Duration::from_secs(2),
            max_poll_attempts: 30,
        }
    }

    pub fn with_poll_settings(mut self, interval: std::time::Duration, attempts: usize) -> Self {
        self.poll_interval = interval;
        self.max_poll_attempts = attempts;
        self
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

            let mut final_qty = trade.qty;
            let capacity_result = {
                let exec = self.executor.lock().await;
                exec.get_tradable_capacity(&trade.symbol, trade.price).await
            };

            match capacity_result {
                Ok(capacity) => {
                    let max_available = if trade.side == TradeSide::Buy {
                        capacity.max_buy
                    } else {
                        capacity.max_sell
                    };

                    if trade.is_liquidation {
                        println!("🔥 [Trader - EXIT] Liquidation requested. Setting qty to max available: {}", max_available);
                        final_qty = max_available;
                    } else if trade.is_trim {
                        println!(
                            "✂️ [Trader - TRIM] 50% reduction requested. Setting qty to: {}",
                            max_available * 0.5
                        );
                        final_qty = max_available * 0.5;
                    } else if final_qty > max_available {
                        println!(
                            "⚠️ [Trader - CAPPING] Requested {} for {} exceeds broker capacity {}. Capping to max.",
                            final_qty, trade.symbol, max_available
                        );
                        final_qty = max_available;
                    }
                }
                Err(e) => {
                    println!(
                        "❌ [Trader - ERROR] Failed to query capacity for {}: {}. Aborting trade.",
                        trade.symbol, e
                    );
                    let audit = TradeExecutionAudit {
                        symbol: trade.symbol.clone(),
                        side: format!("{:?}", trade.side),
                        qty_requested: trade.qty,
                        qty_filled: 0.0,
                        price: trade.price,
                        success: false,
                        order_id: None,
                        status: "CapacityQueryFailed".to_string(),
                        error: Some(e.to_string()),
                        failure_reason: crate::trade::trader::OrderFailureReason::Other(
                            999,
                            "Capacity query failed".to_string(),
                        ),
                    };
                    audits.push(audit);
                    errors.push(format!("Capacity query failed for {}", trade.symbol));
                    continue;
                }
            }

            if final_qty <= 0.0 {
                println!(
                    "🚫 [Trader - SKIPPED] Tradable capacity is 0 for {}. Skipping order.",
                    trade.symbol
                );
                continue;
            }

            let req = PlaceOrderRequest {
                symbol: trade.symbol.clone(),
                side: order_side.clone(),
                order_type: OrderType::Normal,
                qty: final_qty,
                price: Some(trade.price),
            };

            let mut audit = TradeExecutionAudit {
                symbol: trade.symbol.clone(),
                side: format!("{:?}", trade.side),
                qty_requested: final_qty,
                qty_filled: 0.0,
                price: trade.price,
                success: false,
                order_id: None,
                status: "Pending".to_string(),
                error: None,
                failure_reason: crate::trade::trader::OrderFailureReason::None,
            };

            // 3. Narrow lock scope for submission
            let submission_result = {
                let exec = self.executor.lock().await;
                exec.place_order(req).await
            };

            match submission_result {
                Ok(res) => {
                    if let Some(order_id) = res.order_id {
                        println!(
                            "🚀 [Trader - SUBMITTED] Order placed for {}. Order ID: {}. Waiting for fill...",
                            trade.symbol, order_id
                        );
                        audit.order_id = Some(order_id.clone());

                        // --- Polling for Order Status (Lifecycle Closure) ---
                        let mut final_details = None;
                        for attempt in 1..=self.max_poll_attempts {
                            // Narrow lock scope for EACH status check
                            let status_query = {
                                let exec = self.executor.lock().await;
                                exec.get_order_status(&order_id).await
                            };

                            match status_query {
                                Ok(details) => match details.status {
                                    crate::trade::trader::OrderStatus::Filled
                                    | crate::trade::trader::OrderStatus::PartiallyFilled
                                    | crate::trade::trader::OrderStatus::Cancelled
                                    | crate::trade::trader::OrderStatus::Rejected
                                    | crate::trade::trader::OrderStatus::Failed => {
                                        final_details = Some(details);
                                        break;
                                    }
                                    _ => {
                                        if attempt % 5 == 0 {
                                            println!(
                                                "⏳ [Trader - WAITING] Order {} still {:?} after {}s...",
                                                order_id,
                                                details.status,
                                                attempt * 2
                                            );
                                        }
                                    }
                                },
                                Err(e) => {
                                    println!(
                                        "⚠️ [Trader - ERROR] Status query failed for {}: {}",
                                        order_id, e
                                    );
                                }
                            }
                            // The lock is released here, allowing other orders to proceed during sleep
                            tokio::time::sleep(self.poll_interval).await;
                        }

                        if let Some(details) = final_details {
                            println!(
                                "🎯 [Trader - FINAL] Order {} status: {:?} (Filled: {} @ {:.2})",
                                order_id, details.status, details.qty_filled, details.avg_price
                            );
                            audit.success = details.qty_filled > 0.0;
                            audit.qty_filled = details.qty_filled;
                            audit.price = if details.avg_price > 0.0 {
                                details.avg_price
                            } else {
                                trade.price
                            };
                            audit.status = format!("{:?}", details.status);
                            audit.failure_reason = details.failure_reason.clone();

                            if audit.qty_filled > 0.0 {
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
                                    qty: details.qty_filled,
                                    price: audit.price,
                                    signal: format!("{:?}", trade.side),
                                });
                            }
                        } else {
                            println!(
                                "🛑 [Trader - TIMEOUT] Order {} status query timed out after {}s.",
                                order_id,
                                self.poll_interval.as_secs() * self.max_poll_attempts as u64
                            );

                            // --- P2-2: Behavioral Closure - Automatic Cancellation on Timeout ---
                            println!(
                                "📡 [Trader - CANCEL] Attempting to cancel timed-out order {}...",
                                order_id
                            );
                            let cancel_res = {
                                let exec = self.executor.lock().await;
                                exec.cancel_order(&order_id).await
                            };

                            match cancel_res {
                                Ok(_) => {
                                    println!(
                                        "✅ [Trader - CANCEL] Order {} cancellation requested.",
                                        order_id
                                    );

                                    // --- P2-2: Absolute Closure - Final Confirmation ---
                                    let final_check = {
                                        let exec = self.executor.lock().await;
                                        exec.get_order_status(&order_id).await
                                    };

                                    match final_check {
                                        Ok(details)
                                            if details.status
                                                == crate::trade::trader::OrderStatus::Cancelled =>
                                        {
                                            println!("🏁 [Trader - CONFIRMED] Order {} is verified CANCELLED at broker.", order_id);
                                            audit.status = "TimedOutCancelledConfirmed".to_string();
                                            audit.qty_filled = details.qty_filled;
                                            audit.price = if details.avg_price > 0.0 {
                                                details.avg_price
                                            } else {
                                                trade.price
                                            };
                                        }
                                        _ => {
                                            println!("❓ [Trader - UNKNOWN] Order {} cancellation requested but not yet confirmed as terminal.", order_id);
                                            audit.status = "TimedOutCancelRequested".to_string();
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!(
                                        "❌ [Trader - CANCEL] Failed to cancel order {}: {}",
                                        order_id, e
                                    );
                                    audit.status = "TimedOutCancellationFailed".to_string();
                                    audit.error = Some(format!("Cancellation failed: {}", e));
                                }
                            }
                        }
                    } else {
                        // Immediate rejection
                        println!(
                            "❌ [Trader - REJECTED] Order rejected by broker for {}: {:?}",
                            trade.symbol, res.failure_reason
                        );
                        audit.status = "Rejected".to_string();
                        audit.failure_reason = res.failure_reason;
                    }
                }
                Err(e) => {
                    println!(
                        "❌ [Trader - FAILED] Failed to submit order for {}: {}",
                        trade.symbol, e
                    );
                    let err_msg: String = e.to_string();
                    audit.error = Some(err_msg);
                    audit.status = "SubmitFailed".to_string();
                    errors.push(format!("{}: {}", trade.symbol, e));
                }
            }
            audits.push(audit);
        }

        let execution_status = if !errors.is_empty() {
            Err(anyhow::anyhow!(
                "Partial trade execution failure: {}",
                errors.join("; ")
            ))
        } else {
            Ok(())
        };

        Ok(ExecutionSummary {
            audits,
            status: execution_status,
        })
    }

    /// 执行持仓对账 (P2-3)
    /// 对比本地 Ledger 导出的理论持仓与 Broker 侧的真实持仓
    pub async fn reconcile_positions(&self) -> Result<ReconciliationReport> {
        println!("🔍 Starting position reconciliation...");

        // 1. 获取本地持仓
        let (_, local_positions) = self.ledger.get_portfolio_stats();

        // 2. 获取 Broker 持仓
        let broker_positions = {
            let exec = self.executor.lock().await;
            exec.get_positions().await?
        };

        let mut mismatches = Vec::new();
        let mut matching_count = 0;

        // 3. 对比逻辑
        let mut broker_map: std::collections::HashMap<String, f64> = broker_positions
            .into_iter()
            .map(|p| (p.symbol, p.qty))
            .collect();

        // Check local vs broker
        for (symbol, (local_qty, _)) in local_positions {
            if local_qty == 0.0 {
                continue;
            }

            let b_qty = broker_map.remove(&symbol).unwrap_or(0.0);
            let diff = local_qty - b_qty;

            if diff.abs() > 0.001 {
                mismatches.push(PositionMismatch {
                    symbol: symbol.clone(),
                    local_qty,
                    broker_qty: b_qty,
                    diff,
                });
            } else {
                matching_count += 1;
            }
        }

        // Check remaining broker positions (not in local ledger)
        for (symbol, broker_qty) in broker_map {
            if broker_qty == 0.0 {
                continue;
            }
            mismatches.push(PositionMismatch {
                symbol: symbol.clone(),
                local_qty: 0.0,
                broker_qty,
                diff: -broker_qty,
            });
        }

        let report = ReconciliationReport {
            timestamp: Local::now().to_rfc3339(),
            mismatches,
            matching_count,
        };

        if report.mismatches.is_empty() {
            println!(
                "✅ Reconciliation successful! All {} positions match.",
                report.matching_count
            );
        } else {
            println!(
                "⚠️ Reconciliation found {} mismatches!",
                report.mismatches.len()
            );
            for m in &report.mismatches {
                println!(
                    "   - {}: Local={} Broker={} Diff={}",
                    m.symbol, m.local_qty, m.broker_qty, m.diff
                );
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::action_matrix::{AssetAction, AssetActionDecision};
    use crate::core::asset_state::{AssetState, AssetStateSnapshot};
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
            low_stability_streak: 0,
            duration_in_state: 1,
            transition_audit: None,
        };
        let mut policy = PortfolioPolicy::from_market_regime(&market);
        policy.risk_assets_mode = RiskAssetsMode::NEUTRAL; // Force NEUTRAL for test stability

        let assets = vec![AssetActionDecision {
            symbol: "AAPL".to_string(),
            price: 150.0,
            asset_state: AssetStateSnapshot {
                symbol: "AAPL".to_string(),
                state: AssetState::OPTIMAL,
                reasons: vec![],
                recovery_streak: 0,
                last_defend_age: 100,
            },
            action: AssetAction::ACCUMULATE,
            reasons: vec![],
            deviation: Some(10.0),
            z_score: Some(1.0),
            trade_enabled: true,
            trade_amount: 5000.0,
            config_multiplier: 1.0,
            prev_action: None,
            action_changed: false,
            position_intent: crate::core::exit::PositionIntent::ADD,
            ..Default::default()
        }];

        use crate::core::decision::DecisionPacket;
        use crate::core::execution_gate::ExecutionGate;

        let market_features = MarketFeatures::default();
        let packet = DecisionPacket::new(
            chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            market_features,
            market,
            None,
            policy,
            assets,
            Vec::new(),
            false,
            crate::core::trend_cohesion::TrendCohesionSnapshot::default(),
            None,
            None,
        );

        let trading_config = config_arc.trading.as_ref().unwrap();
        let execution_result =
            ExecutionGate::gate_packet(&packet, trading_config, 0.0, 100000.0, 0.0);

        let summary = agent
            .execute_signals(execution_result.trades)
            .await
            .expect("Preflight should not fail");
        summary.status.expect("Execution should not fail");

        // Verify audit details
        assert!(!summary.audits.is_empty());
        let audit = &summary.audits[0];
        assert_eq!(audit.symbol, "AAPL");
        assert_eq!(audit.status, "Filled");
        assert_eq!(audit.qty_filled, 33.0); // 5000 / 150 = 33.33 -> 33.0

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

    #[tokio::test]
    async fn test_trader_agent_capping() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();

        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        // --- Set Low Capacity ---
        {
            let exec = mock_exec.lock().await;
            let mut cap = exec.mock_capacity.lock().await;
            cap.max_buy = 5.0; // Very low capacity
        }

        let ledger = Arc::new(Ledger::new(save_dir.clone()));
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        let trade = crate::core::execution_gate::GatedTrade {
            symbol: "AAPL".to_string(),
            side: TradeSide::Buy,
            qty: 50.0, // Requested 50
            price: 150.0,
            reason: "Test".to_string(),
            is_liquidation: false,
            is_trim: false,
        };

        let summary = agent.execute_signals(vec![trade]).await.unwrap();

        assert!(!summary.audits.is_empty());
        let audit = &summary.audits[0];

        // SHOULD BE CAPPED TO 5.0
        assert_eq!(audit.qty_requested, 5.0);
        assert_eq!(audit.qty_filled, 5.0); // Filled because mock returns status Filled for 2nd query
    }

    #[tokio::test]
    async fn test_trader_agent_liquidation_semantics() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        // 1. Setup high capacity (current position = 1000)
        {
            let exec = mock_exec.lock().await;
            let mut cap = exec.mock_capacity.lock().await;
            cap.max_sell = 1000.0;
        }

        let ledger = Arc::new(Ledger::new(save_dir.clone()));
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        // 2. Dispatch a trade with qty=1.0 but is_liquidation=true
        let trade = crate::core::execution_gate::GatedTrade {
            symbol: "EXIT_ASSET".to_string(),
            side: TradeSide::Sell,
            qty: 1.0, // Signal only requested 1 units
            price: 150.0,
            reason: "ExitTest".to_string(),
            is_liquidation: true,
            is_trim: false,
        };

        let summary = agent.execute_signals(vec![trade]).await.unwrap();
        assert!(!summary.audits.is_empty());

        // 3. Verify qty was corrected to 1000.0 (max_sell)
        assert_eq!(summary.audits[0].qty_requested, 1000.0);
    }

    #[tokio::test]
    async fn test_trader_agent_trim_semantics() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        // 1. Setup capacity (current position = 1000)
        {
            let exec = mock_exec.lock().await;
            let mut cap = exec.mock_capacity.lock().await;
            cap.max_sell = 1000.0;
        }

        let ledger = Arc::new(Ledger::new(save_dir.clone()));
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        // 2. Dispatch a trade with is_trim=true
        let trade = crate::core::execution_gate::GatedTrade {
            symbol: "TRIM_ASSET".to_string(),
            side: TradeSide::Sell,
            qty: 0.0, // Gate doesn't specify qty for trim
            price: 150.0,
            reason: "TrimTest".to_string(),
            is_liquidation: false,
            is_trim: true,
        };

        let summary = agent.execute_signals(vec![trade]).await.unwrap();
        assert!(!summary.audits.is_empty());

        // 3. Verify qty was corrected to 500.0 (50% of max_sell)
        assert_eq!(summary.audits[0].qty_requested, 500.0);
    }

    #[tokio::test]
    async fn test_trader_agent_capacity_query_failure() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();

        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        let ledger = Arc::new(Ledger::new(save_dir.clone()));
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        let trade = crate::core::execution_gate::GatedTrade {
            symbol: "FAIL".to_string(),
            side: TradeSide::Buy,
            qty: 50.0,
            price: 150.0,
            reason: "Test Fail".to_string(),
            is_liquidation: false,
            is_trim: false,
        };

        // Note: MockTradeExecutor currently doesn't fail unless we modify it to handle specific symbols.
        // Let's implement a symbol-based failure in MockTradeExecutor for testing.

        let summary = agent.execute_signals(vec![trade]).await.unwrap();

        assert!(!summary.audits.is_empty());
        let audit = &summary.audits[0];

        // CHECK STATUS & REASON
        assert_eq!(audit.status, "CapacityQueryFailed");
        assert!(audit
            .error
            .as_ref()
            .unwrap()
            .contains("Mock capacity query failure"));

        // VERIFY ORDER WAS NOT PLACED
        let count = mock_exec
            .lock()
            .await
            .placed_orders_count
            .load(Ordering::SeqCst);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_trader_agent_timeout_cancellation() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();

        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        // --- Configure Mock to stay in Submitted status ---
        // By default, MockTradeExecutor returns Filled after 2 queries.
        // We don't have a direct way to change the "Filled threshold" without modifying Mock,
        // so let's just assert that it hits the timeout if we mock it to never increment or similar.
        // Actually, let's keep it simple: Mock already hits Timeout if it doesn't return terminal status.

        // Let's modify MockTradeExecutor::get_order_status to stay Submitted for a specific symbol.

        let ledger = Arc::new(Ledger::new(save_dir.clone()));
        // Accelerated polling: 5 attempts * 1ms = 5ms total "timeout"
        let agent = TraderAgent::new(mock_exec.clone(), ledger)
            .with_poll_settings(std::time::Duration::from_millis(1), 5);

        let trade = crate::core::execution_gate::GatedTrade {
            symbol: "STAY_SUBMITTED".to_string(),
            side: TradeSide::Buy,
            qty: 10.0,
            price: 150.0,
            reason: "Timeout Test".to_string(),
            is_liquidation: false,
            is_trim: false,
        };

        let summary = agent
            .execute_signals(vec![trade])
            .await
            .expect("Execution should not fail");

        assert!(!summary.audits.is_empty());
        let audit = &summary.audits[0];

        // VERIFY STATUS: Should be confirmed as Cancelled by the final check
        assert_eq!(audit.status, "TimedOutCancelledConfirmed");

        // VERIFY MOCK STATE: order_id should be in cancelled_orders set
        let order_id = audit.order_id.as_ref().expect("Order ID should exist");
        let mock = mock_exec.lock().await;
        let cancelled = mock.cancelled_orders.lock().await;
        assert!(cancelled.contains(order_id));

        println!(
            "✅ Fast timeout cancellation verified for order {}",
            order_id
        );
    }

    #[tokio::test]
    async fn test_trader_agent_reconciliation() {
        use crate::core::ledger::TradeRecord;
        use crate::trade::trader::{Position, PositionSide};
        use chrono::Local;

        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let ledger = Arc::new(Ledger::new(save_dir.clone()));

        // 1. Setup local ledger: 10 TSLA, 20 AAPL
        ledger
            .record_trade(TradeRecord {
                date: Local::now().date_naive(),
                timestamp: "10:00:00".to_string(),
                symbol: "US.TSLA".to_string(),
                side: "BUY".to_string(),
                qty: 10.0,
                price: 200.0,
                signal: "TEST".to_string(),
            })
            .unwrap();
        ledger
            .record_trade(TradeRecord {
                date: Local::now().date_naive(),
                timestamp: "10:01:00".to_string(),
                symbol: "US.AAPL".to_string(),
                side: "BUY".to_string(),
                qty: 20.0,
                price: 150.0,
                signal: "TEST".to_string(),
            })
            .unwrap();

        // 2. Setup mock executor: 10 TSLA (Match), 25 AAPL (Mismatch), 5 NVDA (Broker only)
        let _mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        struct ReconMock;
        #[async_trait::async_trait]
        impl crate::trade::trader::TradeExecutor for ReconMock {
            async fn get_account_funds(&self) -> Result<crate::trade::trader::AccountFunds> {
                unreachable!()
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::trade::trader::BrokerPermissions> {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::trade::trader::TradableCapacity> {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: crate::trade::trader::PlaceOrderRequest,
            ) -> Result<crate::trade::trader::PlaceOrderResponse> {
                unreachable!()
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::trade::trader::OrderExecutionDetails> {
                unreachable!()
            }
            async fn unlock_trade(&self) -> Result<()> {
                Ok(())
            }
            async fn cancel_order(&self, _: &str) -> Result<()> {
                Ok(())
            }
            async fn get_positions(&self) -> Result<Vec<Position>> {
                Ok(vec![
                    Position {
                        symbol: "US.TSLA".to_string(),
                        side: PositionSide::Long,
                        qty: 10.0,
                        can_sell_qty: 10.0,
                        cost_price: 200.0,
                        market_val: 2000.0,
                        pl_val: 0.0,
                        pl_ratio: 0.0,
                    },
                    Position {
                        symbol: "US.AAPL".to_string(),
                        side: PositionSide::Long,
                        qty: 25.0, // Mismatch (Local 20)
                        can_sell_qty: 25.0,
                        cost_price: 150.0,
                        market_val: 3750.0,
                        pl_val: 0.0,
                        pl_ratio: 0.0,
                    },
                    Position {
                        symbol: "US.NVDA".to_string(), // Broker only
                        side: PositionSide::Long,
                        qty: 5.0,
                        can_sell_qty: 5.0,
                        cost_price: 800.0,
                        market_val: 4000.0,
                        pl_val: 0.0,
                        pl_ratio: 0.0,
                    },
                ])
            }
        }

        let agent = TraderAgent::new(Arc::new(Mutex::new(ReconMock)), ledger);
        let report = agent.reconcile_positions().await.unwrap();

        // 3. Verify
        assert_eq!(report.matching_count, 1); // Only TSLA matches
        assert_eq!(report.mismatches.len(), 2);

        // AAPL Mismatch
        let aapl = report
            .mismatches
            .iter()
            .find(|m| m.symbol == "US.AAPL")
            .unwrap();
        assert_eq!(aapl.local_qty, 20.0);
        assert_eq!(aapl.broker_qty, 25.0);
        assert_eq!(aapl.diff, -5.0);

        // NVDA Broker-only
        let nvda = report
            .mismatches
            .iter()
            .find(|m| m.symbol == "US.NVDA")
            .unwrap();
        assert_eq!(nvda.local_qty, 0.0);
        assert_eq!(nvda.broker_qty, 5.0);
    }
}
