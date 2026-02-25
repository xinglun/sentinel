use crate::config::AppConfig;
use crate::engine::{TickerSnapshot, TrendStatus, RegimeValidity};
use anyhow::Result;
use chrono::Local;
use std::fs;
use std::path::Path;
use crate::notify::escape_html;
use tabled::{Table, Tabled};
use tabled::settings::Style;

#[derive(Tabled)]
struct TerminalRow {
    #[tabled(rename = "代码")]
    symbol: String,
    #[tabled(rename = "Trend (Days)")]
    trend: String,
    #[tabled(rename = "Owner Dist")]
    owner_dev: String,
    #[tabled(rename = "Sigma (σ)")]
    strength_z: String,
    #[tabled(rename = "Status (State)")]
    state: String,
    #[tabled(rename = "Action Guidance")]
    action: String,
}

pub struct ReportResult {
    #[allow(dead_code)]
    pub markdown: String,
    pub telegram_html: String,
}


pub struct GravityHealth {
    pub up_count: usize,
    pub flat_count: usize,
    pub forming_early_count: usize,
    pub forming_late_count: usize,
    pub universe_count: usize,
    pub total_count: usize, // Valid count (Macro denominator)
    pub up_weight: f64,
    pub flat_weight: f64,
    pub forming_early_weight: f64,
    pub forming_late_weight: f64,
    pub total_weight: f64, // Valid weight (Macro denominator)
    pub global_gravity_strength: f64,
    pub global_potential_energy: f64,
    pub trend_alloc_weight: f64,
    pub reversion_alloc_weight: f64,
    pub config_hash: String,
    pub system_confidence: f64,
    pub market_phase: String,
    pub capital_flow_vector: String,
    pub recommended_exposure: f64,
    // Delta trackers (Change vs Yesterday)
    pub prev_system_confidence: Option<f64>,
    pub prev_dominance_margin: Option<f64>,
    pub prev_recommended_exposure: Option<f64>,
    pub prev_up_count: Option<usize>, // For Trend Breadth Momentum
    pub regime_age: usize,
    pub stability_score: f64,
    pub base_exposure: f64,
    pub adjusted_exposure: f64,
    pub conf_trend_alloc: f64,
    pub conf_inverse_potential: f64,
    pub capital_flow_acceleration: Option<f64>,
    pub universe_integrity: f64,
    pub trend_maturity: f64,
    pub stability_structural: f64,
    pub stability_temporal: f64,
    pub temporal_modifier: f64,
    pub integrity_multiplier: f64,
}

pub struct CapitalPosture {
    pub state_code: String,
    pub display_text: String,
    pub t_share_raw: f64,
    pub r_share_raw: f64,
    pub r_share_adj: f64,
    pub t_ratio_final: f64,
    pub r_ratio_final: f64,
    pub dominance_label: String,
}

impl GravityHealth {

    pub fn format_potential_energy(&self) -> String {
        let (label, intensity) = if self.global_potential_energy < 1.0 {
            ("Cold", "(安定 / 波动极低)")
        } else if self.global_potential_energy < 1.5 {
            ("Warm", "(蓄力 / 趋势健康但警惕波动)")
        } else if self.global_potential_energy < 2.0 {
            ("Hot", "(高张力 / 极端背离，波动即将来临)")
        } else {
            ("Critical", "(极端 / 风险极高，严防剧烈修正)")
        };
        format!("{:.2} {} {}", self.global_potential_energy, label, intensity)
    }

    /// Linear interpolation for smooth Potential Modifier
    /// 1.0 -> 0.7 (Discount)
    /// 1.5 -> 1.0 (Neutral)
    /// 2.0 -> 1.3 (Amplify)
    fn get_potential_mod(&self) -> f64 {
        let p = self.global_potential_energy;
        if p <= 1.0 { 0.7 }
        else if p >= 2.0 { 1.3 }
        else if p < 1.5 {
            0.7 + (p - 1.0) * (1.0 - 0.7) / (1.5 - 1.0)
        } else {
            1.0 + (p - 1.5) * (1.3 - 1.0) / (2.0 - 1.5)
        }
    }
    
    pub fn compute_capital_posture(&self) -> CapitalPosture {
        if self.trend_alloc_weight == 0.0 && self.reversion_alloc_weight == 0.0 {
            return CapitalPosture {
                state_code: "NULL".to_string(),
                display_text: "Transitional (Null)".to_string(),
                t_share_raw: 0.0,
                r_share_raw: 0.0,
                r_share_adj: 0.0,
                t_ratio_final: 0.0,
                r_ratio_final: 0.0,
                dominance_label: "Null".to_string(),
            };
        }
        
        let t_raw = self.trend_alloc_weight;
        let r_raw = self.reversion_alloc_weight;
        let total_raw = t_raw + r_raw;
        
        let t_share_raw = t_raw / total_raw;
        let r_share_raw = r_raw / total_raw;
        
        // 2. Continuous Potential Modifier
        let mod_factor = self.get_potential_mod();
        let r_share_adj = r_share_raw * mod_factor;
        
        // 3. Re-normalization
        let mod_total = t_share_raw + r_share_adj;
        let final_trend_ratio = t_share_raw / mod_total;
        let final_reversion_ratio = 1.0 - final_trend_ratio;
        
        let (state_code, display_text) = if final_trend_ratio >= 0.6 {
            ("TREND_DOMINANT", "Trend Dominant (趋势主导 / 复利优先，谨慎加速)")
        } else if final_trend_ratio <= 0.4 {
            ("REVERSION_DOMINANT", "Reversion Dominant (回归主导 / 分批部署，控制仓位)")
        } else {
            ("TRANSITIONAL", "Transitional (结构转换期 / 防御优先，等待确认)")
        };

        let margin = (final_trend_ratio - final_reversion_ratio).abs();
        let dominance_label = if margin < 0.2 {
            "Neutral"
        } else if margin < 0.5 {
            "Weak"
        } else if margin < 0.8 {
            "Strong"
        } else {
            "Absolute"
        };

        CapitalPosture {
            state_code: state_code.to_string(),
            display_text: display_text.to_string(),
            t_share_raw,
            r_share_raw,
            r_share_adj,
            t_ratio_final: final_trend_ratio,
            r_ratio_final: final_reversion_ratio,
            dominance_label: dominance_label.to_string(),
        }
    }

    pub fn get_interpretation(&self, posture: &CapitalPosture) -> String {
        match posture.state_code.as_str() {
            "TREND_DOMINANT" => {
                let mut interpretation = if self.global_gravity_strength > 0.0 {
                    "趋势强劲主导，延续概率较高。强者继续复利，避免频繁调仓，回撤即机会。".to_string()
                } else {
                    "趋势仍主导但引力减速。保持仓位但由于动能衰减，严禁追高。".to_string()
                };
                if self.stability_score < 0.2 {
                    interpretation.push_str(" ⚠️趋势尚处形成早期，稳定性低，已自动收缩仓位暴露度。");
                }
                interpretation
            },
            "REVERSION_DOMINANT" => {
                if self.global_potential_energy > 1.8 {
                    "极端背离导向。结构性超卖/超买严重，分批部署/防御而非追跌杀涨。".to_string()
                } else {
                    "均值回归主导。震荡格局，避免趋势交易逻辑，关注边缘突破。".to_string()
                }
            },
            "TRANSITIONAL" => {
                "结构转换期。引力方向不明联，防御优先，等待新体制确立。".to_string()
            },
            _ => "系统状态观测中。".to_string()
        }
    }

    pub fn get_regime_maturity(&self) -> (&'static str, &'static str) {
        if self.regime_age <= 10 { ("🟡 Newborn", "(Unstable)") }
        else if self.regime_age <= 60 { ("🟢 Healthy", "(Rising)") }
        else if self.regime_age <= 200 { ("🔵 Mature", "(Compounding)") }
        else { ("🟠 Aging", "(Inertial Risk)") }
    }

    pub fn get_action_bias(&self, posture: &CapitalPosture, buy_zone_empty: bool) -> String {
        let (_maturity_label, _) = self.get_regime_maturity();
        let risk_prefix = if self.regime_age <= 10 { " [High Newborn Risk]" } else { "" };
        let stability_prefix = if self.stability_score < 0.2 { " [LOW STABILITY]" } else { "" };
        
        let base = match posture.state_code.as_str() {
            "OPTIMAL" | "CRUISE" if buy_zone_empty => "HOLD",
            "OPTIMAL" | "CRUISE" | "TREND_DOMINANT" => "SELECTIVE ACCUMULATION (ON PULLBACKS)",
            "REVERSION_DOMINANT" if self.global_potential_energy > 1.8 => "ACCUMULATE (Contrarian)",
            _ => "DEFEND",
        };
        
        format!("{}{}{}", base, risk_prefix, stability_prefix)
    }
}

fn get_z_label(z: f64) -> &'static str {
    let abs_z = z.abs();
    if abs_z < 1.0 { "Neutral" }
    else if abs_z < 2.0 { "Strong" }
    else if abs_z < 3.0 { "Extreme" }
    else { "Panic" }
}

fn get_action_category(state: &str) -> &'static str {
    if state.contains("fear") && !state.contains("down") { "加仓区 (Buy)" }
    else if state.contains("pullback") { "加仓区 (Buy)" }
    else if state.contains("optimal") || state.contains("cruise") { "持有区 (Hold)" }
    else if state.contains("overheat") || state.contains("DEFEND") || state.contains("caution") || (state.contains("fear") && state.contains("down")) { "防御区 (Defend)" }
    else { "观察区 (Watch)" }
}

fn get_rank_priority(state: &str) -> usize {
    if state == "optimal" || state == "cruise" { 3 }
    else if state == "pullback" { 2 }
    else if state.contains("fear") && !state.contains("down") { 1 } // Opportunity fear
    else if state.contains("fear") && state.contains("down") { 5 } // Risk fear
    else if state.contains("overheat") || state.contains("DEFEND") { 6 }
    else if state == "REGIME_FORMING" { 99 }
    else { 4 } 
}

fn format_thermometer(value: f64, max: f64) -> String {
    let width = 10;
    let filled = ((value / max) * width as f64).round() as usize;
    let filled = filled.clamp(0, width);
    let mut bar = String::new();
    for _ in 0..filled { bar.push('█'); }
    for _ in filled..width { bar.push('░'); }
    
    let zone = if value > 1.8 { "PANIC/EXTREME" } else if value > 1.2 { "HIGH" } else if value > 0.8 { "MEDIUM" } else { "LOW" };
    format!("{} {:.2} / {:.1}\nZone: {}", bar, value, max, zone)
}

fn format_sigma(z: f64) -> String {
    let label = if z >= 0.0 { "above equilibrium" } else { "below equilibrium" };
    format!("{:.1}σ {}", z.abs(), label)
}

fn format_stability_bar(value: f64) -> String {
    let width = 10;
    let filled = (value * width as f64).round() as usize;
    let filled = filled.clamp(0, width);
    let mut bar = String::new();
    for _ in 0..filled { bar.push('▉'); } // Use a distinct block for stability
    for _ in filled..width { bar.push('░'); }
    
    let reliability = if value < 0.2 { "Low Reliability" } else { "Established" };
    format!("{} {:.0}% ({})", bar, value * 100.0, reliability)
}

fn get_position_guidance(state_code: &str) -> &str {
    match state_code {
        "optimal" => "Portfolio Allocation: Maintain Full Allocation (Stay Efficient)",
        "cruise" => "Portfolio Allocation: Maintain Full Allocation (Trend Follow)",
        "pullback" => "Portfolio Allocation: +10% (Buy)",
        "CAUTION" => "Neutral / Maintain",
        "DEFEND" | "fear_downtrend" => "Portfolio Allocation: 0-20% (Avoid/Cash)",
        "REGIME_FORMING" => "Allocation: N/A (Observe)",
        _ => "Neutral",
    }
}

fn get_final_order(gravity: &GravityHealth, posture: &CapitalPosture, buy_zone: &[String]) -> String {
    let mut orders = Vec::new();
    
    // Command 1: Exposure
    let floor = (gravity.recommended_exposure * 10.0).floor() * 10.0;
    let ceil = if floor >= 100.0 { 100.0 } else { floor + 10.0 };
    orders.push(format!("Maintain {:.0}-{:.0}% equity exposure.", floor, ceil));
    
    // Command 2: Deployment
    if !buy_zone.is_empty() {
        orders.push(format!("Deploy capital only into pullbacks ({}).", buy_zone.join(", ")));
    }
    
    // Command 3: Rules
    if posture.state_code.contains("Risk") || posture.state_code.contains("Panic") {
        orders.push("Avoid bottom-fishing downtrend assets.".to_string());
    } else {
        orders.push("Do not chase strength.".to_string());
    }
    
    orders.join("\n • ")
}

fn get_state_emoji(state_code: &str) -> &'static str {
    match state_code {
        "overheat_2" => "🔴",
        "overheat_1" => "🟠",
        "cruise" => "🟢",
        "optimal" => "💎",
        "pullback" => "🟡",
        "fear_1" => "🛡️",
        "fear_2" => "🆘",
        "CAUTION" => "⚠️",
        "DEFEND" => "🛑",
        "REGIME_FORMING" => "🌀",
        _ => "⚪",
    }
}

pub fn generate_reports(config: &AppConfig, snapshots: &[TickerSnapshot], gravity_health: &GravityHealth, yesterday_state: &str) -> Result<ReportResult> {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let posture = gravity_health.compute_capital_posture();
    
    let mut snapshots = snapshots.to_vec();
    // Sorting: Opportunity-First (Fear -> Pullback -> Optimal -> Cruise)
    snapshots.sort_by(|a, b| {
        let pa = get_rank_priority(&a.state_code);
        let pb = get_rank_priority(&b.state_code);
        if pa != pb {
            pa.cmp(&pb)
        } else {
            // Tie-break by |Z-score| descending for similar states
            let az = a.dev_z_score.unwrap_or(0.0).abs();
            let bz = b.dev_z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    let mut rows = Vec::new();
    let mut buy_zone = Vec::new();
    let mut watch_zone = Vec::new();
    let mut defend_zone = Vec::new();
    let mut hold_zone = Vec::new();

    for s in &snapshots {
        let cat = get_action_category(&s.state_code);
        match cat {
            "加仓区 (Buy)" => buy_zone.push(s.symbol.clone()),
            "观察区 (Watch)" => watch_zone.push(s.symbol.clone()),
            "防御区 (Defend)" => defend_zone.push(s.symbol.clone()),
            _ => hold_zone.push(s.symbol.clone()),
        }

        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗",
            TrendStatus::Down => "↘",
            TrendStatus::Flat => "→",
            TrendStatus::Unknown => "?",
        };
        let trend_combined = format!("{} ({}d)", trend_icon, s.trend_age);
        
        let _owner_dev_str = if let (true, Some(owner)) = (s.dog_price != 0.0, s.owner_ma) {
            let dev = (s.dog_price - owner) / owner * 100.0;
            format!("{:+.2}%", dev)
        } else {
            "-".to_string()
        };

        let owner_dev_val = s.owner_deviation_pct.unwrap_or(0.0);
        
        let emoji = get_state_emoji(&s.state_code);
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} {}", emoji, s.state_code, rc)
        } else {
            format!("{} {}", emoji, s.state_code)
        };

        let percentile_str = if s.validity == RegimeValidity::FormingEarly || s.validity == RegimeValidity::FormingLate {
            "".to_string()
        } else {
            s.deviation_percentile.map(|v| format!(" (罕见度: {:.0}%)", v)).unwrap_or_default()
        };
        
        let action_guidance = if s.validity == RegimeValidity::FormingEarly || s.validity == RegimeValidity::FormingLate {
            "Allocation: N/A (Observe)".to_string()
        } else {
            get_position_guidance(&s.state_code).to_string()
        };

        let owner_dev_str = if s.validity == RegimeValidity::FormingEarly {
            "Dist: N/A".to_string()
        } else {
            format!("Dist: {:+.1}%{}", owner_dev_val, percentile_str)
        };

        let strength_z_combined = if s.validity == RegimeValidity::FormingEarly {
            "Z-Score: N/A".to_string()
        } else {
            let z_val = s.dev_z_score.unwrap_or(0.0);
            format!("{} ({})", format_sigma(z_val), get_z_label(z_val))
        };

        rows.push(TerminalRow {
            symbol: s.symbol.clone(),
            trend: trend_combined,
            owner_dev: owner_dev_str, 
            strength_z: strength_z_combined,
            state: state_rc,
            action: action_guidance, 
        });
    }

    // Rankings
    let mut opportunities = snapshots.iter()
        .filter(|s| (s.state_code.contains("fear") && !s.state_code.contains("down")) || s.state_code.contains("pullback"))
        .collect::<Vec<_>>();
    opportunities.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let mut risks = snapshots.iter()
        .filter(|s| s.state_code.contains("overheat") || (s.state_code.contains("fear") && s.state_code.contains("down")) || s.state_code.contains("DEFEND"))
        .collect::<Vec<_>>();
    risks.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    // SPY Regime
    let spy_regime = snapshots.iter().find(|s| s.symbol == "SPY").map(|s| {
        if s.state_code.contains("optimal") || s.state_code.contains("cruise") { "Bull Stable" }
        else if s.state_code.contains("fear") || s.state_code.contains("pullback") { "Correction" }
        else if s.trend_status == TrendStatus::Down { "Bear / Crash" }
        else { "Uncertain" }
    }).unwrap_or("Unknown");

    // Previous state comparison
    let _yesterday_state = yesterday_state;
    
    let mut table = Table::new(rows);
    table.with(Style::modern());
    
    println!("Universe Composition: {} Universe | {} Valid | {} Forming ({}E / {}L)", 
        gravity_health.universe_count,
        gravity_health.total_count, 
        gravity_health.forming_early_count + gravity_health.forming_late_count,
        gravity_health.forming_early_count,
        gravity_health.forming_late_count
    );
    println!("{}", table);

    let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;
    let market_structure = format!("{} ({})", gravity_health.market_phase, spy_regime);

    println!("🌍 Macro Indicators (全域状态监测)");
    println!(" • CAPITAL STATE: {}", posture.display_text);
    
    // Exposure Range and Velocity
    let floor = (gravity_health.recommended_exposure * 10.0).floor() * 10.0;
    let ceil = if floor >= 100.0 { 100.0 } else { floor + 10.0 };
    let exp_delta_str = if let Some(prev) = gravity_health.prev_recommended_exposure {
        let diff = gravity_health.recommended_exposure - prev;
        if diff > 0.01 { "↑ Increasing" } else if diff < -0.01 { "↓ Decreasing" } else { "Stable" }
    } else { "New Baseline" };
    println!(" • Recommended Exposure: {:.0}-{:.0}%", floor, ceil);
    println!(" • Exposure Change: {}", exp_delta_str);

    let margin_delta = gravity_health.prev_dominance_margin.map(|p| dominance_margin - p);
    let margin_evolution = if let Some(d) = margin_delta {
        if d.abs() < 0.01 { "→ Stable" } else if d > 0.0 { "↗ Improving" } else { "↘ Weakening" }
    } else { "Baseline" };
    println!(" • Dominance Margin: {:+.2} ({}) {}", dominance_margin, posture.dominance_label, margin_evolution);
    
    println!(" • Market Structure: {}", market_structure);
    let (maturity_label, maturity_desc) = gravity_health.get_regime_maturity();
    println!(" • Regime Age: {} days {} {}", gravity_health.regime_age, maturity_label, maturity_desc);
    println!(" • Stability:   {}", format_stability_bar(gravity_health.stability_score));
    println!(" • Capital Flow Vector: {}", gravity_health.capital_flow_vector);
    println!(" • Action Bias: {}", gravity_health.get_action_bias(&posture, buy_zone.is_empty()));
    println!(" • GRAVITY POTENTIAL:\n{}", format_thermometer(gravity_health.global_potential_energy, 2.0));
    println!("\n> 📡 Interpretation: {}", gravity_health.get_interpretation(&posture));

    println!("\n🧭 今日执行指令 (Final Order)");
    println!(" • {}", get_final_order(gravity_health, &posture, &buy_zone));

    println!("\n🎯 Tactical Summary (今日行动要领)");
    println!(" • 加仓区: {}", buy_zone.join(" / "));
    println!(" • 观察区: {}", watch_zone.join(" / "));
    println!(" • 防御区: {}", defend_zone.join(" / "));
    println!(" • 持有区: {}", hold_zone.join(" / "));

    println!("\n🔥 Highest Opportunity");
    for (i, s) in opportunities.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        let bias = if z < 0.0 { "Mean Reversion ↑" } else { "Mean Reversion ↓" };
        let guidance = get_position_guidance(&s.state_code);
        println!(" {}. {} ({} / Bias: {} / {})", i+1, s.symbol, format_sigma(z), bias, guidance);
    }
    
    println!("\n☠️ Highest Risk");
    for (i, s) in risks.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        let bias = if z > 2.0 { "Overheat Correction ↓" } else { "Trend Breakdown ↓" };
        println!(" {}. {} ({} / Bias: {})", i+1, s.symbol, format_sigma(z), bias);
    }

    println!("\n🎯 Execution Radar (个股雷达)");
    println!(" > Sorted by: Extreme Opportunities (Fear) > Pullbacks > Optimal > Cruise > Risks/Stable\n");
    println!("{}", table);

    let md_content = generate_markdown(config, &snapshots, &date_str, gravity_health, &posture);
    let tg_html = generate_telegram_html(config, &snapshots, &date_str, gravity_health, &posture);

    if !config.output.save_to.is_empty() {
        fs::create_dir_all(&config.output.save_to)?;
        
        let json_path = Path::new(&config.output.save_to).join(format!("{}.json", date_str));
        let json_content = serde_json::to_string_pretty(&snapshots)?;
        fs::write(json_path, json_content)?;
        
        let md_path = Path::new(&config.output.save_to).join(format!("{}.md", date_str));
        fs::write(&md_path, &md_content)?;
        
        // --- Phase 26: Data Freshness Guard ---
        // If the data date is significantly older than today (e.g. > 3 days on a weekday),
        // it means the API hasn't updated yet. We still show the report but skip telemetry.
        let data_date = snapshots.first().map(|s| s.current_date).unwrap_or_else(|| Local::now().date_naive());
        let today = Local::now().date_naive();
        let days_diff = (today - data_date).num_days();
        
        let should_write_telemetry = if days_diff > 3 {
            println!("⚠️ [WARNING] Data date ({}) is too old. Skipping telemetry to avoid pollution.", data_date);
            false
        } else {
            true
        };

        if should_write_telemetry {
            // --- 📊 Telemetry System Heartbeat (V3 Ultimate Schema) ---
            let telemetry_path = Path::new(&config.output.save_to).join("telemetry.csv");
            let file_exists = telemetry_path.exists();
            
            let forming_count = gravity_health.forming_early_count + gravity_health.forming_late_count;
            let forming_weight = gravity_health.forming_early_weight + gravity_health.forming_late_weight;

            let up_share = if gravity_health.total_count == 0 { 0.0 } else { gravity_health.up_count as f64 / gravity_health.total_count as f64 };
            let flat_share = if gravity_health.total_count == 0 { 0.0 } else { gravity_health.flat_count as f64 / gravity_health.total_count as f64 };
            let regime_forming_share_c = if gravity_health.total_count == 0 { 0.0 } else { forming_count as f64 / gravity_health.total_count as f64 };
            let down_share = 1.0 - up_share - flat_share - regime_forming_share_c;
            
            let w_up_share = if gravity_health.total_weight <= 0.0 { 0.0 } else { gravity_health.up_weight / gravity_health.total_weight };
            let w_flat_share = if gravity_health.total_weight <= 0.0 { 0.0 } else { gravity_health.flat_weight / gravity_health.total_weight };
            let regime_forming_share_w = if gravity_health.total_weight <= 0.0 { 0.0 } else { forming_weight / gravity_health.total_weight };
            let w_down_share = 1.0 - w_up_share - w_flat_share - regime_forming_share_w;

            let timestamp = Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

            let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;

            // Ultimate Schema (24 Columns): 
            // date,timestamp,config_hash,state_code,state_text,gs,gp,t_raw,r_raw,r_adj,t_final,r_final,margin,exposure,size,c_up,c_flat,c_down,c_forming,w_up,w_flat,w_down,w_forming,integrity,accel
            let telemetry_row = format!("{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}\n",
                date_str,
                timestamp,
                gravity_health.config_hash,
                posture.state_code,
                posture.display_text,
                gravity_health.global_gravity_strength,
                gravity_health.global_potential_energy,
                posture.t_share_raw,
                posture.r_share_raw,
                posture.r_share_adj,
                posture.t_ratio_final,
                posture.r_ratio_final,
                dominance_margin,
                gravity_health.adjusted_exposure,
                snapshots.len(),
                up_share,
                flat_share,
                down_share,
                regime_forming_share_c,
                w_up_share,
                w_flat_share,
                w_down_share,
                regime_forming_share_w,
                gravity_health.universe_integrity,
                dominance_margin - gravity_health.prev_dominance_margin.unwrap_or(dominance_margin)
            );

            if !file_exists {
                let header = "date,timestamp,config_hash,state_code,state_text,gravity_strength,gravity_potential,t_share_raw,r_share_raw,r_share_adj,t_ratio_final,r_ratio_final,dominance_margin,exposure,watchlist_size,count_up_share,count_flat_share,count_down_share,count_forming_share,weight_up_share,weight_flat_share,weight_down_share,weight_forming_share,universe_integrity,capital_flow_acceleration\n";
                fs::write(&telemetry_path, format!("{}{}", header, telemetry_row))?;
            } else {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new().append(true).open(&telemetry_path)?;
                file.write_all(telemetry_row.as_bytes())?;
            }
        }
    }
    
    Ok(ReportResult {
        markdown: md_content,
        telegram_html: tg_html,
    })
}

fn generate_markdown(_config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth, posture: &CapitalPosture) -> String {
    let mut snapshots = snapshots.to_vec();
    snapshots.sort_by(|a, b| {
        let pa = get_rank_priority(&a.state_code);
        let pb = get_rank_priority(&b.state_code);
        if pa != pb {
            pa.cmp(&pb)
        } else {
            let az = a.dev_z_score.unwrap_or(0.0).abs();
            let bz = b.dev_z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    let mut buy_zone = Vec::new();
    let mut watch_zone = Vec::new();
    let mut defend_zone = Vec::new();
    let mut hold_zone = Vec::new();
    for s in &snapshots {
        let cat = get_action_category(&s.state_code);
        match cat {
            "加仓区 (Buy)" => buy_zone.push(s.symbol.clone()),
            "观察区 (Watch)" => watch_zone.push(s.symbol.clone()),
            "防御区 (Defend)" => defend_zone.push(s.symbol.clone()),
            _ => hold_zone.push(s.symbol.clone()),
        }
    }

    let mut opportunities = snapshots.iter()
        .filter(|s| (s.state_code.contains("fear") && !s.state_code.contains("down")) || s.state_code.contains("pullback"))
        .collect::<Vec<_>>();
    opportunities.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let mut risks = snapshots.iter()
        .filter(|s| s.state_code.contains("overheat") || (s.state_code.contains("fear") && s.state_code.contains("down")) || s.state_code.contains("DEFEND"))
        .collect::<Vec<_>>();
    risks.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let spy_regime = snapshots.iter().find(|s| s.symbol == "SPY").map(|s| {
        if s.state_code.contains("optimal") || s.state_code.contains("cruise") { "Bull Stable" }
        else if s.state_code.contains("fear") || s.state_code.contains("pullback") { "Correction" }
        else if s.trend_status == TrendStatus::Down { "Bear / Crash" }
        else { "Uncertain" }
    }).unwrap_or("Unknown");

    let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;
    let market_structure = format!("{} ({})", gravity.market_phase, spy_regime);

    let mut md = format!("# 🐕 Stock Sentinel 每日观测雷达\n📅 **日期**: {}\n\n", date_str);
    
    md.push_str("## 🌍 Macro Indicators (全域状态监测)\n");
    let integrity_pct = gravity.universe_integrity * 100.0;
    md.push_str(&format!("- **Universe Composition**: {} Universe | {} Valid | {} Forming ({}E / {}L)\n", 
        gravity.universe_count, gravity.total_count, 
        gravity.forming_early_count + gravity.forming_late_count,
        gravity.forming_early_count, gravity.forming_late_count));
    md.push_str(&format!("- **Universe Integrity**: {:.1}% Valid Ratio\n", integrity_pct));
    md.push_str(&format!("- **CAPITAL STATE**: {}\n", posture.display_text));
    
    let conf_delta = gravity.prev_system_confidence.map(|p| gravity.system_confidence - p);
    let conf_str = if let Some(d) = conf_delta {
        format!("{}% (Δ {:+.2}%)", gravity.system_confidence, d)
    } else {
        format!("{}%", gravity.system_confidence)
    };
    md.push_str(&format!("- **System Confidence**: {}\n", conf_str));
    md.push_str("  - *Confidence Source*:\n");
    md.push_str(&format!("    - Trend Strength (Max 50%): {:.1}%\n", gravity.conf_trend_alloc));
    md.push_str(&format!("    - Structural Stability (Max 50%): {:.1}%\n", gravity.conf_inverse_potential));
    
    // Delta for Dominance Margin
    let margin_delta = gravity.prev_dominance_margin.map(|p| dominance_margin - p);
    let margin_evolution = if let Some(d) = margin_delta {
        if d.abs() < 0.01 { "→ Stable" } else if d > 0.0 { "↗ Improving" } else { "↘ Weakening" }
    } else { "Baseline" };
    
    let accel_str = if let Some(acc) = gravity.capital_flow_acceleration {
        if acc.abs() < 0.02 { format!("{:+.2} (Stable)", acc.abs()) } // Force +0.00
        else if acc >= 0.02 { format!("{:+.2} (Strong)", acc) }
        else { format!("{:+.2} (Severe)", acc) }
    } else { "Baseline".to_string() };
    
    md.push_str(&format!("- **Momentum State**: {}\n", gravity.capital_flow_vector));
    md.push_str(&format!("- **Flow Acceleration**: {}\n", accel_str));
    md.push_str(&format!("- **Market Structure**: {}\n", market_structure));
    md.push_str(&format!("- **Dominance Margin**: {:+.2} ({} / {})\n", dominance_margin, posture.dominance_label, margin_evolution));
    
    // Exposure Scope
    let b_floor = (gravity.base_exposure * 10.0).floor() * 10.0;
    let b_ceil = if b_floor >= 100.0 { 100.0 } else { b_floor + 10.0 };
    let a_floor = (gravity.adjusted_exposure * 10.0).floor() * 10.0;
    let a_ceil = if a_floor >= 100.0 { 100.0 } else { a_floor + 10.0 };

    let exp_delta_str = if let Some(prev) = gravity.prev_recommended_exposure {
        let diff = gravity.adjusted_exposure - prev;
        if diff > 0.01 { "↑ Increasing" } else if diff < -0.01 { "↓ Decreasing" } else { "Stable" }
    } else { "New Baseline" };
    
    md.push_str("- **Exposure Calculation Breakdown**:\n");
    md.push_str(&format!("  - Base Exposure (Direction): {:.0}-{:.0}%\n", b_floor, b_ceil));
    md.push_str(&format!("  - Confidence Mod (Integrity Adj): × {:.2}\n", (gravity.system_confidence / 100.0) * gravity.integrity_multiplier));
    md.push_str(&format!("  - **Final Adjusted Exposure**: **{:.0}-{:.0}%**\n", a_floor, a_ceil));
    md.push_str(&format!("  - *Exposure Change vs Yesterday*: {}\n", exp_delta_str));
    let (maturity_label, maturity_desc) = gravity.get_regime_maturity();
    md.push_str(&format!("- **Regime Age**: {} days ({} {})\n", gravity.regime_age, maturity_label, maturity_desc));
    md.push_str(&format!("- **Trend Maturity**: {:.1}%\n", gravity.trend_maturity * 100.0));
    md.push_str(&format!("  └ Trend Quality (Temporal): {:.2}x\n", gravity.temporal_modifier));
    md.push_str(&format!("- **Stability**: {}\n", format_stability_bar(gravity.stability_score)));
    md.push_str(&format!("  ├ Structural: {:.1}%\n", gravity.stability_structural));
    md.push_str(&format!("  └ Temporal: {:.1}%\n", gravity.stability_temporal));
    
    // Phase 40: Trend Breadth Momentum
    let breadth_str = if let Some(prev_up) = gravity.prev_up_count {
        let diff = gravity.up_count as isize - prev_up as isize;
        format!("{:+} assets", diff)
    } else {
        "Baseline".to_string()
    };
    md.push_str(&format!("- **Trend Breadth Change**: {}\n", breadth_str));
    
    md.push_str(&format!("- **Action Bias**: **{}**\n", gravity.get_action_bias(posture, buy_zone.is_empty())));

    
    md.push_str(&format!("- **GRAVITY POTENTIAL**:\n```\n{}\n```\n({})\n\n", format_thermometer(gravity.global_potential_energy, 2.0), gravity.format_potential_energy()));

    md.push_str(&format!("> 📡 Interpretation: {}\n\n", gravity.get_interpretation(posture)));

    md.push_str("## 🧭 今日执行指令 (Final Order)\n");
    md.push_str(&format!("- **Command**:\n • {}\n\n", get_final_order(gravity, posture, &buy_zone)));

    md.push_str("## 🎯 Tactical Summary (今日行动要领)\n");
    md.push_str("### 今日行动摘要\n");
    md.push_str(&format!("- **加仓区 (Buy)**: {}\n", buy_zone.join(" / ")));
    md.push_str(&format!("- **观察区 (Watch)**: {}\n", watch_zone.join(" / ")));
    md.push_str(&format!("- **防御区 (Defend)**: {}\n", defend_zone.join(" / ")));
    md.push_str(&format!("- **持有区 (Hold)**: {}\n\n", hold_zone.join(" / ")));

    md.push_str("### 🔥 Highest Opportunity (均值回归弹性)\n");
    for (i, s) in opportunities.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        let elasticity = if z.abs() > 2.0 { "High" } else if z.abs() > 1.0 { "Medium" } else { "Normal" };
        let bias = if z < 0.0 { "Mean Reversion ↑" } else { "Mean Reversion ↓" };
        let guidance = get_position_guidance(&s.state_code);
        md.push_str(&format!("{}. **{}** ({} / {} / Elasticity: {} / Bias: {} / {})\n", i+1, s.symbol, s.state_code, format_sigma(z), elasticity, bias, guidance));
    }
    md.push_str("\n### ☠️ Highest Risk (系统性偏离风险)\n");
    for (i, s) in risks.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        let bias = if z > 2.0 { "Overheat Correction ↓" } else { "Trend Breakdown ↓" };
        md.push_str(&format!("{}. **{}** ({} / {} / Bias: {})\n", i+1, s.symbol, s.state_code, format_sigma(z), bias));
    }
    md.push_str("\n");

    md.push_str("| # | 代码 | Status (State) | Portfolio Allocation | Owner Dist | Sigma (σ) | Trend (Days) | Action Guidance |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
    
    for (idx, s) in snapshots.iter().enumerate() {
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️",
            TrendStatus::Down => "↘️",
            TrendStatus::Flat => "➡️",
            TrendStatus::Unknown => "❓",
        };
        let trend_combined = format!("{} ({}d)", trend_icon, s.trend_age);
        
        let z_val = s.dev_z_score.unwrap_or(0.0);
        let z_label = get_z_label(z_val);
        let strength_z_combined = if s.validity == RegimeValidity::FormingEarly {
            "N/A".to_string()
        } else {
            format!("{} ({})", format_sigma(z_val), z_label)
        };
        
        let owner_dev = s.owner_deviation_pct.unwrap_or(0.0);
        let percentile_str = if s.validity == RegimeValidity::FormingEarly || s.validity == RegimeValidity::FormingLate {
            "".to_string()
        } else {
            s.deviation_percentile.map(|v| format!(" (罕见度: {:.0}%)", v)).unwrap_or_default()
        };
        let recovery_str = if s.validity == RegimeValidity::FormingEarly {
            "N/A".to_string()
        } else {
            format!("{:+.1}%{}", owner_dev, percentile_str)
        };

        let action_guidance = if s.validity == RegimeValidity::FormingEarly || s.validity == RegimeValidity::FormingLate {
            "Allocation: N/A (Observe)".to_string()
        } else {
            get_position_guidance(&s.state_code).to_string()
        };

        let emoji = get_state_emoji(&s.state_code);
        let state_name = if let Some(rc) = &s.reason_code {
            format!("{} {} {}", emoji, s.state_code, rc)
        } else {
            format!("{} {}", emoji, s.state_code)
        };
        
        md.push_str(&format!("| {} | **{}** | {} | **{}** | {} | {} | {} | {} |\n",
            idx + 1,
            s.symbol,
            state_name,
            action_guidance,
            recovery_str,
            strength_z_combined,
            trend_combined,
            s.action_text
        ));
    }
    
    md
}

fn generate_telegram_html(_config: &AppConfig, snapshots_raw: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth, posture: &CapitalPosture) -> String {
    let mut snapshots = snapshots_raw.to_vec();
    snapshots.sort_by(|a, b| {
        let pa = get_rank_priority(&a.state_code);
        let pb = get_rank_priority(&b.state_code);
        if pa != pb {
            pa.cmp(&pb)
        } else {
            let az = a.dev_z_score.unwrap_or(0.0).abs();
            let bz = b.dev_z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    let mut buy_zone = Vec::new();
    let mut watch_zone = Vec::new();
    let mut defend_zone = Vec::new();
    let mut hold_zone = Vec::new();
    for s in &snapshots {
        let cat = get_action_category(&s.state_code);
        match cat {
            "加仓区 (Buy)" => buy_zone.push(s.symbol.clone()),
            "观察区 (Watch)" => watch_zone.push(s.symbol.clone()),
            "防御区 (Defend)" => defend_zone.push(s.symbol.clone()),
            _ => hold_zone.push(s.symbol.clone()),
        }
    }

    let mut opportunities = snapshots.iter()
        .filter(|s| (s.state_code.contains("fear") && !s.state_code.contains("down")) || s.state_code.contains("pullback"))
        .collect::<Vec<_>>();
    opportunities.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let mut risks = snapshots.iter()
        .filter(|s| s.state_code.contains("overheat") || (s.state_code.contains("fear") && s.state_code.contains("down")) || s.state_code.contains("DEFEND"))
        .collect::<Vec<_>>();
    risks.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let spy_regime = snapshots.iter().find(|s| s.symbol == "SPY").map(|s| {
        if s.state_code.contains("optimal") || s.state_code.contains("cruise") { "Bull Stable" }
        else if s.state_code.contains("fear") || s.state_code.contains("pullback") { "Correction" }
        else if s.trend_status == TrendStatus::Down { "Bear / Crash" }
        else { "Uncertain" }
    }).unwrap_or("Unknown");

    let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;
    let market_structure = format!("{} ({})", gravity.market_phase, spy_regime);

    let mut html = format!("🐕 <b>Stock Sentinel 每日观测雷达</b>\n📅 <b>日期:</b> {}\n\n", date_str);

    html.push_str("<b>🌍 Macro Indicators</b>\n");
    let integrity_pct = gravity.universe_integrity * 100.0;
    html.push_str(&format!(" • Universe Composition: {} Universe | {} Valid | {} Forming ({}E/{}L)\n", 
        gravity.universe_count, gravity.total_count, 
        gravity.forming_early_count + gravity.forming_late_count,
        gravity.forming_early_count, gravity.forming_late_count));
    html.push_str(&format!(" • Universe Integrity: <code>{:.1}%</code> Valid Ratio\n", integrity_pct));
    html.push_str(&format!(" • CAPITAL STATE: <code>{}</code>\n", escape_html(&posture.display_text)));
    
    let conf_delta = gravity.prev_system_confidence.map(|p| gravity.system_confidence - p);
    let conf_str = if let Some(d) = conf_delta {
        format!("<code>{}%</code> (Δ <code>{:+.2}%</code>)", gravity.system_confidence, d)
    } else {
        format!("<code>{}%</code>", gravity.system_confidence)
    };
    html.push_str(&format!(" • System Confidence: {}\n", conf_str));
    html.push_str(&format!("   ├ Trend Strength: <code>{:.1}%</code>\n", gravity.conf_trend_alloc));
    html.push_str(&format!("   └ Structural Stability: <code>{:.1}%</code>\n", gravity.conf_inverse_potential));
    let margin_delta = gravity.prev_dominance_margin.map(|p| dominance_margin - p);
    let margin_evolution = if let Some(d) = margin_delta {
        if d.abs() < 0.01 { "→ Stable" } else if d > 0.0 { "↗ Improving" } else { "↘ Weakening" }
    } else { "Baseline" };
    let accel_str = if let Some(acc) = gravity.capital_flow_acceleration {
        if acc.abs() < 0.02 { format!("<code>{:+.2}</code> (Stable)", acc.abs()) } // Force +0.00
        else if acc >= 0.02 { format!("<code>{:+.2}</code> (Strong)", acc) }
        else { format!("<code>{:+.2}</code> (Severe)", acc) }
    } else { "<code>Baseline</code>".to_string() };
    
    html.push_str(&format!(" • Momentum State: <code>{}</code>\n", escape_html(&gravity.capital_flow_vector)));
    html.push_str(&format!(" • Flow Acceleration: {}\n", accel_str));
    html.push_str(&format!(" • Market Structure: <code>{}</code>\n", escape_html(&market_structure)));
    html.push_str(&format!(" • Dominance Margin: <code>{:+.2}</code> ({})\n", dominance_margin, escape_html(margin_evolution)));
    
    let b_floor = (gravity.base_exposure * 10.0).floor() * 10.0;
    let b_ceil = if b_floor >= 100.0 { 100.0 } else { b_floor + 10.0 };
    let a_floor = (gravity.adjusted_exposure * 10.0).floor() * 10.0;
    let a_ceil = if a_floor >= 100.0 { 100.0 } else { a_floor + 10.0 };
    
    html.push_str("<b> • Exposure Calculation Breakdown:</b>\n");
    html.push_str(&format!("   ├ Base (Direction): <b>{:.0}-{:.0}%</b>\n", b_floor, b_ceil));
    html.push_str(&format!("   ├ Confidence Mod (Integrity Adj): × {:.2}\n", (gravity.system_confidence / 100.0) * gravity.integrity_multiplier));
    html.push_str(&format!("   └ <b>Final Adjusted: {:.0}-{:.0}%</b>\n", a_floor, a_ceil));
    
    let exp_delta_str = if let Some(prev) = gravity.prev_recommended_exposure {
        let diff = gravity.adjusted_exposure - prev;
        if diff > 0.01 { "↑ Increasing" } else if diff < -0.01 { "↓ Decreasing" } else { "Stable" }
    } else { "New Baseline" };
    
    let (maturity_label, _) = gravity.get_regime_maturity();
    html.push_str(&format!(" • Exposure Change: <code>{}</code>\n", exp_delta_str));
    html.push_str(&format!(" • Regime Age: <code>{} days</code> ({})\n", gravity.regime_age, maturity_label));
    html.push_str(&format!(" • Trend Maturity: <code>{:.1}%</code>\n", gravity.trend_maturity * 100.0));
    html.push_str(&format!("   └ Trend Quality (Temporal): <code>{:.2}x</code>\n", gravity.temporal_modifier));
    html.push_str(&format!(" • Stability: <code>{}</code>\n", format_stability_bar(gravity.stability_score)));
    html.push_str(&format!("   ├ Structural: <code>{:.1}%</code>\n", gravity.stability_structural));
    html.push_str(&format!("   └ Temporal: <code>{:.1}%</code>\n", gravity.stability_temporal));
    html.push_str(&format!(" • Action Bias: <b>{}</b>\n", escape_html(&gravity.get_action_bias(posture, buy_zone.is_empty()))));
    
    html.push_str(&format!(" • GRAVITY POTENTIAL: <code>{}</code>\n\n", format_thermometer(gravity.global_potential_energy, 2.0).replace("\n", " | ")));

    html.push_str(&format!("<i>📡 Interpretation: {}</i>\n\n", escape_html(&gravity.get_interpretation(posture))));

    html.push_str("<b>🧭 今日执行指令 (Final Order)</b>\n");
    html.push_str(&format!(" • <code>{}</code>\n\n", escape_html(&get_final_order(gravity, posture, &buy_zone).replace("\n", ""))));

    html.push_str("<b>🎯 Tactical Summary</b>\n");
    html.push_str(&format!(" • 加仓区: <code>{}</code>\n", escape_html(&buy_zone.join(" / "))));
    html.push_str(&format!(" • 观察区: <code>{}</code>\n", escape_html(&watch_zone.join(" / "))));
    html.push_str(&format!(" • 防御区: <code>{}</code>\n", escape_html(&defend_zone.join(" / "))));
    html.push_str(&format!(" • 持有区: <code>{}</code>\n\n", escape_html(&hold_zone.join(" / "))));

    html.push_str("<b>🔥 Highest Opportunity</b>\n");
    for (i, s) in opportunities.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        html.push_str(&format!("{}. <b>{}</b> (<code>{}</code> / <code>{}</code>)\n", i+1, s.symbol, escape_html(&s.state_code), format_sigma(z)));
    }
    html.push_str("\n<b>☠️ Highest Risk</b>\n");
    for (i, s) in risks.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        html.push_str(&format!("{}. <b>{}</b> (<code>{}</code> / <code>{}</code>)\n", i+1, s.symbol, escape_html(&s.state_code), format_sigma(z)));
    }
    html.push_str("\n");
    
    html.push_str("<b>🎯 个股雷达 (Execution Radar)</b>\n");
    html.push_str("<i>ℹ️ Sorted by: Extreme Opportunities (Fear) > Pullbacks > Optimal > Cruise > Risks/Stable</i>\n\n");

    for (idx, s) in snapshots.iter().enumerate() {
        let _trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️",
            TrendStatus::Down => "↘️",
            _ => "➡️",
        };
        let z_val = s.dev_z_score.unwrap_or(0.0);
        let owner_dev_val = s.owner_deviation_pct.unwrap_or(0.0);
        let percentile_str = if s.validity == RegimeValidity::FormingEarly || s.validity == RegimeValidity::FormingLate {
            "".to_string()
        } else {
            s.deviation_percentile.map(|v| format!(" (罕见度: {:.0}%)", v)).unwrap_or_default()
        };
        let emoji = get_state_emoji(&s.state_code);
        
        let action_guidance = if s.validity == RegimeValidity::FormingEarly || s.validity == RegimeValidity::FormingLate {
            "Allocation: N/A (Observe)".to_string()
        } else {
            get_position_guidance(&s.state_code).to_string()
        };

        let owner_dev_str = if s.validity == RegimeValidity::FormingEarly {
            "Dist: N/A".to_string()
        } else {
            format!("Owner Dist: {:+.1}%{}", owner_dev_val, percentile_str)
        };

        let strength_z_combined = if s.validity == RegimeValidity::FormingEarly {
            "Z-Score: N/A".to_string()
        } else {
            format!("{}", format_sigma(z_val))
        };

        let state_name = if let Some(rc) = &s.reason_code {
            format!("{} {} {}", emoji, escape_html(&s.state_code), escape_html(rc))
        } else {
            format!("{} {}", emoji, escape_html(&s.state_code))
        };

        html.push_str(&format!("{}. <b>{}</b> | {} | <code>{}</code>\n", idx+1, s.symbol, state_name, escape_html(&action_guidance)));
        html.push_str(&format!("└ <code>{}</code> | {} | {}\n", escape_html(&owner_dev_str), strength_z_combined, escape_html(&s.action_text)));

        html.push_str("\n");
    }
    
    html
}
