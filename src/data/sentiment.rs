use anyhow::Result;
use async_trait::async_trait;

/// Represents a sentiment evaluation for a specific asset.
/// `score` ranges from 0.0 (Extreme Fear) to 100.0 (Extreme Greed).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SentimentScore {
    pub symbol: String,
    pub score: f64,
    pub label: String,
}

#[async_trait]
pub trait SentimentProvider: Send + Sync {
    /// Fetch the latest sentiment score for the given symbol.
    async fn fetch_sentiment(&self, symbol: &str) -> Result<SentimentScore>;
}

/// A mock implementation of `SentimentProvider` for development and testing.
/// In a real scenario, this would be replaced by an HTTP client connecting to an NLP API.
pub struct MockSentimentProvider;

impl MockSentimentProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SentimentProvider for MockSentimentProvider {
    async fn fetch_sentiment(&self, symbol: &str) -> Result<SentimentScore> {
        // Return a mocked value.
        // Let's pretend SPY is neutral, TSLA is slightly greedy, NVDA is euphoric, others are fear.
        let (score, label) = match symbol {
            "SPY" => (50.0, "Neutral".to_string()),
            "TSLA" => (60.0, "Greed".to_string()),
            "NVDA" => (90.0, "Extreme Greed".to_string()),
            "PLTR" => (45.0, "Fear".to_string()), // Added fear
            "GOOG" => (55.0, "Neutral".to_string()),
            "MSFT" => (65.0, "Greed".to_string()),
            _ => (15.0, "Extreme Fear".to_string()),
        };

        Ok(SentimentScore {
            symbol: symbol.to_string(),
            score,
            label,
        })
    }
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct FinnhubSentiment {
    #[serde(rename = "bearishPercent")]
    bearish_percent: f64,
    #[serde(rename = "bullishPercent")]
    bullish_percent: f64,
}

#[derive(Debug, serde::Deserialize)]
struct FinnhubResponse {
    sentiment: Option<FinnhubSentiment>,
}

pub struct FinnhubSentimentProvider {
    pub api_key: String,
    pub client: reqwest::Client,
}

impl FinnhubSentimentProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait]
impl SentimentProvider for FinnhubSentimentProvider {
    async fn fetch_sentiment(&self, symbol: &str) -> Result<SentimentScore> {
        let url = format!(
            "https://finnhub.io/api/v1/news-sentiment?symbol={}&token={}",
            symbol, self.api_key
        );

        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Finnhub API error: HTTP {}", resp.status());
        }

        let data: FinnhubResponse = resp.json().await?;

        let (score, label) = if let Some(sent) = data.sentiment {
            let s = sent.bullish_percent * 100.0;
            let l = if s >= 80.0 {
                "Extreme Greed".to_string()
            } else if s >= 60.0 {
                "Greed".to_string()
            } else if s <= 20.0 {
                "Extreme Fear".to_string()
            } else if s <= 40.0 {
                "Fear".to_string()
            } else {
                "Neutral".to_string()
            };
            (s, l)
        } else {
            (50.0, "Neutral".to_string())
        };

        Ok(SentimentScore {
            symbol: symbol.to_string(),
            score,
            label,
        })
    }
}
