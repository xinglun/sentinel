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
                println!("{}", unlock_password_missing_notice());
                return Ok(());
            }
        };

        println!("🔑 OpenD に Trd_UnlockTrade を送信します...");

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
            println!("✅ Trd_UnlockTrade 成功。取引機能の認証が完了しました。");
            Ok(())
        } else {
            Err(anyhow!(
                "取引ロック解除に失敗しました: ret_type={}, msg={:?}, err_code={:?}",
                res.ret_type,
                res.ret_msg,
                res.err_code
            ))
        }
    }

    async fn get_account_funds(&self) -> Result<AccountFunds> {
        println!("💰 Trd_GetFunds で口座資金を照会します...");

        let header = self.build_trd_header()?;

        let req = trd_get_funds::Request {
            c2s: trd_get_funds::C2s {
                header,
                refresh_cache: Some(false), // OpenD キャッシュを利用して照会する。
                currency: None,             // 口座の基準通貨を使用する。
            },
        };

        let raw_res = self.client.send_request(3201, &req).await?;
        let res = trd_get_funds::Response::decode(&raw_res[..])?;

        if res.ret_type != 0 {
            return Err(anyhow!(
                "口座資金の照会に失敗しました: ret_type={}, msg={:?}",
                res.ret_type,
                res.ret_msg
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Trd_GetFunds 応答の payload がありません"))?;
        let funds = s2c
            .funds
            .ok_or_else(|| anyhow!("payload に funds 詳細がありません"))?;

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
            OrderType::Normal => 2, // 指値注文
            OrderType::Market => 1, // 成行注文
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
                adjust_price: Some(true), // 最寄りの有効ティックへ自動補正する。
                adjust_side_and_limit: Some(0.0), // 0 は最寄り補正のみで上下限を強制しない。
                sec_market: None,
                remark: Some("API Sentinel".to_string()),
                time_in_force: None,           // 通常は Day を既定とする。
                fill_outside_rth: Some(false), // 安全のためプレ/ポスト市場を無効化する。
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
            order_id: Some(required_order_id(s2c.order_id)?),
            failure_reason: OrderFailureReason::None,
        })
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderExecutionDetails> {
        println!("🔍 注文 ID {} の状態を照会します...", order_id);

        let header = self.build_trd_header()?;
        let order_id_u64 = order_id
            .parse::<u64>()
            .map_err(|_| anyhow!("Invalid u64 order_id format"))?;

        // Trd_GetOrderList の Proto ID は 3202。
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
                "注文 {} の照会に失敗しました: {}",
                order_id,
                res.ret_msg.as_deref().unwrap_or("不明なエラー")
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Trd_GetOrderList 応答の payload がありません"))?;
        let futu_order = s2c
            .order_list
            .first()
            .ok_or_else(|| anyhow!("ブローカー応答に注文 {} が見つかりません", order_id))?;

        // OrderStatus を変換する。値の詳細は trd_common.rs または docs を参照する。
        // 10: Filled_Part, 11: Filled_All, 12: Cancelled_All など。
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
            error_msg: None, // 成功経路
            failure_reason: OrderFailureReason::None,
        })
    }

    async fn get_broker_permissions(&self) -> Result<BrokerPermissions> {
        // 1. GetUserInfo (1001) で market right を取得する。
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

        // 2. Qot_GetSubInfo (3003) で sub quota を取得する。
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
            "📊 {} の取引可能数量を @ ${:.2} で照会します...",
            symbol, price
        );

        let header = self.build_trd_header()?;

        // Trd_GetMaxTrdQtys の Proto ID は 3207。
        let req = trd_get_max_trd_qtys::Request {
            c2s: trd_get_max_trd_qtys::C2s {
                header,
                order_type: 2, // 計算は指値注文を前提にする。
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
                "最大取引数量の照会に失敗しました: ret_type={}, msg={:?}",
                res.ret_type,
                res.ret_msg
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Trd_GetMaxTrdQtys 応答の payload がありません"))?;
        let qtys = s2c
            .max_trd_qtys
            .ok_or_else(|| anyhow!("S2c に max_trd_qtys がありません"))?;

        Ok(TradableCapacity {
            max_buy: qtys.max_cash_buy,
            max_sell: qtys.max_position_sell,
        })
    }

    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        println!("🚫 Futu OpenAPI で注文 {} をキャンセルします...", order_id);

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
                "注文 {} のキャンセルに失敗しました: ret_type={}, msg={:?}",
                order_id,
                res.ret_type,
                res.ret_msg
            ));
        }

        println!("✅ 注文 {} のキャンセルを依頼しました。", order_id);
        Ok(())
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        println!("📋 Trd_GetPositionList で口座ポジションを照会します...");

        let header = self.build_trd_header()?;

        // Trd_GetPositionList の Proto ID は 3208。
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
                "ポジション一覧の照会に失敗しました: {:?}",
                res.ret_msg.unwrap_or_default()
            ));
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow!("Trd_GetPositionList 応答の s2c が空です"))?;

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
        .ok_or_else(|| anyhow!("Moomoo account ID (FUTU_ACC_ID) が設定されていません"))?;

    Ok(TrdHeader {
        trd_env: config.trd_env as i32,
        acc_id,
        trd_market: config.market as i32,
    })
}

fn required_order_id(order_id: Option<u64>) -> Result<String> {
    let order_id = order_id
        .filter(|order_id| *order_id > 0)
        .ok_or_else(|| anyhow!("Futu の成功応答に有効な order_id がありません"))?;
    Ok(order_id.to_string())
}

fn unlock_password_missing_notice() -> &'static str {
    "ℹ️  FUTU_UNLOCK_PASSWORD_MD5 が未設定です。読み取り専用接続または既に解除済みとして扱います。"
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
            err.to_string().contains("設定されていません"),
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

    #[test]
    fn unlock_password_missing_notice_is_localized() {
        assert!(unlock_password_missing_notice().contains("未設定"));
    }

    #[test]
    fn required_order_id_rejects_missing_or_zero_id() {
        assert!(required_order_id(None).is_err());
        assert!(required_order_id(Some(0)).is_err());
    }

    #[test]
    fn required_order_id_preserves_positive_id() {
        assert_eq!(required_order_id(Some(123)).unwrap(), "123");
    }
}
