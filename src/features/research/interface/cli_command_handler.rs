use crate::config;
use crate::features::research::interface::cognitive_reports::{
    build_asset_thesis_report, build_research_attention_report,
};
use crate::features::research::interface::gray_rhino_report::build_gray_rhino_escalation_report;
use crate::features::shared::acl::notification_factory::send_required_telegram_notification;
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;

async fn print_and_maybe_notify(
    app_config: &config::AppConfig,
    report: String,
    notify: bool,
    label: &str,
) -> Result<()> {
    println!("{}", report);
    if notify {
        send_required_telegram_notification(app_config.telegram.as_ref(), &report, label).await?;
    }
    Ok(())
}

/// 研究注意力 report command を実行する。
pub(crate) async fn run_research_attention_command(
    app_config: &config::AppConfig,
    language: Language,
    notify: bool,
) -> Result<()> {
    print_and_maybe_notify(
        app_config,
        build_research_attention_report(app_config, language),
        notify,
        "research-attention",
    )
    .await
}

/// 資産 thesis report command を実行する。
pub(crate) async fn run_asset_thesis_command(
    app_config: &config::AppConfig,
    language: Language,
    notify: bool,
) -> Result<()> {
    print_and_maybe_notify(
        app_config,
        build_asset_thesis_report(app_config, language),
        notify,
        "asset-thesis",
    )
    .await
}

/// 灰犀牛 escalation report command を実行する。
pub(crate) async fn run_gray_rhino_escalation_command(
    app_config: &config::AppConfig,
    language: Language,
    notify: bool,
) -> Result<()> {
    print_and_maybe_notify(
        app_config,
        build_gray_rhino_escalation_report(app_config, language),
        notify,
        "gray-rhino-escalation",
    )
    .await
}
