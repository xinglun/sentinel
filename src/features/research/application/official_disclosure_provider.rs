// WI-4 の Resolver 接続前は独立した application port として保持する。
#![allow(dead_code)]

use super::corporate_event_provider::CorporateEventSource;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::BTreeMap;

/// SEC 官方披露 provider 的取得状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum OfficialDisclosureProviderHealth {
    Healthy,
    #[default]
    Unavailable,
}

/// SEC 官方披露的第一版分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OfficialDisclosureKind {
    EarningsRelated,
    QuarterlyReport,
    AnnualReport,
    OtherMaterialDisclosure,
    Unknown,
}

/// SEC provider 使用的公司身份；CIK 不应通过 symbol 猜测。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompanyIdentity {
    pub symbol: String,
    pub cik: Option<String>,
}

impl CompanyIdentity {
    pub(crate) fn new(symbol: impl Into<String>, cik: Option<String>) -> Self {
        Self {
            symbol: symbol.into(),
            cik,
        }
    }
}

/// 一条可追溯的 SEC 官方披露 observation。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OfficialDisclosureObservation {
    pub symbol: String,
    pub cik: String,
    pub form: String,
    pub accession_number: String,
    pub filing_date: NaiveDate,
    pub report_date: Option<NaiveDate>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub primary_document: Option<String>,
    pub disclosure_kind: OfficialDisclosureKind,
    pub source: CorporateEventSource,
    pub retrieved_at: DateTime<Utc>,
}

/// SEC provider 的独立 read model。
///
/// `observations.is_empty()` 只在 provider 健康且没有匹配披露时表示 no event；
/// `unavailable_symbols` 则明确保留 per-symbol 的数据不可用状态。
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct OfficialDisclosureProviderReadModel {
    pub health: OfficialDisclosureProviderHealth,
    pub observations: Vec<OfficialDisclosureObservation>,
    pub unavailable_symbols: BTreeMap<String, String>,
    pub retrieved_at: Option<DateTime<Utc>>,
    pub diagnostic: Option<String>,
}

impl OfficialDisclosureProviderReadModel {
    pub(crate) fn healthy(
        retrieved_at: DateTime<Utc>,
        observations: Vec<OfficialDisclosureObservation>,
    ) -> Self {
        Self {
            health: OfficialDisclosureProviderHealth::Healthy,
            observations,
            retrieved_at: Some(retrieved_at),
            ..Self::default()
        }
    }

    pub(crate) fn unavailable(
        retrieved_at: Option<DateTime<Utc>>,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self {
            health: OfficialDisclosureProviderHealth::Unavailable,
            retrieved_at,
            diagnostic: Some(diagnostic.into()),
            ..Self::default()
        }
    }

    pub(crate) fn mark_symbol_unavailable(
        &mut self,
        symbol: impl Into<String>,
        diagnostic: impl Into<String>,
    ) {
        self.health = OfficialDisclosureProviderHealth::Unavailable;
        self.unavailable_symbols
            .insert(symbol.into(), diagnostic.into());
    }

    pub(crate) fn is_symbol_unavailable(&self, symbol: &str) -> bool {
        self.unavailable_symbols.contains_key(symbol)
    }
}

/// 官方披露 provider 的 application port。
pub(crate) trait OfficialDisclosureProvider {
    fn load_for_market_date(
        &self,
        market_date: NaiveDate,
        subjects: &[CompanyIdentity],
    ) -> OfficialDisclosureProviderReadModel;
}

#[cfg(test)]
mod tests {
    use super::{
        CompanyIdentity, OfficialDisclosureKind, OfficialDisclosureObservation,
        OfficialDisclosureProviderHealth, OfficialDisclosureProviderReadModel,
    };
    use crate::features::research::application::corporate_event_provider::{
        CorporateEventSource, CorporateEventSourceKind,
    };
    use chrono::{NaiveDate, TimeZone, Utc};

    #[test]
    fn company_identity_keeps_symbol_and_optional_cik_separate() {
        let identity = CompanyIdentity::new("NVDA", Some("0001045810".to_string()));

        assert_eq!(identity.symbol, "NVDA");
        assert_eq!(identity.cik.as_deref(), Some("0001045810"));
    }

    #[test]
    fn healthy_empty_observations_are_distinct_from_unavailable() {
        let retrieved_at = Utc.with_ymd_and_hms(2026, 8, 30, 0, 0, 0).unwrap();
        let no_event = OfficialDisclosureProviderReadModel::healthy(retrieved_at, vec![]);
        let unavailable = OfficialDisclosureProviderReadModel::unavailable(
            Some(retrieved_at),
            "SEC response unavailable",
        );

        assert_eq!(no_event.health, OfficialDisclosureProviderHealth::Healthy);
        assert!(no_event.observations.is_empty());
        assert_eq!(
            unavailable.health,
            OfficialDisclosureProviderHealth::Unavailable
        );
        assert!(unavailable.observations.is_empty());
        assert_eq!(
            unavailable.diagnostic.as_deref(),
            Some("SEC response unavailable")
        );
    }

    #[test]
    fn symbol_unavailability_is_not_reported_as_no_event() {
        let mut model = OfficialDisclosureProviderReadModel::healthy(
            Utc.with_ymd_and_hms(2026, 8, 30, 0, 0, 0).unwrap(),
            vec![],
        );
        model.mark_symbol_unavailable("NVDA", "CIK mismatch");

        assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
        assert!(model.is_symbol_unavailable("NVDA"));
        assert!(model.observations.is_empty());
    }

    #[test]
    fn observation_preserves_official_fields_and_provider_neutral_source() {
        let retrieved_at = Utc.with_ymd_and_hms(2026, 8, 30, 1, 2, 3).unwrap();
        let observation = OfficialDisclosureObservation {
            symbol: "NVDA".to_string(),
            cik: "0001045810".to_string(),
            form: "8-K".to_string(),
            accession_number: "0001045810-26-000001".to_string(),
            filing_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            report_date: Some(NaiveDate::from_ymd_opt(2026, 7, 31).unwrap()),
            accepted_at: Some(retrieved_at),
            primary_document: Some("nvda-20260827.htm".to_string()),
            disclosure_kind: OfficialDisclosureKind::EarningsRelated,
            source: CorporateEventSource {
                provider_id: "sec-edgar".to_string(),
                source_kind: CorporateEventSourceKind::OfficialFiling,
                source_url: Some(
                    "https://www.sec.gov/Archives/edgar/data/1045810/000104581026000001/nvda-20260827.htm".to_string(),
                ),
            },
            retrieved_at,
        };

        assert_eq!(observation.form, "8-K");
        assert_eq!(
            observation.disclosure_kind,
            OfficialDisclosureKind::EarningsRelated
        );
        assert_eq!(observation.source.provider_id, "sec-edgar");
        assert_eq!(
            observation.source.source_kind,
            CorporateEventSourceKind::OfficialFiling
        );
    }
}
