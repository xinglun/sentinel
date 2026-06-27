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

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn sanitize_telegram_html(message_text: &str) -> String {
    const OPEN_BOLD: &str = "__TG_OPEN_B__";
    const CLOSE_BOLD: &str = "__TG_CLOSE_B__";
    const OPEN_ITALIC: &str = "__TG_OPEN_I__";
    const CLOSE_ITALIC: &str = "__TG_CLOSE_I__";

    escape_html(
        &message_text
            .replace("<b>", OPEN_BOLD)
            .replace("</b>", CLOSE_BOLD)
            .replace("<i>", OPEN_ITALIC)
            .replace("</i>", CLOSE_ITALIC),
    )
    .replace(OPEN_BOLD, "<b>")
    .replace(CLOSE_BOLD, "</b>")
    .replace(OPEN_ITALIC, "<i>")
    .replace(CLOSE_ITALIC, "</i>")
}

#[cfg(test)]
fn build_payload(config: &TelegramConfig, message_text: &str) -> TelegramPayload {
    TelegramPayload {
        chat_id: config.chat_id.clone(),
        text: sanitize_telegram_html(message_text),
        parse_mode: "HTML".to_string(),
        disable_web_page_preview: true,
    }
}

fn chunk_telegram_html_message(message_text: &str, max_len: usize) -> Vec<String> {
    if message_text.len() <= max_len {
        return vec![message_text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in message_text.split('\n') {
        let candidate_len = if current.is_empty() {
            line.len()
        } else {
            current.len() + 1 + line.len()
        };

        if candidate_len <= max_len {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
            continue;
        }

        if !current.is_empty() {
            chunks.push(current);
            current = String::new();
        }

        if line.len() <= max_len {
            current.push_str(line);
            continue;
        }

        let mut start = 0;
        while start < line.len() {
            let mut end = (start + max_len).min(line.len());
            while !line.is_char_boundary(end) {
                end -= 1;
            }
            if end == start {
                break;
            }
            chunks.push(line[start..end].to_string());
            start = end;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
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
    let sanitized = sanitize_telegram_html(message_text);
    let chunks = chunk_telegram_html_message(&sanitized, 3800);
    let client = Client::new();

    let total_chunks = chunks.len();
    for (index, chunk) in chunks.into_iter().enumerate() {
        let text = if total_chunks > 1 {
            format!("<i>Part {}/{}</i>\n\n{}", index + 1, total_chunks, chunk)
        } else {
            chunk
        };
        let payload = TelegramPayload {
            chat_id: config.chat_id.clone(),
            text,
            parse_mode: "HTML".to_string(),
            disable_web_page_preview: true,
        };

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
    }

    println!(
        "✅ Telegram notification successfully sent to: {}",
        config.chat_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_payload, chunk_telegram_html_message, escape_html, sanitize_telegram_html};
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

    #[test]
    fn telegram_payload_preserves_bold_tags_for_html_body() {
        let sanitized = sanitize_telegram_html("<b>headline</b>\nstability < 10.0");

        assert!(sanitized.contains("<b>headline</b>"));
        assert!(sanitized.contains("stability &lt; 10.0"));
    }

    #[test]
    fn escape_html_escapes_raw_markup() {
        assert_eq!(escape_html("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn telegram_chunking_splits_long_message() {
        let text = "a".repeat(9000);
        let chunks = chunk_telegram_html_message(&text, 3800);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.len() <= 3800));
        assert_eq!(chunks.concat().len(), 9000);
    }

    #[test]
    fn telegram_chunking_keeps_line_blocks_when_possible() {
        let text = format!(
            "{}\n{}\n{}",
            "a".repeat(2000),
            "b".repeat(2000),
            "c".repeat(2000)
        );
        let chunks = chunk_telegram_html_message(&text, 3800);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.len() <= 3800));
        assert_eq!(chunks.join("\n"), text);
    }

    #[test]
    fn telegram_chunk_prefix_fits_inside_api_limit() {
        let text = "a".repeat(7601);
        let chunks = chunk_telegram_html_message(&text, 3700);
        let total_chunks = chunks.len();
        let rendered: Vec<String> = chunks
            .into_iter()
            .enumerate()
            .map(|(index, chunk)| {
                format!("<i>Part {}/{}</i>\n\n{}", index + 1, total_chunks, chunk)
            })
            .collect();

        assert!(total_chunks > 1);
        assert!(rendered.iter().all(|chunk| chunk.len() <= 4096));
    }
}
