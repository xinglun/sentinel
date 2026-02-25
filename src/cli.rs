use anyhow::{anyhow, Result};
use chrono::{Local, Utc};
use futures::stream::{self, StreamExt};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::backtest;
use crate::config;
use crate::core::engine::{self, TickerSnapshot};
use crate::core::notify;
use crate::core::report;
use crate::core::trader_agent::TraderAgent;
use crate::data::provider::MarketDataProvider;
use crate::trade::trader::TradeExecutor;

// Conditionally import Futu adapter (it requires Tokio runtime and TCP stream)
use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::provider::FutuProvider;
use crate::adapters::futu::trader::FutuTrader;

#[derive(Clone, Copy, PartialEq)]
enum ProviderType {
    Yahoo,
    Futu,
}

pub async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mut command = "radar";
    let mut provider_type = ProviderType::Yahoo;
    let mut futu_addr = "127.0.0.1:11111".to_string();

    // Simple argument parsing
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "backtest" => command = "backtest",
            "daemon" | "trade" => command = "daemon",
            "radar" => command = "radar",
            "--provider" => {
                if i + 1 < args.len() {
                    if args[i + 1].to_lowercase() == "futu" {
                        provider_type = ProviderType::Futu;
                    }
                    i += 1;
                }
            }
            "--opend" => {
                if i + 1 < args.len() {
                    futu_addr = args[i + 1].clone();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    match command {
        "backtest" => {
            println!("🔬 资本望远镜：回测模式启动 (Backtest Mode)");
            let mut from_date = "2015-01-01".to_string();
            let mut to_date = "2025-01-01".to_string();

            let mut iter = args.iter().skip(1);
            while let Some(arg) = iter.next() {
                if arg == "--from" {
                    if let Some(val) = iter.next() {
                        from_date = val.clone();
                    }
                } else if arg == "--to" {
                    if let Some(val) = iter.next() {
                        to_date = val.clone();
                    }
                }
            }
            let app_config = config::AppConfig::load("config.toml")?;
            backtest::run_backtest(&app_config, &from_date, &to_date).await?;
        }
        "daemon" => {
            println!("🤖 哨兵守卫：交易守护进程启动 (Daemon Mode)");
            println!(
                "   数据引擎: {:?}",
                if provider_type == ProviderType::Futu {
                    "Moomoo OpenD"
                } else {
                    "Yahoo Finance"
                }
            );

            let app_config = config::AppConfig::load("config.toml")?;

            if provider_type == ProviderType::Futu {
                let futu_cfg = app_config
                    .futu
                    .clone()
                    .ok_or_else(|| anyhow!("Missing [futu] config section"))?;

                println!("🔌 正在建立协议封装和心跳机制...");
                let client = Arc::new(FutuClient::connect(&futu_addr).await?);
                let _provider = Arc::new(FutuProvider::new(client.clone()));

                println!("✅ 成功连接至 Moomoo OpenD ({})", futu_addr);

                let trader = FutuTrader::new(client.clone(), futu_cfg);

                println!("🔑 正在尝试鉴权与解锁交易核心...");
                match trader.unlock_trade().await {
                    Ok(_) => println!("✅ 交易授权解锁成功。"),
                    Err(e) => println!("⚠️ 交易授权未解锁或失败 (通常仅支持读取模式): {}", e),
                }

                println!("💰 查询本地网关账户资金情况...");
                match trader.get_funds().await {
                    Ok(funds) => println!(
                        "   -> 现金: ${:.2}, 购买力: ${:.2}, 总资产: ${:.2}",
                        funds.cash, funds.power, funds.total_assets
                    ),
                    Err(e) => println!("   -> 获取账户资金失败: {}", e),
                }

                println!("🛡️ 哨兵自动化交易模块挂载完毕，进入监听循环...");

                let trader_arc: Arc<Mutex<dyn TradeExecutor + Send + Sync>> =
                    Arc::new(Mutex::new(trader));
                let trader_agent =
                    TraderAgent::new(Arc::new(app_config.clone()), trader_arc.clone());
                let rules_arc = Arc::new(app_config.get_parsed_rules());

                // Keep the daemon alive and executing
                loop {
                    println!(
                        "\n▶️ [Daemon] {} - 开始本轮行情拉取与策略评估...",
                        Local::now().format("%Y-%m-%d %H:%M:%S")
                    );

                    let mut current_snapshots = Vec::new();

                    for entry in app_config.watchlist.iter().filter(|w| w.enable) {
                        match _provider.fetch_history(&entry.symbol, None, None).await {
                            Ok(history) => {
                                let snapshot =
                                    engine::evaluate_snapshot(&history, entry, &rules_arc);
                                current_snapshots.push(snapshot);
                            }
                            Err(e) => {
                                println!("❌ [Daemon] 无法拉取 {} 行情: {}", entry.symbol, e);
                            }
                        }
                    }

                    if !current_snapshots.is_empty() {
                        if let Err(e) = trader_agent.execute_signals(&current_snapshots).await {
                            println!("❌ [Daemon] 交易代理执行信号失败: {}", e);
                        }
                    } else {
                        println!("⚠️ [Daemon] 本轮未获取到任何有效的行情快照。");
                    }

                    println!("⏳ [Daemon] 评估结束。等待下一次心跳周期 (60s)...");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            } else {
                println!("⚠️ Daemon 模式建议使用 --provider futu 配合本地网关运行以获得最新实盘数据和报单支持。");
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    println!("💓 Daemon Heartbeat (Yahoo/Offline Mode)...");
                }
            }
        }
        _ => {
            // default radar
            println!("🐕 Stock Sentinel initializing (Radar Mode)...");

            let provider: Arc<dyn MarketDataProvider> = match provider_type {
                ProviderType::Futu => {
                    println!("🔌 尝试通过 Moomoo OpenD ({}) 获取行情...", futu_addr);
                    match FutuClient::connect(&futu_addr).await {
                        Ok(client) => Arc::new(FutuProvider::new(Arc::new(client))),
                        Err(e) => {
                            println!(
                                "❌ 无法连接至 Moomoo OpenD: {}。将自动降级使用 Yahoo Finance。",
                                e
                            );
                            Arc::new(YahooProviderAdapter)
                        }
                    }
                }
                ProviderType::Yahoo => Arc::new(YahooProviderAdapter),
            };

            run_radar(provider).await?;
        }
    }

    Ok(())
}

// Wrapper to make existing yahoo_provider fit the MarketDataProvider trait
struct YahooProviderAdapter;

#[async_trait::async_trait]
impl MarketDataProvider for YahooProviderAdapter {
    async fn fetch_history(
        &self,
        symbol: &str,
        start_date: Option<OffsetDateTime>,
        end_date: Option<OffsetDateTime>,
    ) -> Result<crate::data::yahoo_provider::TickerHistory> {
        crate::data::yahoo_provider::fetch_history(symbol, start_date, end_date).await
    }
}

async fn run_radar(provider: Arc<dyn MarketDataProvider>) -> Result<()> {
    let app_config = config::AppConfig::load("config.toml")?;
    let parsed_rules = app_config.get_parsed_rules();

    let config_content = std::fs::read_to_string("config.toml").unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(config_content.as_bytes());
    let config_hash = format!("{:x}", hasher.finalize())[..8].to_string();

    let config_arc = Arc::new(app_config);
    let rules_arc = Arc::new(parsed_rules);

    let watchlist = &config_arc.watchlist;
    let enabled_count = watchlist.iter().filter(|w| w.enable).count();

    let mut yesterday_state = "Unknown".to_string();
    let mut prev_margin = None;
    let mut prev_exposure = None;
    let mut prev_up_count = None;
    let mut prev_ema_accel = None;

    if let Ok(content) = std::fs::read_to_string(
        std::path::Path::new(&config_arc.output.save_to).join("telemetry.csv"),
    ) {
        let mut historical_margins = Vec::new();
        for line in content.lines().skip(1) {
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() > 12 {
                if let Ok(m) = cols[12].parse::<f64>() {
                    historical_margins.push(m);
                }
            }
        }

        if historical_margins.len() > 1 {
            let mut accelerations = Vec::new();
            for i in 1..historical_margins.len() {
                accelerations.push(historical_margins[i] - historical_margins[i - 1]);
            }
            if !accelerations.is_empty() {
                let mut ema = accelerations[0];
                let alpha = 2.0 / (5.0 + 1.0);
                for &acc in accelerations.iter().skip(1) {
                    ema = alpha * acc + (1.0 - alpha) * ema;
                }
                prev_ema_accel = Some(ema);
            }
        }

        if let Some(last_line) = content.lines().last() {
            let cols: Vec<&str> = last_line.split(',').collect();
            if cols.len() > 12 {
                yesterday_state = cols[3].to_string();
                prev_margin = cols[12].parse::<f64>().ok();
                if cols.len() > 13 {
                    prev_exposure = cols[13].parse::<f64>().ok();
                }
                if cols.len() > 15 {
                    if let (Ok(universe), Ok(up_share)) =
                        (cols[14].parse::<f64>(), cols[15].parse::<f64>())
                    {
                        prev_up_count = Some((universe * up_share).round() as usize);
                    }
                }
            }
        }
    }

    println!("📊 Fetching data for {} enabled assets...", enabled_count);

    let mut snapshots = Vec::new();
    let mut quote_timestamps = Vec::new();

    let fetches = stream::iter(watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let rules_ref = Arc::clone(&rules_arc);
            let provider_ref = Arc::clone(&provider);
            async move {
                let symbol = &entry.symbol;
                match provider_ref.fetch_history(symbol, None, None).await {
                    Ok(history) => {
                        let latest_ts = history.latest_quote_timestamp;
                        let snapshot = engine::evaluate_snapshot(&history, entry, &rules_ref);
                        (Some(snapshot), latest_ts)
                    }
                    Err(e) => {
                        println!("[ERROR] Error processing {}: {}", symbol, e);
                        let err_snap = TickerSnapshot {
                            symbol: symbol.clone(),
                            name: entry.name.clone().unwrap_or_else(|| symbol.clone()),
                            weight: entry.weight.unwrap_or(1.0),
                            reason_code: Some("[API ERROR]".to_string()),
                            current_date: chrono::Local::now().date_naive(),
                            dog_price: 0.0,
                            owner_ma: None,
                            leash_ma: None,
                            owner_ma_slope_pct: None,
                            dev_z_score: None,
                            curvature: None,
                            confidence_score: 0,
                            trend_status: engine::TrendStatus::Unknown,
                            deviation_pct: None,
                            deviation_basis_used: format!("{:?}", entry.deviation_basis)
                                .to_lowercase(),
                            state_code: "ERROR".to_string(),
                            action_text: format!("Fetch failed: {}", e),
                            is_bear_mode_active: false,
                            is_caution_mode_active: false,
                            trend_age: 0,
                            owner_deviation_pct: None,
                            deviation_percentile: None,
                            validity: engine::RegimeValidity::Invalid,
                            history_days: 0,
                        };
                        (Some(err_snap), None)
                    }
                }
            }
        })
        .buffer_unordered(10);

    let mut results_map = std::collections::HashMap::new();
    let results: Vec<(Option<TickerSnapshot>, Option<i64>)> = fetches.collect().await;

    for (res_opt, ts_opt) in results {
        if let Some(res) = res_opt {
            results_map.insert(res.symbol.clone(), res);
        }
        if let Some(ts) = ts_opt {
            quote_timestamps.push(ts);
        }
    }

    for entry in watchlist.iter().filter(|w| w.enable) {
        if let Some(snap) = results_map.remove(&entry.symbol) {
            snapshots.push(snap);
        }
    }

    if !snapshots.is_empty() {
        let mut up_count = 0;
        let mut flat_count = 0;
        let mut down_count = 0;
        let mut up_weight = 0.0;
        let mut flat_weight = 0.0;
        let mut down_weight = 0.0;
        let mut forming_early_count = 0;
        let mut forming_late_count = 0;
        let mut forming_early_weight = 0.0;
        let mut forming_late_weight = 0.0;

        for s in &snapshots {
            if s.validity == engine::RegimeValidity::FormingEarly
                || s.validity == engine::RegimeValidity::FormingLate
            {
                if s.validity == engine::RegimeValidity::FormingEarly {
                    forming_early_count += 1;
                    forming_early_weight += s.weight;
                } else {
                    forming_late_count += 1;
                    forming_late_weight += s.weight;
                }
                continue;
            }

            match s.trend_status {
                engine::TrendStatus::Up => {
                    up_count += 1;
                    up_weight += s.weight;
                }
                engine::TrendStatus::Flat => {
                    flat_count += 1;
                    flat_weight += s.weight;
                }
                engine::TrendStatus::Down | engine::TrendStatus::Unknown => {
                    down_count += 1;
                    down_weight += s.weight;
                }
            }
        }

        let total_count = up_count + flat_count + down_count;
        let total_weight = up_weight + flat_weight + down_weight;

        let mut total_strength_sum = 0.0;
        let mut weight_for_strength = 0.0;

        let mut total_potential_sum = 0.0;
        let mut weight_for_potential = 0.0;

        let mut trend_alloc_weight = 0.0;
        let mut reversion_alloc_weight = 0.0;

        for s in &snapshots {
            if s.validity == engine::RegimeValidity::FormingEarly
                || s.validity == engine::RegimeValidity::FormingLate
            {
                continue;
            }

            if let Some(strength) = s.owner_ma_slope_pct {
                total_strength_sum += strength * s.weight;
                weight_for_strength += s.weight;
            }
            if let Some(z) = s.dev_z_score {
                total_potential_sum += z.abs() * s.weight;
                weight_for_potential += s.weight;

                if s.confidence_score >= 80 {
                    trend_alloc_weight += s.weight * (s.confidence_score as f64 / 100.0);
                } else if s.confidence_score <= 60 {
                    reversion_alloc_weight +=
                        s.weight * ((100.0 - s.confidence_score as f64) / 100.0) * z.abs();
                }
            }
        }

        let global_gravity_strength = if weight_for_strength > 0.0 {
            total_strength_sum / weight_for_strength
        } else {
            0.0
        };

        let global_potential_energy = if weight_for_potential > 0.0 {
            total_potential_sum / weight_for_potential
        } else {
            0.0
        };

        let dominance_margin = (trend_alloc_weight - reversion_alloc_weight) / total_weight;
        let conf_trend_alloc = (trend_alloc_weight / total_weight * 50.0).clamp(0.0, 50.0);
        let conf_inverse_potential =
            (1.0 / (1.0 + global_potential_energy) * 50.0).clamp(0.0, 50.0);
        let confidence_score = conf_trend_alloc + conf_inverse_potential;
        let system_confidence = (confidence_score.clamp(0.0, 100.0) * 100.0).round() / 100.0;

        let market_phase = if dominance_margin > 0.5 {
            if global_gravity_strength > 0.5 {
                "Mid Bull"
            } else {
                "Late Bull"
            }
        } else if dominance_margin > 0.2 {
            "Early Bull"
        } else if dominance_margin < -0.5 {
            "Bear Market"
        } else if dominance_margin < -0.1 {
            "Correction"
        } else {
            "Neutral / Transition"
        };

        let t_share_raw = trend_alloc_weight / total_weight;
        let r_share_raw = reversion_alloc_weight / total_weight;
        let r_share_adj = r_share_raw * (1.0 + global_potential_energy);
        let total_adjusted = t_share_raw + r_share_adj;
        let t_ratio_final = t_share_raw / total_adjusted;
        let r_ratio_final = r_share_adj / total_adjusted;
        let energy_adjusted_margin = t_ratio_final - r_ratio_final;

        let mut capital_flow_acceleration = None;
        if let Some(pm) = prev_margin {
            let today_accel = energy_adjusted_margin - pm;
            let ema_accel = match prev_ema_accel {
                Some(prev_ema) => {
                    let alpha = 2.0 / (5.0 + 1.0);
                    alpha * today_accel + (1.0 - alpha) * prev_ema
                }
                None => today_accel,
            };
            capital_flow_acceleration = Some(ema_accel);
        }

        let capital_flow_vector = if energy_adjusted_margin > 0.0 {
            let acc = capital_flow_acceleration.unwrap_or(0.0);
            if acc.abs() < 0.02 {
                "Stable Uptrend ↗️"
            } else if acc >= 0.02 {
                "Accelerating Uptrend 🚀"
            } else {
                "Decelerating Uptrend ⚠️"
            }
        } else {
            let acc = capital_flow_acceleration.unwrap_or(0.0);
            if acc.abs() < 0.02 {
                "Stable Downtrend ↘️"
            } else if acc <= -0.02 {
                "Accelerating Downtrend 🩸"
            } else {
                "Decelerating Downtrend (Bottoming) ⏳"
            }
        };

        let base_exposure = (0.5 + (dominance_margin * 0.5)).clamp(0.0, 1.0);
        let mut adjusted_exposure = base_exposure;

        let mut temp_health = report::GravityHealth {
            up_count,
            flat_count,
            total_count,
            up_weight,
            flat_weight,
            total_weight,
            global_gravity_strength,
            global_potential_energy,
            trend_alloc_weight,
            reversion_alloc_weight,
            config_hash,
            system_confidence,
            market_phase: market_phase.to_string(),
            capital_flow_vector: capital_flow_vector.to_string(),
            recommended_exposure: 0.0,
            forming_early_count,
            forming_late_count,
            forming_early_weight,
            forming_late_weight,
            universe_count: snapshots.len(),
            prev_system_confidence: None,
            prev_dominance_margin: prev_margin,
            prev_recommended_exposure: prev_exposure,
            prev_up_count,
            regime_age: 0,
            stability_score: 0.0,
            base_exposure,
            adjusted_exposure,
            conf_trend_alloc,
            conf_inverse_potential,
            capital_flow_acceleration,
            universe_integrity: if !snapshots.is_empty() {
                total_count as f64 / snapshots.len() as f64
            } else {
                0.0
            },
            trend_maturity: 0.0,
            stability_structural: 0.0,
            stability_temporal: 0.0,
            temporal_modifier: 1.0,
            integrity_multiplier: 1.0,
        };
        let posture = temp_health.compute_capital_posture();
        let regime_age = calculate_regime_age(
            std::path::Path::new(&config_arc.output.save_to),
            &posture.state_code,
        );
        temp_health.regime_age = regime_age;

        let trend_maturity = (regime_age as f64 / 40.0).min(1.0);
        temp_health.trend_maturity = trend_maturity;

        let stability_structural = conf_inverse_potential / 50.0;
        let stability_temporal = trend_maturity;

        temp_health.stability_structural = conf_inverse_potential;
        temp_health.stability_temporal = stability_temporal * 100.0;

        let stability_score = stability_structural * stability_temporal;
        temp_health.stability_score = stability_score;

        let temporal_modifier = 0.85 + (trend_maturity * 0.15).min(0.15);
        temp_health.temporal_modifier = temporal_modifier;

        let integrity_multiplier = temp_health.universe_integrity;
        temp_health.integrity_multiplier = integrity_multiplier;

        let conf_multiplier = (system_confidence / 100.0) * integrity_multiplier;
        let mut final_exposure = base_exposure * conf_multiplier;
        final_exposure = final_exposure.clamp(0.0, 1.0);

        adjusted_exposure = final_exposure;

        temp_health.adjusted_exposure = adjusted_exposure;
        temp_health.recommended_exposure = adjusted_exposure;

        let gravity_health = temp_health;

        let report_result =
            report::generate_reports(&config_arc, &snapshots, &gravity_health, &yesterday_state)?;

        if let Some(ref tg_cfg) = config_arc.telegram {
            let combined_output =
                format!("{}{}", report_result.markdown, report_result.telegram_html);
            if combined_output.contains(&tg_cfg.bot_token) {
                panic!("FATAL SECURITY ERROR: bot_token leak detected in reports!");
            }
        }

        let mut max_age_minutes: Option<i64> = None;
        if !quote_timestamps.is_empty() {
            let now = Utc::now().timestamp();
            let mut min_ts = quote_timestamps[0];
            for &ts in &quote_timestamps {
                if ts < min_ts {
                    min_ts = ts;
                }
            }
            let age_sec = now - min_ts;
            max_age_minutes = Some(age_sec / 60);
        }

        let freshness_data = serde_json::json!({
            "max_age_minutes": max_age_minutes,
            "stale": max_age_minutes.is_none_or(|age| age > 15),
            "timestamp_utc": Utc::now().to_rfc3339(),
        });

        if let Ok(freshness_content) = serde_json::to_string_pretty(&freshness_data) {
            let freshness_path =
                std::path::Path::new(&config_arc.output.save_to).join("freshness.json");
            let _ = std::fs::write(freshness_path, freshness_content);
        }

        println!("✅ Report generated: {}", config_arc.output.save_to);

        if let Some(ref tg_cfg) = config_arc.telegram {
            if tg_cfg.enabled {
                println!("📤 Sending report to Telegram...");
                if let Err(e) =
                    notify::send_telegram_message(tg_cfg, &report_result.telegram_html).await
                {
                    println!("❌ Failed to send Telegram message: {}", e);
                }
            }
        }
    } else {
        println!("⚠️ No valid data found. Report generation skipped.");
    }

    Ok(())
}

fn calculate_regime_age(save_dir: &std::path::Path, current_state: &str) -> usize {
    let telemetry_path = save_dir.join("telemetry.csv");
    if let Ok(content) = std::fs::read_to_string(telemetry_path) {
        let mut lines: Vec<&str> = content.lines().collect();
        if lines.len() <= 1 {
            return 1;
        }

        let mut age = 1;
        let mut last_date = "";

        lines.remove(0);

        for line in lines.iter().rev() {
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() > 3 {
                let date = cols[0];
                let state = cols[3];

                if state == current_state {
                    if date != last_date {
                        age += 1;
                        last_date = date;
                    }
                } else {
                    break;
                }
            }
        }
        age
    } else {
        1
    }
}
