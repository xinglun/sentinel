use crate::config;
use crate::features::radar::interface::audit_daily_report::build_daily_calibration_context;
use crate::features::research::interface::cognitive_reports::{
    build_daily_calibration_report_from_context, enabled_asset_thesis_count,
    enabled_research_attention_count,
};
use crate::features::shared::acl::notification_factory::send_required_telegram_notification;
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;

/// Daily Calibration command の orchestration と通知境界を保持したまま実行する。
pub(crate) async fn run_daily_calibration_command(
    app_config: &config::AppConfig,
    target_date_arg: Option<&str>,
    window_days: usize,
    language: Language,
    notify: bool,
) -> Result<()> {
    let report =
        build_daily_calibration_report(app_config, target_date_arg, window_days, language).await?;
    println!("{}", report);
    if notify {
        let telegram_report = build_daily_calibration_telegram_digest(&report, language);
        send_required_telegram_notification(
            app_config.telegram.as_ref(),
            &telegram_report,
            "daily-calibration",
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn build_daily_calibration_report(
    app_config: &config::AppConfig,
    target_date_arg: Option<&str>,
    window_days: usize,
    language: Language,
) -> Result<String> {
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let context = build_daily_calibration_context(
        save_dir.as_path(),
        target_date_arg,
        window_days,
        enabled_research_attention_count(app_config),
        enabled_asset_thesis_count(app_config),
        language,
    )
    .await?;
    build_daily_calibration_report_from_context(
        app_config,
        &context.audit_section,
        &context.questions_section,
        context.calibration_date,
        window_days,
        language,
    )
    .await
}

pub(crate) fn build_daily_calibration_telegram_digest(report: &str, language: Language) -> String {
    const MAX_LINES: usize = 42;
    const MAX_CHARS: usize = 3200;

    let mut out = String::new();
    let mut retained = 0usize;
    let mut omitted = 0usize;
    let mut keep_next_content_line = false;

    for line in report.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            continue;
        }
        let keep = should_keep_daily_calibration_digest_line(line) || keep_next_content_line;
        keep_next_content_line = line.starts_with('#') || line.starts_with("## ");
        if keep && retained < MAX_LINES && out.len() + line.len() < MAX_CHARS {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
            retained += 1;
        } else {
            omitted += 1;
        }
    }

    if omitted > 0 {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&daily_calibration_digest_omission_notice(language, omitted));
    }
    out
}

fn daily_calibration_digest_omission_notice(language: Language, omitted: usize) -> String {
    match language {
        Language::ZhCn => format!(
            "- Telegram 摘要: 已省略 {} 行明细；CLI 输出保留完整 daily calibration report。",
            omitted
        ),
        Language::JaJp => format!(
            "- Telegram 要約: {} 行の詳細を省略。CLI 出力には daily calibration report の全文を保持。",
            omitted
        ),
        Language::EnUs => format!(
            "- Telegram digest: {} detail line(s) omitted; CLI output keeps the full daily calibration report.",
            omitted
        ),
    }
}

fn should_keep_daily_calibration_digest_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_digest_heading(trimmed)
        || is_structured_digest_line(trimmed)
        || is_digest_question_line(trimmed)
        || contains_decision_status_token(trimmed)
}

fn is_digest_heading(trimmed: &str) -> bool {
    trimmed.starts_with('#')
}

fn is_structured_digest_line(trimmed: &str) -> bool {
    if is_noisy_digest_detail(trimmed) {
        return false;
    }
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    body.contains(':') || body.contains('：')
}

fn is_digest_question_line(trimmed: &str) -> bool {
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    body.ends_with('?') || body.ends_with('？')
}

fn contains_decision_status_token(trimmed: &str) -> bool {
    trimmed.contains("NO TRADE") || trimmed.contains("READY") || trimmed.contains("WATCH")
}

fn is_noisy_digest_detail(trimmed: &str) -> bool {
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    let lower = body.to_ascii_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.starts_with("source detail")
        || lower.starts_with("raw ")
        || lower.starts_with("raw extract")
        || lower.starts_with("source:")
        || lower.starts_with("sources:")
}
