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

fn sanitize_telegram_html(message_text: &str) -> String {
    const OPEN_ITALIC: &str = "__TG_OPEN_I__";
    const CLOSE_ITALIC: &str = "__TG_CLOSE_I__";

    escape_html(
        &message_text
            .replace("<i>", OPEN_ITALIC)
            .replace("</i>", CLOSE_ITALIC),
    )
    .replace(OPEN_ITALIC, "<i>")
    .replace(CLOSE_ITALIC, "</i>")
}

fn build_payload(config: &TelegramConfig, message_text: &str) -> TelegramPayload {
    TelegramPayload {
        chat_id: config.chat_id.clone(),
        text: sanitize_telegram_html(message_text),
        parse_mode: "HTML".to_string(),
        disable_web_page_preview: true,
    }
}

pub async fn send_telegram_message(config: &TelegramConfig, message_text: &str) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }
    if config.bot_token.is_empty() || config.chat_id.is_empty() {
        return Err(anyhow!(
            "Telegram is enabled but bot_token/chat_id is missing"
        ));
    }

    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        config.bot_token
    );
    let payload = build_payload(config, message_text);

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

#[cfg(test)]
mod tests {
    use super::{build_payload, sanitize_telegram_html};
    use crate::config::TelegramConfig;

    #[test]
    fn telegram_payload_uses_html_parse_mode() {
        let cfg = TelegramConfig {
            enabled: true,
            bot_token: "token".to_string(),
            chat_id: "chat".to_string(),
        };

        let payload = build_payload(&cfg, "## heading\n\n**body**");
        let json = serde_json::to_value(payload).unwrap();

        assert_eq!(json["chat_id"], "chat");
        assert_eq!(json["text"], "## heading\n\n**body**");
        assert_eq!(json["parse_mode"], "HTML");
    }

    #[test]
    fn telegram_payload_escapes_raw_angle_brackets_but_preserves_italic_tags() {
        let sanitized = sanitize_telegram_html("stability < 10.0\n<i>setup</i>\ncontinuity < 3d");

        assert!(sanitized.contains("stability &lt; 10.0"));
        assert!(sanitized.contains("continuity &lt; 3d"));
        assert!(sanitized.contains("<i>setup</i>"));
        assert!(!sanitized.contains("< 10.0"));
    }
}
