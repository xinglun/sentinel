use anyhow::{anyhow, Result};
use prost::Message;
use std::sync::Arc;

use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::protocol::generated::trd_common::{
    OrderType as FutuOrderType, TrdHeader, TrdSide,
};
use crate::adapters::futu::protocol::generated::trd_get_funds;
use crate::adapters::futu::protocol::generated::trd_place_order;
use crate::adapters::futu::protocol::generated::trd_unlock_trade;
use crate::trade::trader::{
    AccountFunds, OrderSide, OrderType, PlaceOrderRequest, PlaceOrderResponse, TradeExecutor,
};

pub struct FutuTrader {
    client: Arc<FutuClient>,
    config: crate::config::FutuConfig,
}

impl FutuTrader {
    pub fn new(client: Arc<FutuClient>, config: crate::config::FutuConfig) -> Self {
        Self { client, config }
    }

    /// Helper function to build the common TrdHeader required by all trading APIs
    fn build_trd_header(&self) -> Result<TrdHeader> {
        let acc_id = self
            .config
            .acc_id
            .ok_or_else(|| anyhow!("Moomoo account ID (FUTU_ACC_ID) is not configured"))?;

        Ok(TrdHeader {
            trd_env: self.config.trd_env as i32,
            acc_id,
            trd_market: self.config.market as i32,
        })
    }
}

#[async_trait::async_trait]
impl TradeExecutor for FutuTrader {
    async fn unlock_trade(&self) -> Result<()> {
        let pwd_md5 = match &self.config.unlock_password_md5 {
            Some(pwd) if !pwd.is_empty() => pwd.clone(),
            _ => {
                println!("No FUTU_UNLOCK_PASSWORD_MD5 configured. Assuming read-only connection or already unlocked.");
                return Ok(());
            }
        };

        println!("🔑 Sending Trd_UnlockTrade request to OpenD...");

        // Proto ID for Trd_UnlockTrade is 3205
        let req = trd_unlock_trade::Request {
            c2s: trd_unlock_trade::C2s {
                unlock: true,
                pwd_md5: Some(pwd_md5),
                security_firm: None, // Used for specific brokerage firm if needed, usually defaults correctly
            },
        };

        let raw_res = self.client.send_request(3205, &req).await?;
        let res = trd_unlock_trade::Response::decode(&raw_res[..])?;

        if res.ret_type == 0 {
            println!("✅ Trd_UnlockTrade success. Trading functions are now authenticated.");
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to unlock trade: ret_type={}, msg={:?}, err_code={:?}",
                res.ret_type,
                res.ret_msg,
                res.err_code
            ))
        }
    }

    async fn get_funds(&self) -> Result<AccountFunds> {
        println!("💰 Fetching account funds via Trd_GetFunds...");

        let header = self.build_trd_header()?;

        let req = trd_get_funds::Request {
            c2s: trd_get_funds::C2s {
                header,
                refresh_cache: Some(false), // Fetch from OpenD cache for speed
                currency: None,             // Use account base currency
            },
        };

        let raw_res = self.client.send_request(3201, &req).await?;
        let res = trd_get_funds::Response::decode(&raw_res[..])?;

        if res.ret_type != 0 {
            return Err(anyhow!(
                "Failed to get funds: ret_type={}, msg={:?}",
                res.ret_type,
                res.ret_msg
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Missing payload in Trd_GetFunds response"))?;
        let funds = s2c
            .funds
            .ok_or_else(|| anyhow!("Missing funds details in payload"))?;

        Ok(AccountFunds {
            power: funds.power,
            total_assets: funds.total_assets,
            cash: funds.cash,
            market_val: funds.market_val,
            unrealized_pl: funds.unrealized_pl.unwrap_or(0.0),
        })
    }

    async fn place_order(&self, order: PlaceOrderRequest) -> Result<PlaceOrderResponse> {
        println!(
            "🚀 Executing PlaceOrder ({}): {:?} qty={} price={:?}",
            order.symbol, order.side, order.qty, order.price
        );

        let header = self.build_trd_header()?;

        // Map abstract OrderSide to Futu TrdSide
        let trd_side = match order.side {
            OrderSide::Buy => TrdSide::Buy as i32,
            OrderSide::Sell => TrdSide::Sell as i32,
        };

        // Map abstract OrderType to Futu OrderType
        let order_type = match order.order_type {
            OrderType::Normal => FutuOrderType::Normal as i32,
            OrderType::Market => FutuOrderType::Market as i32,
        };

        let req = trd_place_order::Request {
            c2s: trd_place_order::C2s {
                packet_id: crate::adapters::futu::protocol::generated::common::PacketId {
                    conn_id: self.client.conn_id(),
                    serial_no: self.client.next_serial(),
                },
                header,
                trd_side,
                order_type,
                code: order.symbol.clone(),
                qty: order.qty,
                price: order.price,
                adjust_price: Some(true), // Automatically shift to the nearest valid tick (crucial for HK/A shares)
                adjust_side_and_limit: Some(0.0), // 0 means just adjust to nearest, don't force up/down limits
                sec_market: None,
                remark: Some("API Sentinel".to_string()),
                time_in_force: None,           // Default is usually Day
                fill_outside_rth: Some(false), // Pre/Post market off by default for safety
                aux_price: None,
                trail_type: None,
                trail_value: None,
                trail_spread: None,
                session: None,
            },
        };

        let raw_res = self.client.send_request(3203, &req).await?;
        let res = trd_place_order::Response::decode(&raw_res[..])?;

        if res.ret_type != 0 {
            return Err(anyhow!(
                "Failed to place order: ret_type={}, msg={:?}",
                res.ret_type,
                res.ret_msg
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Missing payload in Trd_PlaceOrder response"))?;

        Ok(PlaceOrderResponse {
            order_id: s2c.order_id.unwrap_or(0).to_string(),
            status: "Submitted".to_string(), // Initial successful submission
        })
    }
}
