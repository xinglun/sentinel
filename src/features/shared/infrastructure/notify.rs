use crate::config::TelegramConfig;
use anyhow::{anyhow, Result};
use reqwest::{Client, StatusCode};
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

    let message_text = strip_internal_report_run_markers(message_text);
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

fn strip_internal_report_run_markers(message_text: &str) -> String {
    message_text
        .split_inclusive('\n')
        .filter(|line| !is_internal_report_run_marker(line))
        .collect()
}

fn is_internal_report_run_marker(line: &str) -> bool {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    let Some(run_id) = line
        .strip_prefix("<!-- report_run_id: ")
        .and_then(|line| line.strip_suffix(" -->"))
    else {
        return false;
    };

    !run_id.is_empty() && !run_id.chars().any(char::is_whitespace)
}

#[cfg(test)]
fn build_payload(config: &TelegramConfig, message_text: &str) -> TelegramPayload {
    build_payload_from_sanitized(config, &sanitize_telegram_html(message_text))
}

fn build_payload_from_sanitized(config: &TelegramConfig, message_text: &str) -> TelegramPayload {
    TelegramPayload {
        chat_id: config.chat_id.clone(),
        text: message_text.to_string(),
        parse_mode: "HTML".to_string(),
        disable_web_page_preview: true,
    }
}

fn chunk_telegram_html_message(message_text: &str, max_len: usize) -> Vec<String> {
    if message_text.is_empty() || max_len == 0 {
        return vec![message_text.to_string()];
    }
    if message_text.len() <= max_len {
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut open_tags = Vec::new();
        append_telegram_html_fragment(
            message_text,
            &mut current,
            &mut open_tags,
            &mut chunks,
            max_len,
        );
        if !current.is_empty() {
            finish_telegram_html_chunk(&mut current, &open_tags, &mut chunks);
        }
        return chunks;
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut open_tags = Vec::new();

    for (line_index, line) in message_text.split('\n').enumerate() {
        let separator_len = usize::from(line_index > 0 && !current.is_empty());
        let candidate_len =
            current.len() + separator_len + line.len() + telegram_html_closing_tags_len(&open_tags);

        if !current.is_empty() && candidate_len > max_len {
            finish_telegram_html_chunk(&mut current, &open_tags, &mut chunks);
        } else if separator_len > 0 {
            current.push('\n');
        }

        append_telegram_html_fragment(line, &mut current, &mut open_tags, &mut chunks, max_len);
    }

    if !current.is_empty() {
        finish_telegram_html_chunk(&mut current, &open_tags, &mut chunks);
    }

    chunks
}

fn next_telegram_html_tag(text: &str) -> Option<(usize, &'static str)> {
    ["<b>", "</b>", "<i>", "</i>"]
        .into_iter()
        .filter_map(|tag| text.find(tag).map(|index| (index, tag)))
        .min_by_key(|(index, _)| *index)
}

fn telegram_html_closing_tag(open_tag: &str) -> &'static str {
    match open_tag {
        "<b>" => "</b>",
        "<i>" => "</i>",
        _ => "",
    }
}

fn telegram_html_closing_tags_len(open_tags: &[&'static str]) -> usize {
    open_tags
        .iter()
        .map(|tag| telegram_html_closing_tag(tag).len())
        .sum()
}

fn finish_telegram_html_chunk(
    current: &mut String,
    open_tags: &[&'static str],
    chunks: &mut Vec<String>,
) {
    for tag in open_tags.iter().rev() {
        current.push_str(telegram_html_closing_tag(tag));
    }
    chunks.push(std::mem::take(current));
    for tag in open_tags {
        current.push_str(tag);
    }
}

fn append_telegram_html_text(
    text: &str,
    current: &mut String,
    open_tags: &[&'static str],
    chunks: &mut Vec<String>,
    max_len: usize,
) {
    let mut remaining = text;
    while !remaining.is_empty() {
        let reserved_len = telegram_html_closing_tags_len(open_tags);
        let available_len = max_len.saturating_sub(current.len() + reserved_len);
        if available_len == 0 {
            finish_telegram_html_chunk(current, open_tags, chunks);
            continue;
        }

        let mut end = remaining.len().min(available_len);
        while end > 0 && !remaining.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            finish_telegram_html_chunk(current, open_tags, chunks);
            continue;
        }

        current.push_str(&remaining[..end]);
        remaining = &remaining[end..];
        if !remaining.is_empty() {
            finish_telegram_html_chunk(current, open_tags, chunks);
        }
    }
}

fn append_telegram_html_fragment(
    fragment: &str,
    current: &mut String,
    open_tags: &mut Vec<&'static str>,
    chunks: &mut Vec<String>,
    max_len: usize,
) {
    let mut cursor = 0;
    while cursor < fragment.len() {
        let Some((relative_tag_start, tag)) = next_telegram_html_tag(&fragment[cursor..]) else {
            append_telegram_html_text(&fragment[cursor..], current, open_tags, chunks, max_len);
            break;
        };
        let tag_start = cursor + relative_tag_start;

        if tag_start > cursor {
            append_telegram_html_text(
                &fragment[cursor..tag_start],
                current,
                open_tags,
                chunks,
                max_len,
            );
        }

        if let Some(open_tag) = tag.strip_prefix("</") {
            let matching_open_tag = match open_tag {
                "b>" => "<b>",
                "i>" => "<i>",
                _ => "",
            };
            if open_tags.last().copied() != Some(matching_open_tag) {
                append_telegram_html_text(&escape_html(tag), current, open_tags, chunks, max_len);
            } else {
                let remaining_closing_len =
                    telegram_html_closing_tags_len(&open_tags[..open_tags.len() - 1]);
                if current.len() + tag.len() + remaining_closing_len > max_len {
                    finish_telegram_html_chunk(current, open_tags, chunks);
                }
                current.push_str(tag);
                open_tags.pop();
            }
        } else {
            let closing_len =
                telegram_html_closing_tags_len(open_tags) + telegram_html_closing_tag(tag).len();
            if current.len() + tag.len() + closing_len > max_len {
                finish_telegram_html_chunk(current, open_tags, chunks);
            }
            current.push_str(tag);
            open_tags.push(tag);
        }
        cursor = tag_start + tag.len();
    }
}

fn validate_telegram_response(status: StatusCode, body: &str) -> Result<()> {
    if !status.is_success() {
        return Err(anyhow!(
            "Telegram API returned an HTTP error: status={status}"
        ));
    }

    let response: serde_json::Value =
        serde_json::from_str(body).map_err(|_| anyhow!("Telegram API returned invalid JSON"))?;
    if response.get("ok") != Some(&serde_json::Value::Bool(true)) {
        let description = response
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ok=false");
        return Err(anyhow!("Telegram API returned an error: {description}"));
    }

    Ok(())
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
        let payload = build_payload_from_sanitized(config, &text);

        let res = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send Telegram request: {}", e))?;

        let status = res.status();
        let response_body = res.text().await.unwrap_or_default();
        validate_telegram_response(status, &response_body)?;
    }

    println!(
        "✅ Telegram notification successfully sent to: {}",
        config.chat_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_payload, build_payload_from_sanitized, chunk_telegram_html_message, escape_html,
        sanitize_telegram_html, validate_telegram_response,
    };
    use crate::config::TelegramConfig;
    use reqwest::StatusCode;

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
    fn telegram_payload_removes_internal_report_run_marker() {
        let sanitized =
            sanitize_telegram_html("<!-- report_run_id: run-2026-09-02 -->\n\n<b>headline</b>");

        assert!(!sanitized.contains("report_run_id"));
        assert!(!sanitized.contains("run-2026-09-02"));
        assert!(sanitized.contains("<b>headline</b>"));
    }

    #[test]
    fn telegram_payload_builder_uses_sanitized_text_sent_to_api() {
        let cfg = TelegramConfig {
            enabled: true,
            bot_token: "token".to_string(),
            chat_id: "chat".to_string(),
        };
        let sanitized = sanitize_telegram_html(
            "<!-- report_run_id: run-2026-09-02 -->\n<b>headline</b> inline <!-- report_run_id: keep-me -->",
        );
        let payload = build_payload_from_sanitized(&cfg, &sanitized);
        let json = serde_json::to_value(payload).unwrap();

        assert!(!json["text"].as_str().unwrap().contains("run-2026-09-02"));
        assert!(json["text"]
            .as_str()
            .unwrap()
            .contains("&lt;!-- report_run_id: keep-me --&gt;"));
        assert_eq!(json["parse_mode"], "HTML");
    }

    #[test]
    fn telegram_payload_keeps_marker_like_inline_text_visible_and_escaped() {
        let sanitized = sanitize_telegram_html("prefix <!-- report_run_id: keep-me --> suffix");

        assert!(sanitized.contains("keep-me"));
        assert!(sanitized.contains("&lt;!-- report_run_id: keep-me --&gt;"));
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

    #[test]
    fn telegram_chunking_does_not_split_allowed_html_tags() {
        let text = format!("<i>{}</i>", "a".repeat(3800));
        let chunks = chunk_telegram_html_message(&text, 3800);

        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| {
            chunk.len() <= 3800 && chunk.matches("<i>").count() == chunk.matches("</i>").count()
        }));
    }

    #[test]
    fn telegram_api_business_error_is_rejected_even_when_http_succeeds() {
        let result = validate_telegram_response(
            StatusCode::OK,
            r#"{"ok":false,"error_code":400,"description":"Bad Request"}"#,
        );

        let error = result.expect_err("Telegram ok=false must fail the notification");
        assert!(error.to_string().contains("Telegram API returned an error"));
    }

    #[test]
    fn telegram_api_ok_response_is_accepted() {
        validate_telegram_response(StatusCode::OK, r#"{"ok":true,"result":{"message_id":1}}"#)
            .expect("Telegram ok=true must succeed");
    }
}
