use crate::features::shared::acl::ledger_factory::{LedgerAdapter, TradeRecordAdapter};
use crate::features::trading::application::execution_signal::{GatedTrade, TradeSide};
use crate::features::trading::application::trade_executor::{
    OrderSide, OrderType, PlaceOrderRequest, TradeExecutor,
};
use crate::features::trading::domain::reconciliation::{PositionMismatch, ReconciliationReport};
use anyhow::{anyhow, Result};
use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TraderAgent {
    executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>,
    ledger: Arc<LedgerAdapter>,
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
    pub failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason,
}

#[derive(Debug)]
pub struct ExecutionSummary {
    pub audits: Vec<TradeExecutionAudit>,
    pub status: Result<()>,
}

impl TraderAgent {
    pub fn new(
        executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>>,
        ledger: Arc<LedgerAdapter>,
    ) -> Self {
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

    pub async fn execute_signals(&self, gated_trades: Vec<GatedTrade>) -> Result<ExecutionSummary> {
        let mut audits = Vec::new();
        if gated_trades.is_empty() {
            println!("{}", no_trades_notice());
            return Ok(ExecutionSummary {
                audits,
                status: Ok(()),
            });
        }

        let mut errors = Vec::new();
        let mut first = true;

        for trade in gated_trades {
            if (!trade.is_trim && (!trade.qty.is_finite() || trade.qty <= 0.0))
                || !trade.price.is_finite()
                || trade.price <= 0.0
            {
                let detail = format!(
                    "{}: invalid order input qty={}, price={}",
                    trade.symbol, trade.qty, trade.price
                );
                audits.push(TradeExecutionAudit {
                    symbol: trade.symbol.clone(),
                    side: format!("{:?}", trade.side),
                    qty_requested: trade.qty,
                    qty_filled: 0.0,
                    price: trade.price,
                    success: false,
                    order_id: None,
                    status: "OrderInputInvalid".to_string(),
                    error: Some(detail.clone()),
                    failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::InvalidQuantity,
                });
                errors.push(detail);
                continue;
            }

            // 2. レート制御: Moomoo の制限（15/30 秒）に合わせて注文間隔を 1 秒空ける。
            if !first {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            first = false;

            // 同日・同 symbol・同 side の重複取引を避けるため、既に処理済みか確認する。
            let side_str_upper = format!("{:?}", trade.side).to_uppercase();
            if self.ledger.has_acted_today(&trade.symbol, &side_str_upper) {
                continue;
            }

            println!(
                "🛰️  トレーダー: {} を {} 単位 @ ${:.2} で送信します。理由: {}",
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

                    if !max_available.is_finite() || max_available < 0.0 {
                        let detail = format!(
                            "{}: invalid broker capacity {}",
                            trade.symbol, max_available
                        );
                        audits.push(TradeExecutionAudit {
                            symbol: trade.symbol.clone(),
                            side: format!("{:?}", trade.side),
                            qty_requested: trade.qty,
                            qty_filled: 0.0,
                            price: trade.price,
                            success: false,
                            order_id: None,
                            status: "CapacityInvalid".to_string(),
                            error: Some(detail.clone()),
                            failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::Other(
                                998,
                                "ブローカー容量が不正です".to_string(),
                            ),
                        });
                        errors.push(detail);
                        continue;
                    }

                    if trade.is_liquidation {
                        println!(
                            "🔥 [Trader - EXIT] 清算要求のため、数量を最大値 {} に設定します。",
                            max_available
                        );
                        final_qty = max_available;
                    } else if trade.is_trim {
                        println!(
                            "✂️ [Trader - TRIM] 50% 削減要求のため、数量を {} に設定します。",
                            max_available * 0.5
                        );
                        final_qty = max_available * 0.5;
                    } else if final_qty > max_available {
                        println!(
                            "⚠️ [Trader - CAPPING] {} の要求 {} はブローカー容量 {} を超えるため、最大値へ切り詰めます。",
                            trade.symbol, final_qty, max_available
                        );
                        final_qty = max_available;
                    }
                }
                Err(e) => {
                    println!(
                        "❌ [Trader - ERROR] {} の容量照会に失敗しました: {}。取引を中止します。",
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
                        failure_reason:
                            crate::features::trading::application::trade_executor::OrderFailureReason::Other(
                                999,
                                "容量照会に失敗しました".to_string(),
                            ),
                    };
                    audits.push(audit);
                    errors.push(format!("容量照会に失敗しました: {}", trade.symbol));
                    continue;
                }
            }

            if final_qty <= 0.0 {
                println!(
                    "🚫 [Trader - SKIPPED] {} の取引可能数量は 0 です。注文をスキップします。",
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
                failure_reason:
                    crate::features::trading::application::trade_executor::OrderFailureReason::None,
            };

            // 3. submit 時のロック範囲を狭める。
            let submission_result = {
                let exec = self.executor.lock().await;
                exec.place_order(req).await
            };

            match submission_result {
                Ok(res) => {
                    if let Some(order_id) = res.order_id {
                        println!(
                            "🚀 [Trader - SUBMITTED] {} の注文が発注されました。注文ID: {}。約定待ちです。",
                            trade.symbol, order_id
                        );
                        audit.order_id = Some(order_id.clone());

                        // --- 注文状態のポーリング（ライフサイクル完了） ---
                        let mut final_details = None;
                        for attempt in 1..=self.max_poll_attempts {
                            // 各状態確認のロック範囲を狭める。
                            let status_query = {
                                let exec = self.executor.lock().await;
                                exec.get_order_status(&order_id).await
                            };

                            match status_query {
                                Ok(details) => match details.status {
                                    crate::features::trading::application::trade_executor::OrderStatus::Filled
                                    | crate::features::trading::application::trade_executor::OrderStatus::PartiallyFilled
                                    | crate::features::trading::application::trade_executor::OrderStatus::Cancelled
                                    | crate::features::trading::application::trade_executor::OrderStatus::Rejected
                                    | crate::features::trading::application::trade_executor::OrderStatus::Failed => {
                                        final_details = Some(details);
                                        break;
                                    }
                                    _ => {
                                        if attempt % 5 == 0 {
                                            println!(
                                                "⏳ [Trader - WAITING] 注文 {} は {} 秒経過時点でまだ {:?} です。",
                                                order_id, attempt * 2, details.status
                                            );
                                        }
                                    }
                                },
                                Err(e) => {
                                    println!(
                                        "⚠️ [Trader - ERROR] {} の状態照会に失敗しました: {}",
                                        order_id, e
                                    );
                                }
                            }
                            // ここでロックを解放し、sleep 中も他の注文を進められるようにする。
                            tokio::time::sleep(self.poll_interval).await;
                        }

                        if let Some(details) = final_details {
                            println!(
                                "🎯 [Trader - FINAL] 注文 {} の最終状態: {:?} (約定: {} @ {:.2})",
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

                            let fill_status = matches!(
                                details.status,
                                crate::features::trading::application::trade_executor::OrderStatus::Filled
                                    | crate::features::trading::application::trade_executor::OrderStatus::PartiallyFilled
                            );
                            let valid_fill = details.qty_filled.is_finite()
                                && details.qty_filled > 0.0
                                && details.avg_price.is_finite()
                                && details.avg_price > 0.0;
                            if fill_status && !valid_fill {
                                audit.success = false;
                                audit.qty_filled = 0.0;
                                audit.price = trade.price;
                                audit.status = "FillDataInvalid".to_string();
                                audit.error = Some(
                                    "broker fill quantity or average price is invalid".to_string(),
                                );
                                audit.failure_reason = crate::features::trading::application::trade_executor::OrderFailureReason::Other(
                                    997,
                                    "約定数量または平均価格が不正です".to_string(),
                                );
                                errors.push(format!(
                                    "{}: invalid fill data qty={}, avg_price={}",
                                    trade.symbol, details.qty_filled, details.avg_price
                                ));
                            }

                            if matches!(
                                details.status,
                                crate::features::trading::application::trade_executor::OrderStatus::Rejected
                                    | crate::features::trading::application::trade_executor::OrderStatus::Failed
                            ) {
                                errors.push(format!(
                                    "{}: broker terminal status {:?}",
                                    trade.symbol, details.status
                                ));
                            }

                            if audit.qty_filled > 0.0 {
                                let side_str = if trade.side == TradeSide::Buy {
                                    "BUY"
                                } else {
                                    "SELL"
                                };
                                let ledger_result = self.ledger.record_trade(TradeRecordAdapter {
                                    date: Local::now().date_naive(),
                                    timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                                    symbol: trade.symbol.clone(),
                                    side: side_str.to_string(),
                                    qty: details.qty_filled,
                                    price: audit.price,
                                    signal: format!("{:?}", trade.side),
                                });
                                if let Err(error) = ledger_result {
                                    audit.success = false;
                                    audit.status = "LedgerWriteFailed".to_string();
                                    audit.error = Some(format!("账本写入失败: {}", error));
                                    errors.push(format!(
                                        "{}: ledger write failed: {}",
                                        trade.symbol, error
                                    ));
                                }
                            }
                        } else {
                            println!(
                                "🛑 [Trader - TIMEOUT] 注文 {} の状態照会が {} 秒でタイムアウトしました。",
                                order_id,
                                self.poll_interval.as_secs() * self.max_poll_attempts as u64
                            );

                            // --- P2-2: タイムアウト時の自動キャンセル処理 ---
                            println!(
                                "📡 [Trader - CANCEL] タイムアウトした注文 {} をキャンセルします...",
                                order_id
                            );
                            let cancel_res = {
                                let exec = self.executor.lock().await;
                                exec.cancel_order(&order_id).await
                            };

                            match cancel_res {
                                Ok(_) => {
                                    println!(
                                        "✅ [Trader - CANCEL] 注文 {} のキャンセルを依頼しました。",
                                        order_id
                                    );

                                    // --- P2-2: 最終確認 ---
                                    let final_check = {
                                        let exec = self.executor.lock().await;
                                        exec.get_order_status(&order_id).await
                                    };

                                    match final_check {
                                        Ok(details)
                                            if details.status
                                                == crate::features::trading::application::trade_executor::OrderStatus::Cancelled =>
                                        {
                                            println!(
                                                "🏁 [Trader - CONFIRMED] 注文 {} はブローカー側で CANCELLED 確定です。",
                                                order_id
                                            );
                                            audit.status = "TimedOutCancelledConfirmed".to_string();
                                            audit.qty_filled = details.qty_filled;
                                            audit.price = if details.avg_price > 0.0 {
                                                details.avg_price
                                            } else {
                                                trade.price
                                            };
                                            if details.qty_filled > 0.0 || details.avg_price > 0.0 {
                                                let valid_partial_fill = details.qty_filled.is_finite()
                                                    && details.qty_filled > 0.0
                                                    && details.avg_price.is_finite()
                                                    && details.avg_price > 0.0;
                                                if !valid_partial_fill {
                                                    audit.status = "FillDataInvalid".to_string();
                                                    audit.error = Some(
                                                        "broker partial fill data is invalid after cancellation"
                                                            .to_string(),
                                                    );
                                                    errors.push(format!(
                                                        "{}: invalid partial fill after cancellation qty={}, avg_price={}",
                                                        trade.symbol, details.qty_filled, details.avg_price
                                                    ));
                                                } else {
                                                    let side_str = if trade.side == TradeSide::Buy {
                                                        "BUY"
                                                    } else {
                                                        "SELL"
                                                    };
                                                    if let Err(error) = self.ledger.record_trade(
                                                        TradeRecordAdapter {
                                                            date: Local::now().date_naive(),
                                                            timestamp: Local::now()
                                                                .format("%Y-%m-%d %H:%M:%S")
                                                                .to_string(),
                                                            symbol: trade.symbol.clone(),
                                                            side: side_str.to_string(),
                                                            qty: details.qty_filled,
                                                            price: details.avg_price,
                                                            signal: format!("{:?}", trade.side),
                                                        },
                                                    ) {
                                                        audit.status = "LedgerWriteFailed".to_string();
                                                        audit.error = Some(format!(
                                                            "账本写入失败: {}",
                                                            error
                                                        ));
                                                        errors.push(format!(
                                                            "{}: ledger write failed after cancellation: {}",
                                                            trade.symbol, error
                                                        ));
                                                    } else {
                                                        errors.push(format!(
                                                            "{}: order cancelled after partial fill ({})",
                                                            trade.symbol, details.qty_filled
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                        _ => {
                                            println!(
                                                "❓ [Trader - UNKNOWN] 注文 {} のキャンセルは依頼済みですが、まだ終端状態が確認できません。",
                                                order_id
                                            );
                                            audit.status = "TimedOutCancelRequested".to_string();
                                            errors.push(format!(
                                                "{}: timeout cancellation status is unconfirmed",
                                                trade.symbol
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!(
                                        "❌ [Trader - CANCEL] 注文 {} のキャンセルに失敗しました: {}",
                                        order_id, e
                                    );
                                    audit.status = "TimedOutCancellationFailed".to_string();
                                    audit.error = Some(format!("キャンセルに失敗しました: {}", e));
                                    errors.push(format!(
                                        "{}: timeout cancellation failed: {}",
                                        trade.symbol, e
                                    ));
                                }
                            }
                        }
                    } else {
                        // 即時リジェクト。
                        println!(
                            "❌ [Trader - REJECTED] ブローカーが {} の注文を拒否しました: {:?}",
                            trade.symbol, res.failure_reason
                        );
                        audit.status = "Rejected".to_string();
                        audit.failure_reason = res.failure_reason.clone();
                        errors.push(format!(
                            "{}: broker rejected order: {:?}",
                            trade.symbol, res.failure_reason
                        ));
                    }
                }
                Err(e) => {
                    println!(
                        "❌ [Trader - FAILED] {} の注文送信に失敗しました: {}",
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
                "取引実行は一部失敗しました: {}",
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

    /// ポジションの reconciliation を実行する（P2-3）。
    /// local Ledger 由来の理論ポジションと broker 側の実ポジションを比較する。
    pub async fn reconcile_positions(&self) -> Result<ReconciliationReport> {
        println!("🔍 ポジション照合を開始します...");

        // 1. local position を取得する。
        let (_, local_positions) = self
            .ledger
            .get_portfolio_stats_checked()
            .map_err(|error| anyhow!("local ledger is invalid: {error}"))?;

        // 2. broker position を取得する。
        let broker_positions = {
            let exec = self.executor.lock().await;
            exec.get_positions().await?
        };

        // 数量が不正な状態で差分比較すると、NaN は比較をすり抜けて一致扱いになり、
        // 無限大や負数は照合結果を壊すため、正常な照合として返さない。
        for (symbol, (local_qty, _)) in &local_positions {
            if !local_qty.is_finite() || *local_qty < 0.0 {
                return Err(anyhow!(
                    "local ledger position quantity is invalid: symbol={}, qty={}",
                    symbol,
                    local_qty
                ));
            }
        }
        for position in &broker_positions {
            if position.symbol.trim().is_empty() {
                return Err(anyhow!("broker position symbol is empty"));
            }
            if matches!(
                position.side,
                crate::features::trading::application::trade_executor::PositionSide::Unknown
            ) {
                return Err(anyhow!(
                    "broker position side is unknown: symbol={}",
                    position.symbol
                ));
            }
            if !position.qty.is_finite() || position.qty < 0.0 {
                return Err(anyhow!(
                    "broker position quantity is invalid: symbol={}, qty={}",
                    position.symbol,
                    position.qty
                ));
            }
        }

        let mut mismatches = Vec::new();
        let mut matching_count = 0;

        // 3. 比較ロジック
        let mut broker_map: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        for position in broker_positions {
            let total = broker_map.entry(position.symbol.clone()).or_insert(0.0);
            *total += position.qty;
            if !total.is_finite() {
                return Err(anyhow!(
                    "broker position quantity overflowed: symbol={}, qty={}",
                    position.symbol,
                    total
                ));
            }
        }

        // local と broker を比較する。
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

        // local ledger に存在しない broker position を確認する。
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
                "✅ 照合成功: {} 件のポジションがすべて一致しました。",
                report.matching_count
            );
        } else {
            println!(
                "⚠️ 照合で {} 件の不一致が見つかりました。",
                report.mismatches.len()
            );
            for m in &report.mismatches {
                println!(
                    "   - {}: ローカル={} ブローカー={} 差分={}",
                    m.symbol, m.local_qty, m.broker_qty, m.diff
                );
            }
        }

        Ok(report)
    }
}

fn no_trades_notice() -> &'static str {
    "ℹ️  トレーダー: フィルタ後の取引対象がないため、実行しません。"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::action_matrix::{AssetAction, AssetActionDecision};
    use crate::features::radar::domain::asset_state::{AssetState, AssetStateSnapshot};
    use crate::features::radar::domain::market_regime::{
        LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
    };
    use crate::features::radar::domain::portfolio_policy::{PortfolioPolicy, RiskAssetsMode};

    use crate::config::AppConfig;
    use crate::features::radar::domain::features::MarketFeatures;
    use crate::features::trading::application::trade_executor::MockTradeExecutor;
    use crate::features::trading::application::trade_executor::{Position, PositionSide};
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
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );

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
        policy.risk_assets_mode = RiskAssetsMode::NEUTRAL; // テスト安定性のため NEUTRAL に固定する。

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
            position_intent: crate::features::radar::domain::exit::PositionIntent::ADD,
            ..Default::default()
        }];

        use crate::features::radar::application::execution_gate::{ExecutionGate, TradingLimits};
        use crate::features::radar::domain::decision::DecisionPacket;

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
            crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot::default(),
            None,
            None,
        );

        let trading_config = config_arc.trading.as_ref().unwrap();
        let trading_limits = TradingLimits {
            enabled: trading_config.enabled,
            global_budget: trading_config.global_budget,
            max_daily_budget: trading_config.max_daily_budget,
        };
        let execution_result =
            ExecutionGate::gate_packet(&packet, &trading_limits, 0.0, 100000.0, 0.0);

        let summary = agent
            .execute_signals(execution_result.trades)
            .await
            .expect("Preflight should not fail");
        summary.status.expect("Execution should not fail");

        // audit details を確認する。
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
    async fn test_trader_agent_empty_signal_list_returns_success_without_orders() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );

        let agent = TraderAgent::new(mock_exec.clone(), ledger);
        let summary = agent.execute_signals(vec![]).await.unwrap();

        assert!(summary.audits.is_empty());
        summary.status.expect("empty signal list should succeed");

        let count = mock_exec
            .lock()
            .await
            .placed_orders_count
            .load(Ordering::SeqCst);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_trader_agent_rejected_order_returns_error_status() {
        struct RejectExecutor;

        #[async_trait::async_trait]
        impl TradeExecutor for RejectExecutor {
            async fn unlock_trade(&self) -> Result<()> {
                Ok(())
            }
            async fn get_account_funds(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::AccountFunds>
            {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: PlaceOrderRequest,
            ) -> Result<crate::features::trading::application::trade_executor::PlaceOrderResponse>
            {
                Ok(crate::features::trading::application::trade_executor::PlaceOrderResponse {
                    order_id: None,
                    failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::InsufficientFunds,
                })
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::features::trading::application::trade_executor::OrderExecutionDetails>
            {
                unreachable!()
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::BrokerPermissions>
            {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::features::trading::application::trade_executor::TradableCapacity>
            {
                Ok(
                    crate::features::trading::application::trade_executor::TradableCapacity {
                        max_buy: 10.0,
                        max_sell: 10.0,
                    },
                )
            }
            async fn cancel_order(&self, _: &str) -> Result<()> {
                unreachable!()
            }
            async fn get_positions(
                &self,
            ) -> Result<Vec<crate::features::trading::application::trade_executor::Position>>
            {
                unreachable!()
            }
        }

        let temp = tempdir().unwrap();
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                temp.path().to_path_buf(),
            ),
        );
        let agent = TraderAgent::new(Arc::new(Mutex::new(RejectExecutor)), ledger);
        let summary = agent
            .execute_signals(vec![
                crate::features::radar::application::execution_gate::GatedTrade {
                    symbol: "REJECT".to_string(),
                    side: TradeSide::Buy,
                    qty: 1.0,
                    price: 100.0,
                    reason: "rejection regression".to_string(),
                    is_liquidation: false,
                    is_trim: false,
                },
            ])
            .await
            .unwrap();

        assert!(summary.status.is_err());
        assert_eq!(summary.audits[0].status, "Rejected");
        assert_eq!(
            summary.audits[0].failure_reason,
            crate::features::trading::application::trade_executor::OrderFailureReason::InsufficientFunds
        );
    }

    #[tokio::test]
    async fn test_trader_agent_terminal_rejection_and_failure_return_error_status() {
        struct TerminalExecutor {
            status: crate::features::trading::application::trade_executor::OrderStatus,
        }

        #[async_trait::async_trait]
        impl TradeExecutor for TerminalExecutor {
            async fn unlock_trade(&self) -> Result<()> {
                Ok(())
            }
            async fn get_account_funds(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::AccountFunds>
            {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: PlaceOrderRequest,
            ) -> Result<crate::features::trading::application::trade_executor::PlaceOrderResponse>
            {
                Ok(crate::features::trading::application::trade_executor::PlaceOrderResponse { order_id: Some("terminal-1".to_string()), failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::None })
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::features::trading::application::trade_executor::OrderExecutionDetails>
            {
                Ok(crate::features::trading::application::trade_executor::OrderExecutionDetails {
                    order_id: "terminal-1".to_string(), symbol: "TERMINAL".to_string(), status: self.status.clone(), qty_requested: 1.0, qty_filled: 0.0, avg_price: 0.0, error_msg: None, failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::Other(9001, "terminal".to_string())
                })
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::BrokerPermissions>
            {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::features::trading::application::trade_executor::TradableCapacity>
            {
                Ok(
                    crate::features::trading::application::trade_executor::TradableCapacity {
                        max_buy: 1.0,
                        max_sell: 1.0,
                    },
                )
            }
            async fn cancel_order(&self, _: &str) -> Result<()> {
                unreachable!()
            }
            async fn get_positions(
                &self,
            ) -> Result<Vec<crate::features::trading::application::trade_executor::Position>>
            {
                unreachable!()
            }
        }

        for status in [
            crate::features::trading::application::trade_executor::OrderStatus::Rejected,
            crate::features::trading::application::trade_executor::OrderStatus::Failed,
        ] {
            let temp = tempdir().unwrap();
            let ledger = Arc::new(
                crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                    temp.path().to_path_buf(),
                ),
            );
            let agent = TraderAgent::new(
                Arc::new(Mutex::new(TerminalExecutor {
                    status: status.clone(),
                })),
                ledger,
            );
            let summary = agent
                .execute_signals(vec![
                    crate::features::radar::application::execution_gate::GatedTrade {
                        symbol: format!("{:?}_TERMINAL", status),
                        side: TradeSide::Buy,
                        qty: 1.0,
                        price: 100.0,
                        reason: "terminal regression".to_string(),
                        is_liquidation: false,
                        is_trim: false,
                    },
                ])
                .await
                .unwrap();
            assert!(summary.status.is_err());
            assert_eq!(summary.audits[0].status, format!("{:?}", status));
        }
    }

    #[tokio::test]
    async fn test_trader_agent_rejects_invalid_fill_data_without_ledger_write() {
        struct InvalidFillExecutor {
            qty_filled: f64,
            avg_price: f64,
        }

        #[async_trait::async_trait]
        impl TradeExecutor for InvalidFillExecutor {
            async fn unlock_trade(&self) -> Result<()> {
                Ok(())
            }
            async fn get_account_funds(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::AccountFunds>
            {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: PlaceOrderRequest,
            ) -> Result<crate::features::trading::application::trade_executor::PlaceOrderResponse>
            {
                Ok(crate::features::trading::application::trade_executor::PlaceOrderResponse { order_id: Some("fill-1".to_string()), failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::None })
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::features::trading::application::trade_executor::OrderExecutionDetails>
            {
                Ok(crate::features::trading::application::trade_executor::OrderExecutionDetails { order_id: "fill-1".to_string(), symbol: "FILL".to_string(), status: crate::features::trading::application::trade_executor::OrderStatus::Filled, qty_requested: 1.0, qty_filled: self.qty_filled, avg_price: self.avg_price, error_msg: None, failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::None })
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::BrokerPermissions>
            {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::features::trading::application::trade_executor::TradableCapacity>
            {
                Ok(
                    crate::features::trading::application::trade_executor::TradableCapacity {
                        max_buy: 1.0,
                        max_sell: 1.0,
                    },
                )
            }
            async fn cancel_order(&self, _: &str) -> Result<()> {
                unreachable!()
            }
            async fn get_positions(
                &self,
            ) -> Result<Vec<crate::features::trading::application::trade_executor::Position>>
            {
                unreachable!()
            }
        }

        for (qty_filled, avg_price) in [(f64::NAN, 100.0), (1.0, f64::INFINITY), (0.0, 100.0)] {
            let temp = tempdir().unwrap();
            let ledger = Arc::new(
                crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                    temp.path().to_path_buf(),
                ),
            );
            let agent = TraderAgent::new(
                Arc::new(Mutex::new(InvalidFillExecutor {
                    qty_filled,
                    avg_price,
                })),
                ledger.clone(),
            );
            let summary = agent
                .execute_signals(vec![
                    crate::features::radar::application::execution_gate::GatedTrade {
                        symbol: "FILL".to_string(),
                        side: TradeSide::Buy,
                        qty: 1.0,
                        price: 100.0,
                        reason: "fill regression".to_string(),
                        is_liquidation: false,
                        is_trim: false,
                    },
                ])
                .await
                .unwrap();
            assert!(summary.status.is_err());
            assert_eq!(summary.audits[0].status, "FillDataInvalid");
            assert!(!summary.audits[0].success);
            let (_, positions) = ledger.get_portfolio_stats();
            assert!(positions.is_empty());
        }
    }

    #[tokio::test]
    async fn test_trader_agent_reports_ledger_write_failure_after_fill() {
        let temp = tempdir().unwrap();
        let blocked_path = temp.path().join("ledger-parent-file");
        std::fs::write(&blocked_path, "not a directory").unwrap();
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(blocked_path),
        );
        let executor = Arc::new(Mutex::new(MockTradeExecutor::new()));
        let agent =
            TraderAgent::new(executor, ledger).with_poll_settings(std::time::Duration::ZERO, 2);

        let summary = agent
            .execute_signals(vec![
                crate::features::radar::application::execution_gate::GatedTrade {
                    symbol: "LEDGER_FAIL".to_string(),
                    side: TradeSide::Buy,
                    qty: 1.0,
                    price: 100.0,
                    reason: "ledger failure regression".to_string(),
                    is_liquidation: false,
                    is_trim: false,
                },
            ])
            .await
            .unwrap();

        assert!(summary.status.is_err());
        assert_eq!(summary.audits[0].status, "LedgerWriteFailed");
        assert!(!summary.audits[0].success);
        assert!(summary.audits[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("账本写入失败"));
    }

    #[tokio::test]
    async fn test_reconcile_positions_rejects_invalid_quantities() {
        struct InvalidPositionExecutor {
            positions: Vec<Position>,
        }

        #[async_trait::async_trait]
        impl TradeExecutor for InvalidPositionExecutor {
            async fn unlock_trade(&self) -> Result<()> {
                Ok(())
            }
            async fn get_account_funds(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::AccountFunds>
            {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: PlaceOrderRequest,
            ) -> Result<crate::features::trading::application::trade_executor::PlaceOrderResponse>
            {
                unreachable!()
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::features::trading::application::trade_executor::OrderExecutionDetails>
            {
                unreachable!()
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::BrokerPermissions>
            {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::features::trading::application::trade_executor::TradableCapacity>
            {
                unreachable!()
            }
            async fn cancel_order(&self, _: &str) -> Result<()> {
                unreachable!()
            }
            async fn get_positions(&self) -> Result<Vec<Position>> {
                Ok(self.positions.clone())
            }
        }

        for qty in [f64::NAN, f64::INFINITY, -1.0] {
            let temp = tempdir().unwrap();
            let ledger = Arc::new(
                crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                    temp.path().to_path_buf(),
                ),
            );
            let executor = InvalidPositionExecutor {
                positions: vec![Position {
                    symbol: "INVALID".to_string(),
                    side: PositionSide::Long,
                    qty,
                    can_sell_qty: 0.0,
                    cost_price: 100.0,
                    market_val: 0.0,
                    pl_val: 0.0,
                    pl_ratio: 0.0,
                }],
            };
            let agent = TraderAgent::new(Arc::new(Mutex::new(executor)), ledger);
            let error = agent.reconcile_positions().await.unwrap_err().to_string();
            assert!(error.contains("broker position quantity is invalid"));
            assert!(error.contains("INVALID"));
        }

        let temp = tempdir().unwrap();
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                temp.path().to_path_buf(),
            ),
        );
        let executor = InvalidPositionExecutor {
            positions: vec![Position {
                symbol: "UNKNOWN_SIDE".to_string(),
                side: PositionSide::Unknown,
                qty: 1.0,
                can_sell_qty: 0.0,
                cost_price: 100.0,
                market_val: 100.0,
                pl_val: 0.0,
                pl_ratio: 0.0,
            }],
        };
        let agent = TraderAgent::new(Arc::new(Mutex::new(executor)), ledger);
        let error = agent.reconcile_positions().await.unwrap_err().to_string();
        assert!(error.contains("broker position side is unknown"));
        assert!(error.contains("UNKNOWN_SIDE"));

        let temp = tempdir().unwrap();
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                temp.path().to_path_buf(),
            ),
        );
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(temp.path().join("ledger.csv"))
            .unwrap();
        writeln!(
            file,
            "{},10:00:00,LOCAL_INVALID,BUY,NaN,100.00000000,TEST",
            Local::now().date_naive()
        )
        .unwrap();
        let agent = TraderAgent::new(
            Arc::new(Mutex::new(InvalidPositionExecutor { positions: vec![] })),
            ledger,
        );
        let error = agent.reconcile_positions().await.unwrap_err().to_string();
        assert!(error.contains("local ledger is invalid"));
        assert!(error.contains("LOCAL_INVALID"));
    }

    #[tokio::test]
    async fn test_reconcile_positions_aggregates_duplicate_broker_rows() {
        struct DuplicatePositionExecutor;

        #[async_trait::async_trait]
        impl TradeExecutor for DuplicatePositionExecutor {
            async fn unlock_trade(&self) -> Result<()> {
                Ok(())
            }
            async fn get_account_funds(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::AccountFunds>
            {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: PlaceOrderRequest,
            ) -> Result<crate::features::trading::application::trade_executor::PlaceOrderResponse>
            {
                unreachable!()
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::features::trading::application::trade_executor::OrderExecutionDetails>
            {
                unreachable!()
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::BrokerPermissions>
            {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::features::trading::application::trade_executor::TradableCapacity>
            {
                unreachable!()
            }
            async fn cancel_order(&self, _: &str) -> Result<()> {
                unreachable!()
            }
            async fn get_positions(&self) -> Result<Vec<Position>> {
                Ok(vec![
                    Position {
                        symbol: "DUP".to_string(),
                        side: PositionSide::Long,
                        qty: 4.0,
                        can_sell_qty: 4.0,
                        cost_price: 100.0,
                        market_val: 400.0,
                        pl_val: 0.0,
                        pl_ratio: 0.0,
                    },
                    Position {
                        symbol: "DUP".to_string(),
                        side: PositionSide::Long,
                        qty: 6.0,
                        can_sell_qty: 6.0,
                        cost_price: 100.0,
                        market_val: 600.0,
                        pl_val: 0.0,
                        pl_ratio: 0.0,
                    },
                ])
            }
        }

        let temp = tempdir().unwrap();
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                temp.path().to_path_buf(),
            ),
        );
        ledger
            .record_trade(TradeRecordAdapter {
                date: Local::now().date_naive(),
                timestamp: "10:00:00".to_string(),
                symbol: "DUP".to_string(),
                side: "BUY".to_string(),
                qty: 10.0,
                price: 100.0,
                signal: "TEST".to_string(),
            })
            .unwrap();

        let agent = TraderAgent::new(Arc::new(Mutex::new(DuplicatePositionExecutor)), ledger);
        let report = agent.reconcile_positions().await.unwrap();
        assert_eq!(report.matching_count, 1);
        assert!(report.mismatches.is_empty());
    }

    #[tokio::test]
    async fn test_trader_agent_capping() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();

        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        // --- 低い capacity を設定する ---
        {
            let exec = mock_exec.lock().await;
            let mut cap = exec.mock_capacity.lock().await;
            cap.max_buy = 5.0; // 非常に低い capacity
        }

        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        let trade = crate::features::radar::application::execution_gate::GatedTrade {
            symbol: "AAPL".to_string(),
            side: TradeSide::Buy,
            qty: 50.0, // 要求数量は 50
            price: 150.0,
            reason: "Test".to_string(),
            is_liquidation: false,
            is_trim: false,
        };

        let summary = agent.execute_signals(vec![trade]).await.unwrap();

        assert!(!summary.audits.is_empty());
        let audit = &summary.audits[0];

        // 5.0 に切り詰められること
        assert_eq!(audit.qty_requested, 5.0);
        assert_eq!(audit.qty_filled, 5.0); // モックは 2 回目の照会で Filled を返す。
    }

    #[tokio::test]
    async fn test_trader_agent_rejects_invalid_capacity_without_submitting() {
        for invalid_capacity in [f64::NAN, f64::INFINITY, -1.0] {
            let temp = tempdir().unwrap();
            let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));
            {
                let exec = mock_exec.lock().await;
                let mut capacity = exec.mock_capacity.lock().await;
                capacity.max_buy = invalid_capacity;
            }
            let ledger = Arc::new(
                crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                    temp.path().to_path_buf(),
                ),
            );
            let agent = TraderAgent::new(mock_exec.clone(), ledger);
            let summary = agent
                .execute_signals(vec![
                    crate::features::radar::application::execution_gate::GatedTrade {
                        symbol: "INVALID_CAPACITY".to_string(),
                        side: TradeSide::Buy,
                        qty: 1.0,
                        price: 100.0,
                        reason: "capacity regression".to_string(),
                        is_liquidation: false,
                        is_trim: false,
                    },
                ])
                .await
                .unwrap();

            assert!(summary.status.is_err());
            assert_eq!(summary.audits[0].status, "CapacityInvalid");
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

    #[tokio::test]
    async fn test_trader_agent_rejects_invalid_order_input_before_capacity_query() {
        for (qty, price) in [
            (f64::NAN, 100.0),
            (f64::INFINITY, 100.0),
            (0.0, 100.0),
            (-1.0, 100.0),
            (1.0, f64::NAN),
            (1.0, f64::INFINITY),
            (1.0, 0.0),
            (1.0, -1.0),
        ] {
            let temp = tempdir().unwrap();
            let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));
            let ledger = Arc::new(
                crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                    temp.path().to_path_buf(),
                ),
            );
            let agent = TraderAgent::new(mock_exec.clone(), ledger);
            let summary = agent
                .execute_signals(vec![
                    crate::features::radar::application::execution_gate::GatedTrade {
                        symbol: "INVALID_INPUT".to_string(),
                        side: TradeSide::Buy,
                        qty,
                        price,
                        reason: "input regression".to_string(),
                        is_liquidation: false,
                        is_trim: false,
                    },
                ])
                .await
                .unwrap();
            assert!(summary.status.is_err());
            assert_eq!(summary.audits[0].status, "OrderInputInvalid");
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

    #[tokio::test]
    async fn test_trader_agent_liquidation_semantics() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        // 1. 十分な capacity を設定する（current position = 1000）。
        {
            let exec = mock_exec.lock().await;
            let mut cap = exec.mock_capacity.lock().await;
            cap.max_sell = 1000.0;
        }

        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        // 2. qty=1.0 だが is_liquidation=true の注文を送る。
        let trade = crate::features::radar::application::execution_gate::GatedTrade {
            symbol: "EXIT_ASSET".to_string(),
            side: TradeSide::Sell,
            qty: 1.0, // シグナル上の要求数量は 1.
            price: 150.0,
            reason: "ExitTest".to_string(),
            is_liquidation: true,
            is_trim: false,
        };

        let summary = agent.execute_signals(vec![trade]).await.unwrap();
        assert!(!summary.audits.is_empty());

        // 3. qty が 1000.0（max_sell）へ補正されたことを確認する。
        assert_eq!(summary.audits[0].qty_requested, 1000.0);
    }

    #[tokio::test]
    async fn test_trader_agent_trim_semantics() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        // 1. capacity を設定する（current position = 1000）。
        {
            let exec = mock_exec.lock().await;
            let mut cap = exec.mock_capacity.lock().await;
            cap.max_sell = 1000.0;
        }

        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        // 2. is_trim=true の注文を送る。
        let trade = crate::features::radar::application::execution_gate::GatedTrade {
            symbol: "TRIM_ASSET".to_string(),
            side: TradeSide::Sell,
            qty: 0.0, // trim では Gate が数量を指定しない。
            price: 150.0,
            reason: "TrimTest".to_string(),
            is_liquidation: false,
            is_trim: true,
        };

        let summary = agent.execute_signals(vec![trade]).await.unwrap();
        assert!(!summary.audits.is_empty());

        // 3. qty が 500.0（max_sell の 50%）へ補正されたことを確認する。
        assert_eq!(summary.audits[0].qty_requested, 500.0);
    }

    #[tokio::test]
    async fn test_trader_agent_capacity_query_failure() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();

        let mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );
        let agent = TraderAgent::new(mock_exec.clone(), ledger);

        let trade = crate::features::radar::application::execution_gate::GatedTrade {
            symbol: "FAIL".to_string(),
            side: TradeSide::Buy,
            qty: 50.0,
            price: 150.0,
            reason: "Test Fail".to_string(),
            is_liquidation: false,
            is_trim: false,
        };

        // MockTradeExecutor は、特定 symbol を扱うようにしない限り失敗しない。
        // テスト用に symbol ベースの failure を実装する。

        let summary = agent.execute_signals(vec![trade]).await.unwrap();

        assert!(!summary.audits.is_empty());
        let audit = &summary.audits[0];

        // status と reason を確認する。
        assert_eq!(audit.status, "CapacityQueryFailed");
        assert!(audit
            .error
            .as_ref()
            .unwrap()
            .contains("Mock capacity query failure"));

        // order が発注されていないことを確認する。
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

        // --- モックを Submitted 状態に維持する ---
        // デフォルトでは、MockTradeExecutor は 2 回の照会後に Filled を返す。
        // Mock を変更せずに Filled threshold を直接変える手段はない。
        // 終端状態を返さない場合に Timeout へ到達することだけを確認する。

        // 特定 symbol では MockTradeExecutor::get_order_status が Submitted のままになるようにする。

        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );
        // 高速ポーリング: 5 回 × 1ms で合計 5ms の timeout を模擬する。
        let agent = TraderAgent::new(mock_exec.clone(), ledger)
            .with_poll_settings(std::time::Duration::from_millis(1), 5);

        let trade = crate::features::radar::application::execution_gate::GatedTrade {
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
        summary
            .status
            .expect("confirmed cancellation should remain successful");

        assert!(!summary.audits.is_empty());
        let audit = &summary.audits[0];

        // status を確認する。最終 check では Cancelled と確認されるべき。
        assert_eq!(audit.status, "TimedOutCancelledConfirmed");

        // mock state を確認する。order_id は cancelled_orders set に存在するべき。
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
    async fn test_timeout_cancellation_records_partial_fill_and_returns_error() {
        let temp = tempdir().unwrap();
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                temp.path().to_path_buf(),
            ),
        );
        let executor = Arc::new(Mutex::new(MockTradeExecutor::new()));
        let agent = TraderAgent::new(executor, ledger.clone())
            .with_poll_settings(std::time::Duration::from_millis(0), 0);

        let summary = agent
            .execute_signals(vec![
                crate::features::radar::application::execution_gate::GatedTrade {
                    symbol: "PARTIAL_CANCEL".to_string(),
                    side: TradeSide::Buy,
                    qty: 10.0,
                    price: 150.0,
                    reason: "partial cancellation regression".to_string(),
                    is_liquidation: false,
                    is_trim: false,
                },
            ])
            .await
            .unwrap();

        assert!(summary.status.is_err());
        assert_eq!(summary.audits[0].status, "TimedOutCancelledConfirmed");
        assert_eq!(summary.audits[0].qty_filled, 5.0);
        let (_, positions) = ledger.get_portfolio_stats();
        assert_eq!(positions.get("PARTIAL_CANCEL").unwrap().0, 5.0);
    }

    #[tokio::test]
    async fn test_trader_agent_unconfirmed_timeout_is_error() {
        struct TimeoutExecutor {
            cancel_fails: bool,
        }

        #[async_trait::async_trait]
        impl TradeExecutor for TimeoutExecutor {
            async fn unlock_trade(&self) -> Result<()> {
                Ok(())
            }
            async fn get_account_funds(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::AccountFunds>
            {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: PlaceOrderRequest,
            ) -> Result<crate::features::trading::application::trade_executor::PlaceOrderResponse>
            {
                Ok(crate::features::trading::application::trade_executor::PlaceOrderResponse { order_id: Some("timeout-1".to_string()), failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::None })
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::features::trading::application::trade_executor::OrderExecutionDetails>
            {
                Ok(crate::features::trading::application::trade_executor::OrderExecutionDetails { order_id: "timeout-1".to_string(), symbol: "TIMEOUT".to_string(), status: crate::features::trading::application::trade_executor::OrderStatus::Submitted, qty_requested: 1.0, qty_filled: 0.0, avg_price: 0.0, error_msg: None, failure_reason: crate::features::trading::application::trade_executor::OrderFailureReason::None })
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::BrokerPermissions>
            {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::features::trading::application::trade_executor::TradableCapacity>
            {
                Ok(
                    crate::features::trading::application::trade_executor::TradableCapacity {
                        max_buy: 1.0,
                        max_sell: 1.0,
                    },
                )
            }
            async fn cancel_order(&self, _: &str) -> Result<()> {
                if self.cancel_fails {
                    Err(anyhow::anyhow!("cancel failed"))
                } else {
                    Ok(())
                }
            }
            async fn get_positions(
                &self,
            ) -> Result<Vec<crate::features::trading::application::trade_executor::Position>>
            {
                unreachable!()
            }
        }

        for cancel_fails in [true, false] {
            let temp = tempdir().unwrap();
            let ledger = Arc::new(
                crate::features::shared::acl::ledger_factory::build_ledger_adapter(
                    temp.path().to_path_buf(),
                ),
            );
            let agent = TraderAgent::new(
                Arc::new(Mutex::new(TimeoutExecutor { cancel_fails })),
                ledger,
            )
            .with_poll_settings(std::time::Duration::from_millis(0), 0);
            let summary = agent
                .execute_signals(vec![
                    crate::features::radar::application::execution_gate::GatedTrade {
                        symbol: "TIMEOUT".to_string(),
                        side: TradeSide::Buy,
                        qty: 1.0,
                        price: 100.0,
                        reason: "timeout regression".to_string(),
                        is_liquidation: false,
                        is_trim: false,
                    },
                ])
                .await
                .unwrap();
            assert!(summary.status.is_err());
        }
    }

    #[tokio::test]
    async fn test_trader_agent_reconciliation() {
        use crate::features::shared::acl::ledger_factory::TradeRecordAdapter;
        use crate::features::trading::application::trade_executor::{Position, PositionSide};
        use chrono::Local;

        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let ledger = Arc::new(
            crate::features::shared::acl::ledger_factory::build_ledger_adapter(save_dir.clone()),
        );

        // 1. local ledger に 10 TSLA、20 AAPL を設定する。
        ledger
            .record_trade(TradeRecordAdapter {
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
            .record_trade(TradeRecordAdapter {
                date: Local::now().date_naive(),
                timestamp: "10:01:00".to_string(),
                symbol: "US.AAPL".to_string(),
                side: "BUY".to_string(),
                qty: 20.0,
                price: 150.0,
                signal: "TEST".to_string(),
            })
            .unwrap();

        // 2. mock executor に 10 TSLA（一致）、25 AAPL（不一致）、5 NVDA（broker のみ）を設定する。
        let _mock_exec = Arc::new(Mutex::new(MockTradeExecutor::new()));

        struct ReconMock;
        #[async_trait::async_trait]
        impl crate::features::trading::application::trade_executor::TradeExecutor for ReconMock {
            async fn get_account_funds(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::AccountFunds>
            {
                unreachable!()
            }
            async fn get_broker_permissions(
                &self,
            ) -> Result<crate::features::trading::application::trade_executor::BrokerPermissions>
            {
                unreachable!()
            }
            async fn get_tradable_capacity(
                &self,
                _: &str,
                _: f64,
            ) -> Result<crate::features::trading::application::trade_executor::TradableCapacity>
            {
                unreachable!()
            }
            async fn place_order(
                &self,
                _: crate::features::trading::application::trade_executor::PlaceOrderRequest,
            ) -> Result<crate::features::trading::application::trade_executor::PlaceOrderResponse>
            {
                unreachable!()
            }
            async fn get_order_status(
                &self,
                _: &str,
            ) -> Result<crate::features::trading::application::trade_executor::OrderExecutionDetails>
            {
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
                        qty: 25.0, // 不一致（Local 20）
                        can_sell_qty: 25.0,
                        cost_price: 150.0,
                        market_val: 3750.0,
                        pl_val: 0.0,
                        pl_ratio: 0.0,
                    },
                    Position {
                        symbol: "US.NVDA".to_string(), // broker のみ
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

        // 3. 確認する。
        assert_eq!(report.matching_count, 1); // TSLA のみ一致
        assert_eq!(report.mismatches.len(), 2);

        // AAPL の不一致。
        let aapl = report
            .mismatches
            .iter()
            .find(|m| m.symbol == "US.AAPL")
            .unwrap();
        assert_eq!(aapl.local_qty, 20.0);
        assert_eq!(aapl.broker_qty, 25.0);
        assert_eq!(aapl.diff, -5.0);

        // NVDA は broker のみに存在する。
        let nvda = report
            .mismatches
            .iter()
            .find(|m| m.symbol == "US.NVDA")
            .unwrap();
        assert_eq!(nvda.local_qty, 0.0);
        assert_eq!(nvda.broker_qty, 5.0);
    }

    #[test]
    fn localized_trader_agent_notice_is_japanese() {
        assert!(no_trades_notice().contains("取引対象がない"));
    }
}
