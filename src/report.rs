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
    #[tabled(rename = "Ticker")]
    symbol: String,
    #[tabled(rename = "Trend")]
    trend: String,
    #[tabled(rename = "Leash(MA)")]
    leash: String,
    #[tabled(rename = "Dog(Price)")]
    dog: String,
    #[tabled(rename = "Dev %")]
    dev_pct: String,
    #[tabled(rename = "State")]
    state: String,
    #[tabled(rename = "Action")]
    action: String,
}

pub struct ReportResult {
    pub markdown: String,
    pub telegram_html: String,
}

pub fn generate_reports(config: &AppConfig, snapshots: &[TickerSnapshot]) -> Result<ReportResult> {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    
    let mut rows = Vec::new();
    for s in snapshots {
        let trend_str = match s.trend_status {
            TrendStatus::Up => "Up ↗",
            TrendStatus::Down => "Down ↘",
            TrendStatus::Flat => "Flat →",
            TrendStatus::Unknown => "?",
        };
        
        let leash_str = s.leash_ma.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        
        rows.push(TerminalRow {
            symbol: s.symbol.clone(),
            trend: trend_str.to_string(),
            leash: leash_str,
            dog: format!("{:.2}", s.dog_price),
            dev_pct: dev_str,
            state: s.state_code.clone(),
            action: s.action_text.clone(),
        });
    }
    
    let mut table = Table::new(rows);
    table.with(Style::modern());
    println!("\n{}", table);

    let md_content = generate_markdown(config, snapshots, &date_str);
    let tg_html = generate_telegram_html(config, snapshots, &date_str);

    if !config.output.save_to.is_empty() {
        fs::create_dir_all(&config.output.save_to)?;
        
        let json_path = Path::new(&config.output.save_to).join(format!("{}.json", date_str));
        let json_content = serde_json::to_string_pretty(snapshots)?;
        fs::write(json_path, json_content)?;
        
        let md_path = Path::new(&config.output.save_to).join(format!("{}.md", date_str));
        fs::write(md_path, &md_content)?;
    }
    
    Ok(ReportResult {
        markdown: md_content,
        telegram_html: tg_html,
    })
}

fn generate_markdown(_config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str) -> String {
    let mut md = format!("# 🐕 Stock Sentinel 每日观测雷达\n📅 **日期**: {}\n\n", date_str);
    
    md.push_str("### 🎯 个股雷达谱\n\n");
    md.push_str("| 标的 | 人(Owner) | 绳(Leash) | 狗(最新价) | 乖离率(Dev) | 状态 | 行动建议 |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
    
    for s in snapshots {
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        let leash_str = s.leash_ma.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️ 向上",
            TrendStatus::Down => "↘️ 向下",
            TrendStatus::Flat => "➡️ 走平",
            TrendStatus::Unknown => "❓ 未知",
        };
        
        md.push_str(&format!("| **{}** | `{}` | ${} | **${:.2}** | **{}** | {} | {} |\n",
            s.symbol,
            trend_icon,
            leash_str,
            s.dog_price,
            dev_str,
            s.state_code,
            s.action_text
        ));
    }
    
    md
}

fn generate_telegram_html(_config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str) -> String {
    let mut html = format!("🐕 <b>Stock Sentinel 雷达</b>\n📅 <b>日期:</b> {}\n\n", date_str);
    
    for s in snapshots {
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        let leash_str = s.leash_ma.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️ 向上",
            TrendStatus::Down => "↘️ 向下",
            TrendStatus::Flat => "➡️ 走平",
            TrendStatus::Unknown => "❓ 未知",
        };
        
        html.push_str(&format!("<b>{}</b> <code>${:.2}</code> (Dev: <code>{}</code>)\n", s.symbol, s.dog_price, dev_str));
        html.push_str(&format!(" • <b>状态:</b> {}\n", s.state_code));
        html.push_str(&format!(" • <b>建议:</b> {}\n", s.action_text));
        html.push_str(&format!(" • 人: {} | 绳: <code>{}</code>\n\n", trend_icon, leash_str));
    }
    
    html
}
