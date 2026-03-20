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
    pub order_id: String,
    pub status: String,
}

#[async_trait]
pub trait TradeExecutor: Send + Sync {
    /// 1. 解锁交易 (针对Moomoo此类需要密码解锁本地实例的网关)
    /// 如果返回 Ok(()) 则证明已解锁或不需要解锁。
    async fn unlock_trade(&self) -> Result<()>;

    /// 2. 获取资金/购买力状况
    async fn get_funds(&self) -> Result<AccountFunds>;

    /// 3. 下单
    async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResponse>;
}

pub struct MockTradeExecutor {
    pub placed_orders_count: std::sync::atomic::AtomicUsize,
}

impl MockTradeExecutor {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for MockTradeExecutor {
    fn default() -> Self {
        Self {
            placed_orders_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl TradeExecutor for MockTradeExecutor {
    async fn unlock_trade(&self) -> Result<()> {
        Ok(())
    }
    async fn get_funds(&self) -> Result<AccountFunds> {
        Ok(AccountFunds {
            power: 100000.0,
            total_assets: 100000.0,
            cash: 100000.0,
            market_val: 0.0,
            unrealized_pl: 0.0,
        })
    }
    async fn place_order(&self, _req: PlaceOrderRequest) -> Result<PlaceOrderResponse> {
        self.placed_orders_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(PlaceOrderResponse {
            order_id: format!(
                "MOCK-{}",
                self.placed_orders_count
                    .load(std::sync::atomic::Ordering::SeqCst)
            ),
            status: "FILLED".to_string(),
        })
    }
}
