// 既存の主報告パイプライン(`capital_absorption_source_adapter::build_automatic_capital_absorption_snapshot`)
// への接続は、source_coverage 件数の増加が既存の表示・snapshotテストへ波及するため、本WIでは
// 行わない(別途の人間判断を要する接続決定として残す)。`sec_edgar_official_disclosure_provider.rs`
// の "WI-4 の Resolver 接続前" と同じ扱い。純粋関数(`financing_filings_in_window`,
// `build_financing_event` 等)はテストで検証済み。
#![allow(dead_code)]

use crate::config;
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoConfidence, CapitalAbsorptionAutoEvent,
    CapitalAbsorptionAutoEventCategory, CapitalAbsorptionObservationEventType,
    CapitalAbsorptionSourceCoverage, CapitalAbsorptionSourceCoverageStatus,
    CapitalAbsorptionSupplyKind,
};
use crate::features::research::domain::capital_absorption::extract_largest_usd_amount_billions;
use chrono::{Duration, NaiveDate};
use serde_json::Value;
use std::time::Duration as StdDuration;

const SEC_SUBMISSIONS_BASE_URL: &str = "https://data.sec.gov/submissions";
const SEC_ARCHIVES_BASE_URL: &str = "https://www.sec.gov/Archives/edgar/data";
const SEC_FINANCING_FORMS: &[&str] = &["424B5", "424B2", "FWP", "S-3ASR", "8-K"];
const SEC_MAX_FILINGS_PER_ISSUER: usize = 6;
const SEC_MIN_REQUEST_INTERVAL: StdDuration = StdDuration::from_millis(500);
/// TDnet相当ソースの識別子。常に `NotAttempted` を返す(公開JSON APIが無いため未実装)。
/// 網羅完了(Complete)の判定対象には含めない。恒久的に成功しないソースを完了判定に混ぜると、
/// 「網羅完了」状態が永久に到達不能になり、Complete 到達時のみ動く既存機能(IPOキュー履歴の
/// 永続化など)を無関係に壊すため。
pub(crate) const TDNET_EQUIVALENT_SOURCE: &str = "tdnet-equivalent-disclosure";
const TDNET_NOT_CONFIGURED_MESSAGE: &str =
    "TDnet-equivalent disclosure source has no public JSON API; not implemented";

/// 既知供給イベント一覧の発行者に限定した、監視対象の (symbol, CIK, 発行体名) 一覧。
/// 発行体名はチェックリストの `issuer_name` と一致させ、`match_known_supply_event_checklist`
/// が `subject` の完全一致で照合できるようにする。
const SEC_CIK_BY_SYMBOL: &[(&str, &str, &str)] = &[
    ("AAPL", "0000320193", "Apple"),
    ("MSFT", "0000789019", "Microsoft"),
    ("GOOG", "0001652044", "Alphabet"),
    ("GOOGL", "0001652044", "Alphabet"),
    ("AMZN", "0001018724", "Amazon.com"),
    ("META", "0001326801", "Meta Platforms"),
    ("NVDA", "0001045810", "NVIDIA"),
    ("TSLA", "0001318605", "Tesla"),
    ("AVGO", "0001730168", "Broadcom"),
    ("ORCL", "0001341439", "Oracle"),
    ("AMD", "0000002488", "Advanced Micro Devices"),
    ("PLTR", "0001321655", "Palantir"),
    ("IBM", "0000051143", "IBM"),
    ("INTC", "0000050863", "Intel"),
];

/// 既知の1件の filing 参照(提出 JSON から抽出した、本文取得前のメタデータ)。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinancingFilingReference {
    pub form: String,
    pub filing_date: NaiveDate,
    pub accession_number: String,
    pub primary_document: String,
}

/// SEC `submissions/CIK##########.json` の `filings.recent` から、資金調達関連フォームを
/// 指定窗口でフィルタする(純粋関数。ネットワークを伴わず fixture で検証できる)。
pub(crate) fn financing_filings_in_window(
    submissions: &Value,
    forms: &[&str],
    window_start: NaiveDate,
    window_end: NaiveDate,
) -> Result<Vec<FinancingFilingReference>, String> {
    let recent = submissions
        .get("filings")
        .and_then(|value| value.get("recent"))
        .ok_or_else(|| "SEC submissions response is missing recent filings".to_string())?;
    let form_values = required_array(recent, "form")?;
    let filing_dates = required_array(recent, "filingDate")?;
    let accessions = required_array(recent, "accessionNumber")?;
    let primary_documents = required_array(recent, "primaryDocument")?;
    if form_values.is_empty()
        || form_values.len() != filing_dates.len()
        || form_values.len() != accessions.len()
        || form_values.len() != primary_documents.len()
    {
        return Err("SEC submissions recent filing arrays are empty or inconsistent".to_string());
    }

    let mut references = Vec::new();
    for index in 0..form_values.len() {
        let form = form_values[index]
            .as_str()
            .ok_or_else(|| "SEC filing form is invalid".to_string())?;
        if !forms.contains(&form) {
            continue;
        }
        let filing_date_text = filing_dates[index]
            .as_str()
            .ok_or_else(|| "SEC filingDate is invalid".to_string())?;
        let filing_date = NaiveDate::parse_from_str(filing_date_text, "%Y-%m-%d")
            .map_err(|_| "SEC filingDate is invalid".to_string())?;
        if filing_date < window_start || filing_date > window_end {
            continue;
        }
        let accession_number = accessions[index]
            .as_str()
            .ok_or_else(|| "SEC accessionNumber is invalid".to_string())?
            .to_string();
        let primary_document = primary_documents[index]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SEC primaryDocument is missing".to_string())?
            .to_string();
        references.push(FinancingFilingReference {
            form: form.to_string(),
            filing_date,
            accession_number,
            primary_document,
        });
    }
    Ok(references)
}

fn required_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("SEC submissions field {key} is not an array"))
}

/// filing archive URL を組み立てる。CIK の先頭ゼロは EDGAR Archives の慣例で除去する。
pub(crate) fn filing_archive_url(
    cik: &str,
    accession_number: &str,
    primary_document: &str,
) -> String {
    let cik_without_leading_zeroes = cik.trim_start_matches('0');
    let accession_without_dashes = accession_number.replace('-', "");
    format!(
        "{SEC_ARCHIVES_BASE_URL}/{cik_without_leading_zeroes}/{accession_without_dashes}/{primary_document}"
    )
}

/// filing の本文(HTMLタグ除去済みテキスト)から、資金調達イベントを1件構築する。
/// 金额が本文から抽出できない場合は `amount_usd_b: None` のままにし、0 や推測値で埋めない。
pub(crate) fn build_financing_event(
    issuer_name: &str,
    reference: &FinancingFilingReference,
    source_url: String,
    body_text: Option<&str>,
) -> CapitalAbsorptionAutoEvent {
    let amount_usd_b = body_text.and_then(extract_largest_usd_amount_billions);
    CapitalAbsorptionAutoEvent {
        category: CapitalAbsorptionAutoEventCategory::MegaCapFinancing,
        supply_kind: CapitalAbsorptionSupplyKind::Actual,
        event_type: CapitalAbsorptionObservationEventType::Confirmed,
        subject: issuer_name.to_string(),
        description: format!("{} filed with SEC EDGAR", reference.form),
        amount_usd_b,
        ai_capex_related: false,
        source_url: Some(source_url),
        observed_at: reference.filing_date,
        source_count: 1,
        confidence: CapitalAbsorptionAutoConfidence::High,
    }
}

/// 2026-10-27〜11-03 窗口の既知4発行者に限らず、監視対象シンボル全件を EDGAR 発行文書で
/// 走査し、Finnhub ニュースと並列の `source_coverage` を返す。ライブ HTTP 呼び出しを含むため
/// 単体テストの対象にせず、純粋関数(`financing_filings_in_window` と `build_financing_event`)
/// 側でロジックを検証する。
pub(crate) async fn fetch_filing_capital_absorption_events(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    lookback_days: usize,
) -> (
    Vec<CapitalAbsorptionAutoEvent>,
    Vec<CapitalAbsorptionSourceCoverage>,
) {
    let mut events = Vec::new();
    let mut coverage = Vec::new();

    let user_agent = app_config
        .sec
        .as_ref()
        .map(|sec| sec.user_agent.clone())
        .filter(|value| !value.trim().is_empty());
    let Some(user_agent) = user_agent else {
        for (symbol, _, _) in SEC_CIK_BY_SYMBOL {
            coverage.push(not_attempted(
                format!("edgar-filing:{symbol}"),
                "SEC User-Agent is not configured",
            ));
        }
        coverage.push(not_attempted(
            TDNET_EQUIVALENT_SOURCE,
            TDNET_NOT_CONFIGURED_MESSAGE,
        ));
        return (events, coverage);
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            for (symbol, _, _) in SEC_CIK_BY_SYMBOL {
                coverage.push(not_attempted(
                    format!("edgar-filing:{symbol}"),
                    "HTTP client could not be created",
                ));
            }
            coverage.push(not_attempted(
                TDNET_EQUIVALENT_SOURCE,
                TDNET_NOT_CONFIGURED_MESSAGE,
            ));
            return (events, coverage);
        }
    };

    let window_start = as_of_date - Duration::days(lookback_days as i64);
    let window_end = as_of_date;

    for (symbol, cik, issuer_name) in SEC_CIK_BY_SYMBOL {
        let source = format!("edgar-filing:{symbol}");
        match fetch_issuer_financing_events(
            &client,
            &user_agent,
            cik,
            issuer_name,
            window_start,
            window_end,
        )
        .await
        {
            Ok(issuer_events) => {
                events.extend(issuer_events);
                coverage.push(CapitalAbsorptionSourceCoverage {
                    source,
                    status: CapitalAbsorptionSourceCoverageStatus::Succeeded,
                    message: "response processed".to_string(),
                });
            }
            Err(message) => coverage.push(CapitalAbsorptionSourceCoverage {
                source,
                status: CapitalAbsorptionSourceCoverageStatus::Failed,
                message,
            }),
        }
        tokio::time::sleep(SEC_MIN_REQUEST_INTERVAL).await;
    }

    coverage.push(not_attempted(
        TDNET_EQUIVALENT_SOURCE,
        TDNET_NOT_CONFIGURED_MESSAGE,
    ));
    (events, coverage)
}

async fn fetch_issuer_financing_events(
    client: &reqwest::Client,
    user_agent: &str,
    cik: &str,
    issuer_name: &str,
    window_start: NaiveDate,
    window_end: NaiveDate,
) -> Result<Vec<CapitalAbsorptionAutoEvent>, String> {
    let submissions_url = format!("{SEC_SUBMISSIONS_BASE_URL}/CIK{cik}.json");
    let response = client
        .get(&submissions_url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()
        .await
        .map_err(|_| "SEC submissions request failed".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "SEC submissions request returned HTTP {}",
            response.status()
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|_| "SEC submissions response could not be read".to_string())?;
    let value: Value = serde_json::from_str(&body)
        .map_err(|_| "SEC submissions response was invalid JSON".to_string())?;
    let references =
        financing_filings_in_window(&value, SEC_FINANCING_FORMS, window_start, window_end)?;

    let mut events = Vec::new();
    for reference in references.into_iter().take(SEC_MAX_FILINGS_PER_ISSUER) {
        let source_url = filing_archive_url(
            cik,
            &reference.accession_number,
            &reference.primary_document,
        );
        let body_text = fetch_filing_body_text(client, user_agent, &source_url).await;
        events.push(build_financing_event(
            issuer_name,
            &reference,
            source_url,
            body_text.as_deref(),
        ));
    }
    Ok(events)
}

async fn fetch_filing_body_text(
    client: &reqwest::Client,
    user_agent: &str,
    url: &str,
) -> Option<String> {
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let raw = response.text().await.ok()?;
    Some(strip_html_tags(&raw))
}

fn strip_html_tags(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut inside_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => text.push(ch),
            _ => {}
        }
    }
    text
}

fn not_attempted(
    source: impl Into<String>,
    message: impl Into<String>,
) -> CapitalAbsorptionSourceCoverage {
    CapitalAbsorptionSourceCoverage {
        source: source.into(),
        status: CapitalAbsorptionSourceCoverageStatus::NotAttempted,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submissions_fixture(path: &str) -> Value {
        let body = std::fs::read_to_string(path).expect("fixture should be readable");
        serde_json::from_str(&body).expect("fixture should be valid JSON")
    }

    #[test]
    fn filters_financing_forms_within_window_from_real_meta_submissions() {
        let submissions =
            submissions_fixture("tests/fixtures/capital_absorption/edgar_submissions_meta.json");
        let references = financing_filings_in_window(
            &submissions,
            SEC_FINANCING_FORMS,
            NaiveDate::from_ymd_opt(2026, 10, 27).unwrap(),
            NaiveDate::from_ymd_opt(2026, 11, 3).unwrap(),
        )
        .unwrap();

        assert!(!references.is_empty());
        for reference in &references {
            assert!(SEC_FINANCING_FORMS.contains(&reference.form.as_str()));
            assert!(reference.filing_date >= NaiveDate::from_ymd_opt(2026, 10, 27).unwrap());
            assert!(reference.filing_date <= NaiveDate::from_ymd_opt(2026, 11, 3).unwrap());
        }
        // 2026-10-26 の filing (窗口の前日) はカウントされない。
        assert!(references.iter().all(
            |reference| reference.filing_date != NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()
        ));
    }

    #[test]
    fn excludes_non_financing_forms_such_as_144_and_def14c() {
        let submissions =
            submissions_fixture("tests/fixtures/capital_absorption/edgar_submissions_meta.json");
        let references = financing_filings_in_window(
            &submissions,
            SEC_FINANCING_FORMS,
            NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(),
        )
        .unwrap();

        assert!(references.iter().all(|reference| reference.form != "144"));
        assert!(references
            .iter()
            .all(|reference| reference.form != "DEF 14C"));
    }

    #[test]
    fn missing_recent_filings_fails_closed() {
        let result = financing_filings_in_window(
            &Value::Null,
            SEC_FINANCING_FORMS,
            NaiveDate::from_ymd_opt(2026, 10, 27).unwrap(),
            NaiveDate::from_ymd_opt(2026, 11, 3).unwrap(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn filing_archive_url_strips_leading_zero_cik_and_accession_dashes() {
        let url = filing_archive_url("0001326801", "0001193125-26-432549", "d382225dfwp.htm");
        assert_eq!(
            url,
            "https://www.sec.gov/Archives/edgar/data/1326801/000119312526432549/d382225dfwp.htm"
        );
    }

    #[test]
    fn build_financing_event_keeps_amount_unavailable_when_body_has_no_dollar_amount() {
        let reference = FinancingFilingReference {
            form: "8-K".to_string(),
            filing_date: NaiveDate::from_ymd_opt(2026, 10, 29).unwrap(),
            accession_number: "0001018724-26-000038".to_string(),
            primary_document: "amzn-20261029.htm".to_string(),
        };
        let event = build_financing_event(
            "Amazon.com",
            &reference,
            "https://example.com/amzn-8k".to_string(),
            Some("Item 2.02 Results of Operations and Financial Condition."),
        );
        assert_eq!(event.amount_usd_b, None);
        assert_eq!(event.subject, "Amazon.com");
        assert_eq!(
            event.event_type,
            CapitalAbsorptionObservationEventType::Confirmed
        );
    }

    #[test]
    fn build_financing_event_extracts_amount_when_body_is_available() {
        let reference = FinancingFilingReference {
            form: "424B2".to_string(),
            filing_date: NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(),
            accession_number: "0001193125-26-442618".to_string(),
            primary_document: "d150456d424b2.htm".to_string(),
        };
        let event = build_financing_event(
            "Microsoft",
            &reference,
            "https://example.com/msft-424b2".to_string(),
            Some("Prospectus Supplement $25,000,000,000 Microsoft Corporation"),
        );
        assert_eq!(event.amount_usd_b, Some(25.0));
    }

    #[test]
    fn strip_html_tags_keeps_text_and_removes_markup() {
        assert_eq!(
            strip_html_tags("<p>$25,000,000,000 <b>Notes</b></p>"),
            "$25,000,000,000 Notes"
        );
    }
}
