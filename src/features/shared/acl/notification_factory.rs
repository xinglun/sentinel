use crate::config::TelegramConfig;
use crate::features::shared::application::run_status::DeliveryStatus;
use crate::features::shared::infrastructure::notify;
use anyhow::Result;
use chrono::NaiveDate;
use std::path::Path;

pub fn telegram_delivery_precheck(
    config: Option<&TelegramConfig>,
) -> Result<&TelegramConfig, DeliveryStatus> {
    match config {
        Some(cfg) if !cfg.enabled => Err(DeliveryStatus::Skipped),
        Some(cfg) if cfg.bot_token.is_empty() || cfg.chat_id.is_empty() => {
            Err(DeliveryStatus::Failed {
                reason: "Telegram is enabled but bot_token/chat_id is missing".to_string(),
            })
        }
        Some(cfg) => Ok(cfg),
        None => Err(DeliveryStatus::Skipped),
    }
}

pub async fn send_telegram_when_available(
    config: Option<&TelegramConfig>,
    message: &str,
    context: &str,
) -> Result<()> {
    match telegram_delivery_precheck(config) {
        Ok(tg_cfg) => {
            notify::send_telegram_message(tg_cfg, message).await?;
        }
        Err(status) => {
            eprintln!(
                "Telegram notification is not available for {}: {:?}",
                context, status
            );
        }
    }
    Ok(())
}

pub async fn send_required_telegram_notification(
    config: Option<&TelegramConfig>,
    message: &str,
    context: &str,
) -> Result<()> {
    match telegram_delivery_precheck(config) {
        Ok(tg_cfg) => notify::send_telegram_message(tg_cfg, message).await,
        Err(status) => Err(anyhow::anyhow!(
            "Telegram notification is not available for {}: {:?}",
            context,
            status
        )),
    }
}

pub async fn send_telegram_with_status(
    config: Option<&TelegramConfig>,
    message: &str,
) -> DeliveryStatus {
    match telegram_delivery_precheck(config) {
        Ok(tg_cfg) => match notify::send_telegram_message(tg_cfg, message).await {
            Ok(_) => DeliveryStatus::Succeeded,
            Err(err) => {
                eprintln!("⚠️ Telegram notification failed: {}", err);
                DeliveryStatus::Failed {
                    reason: err.to_string(),
                }
            }
        },
        Err(DeliveryStatus::Skipped) => {
            if config.is_some() {
                eprintln!("ℹ️ Telegram notification skipped: config.telegram.enabled = false");
            } else {
                eprintln!("ℹ️ Telegram notification skipped: telegram config is missing");
            }
            DeliveryStatus::Skipped
        }
        Err(DeliveryStatus::Failed { reason }) => {
            eprintln!("⚠️ Telegram notification failed precheck: {}", reason);
            DeliveryStatus::Failed { reason }
        }
        Err(status) => status,
    }
}

pub fn load_run_evidence_collection_status(
    save_dir: &Path,
    date: NaiveDate,
) -> Option<DeliveryStatus> {
    crate::features::shared::infrastructure::run_status_reader::load_run_evidence_collection_status(
        save_dir, date,
    )
}

pub fn load_latest_evidence_collection_status(save_dir: &Path) -> DeliveryStatus {
    crate::features::shared::infrastructure::run_status_reader::load_latest_evidence_collection_status(save_dir)
}
