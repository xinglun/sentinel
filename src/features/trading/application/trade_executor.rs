#![allow(dead_code)]
use anyhow::Result;
use async_trait::async_trait;

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct AccountFunds {
    pub power: f64,         // 购买力
    pub total_assets: f64,  // 总资产
    pub cash: f64,          // 现金
    pub market_val: f64,    // 证券市值
    pub unrealized_pl: f64, // 浮动盈亏
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub enum OrderType {
    Normal, // 普通订单
    Market, // 市价订单
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PlaceOrderRequest {
    pub symbol: String, // ticker (e.g., US.TSLA)
    pub side: OrderSide,
    pub order_type: OrderType,
    pub qty: f64,           // 数量
    pub price: Option<f64>, // 价格 (如果是 limit order则需)
}

#[derive(Debug, Clone)]
pub struct PlaceOrderResponse {
    pub order_id: Option<String>,
    pub failure_reason: OrderFailureReason,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TradableCapacity {
    pub max_buy: f64,
    pub max_sell: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum MarketRight {
    None,
    Unknow,
    BMP,
    Level1,
    Level2,
    SF,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BrokerPermissions {
    pub market_rights: std::collections::HashMap<String, MarketRight>,
    pub sub_quota_total: i32,
    pub sub_quota_used: i32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum OrderStatus {
    Submitted,
    Filled,
    PartiallyFilled,
    Cancelled,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum OrderFailureReason {
    None,
    InsufficientFunds,
    TradingPasswordRequired,
    MarketClosed,
    SecuritySuspended,
    InvalidQuantity,
    InvalidPrice,
    Other(i32, String), // err_code, msg
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum PositionSide {
    Long,
    Short,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Position {
    pub symbol: String,
    pub side: PositionSide,
    pub qty: f64,
    pub can_sell_qty: f64,
    pub cost_price: f64,
    pub market_val: f64,
    pub pl_val: f64,
    pub pl_ratio: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderExecutionDetails {
    pub order_id: String,
    pub symbol: String,
    pub status: OrderStatus,
    pub qty_requested: f64,
    pub qty_filled: f64,
    pub avg_price: f64, // 約定平均価格。
    pub error_msg: Option<String>,
    pub failure_reason: OrderFailureReason,
}

#[async_trait]
pub trait TradeExecutor: Send + Sync {
    /// 取引 unlock を行う（Moomoo / OpenD 固有）。
    async fn unlock_trade(&self) -> Result<()>;

    /// account funds を照会する。
    async fn get_account_funds(&self) -> Result<AccountFunds>;

    /// 注文を発注する。
    async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResponse>;

    /// 注文の最終状態を照会する（回查）。
    async fn get_order_status(&self, order_id: &str) -> Result<OrderExecutionDetails>;

    /// market data 権限と quota を照会する（P1-2）。
    async fn get_broker_permissions(&self) -> Result<BrokerPermissions>;

    /// 最大取引可能数量を照会する（P2-1）。Futu の計算には price が必要。
    async fn get_tradable_capacity(&self, symbol: &str, price: f64) -> Result<TradableCapacity>;

    /// 注文を取り消す（P2-2）。
    async fn cancel_order(&self, order_id: &str) -> Result<()>;

    /// 現在 position を照会する（P2-3）。
    async fn get_positions(&self) -> Result<Vec<Position>>;
}

pub struct MockTradeExecutor {
    pub placed_orders_count: std::sync::atomic::AtomicUsize,
    pub query_counts: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, usize>>>,
    pub mock_capacity: std::sync::Arc<tokio::sync::Mutex<TradableCapacity>>,
    pub order_metadata:
        std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, (String, f64)>>>,
    pub cancelled_orders: std::sync::Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
}

impl MockTradeExecutor {
    pub fn new() -> Self {
        Self {
            placed_orders_count: std::sync::atomic::AtomicUsize::new(0),
            query_counts: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            mock_capacity: std::sync::Arc::new(tokio::sync::Mutex::new(TradableCapacity {
                max_buy: 100000.0,
                max_sell: 100000.0,
            })),
            order_metadata: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            cancelled_orders: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }
}

impl Default for MockTradeExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TradeExecutor for MockTradeExecutor {
    async fn unlock_trade(&self) -> Result<()> {
        Ok(())
    }
    async fn get_account_funds(&self) -> Result<AccountFunds> {
        Ok(AccountFunds {
            power: 100000.0,
            total_assets: 100000.0,
            cash: 100000.0,
            market_val: 0.0,
            unrealized_pl: 0.0,
        })
    }
    async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResponse> {
        self.placed_orders_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let order_id = format!(
            "MOCK-{}",
            self.placed_orders_count
                .load(std::sync::atomic::Ordering::SeqCst)
        );

        {
            let mut metadata = self.order_metadata.lock().await;
            metadata.insert(order_id.clone(), (req.symbol.clone(), req.qty));
        }

        Ok(PlaceOrderResponse {
            order_id: Some(order_id),
            failure_reason: OrderFailureReason::None,
        })
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderExecutionDetails> {
        let mut counts = self.query_counts.lock().await;
        let count = counts.entry(order_id.to_string()).or_insert(0);
        *count += 1;

        {
            let cancelled = self.cancelled_orders.lock().await;
            if cancelled.contains(order_id) {
                let metadata = self.order_metadata.lock().await;
                let (sym, qty) = metadata
                    .get(order_id)
                    .cloned()
                    .unwrap_or(("UNKNOWN".to_string(), 0.0));
                return Ok(OrderExecutionDetails {
                    order_id: order_id.to_string(),
                    symbol: sym,
                    status: OrderStatus::Cancelled,
                    qty_requested: qty,
                    qty_filled: 0.0,
                    avg_price: 0.0,
                    error_msg: None,
                    failure_reason: OrderFailureReason::None,
                });
            }
        }

        let (symbol, qty) = {
            let metadata = self.order_metadata.lock().await;
            metadata
                .get(order_id)
                .cloned()
                .unwrap_or(("UNKNOWN".to_string(), 0.0))
        };

        if symbol == "STAY_SUBMITTED" {
            return Ok(OrderExecutionDetails {
                order_id: order_id.to_string(),
                symbol: symbol.clone(),
                status: OrderStatus::Submitted,
                qty_requested: qty,
                qty_filled: 0.0,
                avg_price: 0.0,
                error_msg: None,
                failure_reason: OrderFailureReason::None,
            });
        }

        if *count < 2 {
            // 初回照会では Submitted を返す。
            Ok(OrderExecutionDetails {
                order_id: order_id.to_string(),
                symbol: "MOCK".to_string(),
                status: OrderStatus::Submitted,
                qty_requested: qty,
                qty_filled: 0.0,
                avg_price: 0.0,
                error_msg: None,
                failure_reason: OrderFailureReason::None,
            })
        } else {
            // 2 回目以降の照会では Filled を返す。
            Ok(OrderExecutionDetails {
                order_id: order_id.to_string(),
                symbol: "MOCK".to_string(),
                status: OrderStatus::Filled,
                qty_requested: qty,
                qty_filled: qty,
                avg_price: 150.0,
                error_msg: None,
                failure_reason: OrderFailureReason::None,
            })
        }
    }

    async fn get_broker_permissions(&self) -> Result<BrokerPermissions> {
        let mut market_rights = std::collections::HashMap::new();
        market_rights.insert("US".to_string(), MarketRight::Level1);
        market_rights.insert("HK".to_string(), MarketRight::Level1);
        market_rights.insert("CN".to_string(), MarketRight::Level1);

        Ok(BrokerPermissions {
            market_rights,
            sub_quota_total: 50,
            sub_quota_used: 0,
        })
    }

    async fn get_tradable_capacity(&self, symbol: &str, _price: f64) -> Result<TradableCapacity> {
        if symbol == "FAIL" {
            return Err(anyhow::anyhow!("Mock capacity query failure"));
        }
        let cap = self.mock_capacity.lock().await;
        Ok(cap.clone())
    }

    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        println!("🚫 [MockTrader] Cancelling order: {}", order_id);
        let mut cancelled = self.cancelled_orders.lock().await;
        cancelled.insert(order_id.to_string());
        Ok(())
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        Ok(vec![])
    }
}
