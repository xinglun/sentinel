use crate::config::AppConfig;
use crate::engine::{TickerSnapshot, TrendStatus};
use anyhow::Result;
use chrono::Local;
use std::fs;
use std::path::Path;
use tabled::{Table, Tabled};
use tabled::settings::Style;

#[derive(Tabled)]
struct TerminalRow {
    #[tabled(rename = "銘柄")]
    symbol: String,
    #[tabled(rename = "トレンド")]
    trend: String,
    #[tabled(rename = "重力強度")]
    strength_pct: String,
    #[tabled(rename = "Z-Score(曲率)")]
    z_score_curv: String,
    #[tabled(rename = "乖離率 %")]
    dev_pct: String,
    #[tabled(rename = "状態 (置信度)")]
    state: String,
    #[tabled(rename = "行動建議")]
    action: String,
}

pub struct ReportResult {
    #[allow(dead_code)]
    pub markdown: String,
    pub telegram_html: String,
}

pub struct GravityHealth {
    pub up_count: usize,
    pub down_count: usize,
    pub total_count: usize,
    pub up_weight: f64,
    pub down_weight: f64,
    pub total_weight: f64,
    pub global_gravity_strength: f64,
    pub global_potential_energy: f64,
    pub trend_alloc_weight: f64,
    pub reversion_alloc_weight: f64,
}

impl GravityHealth {
    pub fn format_count_health(&self) -> String {
        if self.total_count == 0 {
            return "0% UP / 0% DOWN".to_string();
        }
        let up_pct = (self.up_count as f64 / self.total_count as f64 * 100.0).round() as usize;
        let down_pct = (self.down_count as f64 / self.total_count as f64 * 100.0).round() as usize;
        format!("{}% UP / {}% DOWN", up_pct, down_pct)
    }

    pub fn format_weight_health(&self) -> String {
        if self.total_weight <= 0.0 {
            return "0% UP / 0% DOWN".to_string();
        }
        let up_pct = (self.up_weight / self.total_weight * 100.0).round() as usize;
        let down_pct = (self.down_weight / self.total_weight * 100.0).round() as usize;
        format!("{}% UP / {}% DOWN", up_pct, down_pct)
    }

    pub fn format_potential_energy(&self) -> String {
        if self.global_potential_energy < 1.0 {
            format!("{:.2} (LOW / 安定)", self.global_potential_energy)
        } else if self.global_potential_energy < 2.0 {
            format!("{:.2} (MEDIUM / 蓄力)", self.global_potential_energy)
        } else {
            format!("{:.2} (HIGH / 高张力)", self.global_potential_energy)
        }
    }
    
    pub fn format_capital_state(&self) -> String {
        if self.trend_alloc_weight == 0.0 && self.reversion_alloc_weight == 0.0 {
            return "Transitional (Null)".to_string();
        }
        
        // 1. Normalize the raw values so they are on a comparable scale
        let t_raw = self.trend_alloc_weight;
        let r_raw = self.reversion_alloc_weight;
        
        let total_raw = t_raw + r_raw;
        let t_share = t_raw / total_raw;
        let mut r_share = r_raw / total_raw;
        
        // 2. Potential Modifier
        // Reversion requires actual macroeconomic tension (Potential) to be valid.
        // If tension is low, Reversion signals are likely noise. If tension is high, they are validated.
        if self.global_potential_energy < 1.2 {
            r_share *= 0.7; // Discount Reversion weight
        } else if self.global_potential_energy > 1.8 {
            r_share *= 1.3; // Amplify Reversion weight
        }
        
        // Recalculate ratio after modifier
        let mod_total = t_share + r_share;
        let final_trend_ratio = t_share / mod_total;
        
        if final_trend_ratio >= 0.6 {
            "Trend Dominant (趋势主导)".to_string()
        } else if final_trend_ratio <= 0.4 {
            "Reversion Dominant (回归主导)".to_string()
        } else {
            "Transitional (结构转换期)".to_string()
        }
    }
}

pub fn generate_reports(config: &AppConfig, snapshots: &[TickerSnapshot], gravity_health: &GravityHealth) -> Result<ReportResult> {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    
    let mut rows = Vec::new();
    for s in snapshots {
        let trend_str = match s.trend_status {
            TrendStatus::Up => "上昇 ↗",
            TrendStatus::Down => "下落 ↘",
            TrendStatus::Flat => "横ばい →",
            TrendStatus::Unknown => "?",
        };
        
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        let strength_str = s.owner_ma_slope_pct.map(|v| format!("{:+.2}%", v)).unwrap_or_else(|| "-".to_string());
        let z_score_str = s.dev_z_score.map(|v| format!("{:+.1}", v)).unwrap_or_else(|| "-".to_string());
        let curv_str = s.curvature.map(|v| if v > 0.0 { "拐点↗" } else { "下沉↘" }).unwrap_or("-");
        
        let z_curv_combined = format!("{} ({})", z_score_str, curv_str);
        
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} ({}%)", s.state_code, rc, s.confidence_score)
        } else {
            format!("{} ({}%)", s.state_code, s.confidence_score)
        };
        
        rows.push(TerminalRow {
            symbol: s.symbol.clone(),
            trend: trend_str.to_string(),
            strength_pct: strength_str,
            z_score_curv: z_curv_combined,
            dev_pct: dev_str,
            state: state_rc,
            action: s.action_text.clone(),
        });
    }
    
    let weight_kind_label = config.output.weight_kind.clone().unwrap_or_else(|| "Portfolio".to_string());
    
    let mut table = Table::new(rows);
    table.with(Style::modern());
    println!("\n🌍 CAPITAL STATE: {}", gravity_health.format_capital_state());
    println!("🌍 GRAVITY (Count): {}", gravity_health.format_count_health());
    println!("🌍 GRAVITY (Weight/{}): {}", weight_kind_label, gravity_health.format_weight_health());
    println!("🌍 GRAVITY (Strength/{}): {:+.2}%", weight_kind_label, gravity_health.global_gravity_strength);
    println!("🌍 GRAVITY POTENTIAL: {}", gravity_health.format_potential_energy());
    println!("{}", table);

    let md_content = generate_markdown(config, snapshots, &date_str, gravity_health);
    let tg_html = generate_telegram_html(config, snapshots, &date_str, gravity_health);

    if !config.output.save_to.is_empty() {
        fs::create_dir_all(&config.output.save_to)?;
        
        let json_path = Path::new(&config.output.save_to).join(format!("{}.json", date_str));
        let json_content = serde_json::to_string_pretty(snapshots)?;
        fs::write(json_path, json_content)?;
        
        let md_path = Path::new(&config.output.save_to).join(format!("{}.md", date_str));
        fs::write(md_path, &md_content)?;
        
        // --- 📊 Telemetry System Heartbeat ---
        let telemetry_path = Path::new(&config.output.save_to).join("telemetry.csv");
        let file_exists = telemetry_path.exists();
        
        let t_raw = gravity_health.trend_alloc_weight;
        let r_raw = gravity_health.reversion_alloc_weight;
        let total_raw = if t_raw + r_raw == 0.0 { 1.0 } else { t_raw + r_raw };
        let t_share = t_raw / total_raw;
        let r_share = r_raw / total_raw;
        
        let up_share = if gravity_health.total_count == 0 { 0.0 } else { gravity_health.up_count as f64 / gravity_health.total_count as f64 };
        let weight_up_share = if gravity_health.total_weight == 0.0 { 0.0 } else { gravity_health.up_weight / gravity_health.total_weight };

        let telemetry_row = format!("{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}\n",
            date_str,
            gravity_health.format_capital_state(),
            gravity_health.global_gravity_strength,
            gravity_health.global_potential_energy,
            t_share,
            r_share,
            up_share,
            weight_up_share
        );

        if !file_exists {
            let header = "date,capital_state,gravity_strength,gravity_potential,trend_share,reversion_share,count_up_share,weight_up_share\n";
            fs::write(&telemetry_path, format!("{}{}", header, telemetry_row))?;
        } else {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().append(true).open(&telemetry_path)?;
            file.write_all(telemetry_row.as_bytes())?;
        }
    }
    
    Ok(ReportResult {
        markdown: md_content,
        telegram_html: tg_html,
    })
}

fn generate_markdown(config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth) -> String {
    let weight_kind_label = config.output.weight_kind.clone().unwrap_or_else(|| "Portfolio".to_string());
    let mut md = format!("# 🐕 Stock Sentinel 每日観測レーダー\n📅 **日付**: {}\n🌍 **CAPITAL STATE**: {}\n🌍 **GRAVITY (Count)**: {}\n🌍 **GRAVITY (Weight/{})**: {}\n🌍 **GRAVITY (Strength/{})**: {:+.2}%\n🌍 **GRAVITY POTENTIAL**: {}\n\n", 
        date_str, 
        gravity.format_capital_state(),
        gravity.format_count_health(),
        weight_kind_label.clone(),
        gravity.format_weight_health(),
        weight_kind_label,
        gravity.global_gravity_strength,
        gravity.format_potential_energy()
    );
    
    md.push_str("### 🎯 個別銘柄レーダー\n\n");
    md.push_str("| 銘柄 | トレンド | 強度 | Z-Score (曲率) | 乖離率 | 状態 (置信度) | 行動建議 |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
    
    for s in snapshots {
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        
        // Physics metrics
        let strength_str = s.owner_ma_slope_pct.map(|v| format!("{:+.2}%", v)).unwrap_or_else(|| "-".to_string());
        let z_score_str = s.dev_z_score.map(|v| format!("{:+.1}", v)).unwrap_or_else(|| "-".to_string());
        let curv_str = s.curvature.map(|v| if v > 0.0 { "拐点↗" } else { "下沉↘" }).unwrap_or("-");
        
        let z_curv_combined = format!("{} ({})", z_score_str, curv_str);
        
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️ 上昇",
            TrendStatus::Down => "↘️ 下落",
            TrendStatus::Flat => "➡️ 横ばい",
            TrendStatus::Unknown => "❓ 未知",
        };
        
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} ({}%)", s.state_code, rc, s.confidence_score)
        } else {
            format!("{} ({}%)", s.state_code, s.confidence_score)
        };
        
        md.push_str(&format!("| **{}** | `{}` | {} | {} | **{}** | {} | {} |\n",
            s.symbol,
            trend_icon,
            strength_str,
            z_curv_combined,
            dev_str,
            state_rc,
            s.action_text
        ));
    }
    
    md
}

fn generate_telegram_html(config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth) -> String {
    let weight_kind_label = config.output.weight_kind.clone().unwrap_or_else(|| "Portfolio".to_string());
    let mut html = format!("🐕 <b>Stock Sentinel レーダー</b>\n📅 <b>日付:</b> {}\n🌍 <b>GRAVITY (Count):</b> {}\n🌍 <b>GRAVITY (Weight/{}):</b> {}\n🌍 <b>GRAVITY (Strength/{}):</b> {:+.2}%\n🌍 <b>GRAVITY POTENTIAL:</b> {}\n\n", 
        date_str, 
        gravity.format_count_health(),
        weight_kind_label.clone(),
        gravity.format_weight_health(),
        weight_kind_label,
        gravity.global_gravity_strength,
        gravity.format_potential_energy()
    );
    
    for s in snapshots {
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        let _leash_str = s.leash_ma.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        
        let _trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️ 上昇",
            TrendStatus::Down => "↘️ 下落",
            TrendStatus::Flat => "➡️ 横ばい",
            TrendStatus::Unknown => "❓ 未知",
        };
        
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} ({}%)", s.state_code, rc, s.confidence_score)
        } else {
            format!("{} ({}%)", s.state_code, s.confidence_score)
        };
        
        let strength_str = s.owner_ma_slope_pct.map(|v| format!("{:+.2}%", v)).unwrap_or_else(|| "-".to_string());
        let z_score_str = s.dev_z_score.map(|v| format!("{:+.1}", v)).unwrap_or_else(|| "-".to_string());
        
        html.push_str(&format!("<b>{}</b> <code>${:.2}</code> (Dev: <code>{}</code>)\n", s.symbol, s.dog_price, dev_str));
        html.push_str(&format!(" • <b>状態(置信度):</b> {}\n", state_rc));
        html.push_str(&format!(" • <b>物理:</b> 強度 <code>{}</code> | Z-Score <code>{}</code>\n", strength_str, z_score_str));
        html.push_str(&format!(" • <b>建議:</b> {}\n\n", s.action_text));
    }
    
    html
}
