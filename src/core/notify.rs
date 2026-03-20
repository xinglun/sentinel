use crate::config::TelegramConfig;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
struct TelegramPayload {
    chat_id: String,
    text: String,
    parse_mode: String,
    disable_web_page_preview: bool,
}

#[allow(dead_code)]
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub async fn send_telegram_message(config: &TelegramConfig, markdown_text: &str) -> Result<()> {
    if !config.enabled || config.bot_token.is_empty() || config.chat_id.is_empty() {
        return Ok(());
    }

    // Telegram's MarkdownV2 requires escaping specific characters
    // For simplicity of this MVP, we will use 'Markdown' (v1) which is more forgiving
    // but we still need to be careful. If HTML is preferred, let me know.
    // We'll use the basic `Markdown` parse_mode for now.

    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        config.bot_token
    );
    let payload = TelegramPayload {
        chat_id: config.chat_id.clone(),
        text: markdown_text.to_string(),
        parse_mode: "HTML".to_string(),
        disable_web_page_preview: true,
    };

    let client = Client::new();
    let res = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send Telegram request: {}", e))?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(anyhow!("Telegram API returned an error: {}", err_text));
    }

    println!(
        "✅ Telegram notification successfully sent to: {}",
        config.chat_id
    );
    Ok(())
}
