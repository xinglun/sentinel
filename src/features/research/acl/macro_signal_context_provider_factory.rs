use crate::config::AppConfig;
use crate::features::research::interface::macro_event_observation::{
    EvidenceRecord, MacroSignalContextEvent, MacroSignalContextInformationLevel,
    MacroSignalContextLifecycle, MacroSignalContextReadModel, MacroSignalContextSource,
    MacroSignalContextSourceStatus, MarketReaction,
};
use chrono::NaiveDate;

/// Radar composition root が research provider を読み込むための ACL 入口。
pub(crate) async fn load_macro_signal_context(
    app_config: &AppConfig,
    market_date: NaiveDate,
) -> MacroSignalContextReadModel {
    let context = crate::features::research::infrastructure::macro_signal_context_provider::load_macro_signal_context(
        app_config,
        market_date,
    )
    .await;
    MacroSignalContextReadModel {
        market_date: context.market_date,
        rates_credit: map_source(context.rates_credit),
        commodity: map_source(context.commodity),
        geopolitical: map_source(context.geopolitical),
        observed_market_reactions: context
            .observed_market_reactions
            .into_iter()
            .map(map_reaction)
            .collect(),
    }
}

fn map_source(
    source: crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderSource,
) -> MacroSignalContextSource {
    MacroSignalContextSource {
        status: match source.status {
            crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderSourceStatus::Healthy => MacroSignalContextSourceStatus::Healthy,
            crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderSourceStatus::Partial => MacroSignalContextSourceStatus::Partial,
            crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderSourceStatus::Degraded => MacroSignalContextSourceStatus::Degraded,
            crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderSourceStatus::Unavailable => MacroSignalContextSourceStatus::Unavailable,
        },
        events: source.events.into_iter().map(map_event).collect(),
        diagnostics: source.diagnostics,
    }
}

fn map_event(
    event: crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderEvent,
) -> MacroSignalContextEvent {
    MacroSignalContextEvent {
        title: event.title,
        information_content: map_information_level(event.information_content),
        market_relevance: map_information_level(event.market_relevance),
        evidence_quality: map_information_level(event.evidence_quality),
        lifecycle: match event.lifecycle {
            crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderLifecycle::ActiveRepricing => MacroSignalContextLifecycle::ActiveRepricing,
            crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderLifecycle::Aftermath => MacroSignalContextLifecycle::Aftermath,
            crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderLifecycle::Expired => MacroSignalContextLifecycle::Expired,
        },
        event_fact: event.event_fact,
        observed_at: event.observed_at,
        source_published_at: event.source_published_at,
        market_date: event.market_date,
        evidence: event.evidence.into_iter().map(map_evidence).collect(),
        expected_value: event.expected_value,
        actual_value: event.actual_value,
        surprise: event.surprise,
        reason: event.reason,
    }
}

fn map_information_level(
    level: crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderInformationLevel,
) -> MacroSignalContextInformationLevel {
    match level {
        crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderInformationLevel::High => MacroSignalContextInformationLevel::High,
        crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderInformationLevel::Medium => MacroSignalContextInformationLevel::Medium,
        crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderInformationLevel::Low => MacroSignalContextInformationLevel::Low,
        crate::features::research::infrastructure::macro_signal_context_provider::MacroSignalContextProviderInformationLevel::Unavailable => MacroSignalContextInformationLevel::Unavailable,
    }
}

fn map_evidence(
    evidence: crate::features::research::infrastructure::macro_signal_context_provider::ProviderEvidenceRecord,
) -> EvidenceRecord {
    EvidenceRecord {
        source: evidence.source,
        source_url: evidence.source_url,
        timestamp: evidence.timestamp,
        source_published_at: evidence.source_published_at,
        event_type: evidence.event_type,
        subject: evidence.subject,
        importance: evidence.importance,
    }
}

fn map_reaction(
    reaction: crate::features::research::infrastructure::macro_signal_context_provider::ProviderMarketReaction,
) -> MarketReaction {
    MarketReaction {
        observed_at: reaction.observed_at,
        source_published_at: reaction.source_published_at,
        market_date: reaction.market_date,
        subject: reaction.subject,
        observation: reaction.observation,
        evidence: reaction.evidence.into_iter().map(map_evidence).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::map_source;
    use crate::features::research::infrastructure::macro_signal_context_provider as provider;

    #[test]
    fn maps_provider_status_event_and_reaction_to_research_read_model() {
        let evidence = provider::ProviderEvidenceRecord {
            source: "FRED".to_string(),
            source_url: "https://fred.stlouisfed.org/series/DGS10".to_string(),
            timestamp: "2026-09-08T00:00:00Z".to_string(),
            source_published_at: "2026-09-08T00:00:00Z".to_string(),
            event_type: "RATES_CREDIT".to_string(),
            subject: "US 10Y Treasury yield".to_string(),
            importance: "HIGH".to_string(),
        };
        let event = provider::MacroSignalContextProviderEvent {
            title: "US RATES / CREDIT".to_string(),
            information_content: provider::MacroSignalContextProviderInformationLevel::High,
            market_relevance: provider::MacroSignalContextProviderInformationLevel::Medium,
            evidence_quality: provider::MacroSignalContextProviderInformationLevel::Low,
            lifecycle: provider::MacroSignalContextProviderLifecycle::Aftermath,
            event_fact: "US 10Y Treasury yield latest 4.80".to_string(),
            observed_at: "2026-09-08T00:00:00Z".to_string(),
            source_published_at: "2026-09-08T00:00:00Z".to_string(),
            market_date: "2026-09-08".to_string(),
            evidence: vec![evidence.clone()],
            expected_value: Some("4.70".to_string()),
            actual_value: Some("4.80".to_string()),
            surprise: Some("+0.10".to_string()),
            reason: Some("Observed repricing".to_string()),
        };
        let source = provider::MacroSignalContextProviderSource {
            status: provider::MacroSignalContextProviderSourceStatus::Healthy,
            events: vec![event],
            diagnostics: vec!["fixture".to_string()],
        };

        let mapped = map_source(source);

        assert_eq!(
            mapped.status,
            super::MacroSignalContextSourceStatus::Healthy
        );
        assert_eq!(mapped.events.len(), 1);
        assert_eq!(
            mapped.events[0].information_content,
            super::MacroSignalContextInformationLevel::High
        );
        assert_eq!(
            mapped.events[0].market_relevance,
            super::MacroSignalContextInformationLevel::Medium
        );
        assert_eq!(
            mapped.events[0].evidence_quality,
            super::MacroSignalContextInformationLevel::Low
        );
        assert_eq!(
            mapped.events[0].lifecycle,
            super::MacroSignalContextLifecycle::Aftermath
        );
        assert_eq!(mapped.events[0].evidence[0].source, "FRED");
        assert_eq!(mapped.events[0].expected_value.as_deref(), Some("4.70"));

        let reaction = provider::ProviderMarketReaction {
            observed_at: "2026-09-08T00:00:00Z".to_string(),
            source_published_at: "2026-09-08T00:00:00Z".to_string(),
            market_date: "2026-09-08".to_string(),
            subject: "US 10Y Treasury yield".to_string(),
            observation: "latest 4.80; daily change +0.10".to_string(),
            evidence: vec![evidence],
        };
        let mapped_reaction = super::map_reaction(reaction);
        assert_eq!(mapped_reaction.subject, "US 10Y Treasury yield");
        assert_eq!(mapped_reaction.evidence.len(), 1);
    }

    #[test]
    fn maps_all_provider_status_and_lifecycle_variants() {
        for status in [
            provider::MacroSignalContextProviderSourceStatus::Healthy,
            provider::MacroSignalContextProviderSourceStatus::Partial,
            provider::MacroSignalContextProviderSourceStatus::Degraded,
            provider::MacroSignalContextProviderSourceStatus::Unavailable,
        ] {
            let mapped = map_source(provider::MacroSignalContextProviderSource {
                status,
                ..Default::default()
            });
            assert!(mapped.events.is_empty());
        }

        for lifecycle in [
            provider::MacroSignalContextProviderLifecycle::ActiveRepricing,
            provider::MacroSignalContextProviderLifecycle::Aftermath,
            provider::MacroSignalContextProviderLifecycle::Expired,
        ] {
            let mapped = map_source(provider::MacroSignalContextProviderSource {
                events: vec![provider::MacroSignalContextProviderEvent {
                    lifecycle,
                    ..Default::default()
                }],
                ..Default::default()
            });
            assert_eq!(mapped.events.len(), 1);
        }
    }
}
