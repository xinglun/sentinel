use anyhow::{anyhow, Result};
use prost::Message;
use std::sync::Arc;

use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::protocol::generated::trd_common::{
    ModifyOrderOp, PositionSide as FutuPositionSide, TrdHeader,
};
use crate::adapters::futu::protocol::generated::trd_get_funds;
use crate::adapters::futu::protocol::generated::trd_get_max_trd_qtys;
use crate::adapters::futu::protocol::generated::trd_get_position_list;
use crate::adapters::futu::protocol::generated::trd_modify_order;
use crate::adapters::futu::protocol::generated::trd_place_order;
use crate::adapters::futu::protocol::generated::trd_unlock_trade;
use crate::features::trading::application::trade_executor::{
    AccountFunds, BrokerPermissions, MarketRight, OrderExecutionDetails, OrderFailureReason,
    OrderSide, OrderStatus, OrderType, PlaceOrderRequest, PlaceOrderResponse, Position,
    PositionSide, TradableCapacity, TradeExecutor,
};

pub struct FutuTrader {
    client: Arc<FutuClient>,
    config: crate::config::FutuConfig,
}

impl FutuTrader {
    #[allow(dead_code)]
    pub fn new(client: Arc<FutuClient>, config: crate::config::FutuConfig) -> Self {
        Self { client, config }
    }

    /// すべての trading API で必要な共通 TrdHeader を組み立てる。
    fn build_trd_header(&self) -> Result<TrdHeader> {
        build_trd_header_from_config(&self.config)
    }

    fn map_futu_error(ret_type: i32, err_code: i32, msg: &str) -> OrderFailureReason {
        if ret_type == 0 {
            return OrderFailureReason::None;
        }

        match err_code {
            1005 => OrderFailureReason::TradingPasswordRequired,
            11003 => OrderFailureReason::InsufficientFunds,
            11101 => OrderFailureReason::MarketClosed,
            1016 => OrderFailureReason::SecuritySuspended,
            1015 => OrderFailureReason::InvalidQuantity,
            _ => OrderFailureReason::Other(err_code, msg.to_string()),
        }
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

        // Trd_UnlockTrade の Proto ID は 3205。
        let req = trd_unlock_trade::Request {
            c2s: trd_unlock_trade::C2s {
                unlock: true,
                pwd_md5: Some(pwd_md5),
                security_firm: None, // 必要に応じて brokerage firm を指定する。通常は default でよい。
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

    async fn get_account_funds(&self) -> Result<AccountFunds> {
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
        let trd_side = match order.side {
            OrderSide::Buy => 1,
            OrderSide::Sell => 2,
        };

        let order_type = match order.order_type {
            OrderType::Normal => 2, // Limit order
            OrderType::Market => 1, // Market order
        };

        let header = self.build_trd_header()?;

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
            let err_code = res.err_code.unwrap_or(0);
            let msg = res.ret_msg.clone().unwrap_or_default();
            let reason = Self::map_futu_error(res.ret_type, err_code, &msg);

            println!(
                "⚠️ [Trader - REJECTED] ret_type={}, err_code={}, reason={:?}, msg={}",
                res.ret_type, err_code, reason, msg
            );

            return Ok(PlaceOrderResponse {
                order_id: None,
                failure_reason: reason,
            });
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Missing payload in Trd_PlaceOrder response"))?;

        Ok(PlaceOrderResponse {
            order_id: Some(s2c.order_id.unwrap_or(0).to_string()),
            failure_reason: OrderFailureReason::None,
        })
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderExecutionDetails> {
        println!("🔍 Querying order status for ID: {}...", order_id);

        let header = self.build_trd_header()?;
        let order_id_u64 = order_id
            .parse::<u64>()
            .map_err(|_| anyhow!("Invalid u64 order_id format"))?;

        // 3202 is Trd_GetOrderList
        let req = crate::adapters::futu::protocol::generated::trd_get_order_list::Request {
            c2s: crate::adapters::futu::protocol::generated::trd_get_order_list::C2s {
                header,
                filter_conditions: Some(
                    crate::adapters::futu::protocol::generated::trd_common::TrdFilterConditions {
                        id_list: vec![order_id_u64],
                        ..Default::default()
                    },
                ),
                ..Default::default()
            },
        };

        let raw_res = self.client.send_request(3202, &req).await?;
        let res = crate::adapters::futu::protocol::generated::trd_get_order_list::Response::decode(
            &raw_res[..],
        )?;

        if res.ret_type != 0 {
            return Err(anyhow!(
                "Failed to query order {}: {}",
                order_id,
                res.ret_msg.as_deref().unwrap_or("Unknown error")
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Missing payload in Trd_GetOrderList response"))?;
        let futu_order = s2c
            .order_list
            .first()
            .ok_or_else(|| anyhow!("Order {} not found in broker response", order_id))?;

        // status を map する。OrderStatus の値は trd_common.rs または docs を参照する。
        // 10: Filled_Part, 11: Filled_All, 12: Cancelled_All, etc.
        let status = match futu_order.order_status {
            10 => OrderStatus::PartiallyFilled,
            11 => OrderStatus::Filled,
            12..=15 => OrderStatus::Cancelled,
            21..=23 => OrderStatus::Rejected,
            _ => OrderStatus::Submitted,
        };

        Ok(OrderExecutionDetails {
            order_id: order_id.to_string(),
            symbol: futu_order.code.clone(),
            status,
            qty_requested: futu_order.qty,
            qty_filled: futu_order.fill_qty.unwrap_or(0.0),
            avg_price: futu_order.fill_avg_price.unwrap_or(0.0),
            error_msg: None, // Success path
            failure_reason: OrderFailureReason::None,
        })
    }

    async fn get_broker_permissions(&self) -> Result<BrokerPermissions> {
        // 1. Get Market Rights via GetUserInfo (1001)
        let info_req = crate::adapters::futu::protocol::generated::get_user_info::Request {
            c2s: crate::adapters::futu::protocol::generated::get_user_info::C2s {
                flag: Some(crate::adapters::futu::protocol::generated::get_user_info::UserInfoField::QotRight as i32),
            },
        };
        let raw_info = self.client.send_request(1001, &info_req).await?;
        let info_res: crate::adapters::futu::protocol::generated::get_user_info::Response =
            prost::Message::decode(&raw_info[..])?;

        let mut market_rights = std::collections::HashMap::new();
        if let Some(s2c) = info_res.s2c {
            let map_right = |r: Option<i32>| match r {
                Some(1) => MarketRight::BMP,
                Some(2) => MarketRight::Level1,
                Some(3) => MarketRight::Level2,
                Some(4) => MarketRight::SF,
                _ => MarketRight::None,
            };
            market_rights.insert("HK".to_string(), map_right(s2c.hk_qot_right));
            market_rights.insert("US".to_string(), map_right(s2c.us_qot_right));
            market_rights.insert("SH".to_string(), map_right(s2c.sh_qot_right));
            market_rights.insert("SZ".to_string(), map_right(s2c.sz_qot_right));
        }

        // 2. Get Sub Quota via Qot_GetSubInfo (3003)
        let sub_info_req = crate::adapters::futu::protocol::generated::qot_get_sub_info::Request {
            c2s: crate::adapters::futu::protocol::generated::qot_get_sub_info::C2s {
                is_req_all_conn: Some(true),
            },
        };
        let raw_sub = self.client.send_request(3003, &sub_info_req).await?;
        let sub_res: crate::adapters::futu::protocol::generated::qot_get_sub_info::Response =
            prost::Message::decode(&raw_sub[..])?;

        let (total, used) = if let Some(s2c) = sub_res.s2c {
            (
                s2c.remain_quota + s2c.total_used_quota,
                s2c.total_used_quota,
            )
        } else {
            (0, 0)
        };

        Ok(BrokerPermissions {
            market_rights,
            sub_quota_total: total,
            sub_quota_used: used,
        })
    }

    async fn get_tradable_capacity(&self, symbol: &str, price: f64) -> Result<TradableCapacity> {
        println!(
            "📊 Fetching tradable capacity for {} @ ${:.2}...",
            symbol, price
        );

        let header = self.build_trd_header()?;

        // Proto ID for Trd_GetMaxTrdQtys is 3207
        let req = trd_get_max_trd_qtys::Request {
            c2s: trd_get_max_trd_qtys::C2s {
                header,
                order_type: 2, // Limit order is safest for calculation
                code: symbol.to_string(),
                price,
                order_id: None,
                adjust_price: Some(true),
                adjust_side_and_limit: None,
                sec_market: None,
                order_id_ex: None,
            },
        };

        let raw_res = self.client.send_request(3207, &req).await?;
        let res = trd_get_max_trd_qtys::Response::decode(&raw_res[..])?;

        if res.ret_type != 0 {
            return Err(anyhow!(
                "Failed to get max trade quantities: ret_type={}, msg={:?}",
                res.ret_type,
                res.ret_msg
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Missing payload in Trd_GetMaxTrdQtys response"))?;
        let qtys = s2c
            .max_trd_qtys
            .ok_or_else(|| anyhow!("Missing max_trd_qtys in S2c"))?;

        Ok(TradableCapacity {
            max_buy: qtys.max_cash_buy,
            max_sell: qtys.max_position_sell,
        })
    }

    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        println!("🚫 Cancelling order {} via Futu OpenAPI...", order_id);

        let header = self.build_trd_header()?;
        let packet_id = crate::adapters::futu::protocol::generated::common::PacketId {
            conn_id: self.client.conn_id(),
            serial_no: self.client.next_serial(),
        };

        let (numeric_id, string_id) = match order_id.parse::<u64>() {
            Ok(id) => (id, None),
            Err(_) => (0, Some(order_id.to_string())),
        };

        let req = trd_modify_order::Request {
            c2s: trd_modify_order::C2s {
                packet_id,
                header,
                order_id: numeric_id,
                modify_order_op: ModifyOrderOp::Cancel as i32,
                for_all: Some(false),
                trd_market: None,
                qty: None,
                price: None,
                adjust_price: None,
                adjust_side_and_limit: None,
                aux_price: None,
                trail_type: None,
                trail_value: None,
                trail_spread: None,
                order_id_ex: string_id,
            },
        };

        let raw_res = self.client.send_request(2205, &req).await?;
        let res = trd_modify_order::Response::decode(&raw_res[..])?;

        if res.ret_type != 0 {
            return Err(anyhow::anyhow!(
                "Failed to cancel order {}: ret_type={}, msg={:?}",
                order_id,
                res.ret_type,
                res.ret_msg
            ));
        }

        println!("✅ Order {} cancellation requested successfully.", order_id);
        Ok(())
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        println!("📋 Fetching account positions via Trd_GetPositionList...");

        let header = self.build_trd_header()?;

        // Proto ID for Trd_GetPositionList is 3208
        let req = trd_get_position_list::Request {
            c2s: trd_get_position_list::C2s {
                header,
                filter_conditions: None,
                filter_pl_ratio_min: None,
                filter_pl_ratio_max: None,
                refresh_cache: Some(true),
            },
        };

        let raw_res = self.client.send_request(3208, &req).await?;
        let res = trd_get_position_list::Response::decode(&raw_res[..])?;

        if res.ret_type != 0 {
            return Err(anyhow!(
                "Failed to get position list: {:?}",
                res.ret_msg.unwrap_or_default()
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Empty s2c in Trd_GetPositionList response"))?;

        let positions = s2c
            .position_list
            .into_iter()
            .map(|f| Position {
                symbol: f.code,
                side: match FutuPositionSide::try_from(f.position_side)
                    .unwrap_or(FutuPositionSide::Unknown)
                {
                    FutuPositionSide::Long => PositionSide::Long,
                    FutuPositionSide::Short => PositionSide::Short,
                    _ => PositionSide::Unknown,
                },
                qty: f.qty,
                can_sell_qty: f.can_sell_qty,
                cost_price: f.cost_price.unwrap_or(0.0),
                market_val: f.val,
                pl_val: f.pl_val,
                pl_ratio: f.pl_ratio.unwrap_or(0.0),
            })
            .collect();

        Ok(positions)
    }
}

fn build_trd_header_from_config(config: &crate::config::FutuConfig) -> Result<TrdHeader> {
    let acc_id = config
        .acc_id
        .ok_or_else(|| anyhow!("Moomoo account ID (FUTU_ACC_ID) is not configured"))?;

    Ok(TrdHeader {
        trd_env: config.trd_env as i32,
        acc_id,
        trd_market: config.market as i32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FutuConfig;

    #[test]
    fn build_trd_header_from_config_uses_configured_account_metadata() {
        let config = FutuConfig {
            opend_ip: "127.0.0.1".to_string(),
            opend_port: 11111,
            trd_env: 1,
            market: 2,
            acc_id: Some(987654321),
            unlock_password_md5: None,
        };

        let header = build_trd_header_from_config(&config).unwrap();

        assert_eq!(header.trd_env, 1);
        assert_eq!(header.acc_id, 987654321);
        assert_eq!(header.trd_market, 2);
    }

    #[test]
    fn build_trd_header_from_config_rejects_missing_account_id() {
        let config = FutuConfig {
            opend_ip: "127.0.0.1".to_string(),
            opend_port: 11111,
            trd_env: 0,
            market: 1,
            acc_id: None,
            unlock_password_md5: None,
        };

        let err = build_trd_header_from_config(&config).unwrap_err();

        assert!(
            err.to_string().contains("not configured"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn map_futu_error_classifies_known_error_codes() {
        assert_eq!(
            FutuTrader::map_futu_error(0, 0, ""),
            OrderFailureReason::None
        );
        assert_eq!(
            FutuTrader::map_futu_error(1, 1005, "pwd"),
            OrderFailureReason::TradingPasswordRequired
        );
        assert_eq!(
            FutuTrader::map_futu_error(1, 11003, "funds"),
            OrderFailureReason::InsufficientFunds
        );
        assert_eq!(
            FutuTrader::map_futu_error(1, 11101, "closed"),
            OrderFailureReason::MarketClosed
        );
        assert_eq!(
            FutuTrader::map_futu_error(1, 1016, "suspended"),
            OrderFailureReason::SecuritySuspended
        );
        assert_eq!(
            FutuTrader::map_futu_error(1, 1015, "qty"),
            OrderFailureReason::InvalidQuantity
        );
        assert_eq!(
            FutuTrader::map_futu_error(1, 9999, "other"),
            OrderFailureReason::Other(9999, "other".to_string())
        );
    }
}
