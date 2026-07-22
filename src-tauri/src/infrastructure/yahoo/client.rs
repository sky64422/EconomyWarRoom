use super::parse::{parse_quote_from_chart, parse_sparkline_from_chart};
use crate::domain::constants::SparklinePolicy;
use crate::domain::types::{AssetKind, Quote, Sparkline};
use crate::ports::market_data::{MarketDataProvider, ProviderLimits};
use async_trait::async_trait;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const DEFAULT_BASE: &str = "https://query1.finance.yahoo.com";

pub struct YahooProvider {
    client: reqwest::Client,
    base_url: String,
}

impl YahooProvider {
    pub fn new() -> Result<Self, String> {
        Self::with_base_url(DEFAULT_BASE)
    }

    /// Construct with a custom base URL (used by tests with a mock HTTP server).
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    async fn chart_json(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<serde_json::Value, String> {
        let url = format!("{}/v8/finance/chart/{symbol}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[("range", range), ("interval", interval)])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() == 429 {
            return Err("rate_limited".into());
        }
        if !resp.status().is_success() {
            return Err(format!("http {}", resp.status()));
        }
        resp.json().await.map_err(|e| e.to_string())
    }
}

#[async_trait]
impl MarketDataProvider for YahooProvider {
    fn id(&self) -> &'static str {
        "yahoo"
    }

    fn supports(&self, kind: AssetKind) -> bool {
        matches!(
            kind,
            AssetKind::Equity | AssetKind::Crypto | AssetKind::Other
        )
    }

    fn limits(&self) -> ProviderLimits {
        ProviderLimits {
            max_concurrent: 3,
            min_interval: Duration::from_secs(10),
            prefers_batch: false, // per-symbol chart
        }
    }

    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
        let mut out = Vec::new();
        for sym in symbols {
            let json = self
                .chart_json(sym, SparklinePolicy::RANGE, SparklinePolicy::INTERVAL)
                .await?;
            out.push(parse_quote_from_chart(&json)?);
        }
        Ok(out)
    }

    async fn fetch_sparkline(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<Sparkline, String> {
        let json = self.chart_json(symbol, range, interval).await?;
        parse_sparkline_from_chart(&json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::market_data::MarketDataProvider;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn chart_body(symbol: &str, price: f64) -> String {
        format!(
            r#"{{
              "chart": {{
                "result": [{{
                  "meta": {{
                    "currency": "USD",
                    "symbol": "{symbol}",
                    "regularMarketPrice": {price},
                    "previousClose": 100.0
                  }},
                  "timestamp": [1, 2, 3],
                  "indicators": {{ "quote": [{{ "close": [100.0, 101.0, {price}] }}] }}
                }}],
                "error": null
              }}
            }}"#
        )
    }

    #[tokio::test]
    async fn fetch_quotes_ok_from_mock_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v8/finance/chart/AAPL"))
            .and(query_param("range", "1d"))
            .respond_with(ResponseTemplate::new(200).set_body_string(chart_body("AAPL", 110.0)))
            .mount(&server)
            .await;

        let provider = YahooProvider::with_base_url(server.uri()).unwrap();
        assert_eq!(provider.id(), "yahoo");
        assert!(provider.supports(AssetKind::Equity));
        assert!(!provider.supports(AssetKind::Commodity));

        let quotes = provider
            .fetch_quotes(&[String::from("AAPL")])
            .await
            .unwrap();
        assert_eq!(quotes.len(), 1);
        assert!((quotes[0].price - 110.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn rate_limited_maps_to_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v8/finance/chart/MSFT"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let provider = YahooProvider::with_base_url(server.uri()).unwrap();
        let err = provider
            .fetch_quotes(&[String::from("MSFT")])
            .await
            .unwrap_err();
        assert_eq!(err, "rate_limited");
    }

    #[tokio::test]
    async fn http_error_maps_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v8/finance/chart/BAD"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let provider = YahooProvider::with_base_url(server.uri()).unwrap();
        let err = provider
            .fetch_sparkline("BAD", "1d", "5m")
            .await
            .unwrap_err();
        assert!(err.contains("http 500"));
    }

    #[tokio::test]
    async fn fetch_sparkline_ok() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v8/finance/chart/BTC-USD"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(chart_body("BTC-USD", 50000.0)),
            )
            .mount(&server)
            .await;

        let provider = YahooProvider::with_base_url(server.uri()).unwrap();
        let spark = provider
            .fetch_sparkline("BTC-USD", "1d", "5m")
            .await
            .unwrap();
        assert_eq!(spark.symbol, "BTC-USD");
        assert_eq!(spark.points.len(), 3);
    }
}
