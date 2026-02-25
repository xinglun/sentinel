use crate::config::AppConfig;
use crate::core::engine::TickerSnapshot;
use crate::trade::trader::{TradeExecutor, OrderSide, OrderType, PlaceOrderRequest};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TraderAgent {
    config: Arc<AppConfig>,
    executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>,
}

impl TraderAgent {
    pub fn new(config: Arc<AppConfig>, executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>) -> Self {
        Self { config, executor }
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

        println!("🤖 TraderAgent: Processing {} snapshots for potential execution...", snapshots.len());

        // We lock the executor momentarily to get available funds (to respect boundaries)
        let mut available_funds = 0.0;
        {
            let exec = self.executor.lock().await;
            match exec.get_funds().await {
                Ok(funds) => available_funds = funds.cash,
                Err(e) => {
                    println!("❌ TraderAgent Failed to retrieve account funds: {}. Aborting execution run.", e);
                    return Err(e);
                }
            }
        }
        
        println!("💰 TraderAgent Available Cash: ${:.2}", available_funds);

        for snap in snapshots {
            // Find the specific watchlist configuration for the ticker
            let wl_entry = self.config.watchlist.iter().find(|w| w.symbol == snap.symbol);
            
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

                // Core logic mapping
                let (side, action_amount) = match snap.state_code.as_str() {
                    // BUY Signals
                    "optimal" | "pullback" | "fear_1" | "fear_2" => {
                        (Some(OrderSide::Buy), trade_amount)
                    },
                    // SELL Signals
                    "overheat_1" | "overheat_2" => {
                        (Some(OrderSide::Sell), trade_amount)
                    },
                    // HOLD / WAIT (cruise, regime_forming, DEFEND, CAUTION)
                    _ => (None, 0.0)
                };

                if let Some(order_side) = side {
                    // Calculate shares based on targeted trade amount and current market dog_price.
                    if snap.dog_price <= 0.0 {
                        println!("❌ [Trader] Invalid price {:.2} for {}. Skipping.", snap.dog_price, snap.symbol);
                        continue;
                    }

                    // For now, round to nearest whole share. (For HK you'd calculate board lots).
                    let qty = (action_amount / snap.dog_price).floor();

                    if qty <= 0.0 {
                        println!("⚠️  [Trader] Target amount {:.2} for {} is too small to buy 1 share at price {:.2}", action_amount, snap.symbol, snap.dog_price);
                        continue;
                    }

                    let side_str = if order_side == OrderSide::Buy { "BUY" } else { "SELL" };
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
                            println!("✅ [Trader - SUCCESS] Order placed for {}. Order ID: {}", snap.symbol, res.order_id);
                        },
                        Err(e) => {
                            println!("❌ [Trader - FAILED] Failed to complete {} order for {}: {}", side_str, snap.symbol, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
