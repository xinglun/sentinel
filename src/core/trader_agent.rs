use crate::config::AppConfig;
use crate::core::engine::TickerSnapshot;
use crate::core::ledger::{Ledger, TradeRecord};
use crate::trade::trader::{OrderSide, OrderType, PlaceOrderRequest, TradeExecutor};
use anyhow::Result;
use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TraderAgent {
    config: Arc<AppConfig>,
    executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>,
    ledger: Arc<Ledger>,
}

impl TraderAgent {
    pub fn new(
        config: Arc<AppConfig>,
        executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>,
        ledger: Arc<Ledger>,
    ) -> Self {
        Self {
            config,
            executor,
            ledger,
        }
    }

    pub async fn execute_signals(&self, snapshots: &[TickerSnapshot]) -> Result<()> {
        let trading_config = match &self.config.trading {
            Some(tc) => tc,
            None => {
                println!("⚠️  Trading configurations not found in config.toml. Auto-trading is disabled.");
                return Ok(());
            }
        };

        if !trading_config.enabled {
            println!("⏸️  Auto-trading is globally disabled. (trading.enabled = false). Signals will not be executed.");
            return Ok(());
        }

        println!(
            "🤖 TraderAgent: Processing {} snapshots for potential execution...",
            snapshots.len()
        );

        // We lock the executor momentarily to get available funds (to respect boundaries)
        let _available_funds = {
            let exec = self.executor.lock().await;
            match exec.get_funds().await {
                Ok(funds) => funds.cash,
                Err(e) => {
                    println!("❌ TraderAgent Failed to retrieve account funds: {}. Aborting execution run.", e);
                    return Err(e);
                }
            }
        };

        println!("💰 TraderAgent Available Cash: ${:.2}", _available_funds);

        for snap in snapshots {
            // Find the specific watchlist configuration for the ticker
            let wl_entry = self
                .config
                .watchlist
                .iter()
                .find(|w| w.symbol == snap.symbol);

            if let Some(entry) = wl_entry {
                let trade_enabled = entry.trade_enabled.unwrap_or(false);
                let trade_amount = entry.trade_amount.unwrap_or(0.0);

                if !trade_enabled {
                    continue;
                }

                if trade_amount <= 0.0 {
                    println!("⚠️  [Trader] {} is trade_enabled but has no trade_amount configured. Skipping.", snap.symbol);
                    continue;
                }

                // Phase 12 Hardening: Prevent duplicate trades for the same ticker/signal today
                if self.ledger.has_acted_today(&snap.symbol, &snap.state_code) {
                    continue;
                }

                // Core logic mapping
                let (side, action_amount) = match snap.state_code.as_str() {
                    // BUY Signals
                    "optimal" | "pullback" | "fear_1" | "fear_2" => {
                        (Some(OrderSide::Buy), trade_amount)
                    }
                    // SELL Signals
                    "overheat_1" | "overheat_2" => (Some(OrderSide::Sell), trade_amount),
                    // HOLD / WAIT (cruise, regime_forming, DEFEND, CAUTION)
                    _ => (None, 0.0),
                };

                if let Some(order_side) = side {
                    // Calculate shares based on targeted trade amount and current market dog_price.
                    if snap.dog_price <= 0.0 {
                        println!(
                            "❌ [Trader] Invalid price {:.2} for {}. Skipping.",
                            snap.dog_price, snap.symbol
                        );
                        continue;
                    }

                    // For now, round to nearest whole share. (For HK you'd calculate board lots).
                    let qty = (action_amount / snap.dog_price).floor();

                    if qty <= 0.0 {
                        println!("⚠️  [Trader] Target amount {:.2} for {} is too small to buy 1 share at price {:.2}", action_amount, snap.symbol, snap.dog_price);
                        continue;
                    }

                    let side_str = if order_side == OrderSide::Buy {
                        "BUY"
                    } else {
                        "SELL"
                    };
                    println!("⚡ [Trader] Signal [{}] detected for {}! Preparing to {} {} shares @ {:.2} (Targeting ${:.2})", 
                        snap.state_code, snap.symbol, side_str, qty, snap.dog_price, action_amount
                    );

                    // Actually dispatch the order!
                    let exec = self.executor.lock().await;
                    let req = PlaceOrderRequest {
                        symbol: snap.symbol.clone(),
                        side: order_side.clone(),
                        order_type: OrderType::Normal,
                        qty,
                        price: Some(snap.dog_price),
                    };

                    match exec.place_order(req).await {
                        Ok(res) => {
                            println!(
                                "✅ [Trader - SUCCESS] Order placed for {}. Order ID: {}",
                                snap.symbol, res.order_id
                            );

                            // Record to ledger to prevent duplicate execution in next pulse
                            let _ = self.ledger.record_trade(TradeRecord {
                                date: Local::now().date_naive(),
                                timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                                symbol: snap.symbol.clone(),
                                side: side_str.to_string(),
                                qty,
                                price: snap.dog_price,
                                signal: snap.state_code.clone(),
                            });
                        }
                        Err(e) => {
                            println!(
                                "❌ [Trader - FAILED] Failed to complete {} order for {}: {}",
                                side_str, snap.symbol, e
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DeviationBasis, TradingConfig, WatchlistEntry};
    use crate::core::engine::{RegimeValidity, TrendStatus};
    use crate::trade::trader::{AccountFunds, PlaceOrderResponse};
    use async_trait::async_trait;
    use chrono::NaiveDate;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockTradeExecutor {
        pub placed_orders_count: AtomicUsize,
    }

    #[async_trait]
    impl TradeExecutor for MockTradeExecutor {
        async fn unlock_trade(&self) -> Result<()> {
            Ok(())
        }
        async fn get_funds(&self) -> Result<AccountFunds> {
            Ok(AccountFunds {
                power: 10000.0,
                total_assets: 10000.0,
                cash: 10000.0,
                market_val: 0.0,
                unrealized_pl: 0.0,
            })
        }
        async fn place_order(&self, _req: PlaceOrderRequest) -> Result<PlaceOrderResponse> {
            self.placed_orders_count.fetch_add(1, Ordering::SeqCst);
            Ok(PlaceOrderResponse {
                order_id: "mock_id_123".to_string(),
                status: "submitted".to_string(),
            })
        }
    }

    fn create_test_config(
        global_enabled: bool,
        symbol_enabled: bool,
        trade_amount: f64,
    ) -> Arc<AppConfig> {
        let wl = WatchlistEntry {
            symbol: "TEST".to_string(),
            name: None,
            weight: None,
            market: "US".to_string(),
            owner_ma_days: 120,
            leash_ma_days: 20,
            caution_ma_days: None,
            deviation_basis: DeviationBasis::Owner,
            enable: true,
            action_overrides: None,
            trade_enabled: Some(symbol_enabled),
            trade_amount: Some(trade_amount),
        };

        Arc::new(AppConfig {
            version: 1,
            output: crate::config::OutputConfig {
                timezone: "UTC".to_string(),
                format: "md".to_string(),
                save_to: ".".to_string(),
                include_summary: false,
                weight_kind: Some("equal".to_string()),
            },
            telegram: None,
            futu: None,
            trading: Some(TradingConfig {
                enabled: global_enabled,
                global_budget: 10000.0,
            }),
            rules: crate::config::RulesConfig {
                trend: crate::config::TrendConfig {
                    lookback_days: 20,
                    flat_threshold_pct: 0.5,
                },
                deviation_bands: std::collections::BTreeMap::new(),
                actions: std::collections::HashMap::new(),
                bear_mode: crate::config::BearModeConfig {
                    enabled: false,
                    fallback_action: "".to_string(),
                    caution_action: None,
                    buffer_pct: Some(3.0),
                    confirm_days: Some(5),
                    confirm_threshold: Some(3),
                    recover_days: Some(5),
                    recover_threshold: Some(3),
                },
            },
            watchlist: vec![wl],
        })
    }

    fn create_test_snapshot(state_code: &str, dog_price: f64) -> TickerSnapshot {
        TickerSnapshot {
            symbol: "TEST".to_string(),
            name: "TEST".to_string(),
            weight: 1.0,
            reason_code: None,
            current_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            dog_price,
            owner_ma: None,
            leash_ma: None,
            owner_ma_slope_pct: None,
            dev_z_score: None,
            curvature: None,
            confidence_score: 99,
            trend_status: TrendStatus::Up,
            deviation_pct: None,
            deviation_basis_used: "owner".to_string(),
            state_code: state_code.to_string(),
            action_text: "".to_string(),
            is_bear_mode_active: false,
            is_caution_mode_active: false,
            trend_age: 10,
            owner_deviation_pct: None,
            deviation_percentile: None,
            validity: RegimeValidity::Valid,
            history_days: 500,
        }
    }

    #[tokio::test]
    async fn test_trader_agent_dispatch() {
        let config = create_test_config(true, true, 5000.0);
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor {
            placed_orders_count: AtomicUsize::new(0),
        }));
        // Use a temporary directory for the ledger in tests
        let temp_dir = std::env::temp_dir().join(format!(
            "test_ledger_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let ledger = Arc::new(Ledger::new(temp_dir));
        let agent = TraderAgent::new(config, mock_exec.clone(), ledger);

        // Test 1: Optimal signal (Should Buy)
        let snap1 = create_test_snapshot("optimal", 100.0);
        agent.execute_signals(&[snap1]).await.unwrap();
        assert_eq!(
            mock_exec
                .lock()
                .await
                .placed_orders_count
                .load(Ordering::SeqCst),
            1
        );

        // Test 2: Overheat signal (Should Sell)
        let snap2 = create_test_snapshot("overheat_2", 150.0);
        agent.execute_signals(&[snap2]).await.unwrap();
        assert_eq!(
            mock_exec
                .lock()
                .await
                .placed_orders_count
                .load(Ordering::SeqCst),
            2
        );

        // Test 3: Hold signal (Should Ignore)
        let snap3 = create_test_snapshot("cruise", 120.0);
        agent.execute_signals(&[snap3]).await.unwrap();
        assert_eq!(
            mock_exec
                .lock()
                .await
                .placed_orders_count
                .load(Ordering::SeqCst),
            2
        ); // Unchanged
    }

    #[tokio::test]
    async fn test_trader_agent_disabled() {
        // Global disabled
        let config = create_test_config(false, true, 5000.0);
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor {
            placed_orders_count: AtomicUsize::new(0),
        }));
        let temp_dir = std::env::temp_dir().join(format!(
            "test_ledger_disabled_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let ledger = Arc::new(Ledger::new(temp_dir));
        let agent = TraderAgent::new(config, mock_exec.clone(), ledger);
        let snap1 = create_test_snapshot("optimal", 100.0);
        agent.execute_signals(&[snap1]).await.unwrap();
        assert_eq!(
            mock_exec
                .lock()
                .await
                .placed_orders_count
                .load(Ordering::SeqCst),
            0
        );
    }
}
