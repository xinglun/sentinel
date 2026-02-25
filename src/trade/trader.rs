use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
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
