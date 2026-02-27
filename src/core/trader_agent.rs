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

    pub async fn execute_signals(
        &self,
        snapshots: &[TickerSnapshot],
        gravity_health: &crate::core::report::GravityHealth,
    ) -> Result<()> {
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

        // Phase 12 Hardening: Circuit Breaker
        if gravity_health.global_potential_energy > 1.8 {
            println!("🚨 TraderAgent: High Gravity Potential ({:.2} > 1.8). Global Circuit Breaker ACTIVE. Skipping all BUY signals.", gravity_health.global_potential_energy);
        }

        // Phase 12 Hardening: Check daily budget
        let daily_traded = self.ledger.get_daily_traded_amount();
        if let Some(max_daily) = trading_config.max_daily_budget {
            if daily_traded >= max_daily {
                println!("🛑 TraderAgent: Daily budget limit reached (${:.2} / ${:.2}). Skipping further trades.", daily_traded, max_daily);
                return Ok(());
            }
        }

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

                // Phase 12 Hardening: Dynamic Sizing based on Confidence (0-100)
                // Phase 14: Dynamic Risk Sizing Multipliers based on Capital State
                let confidence_factor = snap.confidence_score as f64 / 100.0;
                let state_multiplier = self.config.get_parsed_rules().sizing_multipliers.get(&snap.state_code).copied().unwrap_or(1.0);
                let adjusted_amount = trade_amount * confidence_factor * state_multiplier;

                // Core logic mapping
                let (side, action_amount) = match snap.state_code.as_str() {
                    // BUY Signals
                    "optimal" | "pullback" | "fear_1" | "fear_2" => {
                        if gravity_health.global_potential_energy > 1.8 {
                            (None, 0.0) // Blocked by circuit breaker
                        } else {
                            (Some(OrderSide::Buy), adjusted_amount)
                        }
                    }
                    // SELL Signals
                    "overheat_1" | "overheat_2" => (Some(OrderSide::Sell), adjusted_amount),
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
                    println!("⚡ [Trader] Signal [{}] detected for {}! Preparing to {} {} shares @ {:.2} (Targeting ${:.2} | Base: ${:.2} x Conf: {:.2} x Mul: {:.2})", 
                        snap.state_code, snap.symbol, side_str, qty, snap.dog_price, action_amount, trade_amount, confidence_factor, state_multiplier
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
                max_daily_budget: None,
            }),
            rules: crate::config::RulesConfig {
                trend: crate::config::TrendConfig {
                    lookback_days: 20,
                    flat_threshold_pct: 0.5,
                },
                deviation_bands: std::collections::BTreeMap::new(),
                actions: std::collections::HashMap::new(),
                sizing_multipliers: None,
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

    fn create_test_gravity_health() -> crate::core::report::GravityHealth {
        crate::core::report::GravityHealth {
            up_count: 5,
            flat_count: 2,
            forming_early_count: 0,
            forming_late_count: 0,
            universe_count: 7,
            total_count: 7,
            up_weight: 5.0,
            flat_weight: 2.0,
            forming_early_weight: 0.0,
            forming_late_weight: 0.0,
            total_weight: 7.0,
            global_gravity_strength: 1.0,
            global_potential_energy: 0.5,
            trend_alloc_weight: 5.0,
            reversion_alloc_weight: 0.0,
            config_hash: "test".to_string(),
            system_confidence: 80.0,
            market_phase: "Bull".to_string(),
            capital_flow_vector: "Up".to_string(),
            recommended_exposure: 0.8,
            prev_system_confidence: None,
            prev_dominance_margin: None,
            prev_recommended_exposure: None,
            prev_up_count: None,
            regime_age: 10,
            stability_score: 0.9,
            base_exposure: 0.8,
            adjusted_exposure: 0.8,
            conf_trend_alloc: 40.0,
            conf_inverse_potential: 40.0,
            capital_flow_acceleration: None,
            universe_integrity: 1.0,
            trend_maturity: 0.25,
            stability_structural: 40.0,
            stability_temporal: 25.0,
            temporal_modifier: 0.9,
            integrity_multiplier: 1.0,
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
        let gravity = create_test_gravity_health();
        agent.execute_signals(&[snap1], &gravity).await.unwrap();
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
        agent.execute_signals(&[snap2], &gravity).await.unwrap();
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
        agent.execute_signals(&[snap3], &gravity).await.unwrap();
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
        let gravity = create_test_gravity_health();
        agent.execute_signals(&[snap1], &gravity).await.unwrap();
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
