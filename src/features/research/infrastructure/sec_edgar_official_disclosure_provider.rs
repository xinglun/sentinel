// WI-4 の Resolver 接続前は fixture と ACL の境界から検証する。
#![allow(dead_code)]

use crate::features::research::application::corporate_event_provider::{
    CorporateEventSource, CorporateEventSourceKind,
};
use crate::features::research::application::official_disclosure_provider::{
    CompanyIdentity, OfficialDisclosureKind, OfficialDisclosureObservation,
    OfficialDisclosureProvider, OfficialDisclosureProviderHealth,
    OfficialDisclosureProviderReadModel,
};
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

const SEC_PROVIDER_ID: &str = "sec-edgar";
const SEC_DATA_BASE_URL: &str = "https://data.sec.gov";
const SEC_WWW_BASE_URL: &str = "https://www.sec.gov";
const SEC_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const SEC_MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(500);
const SEC_RETRY_BUDGET: usize = 2;
const SEC_RETRY_BACKOFF: Duration = Duration::from_millis(100);
const SEC_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// SEC HTTP transport 的可替换响应。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SecEdgarTransportResponse {
    pub status: u16,
    pub body: String,
}

/// SEC provider 的网络传输边界；测试通过 fixture transport 隔离网络。
pub(crate) trait SecEdgarTransport: Clone + Send + Sync {
    fn fetch(&self, url: &str, user_agent: &str) -> Result<SecEdgarTransportResponse, String>;
}

#[derive(Clone)]
pub(crate) struct ReqwestSecEdgarTransport {
    client: Client,
}

impl ReqwestSecEdgarTransport {
    fn new() -> Result<Self, String> {
        Client::builder()
            .timeout(SEC_REQUEST_TIMEOUT)
            .build()
            .map(|client| Self { client })
            .map_err(|error| format!("SEC HTTP client could not be created: {error}"))
    }
}

impl SecEdgarTransport for ReqwestSecEdgarTransport {
    fn fetch(&self, url: &str, user_agent: &str) -> Result<SecEdgarTransportResponse, String> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, user_agent)
            .send()
            .map_err(|error| format!("SEC request failed: {error}"))?;
        let status = response.status().as_u16();
        if response
            .content_length()
            .is_some_and(|length| length > SEC_MAX_BODY_BYTES as u64)
        {
            return Err(format!(
                "SEC response exceeded {} bytes",
                SEC_MAX_BODY_BYTES
            ));
        }
        let mut body = Vec::new();
        response
            .take((SEC_MAX_BODY_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| format!("SEC response could not be read: {error}"))?;
        if body.len() > SEC_MAX_BODY_BYTES {
            return Err(format!(
                "SEC response exceeded {} bytes",
                SEC_MAX_BODY_BYTES
            ));
        }
        String::from_utf8(body)
            .map(|body| SecEdgarTransportResponse { status, body })
            .map_err(|_| "SEC response was not valid UTF-8".to_string())
    }
}

#[derive(Clone)]
struct SecRequestExecutor<T> {
    transport: T,
    min_request_interval: Duration,
    retry_budget: usize,
    retry_backoff: Duration,
    last_request_at: Arc<Mutex<Option<Instant>>>,
}

impl<T> SecRequestExecutor<T>
where
    T: SecEdgarTransport,
{
    fn new(transport: T) -> Self {
        Self {
            transport,
            min_request_interval: SEC_MIN_REQUEST_INTERVAL,
            retry_budget: SEC_RETRY_BUDGET,
            retry_backoff: SEC_RETRY_BACKOFF,
            last_request_at: Arc::new(Mutex::new(None)),
        }
    }

    fn get(&self, url: &str, user_agent: &str) -> Result<SecEdgarTransportResponse, String> {
        let attempts = self.retry_budget.saturating_add(1);
        let mut last_failure = "SEC request failed".to_string();

        for attempt in 0..attempts {
            self.wait_for_rate_limit()?;
            match self.transport.fetch(url, user_agent) {
                Ok(response) if (200..300).contains(&response.status) => {
                    if response.body.len() > SEC_MAX_BODY_BYTES {
                        return Err(format!(
                            "SEC response exceeded {} bytes",
                            SEC_MAX_BODY_BYTES
                        ));
                    }
                    return Ok(response);
                }
                Ok(response) => {
                    last_failure = format!("SEC request returned HTTP {}", response.status);
                    if !is_retryable_status(response.status) {
                        return Err(last_failure);
                    }
                }
                Err(error) => {
                    last_failure = error;
                }
            }

            if attempt + 1 < attempts {
                thread::sleep(backoff_for(self.retry_backoff, attempt));
            }
        }

        Err(last_failure)
    }

    fn wait_for_rate_limit(&self) -> Result<(), String> {
        let mut last_request_at = self
            .last_request_at
            .lock()
            .map_err(|_| "SEC rate limiter lock was poisoned".to_string())?;
        let sleep_for = last_request_at
            .and_then(|last| self.min_request_interval.checked_sub(last.elapsed()))
            .unwrap_or_default();
        if !sleep_for.is_zero() {
            thread::sleep(sleep_for);
        }
        *last_request_at = Some(Instant::now());
        Ok(())
    }
}

fn is_retryable_status(status: u16) -> bool {
    status == 429 || status >= 500
}

fn backoff_for(base: Duration, attempt: usize) -> Duration {
    let multiplier = 1u128 << attempt.min(10);
    let millis = base.as_millis().saturating_mul(multiplier);
    Duration::from_millis(millis.min(u64::MAX as u128) as u64)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedCompanyIdentity {
    symbol: String,
    cik: String,
    source: String,
    retrieved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CompanyIdentityCacheFile {
    entries: Vec<CachedCompanyIdentity>,
}

/// CIK 的本地缓存，保存来源和取得时间以便审计和 replay。
#[derive(Clone, Default)]
pub(crate) struct CompanyIdentityCache {
    path: Option<PathBuf>,
    entries: Arc<RwLock<BTreeMap<String, CachedCompanyIdentity>>>,
}

impl CompanyIdentityCache {
    pub(crate) fn in_memory() -> Self {
        Self::default()
    }

    pub(crate) fn from_path(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let cache = Self {
            path: Some(path.clone()),
            entries: Arc::new(RwLock::new(BTreeMap::new())),
        };
        if !path.exists() {
            return Ok(cache);
        }
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("SEC CIK cache could not be read: {error}"))?;
        let file: CompanyIdentityCacheFile = serde_json::from_str(&content)
            .map_err(|error| format!("SEC CIK cache is malformed: {error}"))?;
        {
            let mut entries = cache
                .entries
                .write()
                .map_err(|_| "SEC CIK cache lock was poisoned".to_string())?;
            for entry in file.entries {
                let cik = normalize_cik(&entry.cik)?;
                entries.insert(
                    entry.symbol.to_ascii_uppercase(),
                    CachedCompanyIdentity { cik, ..entry },
                );
            }
        }
        Ok(cache)
    }

    fn lookup(&self, symbol: &str) -> Result<Option<String>, String> {
        let entries = self
            .entries
            .read()
            .map_err(|_| "SEC CIK cache lock was poisoned".to_string())?;
        Ok(entries.get(symbol).map(|entry| entry.cik.clone()))
    }

    fn insert(
        &self,
        symbol: &str,
        cik: &str,
        source: &str,
        retrieved_at: DateTime<Utc>,
    ) -> Result<(), String> {
        let entry = CachedCompanyIdentity {
            symbol: symbol.to_string(),
            cik: cik.to_string(),
            source: source.to_string(),
            retrieved_at,
        };
        {
            let mut entries = self
                .entries
                .write()
                .map_err(|_| "SEC CIK cache lock was poisoned".to_string())?;
            entries.insert(symbol.to_string(), entry);
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        let entries = self
            .entries
            .read()
            .map_err(|_| "SEC CIK cache lock was poisoned".to_string())?;
        let file = CompanyIdentityCacheFile {
            entries: entries.values().cloned().collect(),
        };
        let content = serde_json::to_string_pretty(&file)
            .map_err(|error| format!("SEC CIK cache could not be serialized: {error}"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("SEC CIK cache directory could not be created: {error}")
            })?;
        }
        let temporary_path = path.with_extension("json.tmp");
        fs::write(&temporary_path, content)
            .map_err(|error| format!("SEC CIK cache could not be written: {error}"))?;
        fs::rename(&temporary_path, path)
            .map_err(|error| format!("SEC CIK cache could not be committed: {error}"))
    }
}

/// SEC EDGAR 官方披露 provider。
pub(crate) struct SecEdgarOfficialDisclosureProvider<T> {
    user_agent: Option<String>,
    requests: SecRequestExecutor<T>,
    cache: CompanyIdentityCache,
    base_url_data: String,
    base_url_www: String,
}

impl SecEdgarOfficialDisclosureProvider<ReqwestSecEdgarTransport> {
    pub(crate) fn new(
        user_agent: Option<String>,
        cache_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let transport = ReqwestSecEdgarTransport::new()?;
        let cache = match cache_path {
            Some(path) => CompanyIdentityCache::from_path(path)?,
            None => CompanyIdentityCache::in_memory(),
        };
        Ok(Self::with_transport_and_cache(user_agent, transport, cache))
    }
}

impl<T> SecEdgarOfficialDisclosureProvider<T>
where
    T: SecEdgarTransport,
{
    pub(crate) fn with_transport_and_cache(
        user_agent: Option<String>,
        transport: T,
        cache: CompanyIdentityCache,
    ) -> Self {
        Self {
            user_agent,
            requests: SecRequestExecutor::new(transport),
            cache,
            base_url_data: SEC_DATA_BASE_URL.to_string(),
            base_url_www: SEC_WWW_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    fn with_test_settings(
        user_agent: Option<String>,
        transport: T,
        cache: CompanyIdentityCache,
    ) -> Self {
        let mut provider = Self::with_transport_and_cache(user_agent, transport, cache);
        provider.requests.min_request_interval = Duration::ZERO;
        provider.requests.retry_backoff = Duration::ZERO;
        provider
    }

    #[cfg(test)]
    fn with_base_urls(mut self, data: &str, www: &str) -> Self {
        self.base_url_data = data.to_string();
        self.base_url_www = www.to_string();
        self
    }

    fn configured_user_agent(&self) -> Result<&str, String> {
        let user_agent = self
            .user_agent
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "SEC User-Agent is not configured".to_string())?;
        validate_user_agent(user_agent)?;
        Ok(user_agent)
    }

    fn load_company_ticker_mapping(
        &self,
        user_agent: &str,
        retrieved_at: DateTime<Utc>,
    ) -> Result<BTreeMap<String, String>, String> {
        let url = format!("{}/files/company_tickers.json", self.base_url_www);
        let response = self.requests.get(&url, user_agent)?;
        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| format!("SEC company ticker mapping is malformed: {error}"))?;
        let entries = value
            .as_object()
            .ok_or_else(|| "SEC company ticker mapping must be an object".to_string())?;
        let mut mapping = BTreeMap::new();
        for entry in entries.values() {
            let Some(symbol) = entry.get("ticker").and_then(Value::as_str) else {
                continue;
            };
            let Some(raw_cik) = entry.get("cik_str") else {
                continue;
            };
            let raw_cik = raw_cik
                .as_u64()
                .map(|value| value.to_string())
                .or_else(|| raw_cik.as_str().map(str::to_string));
            let Some(raw_cik) = raw_cik else {
                continue;
            };
            let Ok(cik) = normalize_cik(&raw_cik) else {
                continue;
            };
            let normalized_symbol = normalize_symbol(symbol)?;
            mapping.insert(normalized_symbol.clone(), cik.clone());
            self.cache
                .insert(&normalized_symbol, &cik, &url, retrieved_at)?;
        }
        if mapping.is_empty() {
            return Err("SEC company ticker mapping is empty".to_string());
        }
        Ok(mapping)
    }

    fn load_subject(
        &self,
        market_date: NaiveDate,
        identity: &CompanyIdentity,
        cik: &str,
        user_agent: &str,
        retrieved_at: DateTime<Utc>,
    ) -> Result<Vec<OfficialDisclosureObservation>, String> {
        let url = format!("{}/submissions/CIK{}.json", self.base_url_data, cik);
        let response = self.requests.get(&url, user_agent)?;
        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| format!("SEC submissions response is malformed: {error}"))?;
        validate_submission_identity(&value, &identity.symbol, cik)?;
        parse_observations(
            &value,
            &identity.symbol,
            cik,
            market_date,
            retrieved_at,
            &self.base_url_www,
        )
    }
}

impl<T> OfficialDisclosureProvider for SecEdgarOfficialDisclosureProvider<T>
where
    T: SecEdgarTransport,
{
    fn load_for_market_date(
        &self,
        market_date: NaiveDate,
        subjects: &[CompanyIdentity],
    ) -> OfficialDisclosureProviderReadModel {
        let retrieved_at = Utc::now();
        let mut model = OfficialDisclosureProviderReadModel::healthy(retrieved_at, vec![]);
        let user_agent = match self.configured_user_agent() {
            Ok(user_agent) => user_agent,
            Err(error) => {
                model.health = OfficialDisclosureProviderHealth::Unavailable;
                model.diagnostic = Some(error.clone());
                for subject in subjects {
                    model.mark_symbol_unavailable(&subject.symbol, error.clone());
                }
                return model;
            }
        };
        if subjects.is_empty() {
            return OfficialDisclosureProviderReadModel::unavailable(
                Some(retrieved_at),
                "SEC provider requires a non-empty observation universe",
            );
        }

        let mut identities = BTreeMap::new();
        let mut missing_cik_symbols = Vec::new();
        for subject in subjects {
            let symbol = match normalize_symbol(&subject.symbol) {
                Ok(symbol) => symbol,
                Err(error) => {
                    model.mark_symbol_unavailable(&subject.symbol, error);
                    continue;
                }
            };
            match subject.cik.as_deref().map(normalize_cik) {
                Some(Ok(cik)) => {
                    identities.insert(symbol.clone(), CompanyIdentity::new(symbol, Some(cik)));
                }
                Some(Err(error)) => model.mark_symbol_unavailable(&symbol, error),
                None => match self.cache.lookup(&symbol) {
                    Ok(Some(cik)) => {
                        identities.insert(symbol.clone(), CompanyIdentity::new(symbol, Some(cik)));
                    }
                    Ok(None) => missing_cik_symbols.push(symbol),
                    Err(error) => model.mark_symbol_unavailable(&symbol, error),
                },
            }
        }

        if !missing_cik_symbols.is_empty() {
            match self.load_company_ticker_mapping(user_agent, retrieved_at) {
                Ok(mapping) => {
                    for symbol in missing_cik_symbols {
                        match mapping.get(&symbol) {
                            Some(cik) => {
                                identities.insert(
                                    symbol.clone(),
                                    CompanyIdentity::new(symbol, Some(cik.clone())),
                                );
                            }
                            None => model.mark_symbol_unavailable(
                                &symbol,
                                "SEC CIK is not known for symbol",
                            ),
                        }
                    }
                }
                Err(error) => {
                    for symbol in missing_cik_symbols {
                        model.mark_symbol_unavailable(&symbol, error.clone());
                    }
                }
            }
        }

        for (symbol, identity) in identities {
            let Some(cik) = identity.cik.as_deref() else {
                model.mark_symbol_unavailable(&symbol, "SEC CIK is not resolved");
                continue;
            };
            match self.load_subject(market_date, &identity, cik, user_agent, retrieved_at) {
                Ok(mut observations) => model.observations.append(&mut observations),
                Err(error) => model.mark_symbol_unavailable(&symbol, error),
            }
        }
        model.observations.sort_by(|left, right| {
            (&left.symbol, &left.form, &left.accession_number).cmp(&(
                &right.symbol,
                &right.form,
                &right.accession_number,
            ))
        });
        model.health = if model.unavailable_symbols.is_empty() {
            OfficialDisclosureProviderHealth::Healthy
        } else {
            OfficialDisclosureProviderHealth::Unavailable
        };
        model
    }
}

fn validate_user_agent(value: &str) -> Result<(), String> {
    if value.contains(['\r', '\n']) {
        return Err("SEC User-Agent contains an invalid line break".to_string());
    }
    let Some(open) = value.find('<') else {
        return Err("SEC User-Agent must include a contact email".to_string());
    };
    let Some(close) = value.rfind('>') else {
        return Err("SEC User-Agent must include a contact email".to_string());
    };
    if close != value.len() - 1 {
        return Err("SEC User-Agent has trailing characters after contact email".to_string());
    }
    let company = value[..open].trim();
    let email = value[open + 1..close].trim();
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if company.is_empty()
        || !company.contains(' ')
        || local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || parts.next().is_some()
        || email.contains(['<', '>', ' '])
    {
        return Err("SEC User-Agent format is invalid".to_string());
    }
    Ok(())
}

fn normalize_symbol(value: &str) -> Result<String, String> {
    let symbol = value.trim().to_ascii_uppercase();
    if symbol.is_empty() {
        Err("SEC symbol is empty".to_string())
    } else {
        Ok(symbol)
    }
}

fn normalize_cik(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("SEC CIK is invalid".to_string());
    }
    Ok(format!("{value:0>10}"))
}

fn validate_submission_identity(value: &Value, symbol: &str, cik: &str) -> Result<(), String> {
    let response_cik = value
        .get("cik")
        .and_then(Value::as_str)
        .ok_or_else(|| "SEC submissions response is missing CIK".to_string())?;
    if normalize_cik(response_cik)? != cik {
        return Err("SEC submissions CIK does not match requested CIK".to_string());
    }
    let tickers = value
        .get("tickers")
        .and_then(Value::as_array)
        .ok_or_else(|| "SEC submissions response is missing tickers".to_string())?;
    let has_symbol = tickers.iter().any(|value| {
        value
            .as_str()
            .and_then(|value| normalize_symbol(value).ok())
            .is_some_and(|value| value == symbol)
    });
    if !has_symbol {
        return Err("SEC submissions symbol does not match requested symbol".to_string());
    }
    Ok(())
}

fn parse_observations(
    value: &Value,
    symbol: &str,
    cik: &str,
    market_date: NaiveDate,
    retrieved_at: DateTime<Utc>,
    base_url_www: &str,
) -> Result<Vec<OfficialDisclosureObservation>, String> {
    let recent = value
        .get("filings")
        .and_then(|value| value.get("recent"))
        .ok_or_else(|| "SEC submissions response is missing recent filings".to_string())?;
    let forms = required_array(recent, "form")?;
    let filing_dates = required_array(recent, "filingDate")?;
    let accessions = required_array(recent, "accessionNumber")?;
    let primary_documents = required_array(recent, "primaryDocument")?;
    if forms.is_empty()
        || forms.len() != filing_dates.len()
        || forms.len() != accessions.len()
        || forms.len() != primary_documents.len()
    {
        return Err("SEC submissions recent filing arrays are empty or inconsistent".to_string());
    }

    let report_dates = optional_array(recent, "reportDate")?;
    let accepted_at = optional_array(recent, "acceptanceDateTime")?;
    let items = optional_array(recent, "items")?;
    let mut observations = Vec::new();
    for index in 0..forms.len() {
        let form = forms[index]
            .as_str()
            .ok_or_else(|| "SEC filing form is invalid".to_string())?;
        let filing_date = parse_required_date(&filing_dates[index], "filingDate")?;
        if !matches!(form, "8-K" | "10-Q" | "10-K") || filing_date != market_date {
            continue;
        }
        let accession_number = accessions[index]
            .as_str()
            .ok_or_else(|| "SEC accession number is invalid".to_string())?;
        validate_accession_number(accession_number)?;
        let primary_document = primary_documents[index]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SEC primary document is missing".to_string())?;
        let report_date = optional_date_at(report_dates, index, "reportDate")?;
        let accepted_at = optional_datetime_at(accepted_at, index, "acceptanceDateTime")?;
        let disclosure_kind = classify_disclosure_kind(form, items, index)?;
        let accession_without_dashes = accession_number.replace('-', "");
        let cik_without_leading_zeroes = cik.trim_start_matches('0');
        if cik_without_leading_zeroes.is_empty() {
            return Err("SEC CIK cannot be all zeroes".to_string());
        }
        let source_url = format!(
            "{}/Archives/edgar/data/{}/{}/{}",
            base_url_www, cik_without_leading_zeroes, accession_without_dashes, primary_document
        );
        observations.push(OfficialDisclosureObservation {
            symbol: symbol.to_string(),
            cik: cik.to_string(),
            form: form.to_string(),
            accession_number: accession_number.to_string(),
            filing_date,
            report_date,
            accepted_at,
            primary_document: Some(primary_document.to_string()),
            disclosure_kind,
            source: CorporateEventSource {
                provider_id: SEC_PROVIDER_ID.to_string(),
                source_kind: CorporateEventSourceKind::OfficialFiling,
                source_url: Some(source_url),
            },
            retrieved_at,
        });
    }
    Ok(observations)
}

fn required_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("SEC submissions field {key} is not an array"))
}

fn optional_array<'a>(value: &'a Value, key: &str) -> Result<Option<&'a Vec<Value>>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_array()
            .map(Some)
            .ok_or_else(|| format!("SEC submissions field {key} is not an array")),
    }
}

fn parse_required_date(value: &Value, field: &str) -> Result<NaiveDate, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("SEC {field} is invalid"))?;
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| format!("SEC {field} is invalid"))
}

fn optional_date_at(
    values: Option<&Vec<Value>>,
    index: usize,
    field: &str,
) -> Result<Option<NaiveDate>, String> {
    let Some(values) = values else {
        return Ok(None);
    };
    let Some(value) = values.get(index) else {
        return Err(format!("SEC submissions field {field} is inconsistent"));
    };
    if value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    parse_required_date(value, field).map(Some)
}

fn optional_datetime_at(
    values: Option<&Vec<Value>>,
    index: usize,
    field: &str,
) -> Result<Option<DateTime<Utc>>, String> {
    let Some(values) = values else {
        return Ok(None);
    };
    let Some(value) = values.get(index) else {
        return Err(format!("SEC submissions field {field} is inconsistent"));
    };
    if value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    let value = value
        .as_str()
        .ok_or_else(|| format!("SEC {field} is invalid"))?;
    DateTime::parse_from_rfc3339(value)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| format!("SEC {field} is invalid"))
}

fn validate_accession_number(value: &str) -> Result<(), String> {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3
        || parts[0].len() != 10
        || parts[1].len() != 2
        || parts[2].len() != 6
        || parts
            .iter()
            .any(|part| !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err("SEC accession number is invalid".to_string());
    }
    Ok(())
}

fn classify_disclosure_kind(
    form: &str,
    items: Option<&Vec<Value>>,
    index: usize,
) -> Result<OfficialDisclosureKind, String> {
    match form {
        "10-Q" => Ok(OfficialDisclosureKind::QuarterlyReport),
        "10-K" => Ok(OfficialDisclosureKind::AnnualReport),
        "8-K" => {
            let Some(items) = items else {
                return Ok(OfficialDisclosureKind::Unknown);
            };
            let Some(item_value) = items.get(index) else {
                return Err("SEC submissions field items is inconsistent".to_string());
            };
            let item_values = if let Some(value) = item_value.as_str() {
                vec![value.to_string()]
            } else if let Some(values) = item_value.as_array() {
                values
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(str::to_string)
                            .ok_or_else(|| "SEC filing items is invalid".to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                return Err("SEC filing items is invalid".to_string());
            };
            let item_list: Vec<String> = item_values
                .iter()
                .flat_map(|value| value.split(',').map(str::trim))
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect();
            if item_list.is_empty() {
                return Ok(OfficialDisclosureKind::Unknown);
            }
            if item_list.iter().any(|item| normalize_item(item) == "2.02") {
                Ok(OfficialDisclosureKind::EarningsRelated)
            } else {
                Ok(OfficialDisclosureKind::OtherMaterialDisclosure)
            }
        }
        _ => Err("unsupported SEC form".to_string()),
    }
}

fn normalize_item(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("Item ")
        .trim_start_matches("item ")
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches([',', ';', ':'])
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_cik, CompanyIdentityCache, SecEdgarOfficialDisclosureProvider, SecEdgarTransport,
        SecEdgarTransportResponse,
    };
    use crate::features::research::application::official_disclosure_provider::{
        OfficialDisclosureKind, OfficialDisclosureProvider, OfficialDisclosureProviderHealth,
    };
    use chrono::NaiveDate;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Clone, Default)]
    struct FixtureTransport {
        responses: Arc<Mutex<VecDeque<Result<SecEdgarTransportResponse, String>>>>,
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl FixtureTransport {
        fn with_responses(
            responses: impl IntoIterator<Item = Result<SecEdgarTransportResponse, String>>,
        ) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.into_iter().collect())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn request_count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
    }

    impl SecEdgarTransport for FixtureTransport {
        fn fetch(&self, url: &str, user_agent: &str) -> Result<SecEdgarTransportResponse, String> {
            assert_eq!(user_agent, "Sentinel Test <test@example.com>");
            self.requests.lock().unwrap().push(url.to_string());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err("fixture response queue exhausted".to_string()))
        }
    }

    fn response(status: u16, body: &str) -> Result<SecEdgarTransportResponse, String> {
        Ok(SecEdgarTransportResponse {
            status,
            body: body.to_string(),
        })
    }

    fn provider(
        transport: FixtureTransport,
    ) -> SecEdgarOfficialDisclosureProvider<FixtureTransport> {
        SecEdgarOfficialDisclosureProvider::with_test_settings(
            Some("Sentinel Test <test@example.com>".to_string()),
            transport,
            CompanyIdentityCache::in_memory(),
        )
        .with_base_urls("https://data.fixture.test", "https://www.fixture.test")
    }

    fn subject(
        symbol: &str,
        cik: Option<&str>,
    ) -> crate::features::research::application::official_disclosure_provider::CompanyIdentity {
        crate::features::research::application::official_disclosure_provider::CompanyIdentity::new(
            symbol,
            cik.map(str::to_string),
        )
    }

    #[test]
    fn normalizes_cik_to_sec_zero_padded_representation() {
        assert_eq!(normalize_cik("320193").unwrap(), "0000320193");
        assert_eq!(normalize_cik("0000320193").unwrap(), "0000320193");
        assert!(normalize_cik("not-a-cik").is_err());
    }

    #[test]
    fn valid_nvda_8k_item_2_02_is_earnings_related() {
        let transport = FixtureTransport::with_responses([
            response(
                200,
                include_str!("../../../../tests/fixtures/sec/company_tickers_official.json"),
            ),
            response(
                200,
                include_str!("../../../../tests/fixtures/sec/submissions_nvda_8k_earnings.json"),
            ),
        ]);
        let provider = provider(transport.clone());
        let model = provider.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", None)],
        );

        assert_eq!(model.health, OfficialDisclosureProviderHealth::Healthy);
        assert!(model.unavailable_symbols.is_empty());
        assert_eq!(model.observations.len(), 1);
        assert_eq!(model.observations[0].cik, "0001045810");
        assert_eq!(
            model.observations[0].disclosure_kind,
            OfficialDisclosureKind::EarningsRelated
        );
        assert_eq!(transport.request_count(), 2);
    }

    #[test]
    fn supported_forms_are_classified_without_calling_finnhub() {
        for (fixture, expected) in [
            (
                "submissions_nvda_8k_unrelated.json",
                OfficialDisclosureKind::OtherMaterialDisclosure,
            ),
            (
                "submissions_nvda_10q.json",
                OfficialDisclosureKind::QuarterlyReport,
            ),
            (
                "submissions_nvda_10k.json",
                OfficialDisclosureKind::AnnualReport,
            ),
        ] {
            let body = std::fs::read_to_string(format!("tests/fixtures/sec/{fixture}")).unwrap();
            let transport = FixtureTransport::with_responses([response(200, &body)]);
            let provider = provider(transport);
            let model = provider.load_for_market_date(
                NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                &[subject("NVDA", Some("0001045810"))],
            );
            assert_eq!(model.health, OfficialDisclosureProviderHealth::Healthy);
            assert_eq!(model.observations[0].disclosure_kind, expected);
        }
    }

    #[test]
    fn unknown_8k_items_are_not_silently_classified_as_earnings() {
        let body =
            std::fs::read_to_string("tests/fixtures/sec/submissions_nvda_8k_unknown_items.json")
                .unwrap();
        let transport = FixtureTransport::with_responses([response(200, &body)]);
        let model = provider(transport).load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", Some("0001045810"))],
        );

        assert_eq!(model.health, OfficialDisclosureProviderHealth::Healthy);
        assert_eq!(
            model.observations[0].disclosure_kind,
            OfficialDisclosureKind::Unknown
        );
    }

    #[test]
    fn missing_user_agent_does_not_send_a_request() {
        let transport = FixtureTransport::with_responses([]);
        let provider = SecEdgarOfficialDisclosureProvider::with_test_settings(
            None,
            transport.clone(),
            CompanyIdentityCache::in_memory(),
        );
        let model = provider.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", None)],
        );

        assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
        assert!(model.is_symbol_unavailable("NVDA"));
        assert_eq!(transport.request_count(), 0);
    }

    #[test]
    fn invalid_user_agent_does_not_send_a_request() {
        let transport = FixtureTransport::with_responses([]);
        let provider = SecEdgarOfficialDisclosureProvider::with_test_settings(
            Some("InvalidUA".to_string()),
            transport.clone(),
            CompanyIdentityCache::in_memory(),
        );
        let model = provider.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", None)],
        );

        assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
        assert!(model.is_symbol_unavailable("NVDA"));
        assert_eq!(transport.request_count(), 0);
    }

    #[test]
    fn missing_cik_is_unavailable_and_never_guessed() {
        let transport = FixtureTransport::with_responses([response(200, "{}")]);
        let model = provider(transport).load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("UNKNOWN", None)],
        );

        assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
        assert!(model.is_symbol_unavailable("UNKNOWN"));
        assert!(model.observations.is_empty());
    }

    #[test]
    fn wrong_cik_and_invalid_payloads_fail_closed() {
        for body in [
            include_str!("../../../../tests/fixtures/sec/submissions_wrong_cik.json"),
            include_str!("../../../../tests/fixtures/sec/submissions_invalid_date.json"),
            include_str!("../../../../tests/fixtures/sec/submissions_missing_accession.json"),
            include_str!("../../../../tests/fixtures/sec/submissions_empty.json"),
            "not json",
        ] {
            let transport = FixtureTransport::with_responses([response(200, body)]);
            let model = provider(transport).load_for_market_date(
                NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                &[subject("NVDA", Some("0001045810"))],
            );
            assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
            assert!(model.is_symbol_unavailable("NVDA"));
            assert!(model.observations.is_empty());
        }
    }

    #[test]
    fn http_403_is_not_retried_but_429_and_500_are_bounded() {
        let transport = FixtureTransport::with_responses([response(403, "forbidden")]);
        let model = provider(transport.clone()).load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", Some("0001045810"))],
        );
        assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
        assert_eq!(transport.request_count(), 1);

        let transport = FixtureTransport::with_responses([
            response(429, "rate limited"),
            response(429, "rate limited"),
            response(429, "rate limited"),
        ]);
        let model = provider(transport.clone()).load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", Some("0001045810"))],
        );
        assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
        assert_eq!(transport.request_count(), 3);
    }

    #[test]
    fn connection_failure_is_bounded_and_marked_unavailable() {
        let transport = FixtureTransport::with_responses([
            Err("connection failed".to_string()),
            Err("connection failed".to_string()),
            Err("connection failed".to_string()),
        ]);
        let model = provider(transport.clone()).load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", Some("0001045810"))],
        );
        assert_eq!(model.health, OfficialDisclosureProviderHealth::Unavailable);
        assert_eq!(transport.request_count(), 3);
    }

    #[test]
    fn valid_submission_with_different_market_date_is_healthy_no_event() {
        let body = include_str!("../../../../tests/fixtures/sec/submissions_nvda_8k_earnings.json");
        let transport = FixtureTransport::with_responses([response(200, body)]);
        let model = provider(transport).load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 28).unwrap(),
            &[subject("NVDA", Some("0001045810"))],
        );
        assert_eq!(model.health, OfficialDisclosureProviderHealth::Healthy);
        assert!(model.observations.is_empty());
        assert!(model.unavailable_symbols.is_empty());
    }

    #[test]
    fn cik_cache_is_persisted_and_skips_mapping_request_on_reload() {
        let directory = tempdir().unwrap();
        let cache_path = directory.path().join("sec-cik-cache.json");
        let first_transport = FixtureTransport::with_responses([
            response(
                200,
                include_str!("../../../../tests/fixtures/sec/company_tickers_official.json"),
            ),
            response(
                200,
                include_str!("../../../../tests/fixtures/sec/submissions_nvda_8k_earnings.json"),
            ),
        ]);
        let first = SecEdgarOfficialDisclosureProvider::with_test_settings(
            Some("Sentinel Test <test@example.com>".to_string()),
            first_transport.clone(),
            CompanyIdentityCache::from_path(&cache_path).unwrap(),
        )
        .with_base_urls("https://data.fixture.test", "https://www.fixture.test");
        let first_model = first.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", None)],
        );
        assert_eq!(
            first_model.health,
            OfficialDisclosureProviderHealth::Healthy
        );
        assert_eq!(first_transport.request_count(), 2);

        let second_transport = FixtureTransport::with_responses([response(
            200,
            include_str!("../../../../tests/fixtures/sec/submissions_nvda_8k_earnings.json"),
        )]);
        let second = SecEdgarOfficialDisclosureProvider::with_test_settings(
            Some("Sentinel Test <test@example.com>".to_string()),
            second_transport.clone(),
            CompanyIdentityCache::from_path(&cache_path).unwrap(),
        )
        .with_base_urls("https://data.fixture.test", "https://www.fixture.test");
        let second_model = second.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &[subject("NVDA", None)],
        );
        assert_eq!(
            second_model.health,
            OfficialDisclosureProviderHealth::Healthy
        );
        assert_eq!(second_transport.request_count(), 1);
    }
}
