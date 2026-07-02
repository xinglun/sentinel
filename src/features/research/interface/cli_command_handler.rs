use crate::config;
use crate::features::research::interface::cognitive_reports::{
    build_asset_thesis_report, build_research_attention_report,
};
use crate::features::research::interface::gray_rhino_report::build_gray_rhino_escalation_report;
use crate::features::research::interface::macro_event_official_calendar_adapter::{
    build_official_calendar_smoke_summary, OfficialCalendarSmokeSummary,
};
use crate::features::shared::acl::notification_factory::send_required_telegram_notification;
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;

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

/// 公式日历の live smoke command を実行する。
pub(crate) fn run_official_calendar_smoke_command(as_of_date: NaiveDate) -> Result<()> {
    let summary = build_official_calendar_smoke_summary(as_of_date);
    println!(
        "{}",
        serde_json::to_string_pretty(&build_official_calendar_smoke_payload(&summary))?
    );

    if summary.source_health != crate::features::research::interface::macro_event_observation::MacroEventSourceHealth::Succeeded {
        return Err(anyhow::anyhow!(
            "official calendar smoke failed: health={:?}, attempts={}, successes={}, failures={}, diagnostic={}",
            summary.source_health,
            summary.source_attempts,
            summary.source_successes,
            summary.source_failures,
            summary.diagnostic.as_deref().unwrap_or("none")
        ));
    }

    Ok(())
}

pub(crate) fn build_official_calendar_smoke_payload(
    summary: &OfficialCalendarSmokeSummary,
) -> serde_json::Value {
    json!({
        "smoke": "official-calendar",
        "summary": summary,
    })
}

#[cfg(test)]
mod tests {
    use super::build_official_calendar_smoke_payload;
    use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
    use crate::features::research::interface::macro_event_official_calendar_adapter::OfficialCalendarSmokeSummary;
    use chrono::NaiveDate;

    #[test]
    fn official_calendar_smoke_payload_renders_structured_summary() {
        let summary = OfficialCalendarSmokeSummary {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            source_health: MacroEventSourceHealth::Partial,
            source_attempts: 4,
            source_successes: 3,
            source_failures: 1,
            observation_count: 2,
            source_diagnostics: vec![crate::features::research::interface::macro_event_calendar_adapter::MacroEventSourceDiagnostic {
                family: "Bureau of Labor Statistics".to_string(),
                label: "BLS CPI release".to_string(),
                url: "https://www.bls.gov/schedule/news_release/cpi.htm".to_string(),
                fetch_health: MacroEventSourceHealth::Succeeded,
                observation_count: 1,
                note: "1 release(s)".to_string(),
            }],
            diagnostic: Some("official source failed on one endpoint".to_string()),
        };

        let payload = build_official_calendar_smoke_payload(&summary);

        assert_eq!(payload["smoke"], "official-calendar");
        assert_eq!(payload["summary"]["source_health"], "PARTIAL");
        assert_eq!(payload["summary"]["source_attempts"], 4);
        assert_eq!(payload["summary"]["observation_count"], 2);
        assert_eq!(
            payload["summary"]["source_diagnostics"][0]["label"],
            "BLS CPI release"
        );
        assert_eq!(
            payload["summary"]["diagnostic"],
            "official source failed on one endpoint"
        );
    }
}
