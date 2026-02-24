mod config;
mod engine;
mod fetcher;
mod report;
mod notify;
mod backtest;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use crate::engine::TickerSnapshot;
use sha2::{Sha256, Digest};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "backtest" {
        println!("🔬 资本望远镜：回测模式启动 (Backtest Mode)");
        let mut from_date = "2015-01-01".to_string();
        let mut to_date = "2025-01-01".to_string();
        
        let mut iter = args.iter().skip(2);
        while let Some(arg) = iter.next() {
            if arg == "--from" {
                if let Some(val) = iter.next() { from_date = val.clone(); }
            } else if arg == "--to" {
                if let Some(val) = iter.next() { to_date = val.clone(); }
            }
        }
        
        // Pass to backtest logic
        let app_config = config::AppConfig::load("config.toml")?;
        backtest::run_backtest(&app_config, &from_date, &to_date).await?;
        return Ok(());
    }

    println!("🐕 Stock Sentinel 起床中...");
    
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
    
    // --- Phase 28: Deep Telemetry Parsing for Delta Tracking ---
    let mut yesterday_state = "Unknown".to_string();
    let mut prev_gp = None;
    let mut prev_margin = None;
    let mut prev_exposure = None;

    if let Ok(content) = std::fs::read_to_string(std::path::Path::new(&config_arc.output.save_to).join("telemetry.csv")) {
        if let Some(last_line) = content.lines().last() {
            let cols: Vec<&str> = last_line.split(',').collect();
            if cols.len() > 12 {
                yesterday_state = cols[3].to_string(); // state_code
                prev_gp = cols[6].parse::<f64>().ok();
                prev_margin = cols[12].parse::<f64>().ok();
                // If we added exposure column (index 13), parse it
                if cols.len() > 13 {
                    prev_exposure = cols[13].parse::<f64>().ok();
                }
            }
        }
    }

    println!("📊 {} 個の有効な銘柄のデータを取得しています...", enabled_count);
    
    let mut snapshots = Vec::new();
    
    let fetches = stream::iter(watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let rules_ref = Arc::clone(&rules_arc);
            async move {
                let symbol = &entry.symbol;
                match fetcher::fetch_history(symbol, None, None).await {
                    Ok(history) => {
                        let snapshot = engine::evaluate_snapshot(&history, entry, &rules_ref);
                        Some(snapshot)
                    },
                    Err(e) => {
                        println!("[エラー] {} の処理中にエラーが発生しました: {}", symbol, e);
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
                            deviation_basis_used: format!("{:?}", entry.deviation_basis).to_lowercase(),
                            state_code: "ERROR".to_string(),
                            action_text: format!("取得失敗: {}", e),
                            is_bear_mode_active: false,
                            is_caution_mode_active: false,
                            trend_age: 0,
                            owner_deviation_pct: None,
                            deviation_percentile: None,
                        };
                        Some(err_snap)
                    }
                }
            }
        })
        .buffer_unordered(10);
        
    let mut results_map = std::collections::HashMap::new();
    let results: Vec<Option<TickerSnapshot>> = fetches.collect().await;
    
    for res in results.into_iter().flatten() {
        results_map.insert(res.symbol.clone(), res);
    }
    
    // config.toml で定義された元の順序を維持します
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
        let mut regime_forming_count = 0;
        let mut regime_forming_weight = 0.0;
        
        for s in &snapshots {
            match s.trend_status {
                engine::TrendStatus::Up => {
                    up_count += 1;
                    up_weight += s.weight;
                },
                engine::TrendStatus::Flat => {
                    flat_count += 1;
                    flat_weight += s.weight;
                },
                engine::TrendStatus::Down => {
                    down_count += 1;
                    down_weight += s.weight;
                },
                engine::TrendStatus::Unknown => {
                    down_count += 1; // Unknown counts as non-up/down during transition
                    down_weight += s.weight;
                }
            }
            if s.state_code == "REGIME_FORMING" {
                regime_forming_count += 1;
                regime_forming_weight += s.weight;
            }
        }
        
        let total_count = up_count + flat_count + down_count;
        let total_weight = up_weight + flat_weight + down_weight;
        
        let mut total_strength_sum = 0.0;
        let mut weight_for_strength = 0.0;
        
        // Potential Energy
        let mut total_potential_sum = 0.0;
        let mut weight_for_potential = 0.0;
        
        // Capital State Allocations
        let mut trend_alloc_weight = 0.0;
        let mut reversion_alloc_weight = 0.0;
        
        for s in &snapshots {
            // Only aggregate strength if valid
            if let Some(strength) = s.owner_ma_slope_pct {
                total_strength_sum += strength * s.weight;
                weight_for_strength += s.weight;
            }
            if let Some(z) = s.dev_z_score {
                total_potential_sum += z.abs() * s.weight;
                weight_for_potential += s.weight;
                
                // Capital State Allocation
                if s.confidence_score >= 80 {
                    trend_alloc_weight += s.weight * (s.confidence_score as f64 / 100.0);
                } else if s.confidence_score <= 60 {
                    reversion_alloc_weight += s.weight * ((100.0 - s.confidence_score as f64) / 100.0) * z.abs();
                }
            }
        }
        
        // global_gravity_strength 
        let global_gravity_strength = if weight_for_strength > 0.0 {
            total_strength_sum / weight_for_strength
        } else {
            0.0
        };
        
        // global_potential_energy
        let global_potential_energy = if weight_for_potential > 0.0 {
            total_potential_sum / weight_for_potential
        } else {
            0.0
        };

        // --- Phase 25: Composite Macro Metrics ---
        let dominance_margin = (trend_alloc_weight - reversion_alloc_weight) / total_weight; // Normalized margin
        let confidence_score = (trend_alloc_weight / total_weight * 50.0) + (1.0 / (1.0 + global_potential_energy) * 50.0);
        let system_confidence = (confidence_score.clamp(0.0, 100.0) * 100.0).round() / 100.0;

        let market_phase = if dominance_margin > 0.5 {
            if global_gravity_strength > 0.5 { "Mid Bull" } else { "Late Bull" }
        } else if dominance_margin > 0.2 {
            "Early Bull"
        } else if dominance_margin < -0.5 {
            "Bear Market"
        } else if dominance_margin < -0.1 {
            "Correction"
        } else {
            "Neutral / Transition"
        };

        let capital_flow_vector = if dominance_margin > 0.0 {
            if global_gravity_strength > 0.0 { "Accelerating Upward ↗️" } else { "Weakening Uptrend ↗️ slowing" }
        } else {
            if global_gravity_strength < 0.0 { "Accelerating Downward ↘️" } else { "Stabilizing / Bottoming ↘️ slowing" }
        };

        let recommended_exposure = (0.5 + (dominance_margin * 0.5)).clamp(0.0, 1.0);

        let mut temp_health = report::GravityHealth {
            up_count,
            flat_count,
            down_count,
            total_count,
            up_weight,
            flat_weight,
            down_weight,
            total_weight,
            global_gravity_strength,
            global_potential_energy,
            trend_alloc_weight,
            reversion_alloc_weight,
            config_hash,
            system_confidence,
            market_phase: market_phase.to_string(),
            capital_flow_vector: capital_flow_vector.to_string(),
            recommended_exposure,
            regime_forming_count,
            regime_forming_weight,
            prev_potential_energy: prev_gp,
            prev_system_confidence: None, 
            prev_dominance_margin: prev_margin,
            prev_recommended_exposure: prev_exposure,
            regime_age: 0,
            stability_score: 0.0,
        };
        let posture = temp_health.compute_capital_posture();
        let regime_age = calculate_regime_age(std::path::Path::new(&config_arc.output.save_to), &posture.state_code);
        temp_health.regime_age = regime_age;
        temp_health.stability_score = (regime_age as f64 / 30.0).min(1.0);
        let gravity_health = temp_health;

        let report_result = report::generate_reports(&config_arc, &snapshots, &gravity_health, &yesterday_state)?;

        // Phase 8: Hard block on Telegram Token leaks
        if let Some(ref tg_cfg) = config_arc.telegram {
            let combined_output = format!("{}{}", report_result.markdown, report_result.telegram_html);
            if combined_output.contains(&tg_cfg.bot_token) {
                panic!("FATAL SECURITY ERROR: bot_token leak detected in reports!");
            }
        }

        println!("✅ レポートが提供されました: {}", config_arc.output.save_to);
        
        if let Some(ref tg_cfg) = config_arc.telegram {
            if tg_cfg.enabled {
                println!("📤 Telegramにレポートを送信中...");
                if let Err(e) = notify::send_telegram_message(tg_cfg, &report_result.telegram_html).await {
                    println!("❌ Telegramメッセージの送信に失敗しました: {}", e);
                }
            }
        }
        
    } else {
        println!("⚠️ 有効なデータが見つからなかったため、レポートは生成されませんでした。");
    }
    
    Ok(())
}

fn calculate_regime_age(save_dir: &std::path::Path, current_state: &str) -> usize {
    let telemetry_path = save_dir.join("telemetry.csv");
    if let Ok(content) = std::fs::read_to_string(telemetry_path) {
        let mut lines: Vec<&str> = content.lines().collect();
        if lines.len() <= 1 { return 1; }
        
        let mut age = 1;
        let mut last_date = "";
        
        // Skip header
        lines.remove(0);
        
        // Scan backwards to find consecutive days with the same state_code
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
