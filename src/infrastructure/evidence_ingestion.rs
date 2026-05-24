use crate::application::evidence_ingestion::{EvidenceExtractor, SourceDocument, SourceFetcher};
use crate::domain::evidence::{AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType};
use async_trait::async_trait;
use chrono::Local;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// キーワードベースの抽出エンジン。
#[derive(Default)]
pub struct RuleBasedExtractor;

impl RuleBasedExtractor {
    pub fn new() -> Self {
        Self
    }

    /// コンテンツ内のキーワードをカウントし、スコアリングする簡易的な抽出ロジック。
    fn match_keywords(&self, content: &str, keywords: &[&str]) -> bool {
        let content_lower = content.to_lowercase();
        keywords
            .iter()
            .any(|&k| content_lower.contains(&k.to_lowercase()))
    }
}

impl EvidenceExtractor for RuleBasedExtractor {
    fn extract(&self, doc: &SourceDocument) -> Vec<AutomatedEvidenceRecord> {
        let mut records = Vec::new();
        let collection_date = Local::now().format("%Y-%m-%d").to_string();

        // Finnhub の複数のニュース項目が含まれているか確認
        if let Some(items_json) = doc.metadata.get("items_json") {
            if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(items_json) {
                for item in items {
                    let headline = item.get("headline").and_then(|v| v.as_str()).unwrap_or("");
                    let summary = item.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                    let url = item.get("url").and_then(|v| v.as_str()).unwrap_or(&doc.url);
                    let ts = item.get("datetime").and_then(|v| v.as_i64()).unwrap_or(0);
                    let content = format!("{}\n{}", headline, summary);

                    let item_date = if ts > 0 {
                        chrono::DateTime::from_timestamp(ts, 0)
                            .map(|dt| dt.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| collection_date.clone())
                    } else {
                        collection_date.clone()
                    };

                    self.extract_from_content(
                        &content,
                        doc.symbol.as_str(),
                        doc.source_type,
                        url,
                        &item_date,
                        "", // form_type N/A for news
                        &mut records,
                    );
                }
                return records;
            }
        }

        // SEC 提出書類特有のメタデータがある場合
        let form_type = doc.metadata.get("form_type").cloned().unwrap_or_default();
        let filing_date = doc.metadata.get("filing_date").cloned();
        let event_date = filing_date.unwrap_or(collection_date);

        self.extract_from_content(
            &doc.content,
            doc.symbol.as_str(),
            doc.source_type,
            &doc.url,
            &event_date,
            &form_type,
            &mut records,
        );

        records
    }
}

impl RuleBasedExtractor {
    #[allow(clippy::too_many_arguments)]
    fn extract_from_content(
        &self,
        content: &str,
        symbol: &str,
        source: EvidenceSourceType,
        url: &str,
        event_date: &str,
        form_type: &str,
        records: &mut Vec<AutomatedEvidenceRecord>,
    ) {
        let is_sec = url.contains("sec.gov");

        // CapexPayoff の判定
        let mut capex_keywords = vec![
            "capex",
            "capital expenditures",
            "infrastructure build-out",
            "investment payoff",
        ];
        if is_sec {
            capex_keywords.push("Item 2.02"); // 決算発表セクション
        }

        if self.match_keywords(content, &capex_keywords) {
            let mut confidence = 0.8;
            if form_type == "8-K" || form_type == "10-Q" || form_type == "10-K" {
                confidence = 0.9; // 公式提出書類は信頼度を上げる
            }

            records.push(AutomatedEvidenceRecord::new(
                source,
                EvidenceType::CapexPayoff,
                confidence,
                format!(
                    "Detected CAPEX related keywords in {} ({})",
                    symbol, form_type
                ),
                event_date.to_string(),
                Some(symbol.to_string()),
                Some(url.to_string()),
                String::new(),
            ));
        }

        // EarningsValidation の判定
        let mut earnings_keywords = vec![
            "earnings",
            "revenue growth",
            "profitability",
            "margin expansion",
            "earnings validation",
        ];
        if is_sec {
            earnings_keywords.push("Item 2.02");
        }

        if self.match_keywords(content, &earnings_keywords) {
            let mut confidence = 0.7;
            if form_type == "8-K" || form_type == "10-Q" || form_type == "10-K" {
                confidence = 0.90;
            }

            records.push(AutomatedEvidenceRecord::new(
                source,
                EvidenceType::EarningsValidation,
                confidence,
                format!(
                    "Detected earnings related keywords in {} ({})",
                    symbol, form_type
                ),
                event_date.to_string(),
                Some(symbol.to_string()),
                Some(url.to_string()),
                String::new(),
            ));
        }

        // OrderVisibility の判定
        let mut order_keywords = vec![
            "order visibility",
            "backlog",
            "demand outstripping supply",
            "pipeline",
        ];
        if is_sec {
            order_keywords.push("Item 7.01"); // 適時開示
        }

        if self.match_keywords(content, &order_keywords) {
            let mut confidence = 0.75;
            if is_sec {
                confidence = 0.90;
            }

            records.push(AutomatedEvidenceRecord::new(
                source,
                EvidenceType::OrderVisibility,
                confidence,
                format!(
                    "Detected order visibility related keywords in {} ({})",
                    symbol, form_type
                ),
                event_date.to_string(),
                Some(symbol.to_string()),
                Some(url.to_string()),
                String::new(),
            ));
        }
    }
}

/// テスト用のフィクスチャフェッチャー。
pub struct FixtureFetcher {
    base_path: std::path::PathBuf,
}

impl FixtureFetcher {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: std::path::PathBuf::from(base_path),
        }
    }
}

#[async_trait]
impl SourceFetcher for FixtureFetcher {
    async fn fetch(
        &self,
        url: &str,
        symbol: &str,
        source_type: EvidenceSourceType,
        _days: usize,
    ) -> anyhow::Result<SourceDocument> {
        let file_path = self.base_path.join(url);
        let content = tokio::fs::read_to_string(file_path).await?;

        Ok(SourceDocument {
            title: url.to_string(),
            content,
            url: format!("file://{}", url),
            source_type,
            symbol: symbol.to_string(),
            metadata: HashMap::new(),
        })
    }
}

/// 一般的な Web ページから取得するフェッチャー。
pub struct WebFetcher;

#[async_trait]
impl SourceFetcher for WebFetcher {
    async fn fetch(
        &self,
        url: &str,
        symbol: &str,
        source_type: EvidenceSourceType,
        _days: usize,
    ) -> anyhow::Result<SourceDocument> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Web source returned error: {}",
                response.status()
            ));
        }
        let content = response.text().await?;

        Ok(SourceDocument {
            title: url.to_string(),
            content,
            url: url.to_string(),
            source_type,
            symbol: symbol.to_string(),
            metadata: HashMap::new(),
        })
    }
}

/// Finnhub API を利用してニュースを取得するフェッチャー。
pub struct FinnhubFetcher {
    api_key: String,
}

impl FinnhubFetcher {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait]
impl SourceFetcher for FinnhubFetcher {
    async fn fetch(
        &self,
        _url: &str,
        symbol: &str,
        source_type: EvidenceSourceType,
        days: usize,
    ) -> anyhow::Result<SourceDocument> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let from = (Local::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%d")
            .to_string();
        let endpoint = format!(
            "https://finnhub.io/api/v1/company-news?symbol={}&from={}&to={}&token={}",
            symbol, from, today, self.api_key
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let response = client.get(endpoint).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Finnhub API returned error: {}",
                response.status()
            ));
        }

        let items = response.json::<Vec<serde_json::Value>>().await?;

        let mut combined_content = String::new();
        let mut latest_ts: i64 = 0;
        for item in &items {
            if let Some(ts) = item.get("datetime").and_then(|v| v.as_i64()) {
                if ts > latest_ts {
                    latest_ts = ts;
                }
            }
            if let Some(headline) = item.get("headline").and_then(|h| h.as_str()) {
                combined_content.push_str(&format!("Headline: {}\n", headline));
            }
            if let Some(summary) = item.get("summary").and_then(|s| s.as_str()) {
                combined_content.push_str(&format!("Summary: {}\n\n", summary));
            }
        }

        let mut metadata = HashMap::new();
        if latest_ts > 0 {
            // UNIX 秒を YYYY-MM-DD 形式に変換
            if let Some(dt) = chrono::DateTime::from_timestamp(latest_ts, 0) {
                metadata.insert("filing_date".to_string(), dt.format("%Y-%m-%d").to_string());
            }
        }

        // 各ニュース項目の詳細（URLと日付）を保持するために JSON で埋め込む
        if let Ok(json_str) = serde_json::to_string(&items) {
            metadata.insert("items_json".to_string(), json_str);
        }

        Ok(SourceDocument {
            title: format!("Finnhub News for {}", symbol),
            content: combined_content,
            url: format!("finnhub://company-news/{}", symbol),
            source_type,
            symbol: symbol.to_string(),
            metadata,
        })
    }
}

/// SEC EDGAR から提出書類を取得するフェッチャー。
pub struct SECEDGARFetcher {
    user_agent: String,
    cik_map: Arc<RwLock<HashMap<String, String>>>,
    base_url_data: String,
    base_url_www: String,
}

impl SECEDGARFetcher {
    pub fn new(user_agent: String) -> Self {
        Self {
            user_agent,
            cik_map: Arc::new(RwLock::new(HashMap::new())),
            base_url_data: "https://data.sec.gov".to_string(),
            base_url_www: "https://www.sec.gov".to_string(),
        }
    }

    /// テスト用にベースURLを上書きする。
    pub fn with_base_urls(mut self, data: String, www: String) -> Self {
        self.base_url_data = data;
        self.base_url_www = www;
        self
    }

    /// Ticker から 10桁の CIK を特定する。
    async fn get_cik(&self, symbol: &str) -> anyhow::Result<String> {
        {
            let map = self.cik_map.read().await;
            if let Some(cik) = map.get(symbol) {
                return Ok(cik.clone());
            }
        }

        let url = format!("{}/files/company_tickers.json", self.base_url_www);
        let resp = self.sec_get(&url).await?;
        let data: serde_json::Value = resp.json().await?;

        let mut map = self.cik_map.write().await;
        if let Some(obj) = data.as_object() {
            for (_, val) in obj {
                let ticker = val["ticker"].as_str().unwrap_or_default();
                let cik = val["cik_str"].as_u64().unwrap_or_default();
                let cik_padded = format!("{:010}", cik);
                map.insert(ticker.to_string(), cik_padded);
            }
        }

        map.get(symbol)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("CIK not found for symbol: {}", symbol))
    }

    /// SEC API への共通 GET リクエスト（レート制限とエラーハンドリングを含む）。
    async fn sec_get(&self, url: &str) -> anyhow::Result<reqwest::Response> {
        // SEC API は 10 req/s の制限があるため、常に待機を入れる
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = reqwest::Client::builder()
            .user_agent(&self.user_agent)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "SEC API returned error: {}. URL: {}, User-Agent: '{}'",
                resp.status(),
                url,
                self.user_agent
            ));
        }
        Ok(resp)
    }
}

#[async_trait]
impl SourceFetcher for SECEDGARFetcher {
    async fn fetch(
        &self,
        _url: &str,
        symbol: &str,
        source_type: EvidenceSourceType,
        days: usize,
    ) -> anyhow::Result<SourceDocument> {
        let cik = self.get_cik(symbol).await?;

        let submissions_url = format!("{}/submissions/CIK{}.json", self.base_url_data, cik);
        let resp = self.sec_get(&submissions_url).await?;

        let json: serde_json::Value = resp.json().await?;
        let recent = &json["filings"]["recent"];
        let forms = recent["form"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No filings found"))?;
        let dates = recent["filingDate"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No dates found"))?;
        let accessions = recent["accessionNumber"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No accession numbers found"))?;
        let primary_docs = recent["primaryDocument"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No primary documents found"))?;

        let limit_date = (Local::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%d")
            .to_string();

        for i in 0..forms.len() {
            // 防御的チェック: すべての配列が同じインデックスを持っていることを確認
            if i >= dates.len() || i >= accessions.len() || i >= primary_docs.len() {
                break;
            }

            let form = forms[i].as_str().unwrap_or_default();
            let date = dates[i].as_str().unwrap_or_default();
            let accession = accessions[i].as_str().unwrap_or_default();
            let primary_doc = primary_docs[i].as_str().unwrap_or_default();

            if date < limit_date.as_str() {
                break; // これ以上古いものは対象外
            }

            // 8-K, 10-Q, 10-K を優先
            if form == "8-K" || form == "10-Q" || form == "10-K" {
                let accession_no_dashes = accession.replace("-", "");
                let cik_no_zeros = cik.trim_start_matches('0');
                let doc_url = format!(
                    "{}/Archives/edgar/data/{}/{}/{}",
                    self.base_url_www, cik_no_zeros, accession_no_dashes, primary_doc
                );

                let doc_resp = self.sec_get(&doc_url).await?;
                let content = doc_resp.text().await?;

                // HTML の最低限の検証
                if content.len() < 500 || !content.to_lowercase().contains("<html") {
                    return Err(anyhow::anyhow!(
                        "Fetched SEC document appears invalid or too short. Length: {}, URL: {}, User-Agent: '{}'",
                        content.len(),
                        doc_url,
                        self.user_agent
                    ));
                }

                let mut metadata = HashMap::new();
                metadata.insert("form_type".to_string(), form.to_string());
                metadata.insert("filing_date".to_string(), date.to_string());
                metadata.insert("accession_number".to_string(), accession.to_string());

                return Ok(SourceDocument {
                    title: format!("SEC Filing {} for {}", form, symbol),
                    content,
                    url: doc_url,
                    source_type,
                    symbol: symbol.to_string(),
                    metadata,
                });
            }
        }

        Err(anyhow::anyhow!(
            "No relevant SEC filings found for {} in last {} days",
            symbol,
            days
        ))
    }
}
