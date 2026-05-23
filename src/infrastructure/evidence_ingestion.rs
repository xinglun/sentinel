use crate::application::evidence_ingestion::{EvidenceExtractor, SourceDocument};
use crate::domain::evidence::{AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType};
use chrono::Local;

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

            records.push(AutomatedEvidenceRecord {
                source,
                evidence_type: EvidenceType::CapexPayoff,
                confidence,
                description: format!(
                    "Detected CAPEX related keywords in {} ({})",
                    symbol, form_type
                ),
                event_date: event_date.to_string(),
                symbol: Some(symbol.to_string()),
                source_url: Some(url.to_string()),
                dedupe_key: String::new(),
            });
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

            records.push(AutomatedEvidenceRecord {
                source,
                evidence_type: EvidenceType::EarningsValidation,
                confidence,
                description: format!(
                    "Detected earnings related keywords in {} ({})",
                    symbol, form_type
                ),
                event_date: event_date.to_string(),
                symbol: Some(symbol.to_string()),
                source_url: Some(url.to_string()),
                dedupe_key: String::new(),
            });
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

            records.push(AutomatedEvidenceRecord {
                source,
                evidence_type: EvidenceType::OrderVisibility,
                confidence,
                description: format!(
                    "Detected order visibility related keywords in {} ({})",
                    symbol, form_type
                ),
                event_date: event_date.to_string(),
                symbol: Some(symbol.to_string()),
                source_url: Some(url.to_string()),
                dedupe_key: String::new(),
            });
        }
    }
}
