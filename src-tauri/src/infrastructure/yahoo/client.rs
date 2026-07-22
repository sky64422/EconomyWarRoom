use super::parse::{parse_quote_from_chart, parse_sparkline_from_chart};
use crate::domain::constants::SparklinePolicy;
use crate::domain::types::{AssetKind, Quote, Sparkline};
use crate::ports::market_data::{MarketDataProvider, ProviderLimits};
use async_trait::async_trait;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct YahooProvider {
    client: reqwest::Client,
}

impl YahooProvider {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    async fn chart_json(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<serde_json::Value, String> {
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbol}");
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
