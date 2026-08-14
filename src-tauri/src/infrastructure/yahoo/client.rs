use super::parse::{
    enrich_quote_prior_from_daily, parse_quote_from_chart, parse_search_results,
    parse_sparkline_from_chart,
};
use crate::domain::constants::RefreshPolicy;
use crate::domain::types::{AssetKind, Quote, Sparkline, SymbolSuggestion};
use crate::ports::market_data::{MarketDataProvider, ProviderLimits};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const DEFAULT_BASE: &str = "https://query1.finance.yahoo.com";

/// Cached prior-close enrichment so we avoid a second daily chart call every tick.
type PriorCache = Arc<Mutex<HashMap<String, (f64, Option<f64>)>>>;

pub struct YahooProvider {
    client: reqwest::Client,
    base_url: String,
    prior_cache: PriorCache,
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
            prior_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn chart_json_with(
        client: &reqwest::Client,
        base_url: &str,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<serde_json::Value, String> {
        let url = format!("{base_url}/v8/finance/chart/{symbol}");
        let resp = client
            .get(&url)
            .query(&[
                ("range", range),
                ("interval", interval),
                ("includePrePost", "true"),
            ])
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

    async fn chart_json(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<serde_json::Value, String> {
        Self::chart_json_with(&self.client, &self.base_url, symbol, range, interval).await
    }

    async fn fetch_one_quote(
        client: &reqwest::Client,
        base_url: &str,
        prior_cache: &PriorCache,
        sym: &str,
    ) -> Result<Quote, String> {
        // 1m bars + includePrePost so pre/post last print is available when meta
        // omits preMarketPrice / marketState (common on chart API).
        let json = Self::chart_json_with(client, base_url, sym, "1d", "1m").await?;
        let mut quote = parse_quote_from_chart(&json)?;

        let prior_is_t1 = matches!(
            (quote.prior_close, quote.previous_close),
            (Some(prior), Some(pc)) if pc.is_finite() && pc != 0.0 && {
                let tol = (pc.abs() * 0.005).max(0.01);
                (prior - pc).abs() <= tol
            }
        );
        if quote.prior_close.is_none()
            || quote.previous_day_change_percent.is_none()
            || prior_is_t1
        {
            // Session cache first — skips a second HTTP for crypto / thin meta.
            let cached = prior_cache
                .lock()
                .ok()
                .and_then(|g| g.get(sym).copied());
            if let Some((prior, pct)) = cached {
                if quote.prior_close.is_none() {
                    quote.prior_close = Some(prior);
                }
                if quote.previous_day_change_percent.is_none() {
                    quote.previous_day_change_percent = pct;
                }
            } else if let Ok(daily) =
                Self::chart_json_with(client, base_url, sym, "5d", "1d").await
            {
                enrich_quote_prior_from_daily(&mut quote, &daily);
                if let Some(prior) = quote.prior_close {
                    if let Ok(mut g) = prior_cache.lock() {
                        g.insert(sym.to_string(), (prior, quote.previous_day_change_percent));
                    }
                }
            }
        } else if let Some(prior) = quote.prior_close {
            if let Ok(mut g) = prior_cache.lock() {
                g.insert(sym.to_string(), (prior, quote.previous_day_change_percent));
            }
        }

        Ok(quote)
    }

    /// Yahoo symbol lookup (`/v1/finance/search`) — used for add-flow autocomplete.
    pub async fn search_symbols(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolSuggestion>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let limit = limit.clamp(1, 20);
        let url = format!("{}/v1/finance/search", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("q", q),
                ("quotesCount", &limit.to_string()),
                ("newsCount", "0"),
                ("listsCount", "0"),
                ("enableFuzzyQuery", "false"),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() == 429 {
            return Err("rate_limited".into());
        }
        if !resp.status().is_success() {
            return Err(format!("http {}", resp.status()));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(parse_search_results(&json, q, limit))
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
            max_concurrent: RefreshPolicy::MAX_CONCURRENT,
            // Soft client throttle (not a documented Yahoo SLA). Live smokes were
            // clean well below this with serial batches; parallel caps concurrency.
            min_interval: RefreshPolicy::MIN_QUOTE_INTERVAL,
            prefers_batch: false, // per-symbol chart
        }
    }

    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }

        let max_c = self.limits().max_concurrent.max(1);
        let sem = Arc::new(Semaphore::new(max_c));
        let mut join_set = JoinSet::new();

        for sym in symbols {
            let client = self.client.clone();
            let base = self.base_url.clone();
            let prior = self.prior_cache.clone();
            let sym = sym.clone();
            let sem = sem.clone();
            join_set.spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|_| "semaphore closed".to_string())?;
                Self::fetch_one_quote(&client, &base, &prior, &sym).await
            });
        }

        let mut out = Vec::new();
        let mut last_err: Option<String> = None;
        let mut saw_rate_limit = false;

        while let Some(joined) = join_set.join_next().await {
            match joined {
                Ok(Ok(q)) => out.push(q),
                Ok(Err(e)) => {
                    if e.contains("rate_limited") {
                        saw_rate_limit = true;
                    }
                    last_err = Some(e);
                }
                Err(e) => {
                    last_err = Some(e.to_string());
                }
            }
        }

        if out.is_empty() {
            return Err(if saw_rate_limit {
                "rate_limited".into()
            } else {
                last_err.unwrap_or_else(|| "all quotes failed".into())
            });
        }
        // Partial success: return whatever we got (caller marks only those symbols).
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
    async fn fetch_quotes_partial_success_skips_failed_symbol() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v8/finance/chart/AAPL"))
            .respond_with(ResponseTemplate::new(200).set_body_string(chart_body("AAPL", 110.0)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v8/finance/chart/MSFT"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let provider = YahooProvider::with_base_url(server.uri()).unwrap();
        let quotes = provider
            .fetch_quotes(&[String::from("AAPL"), String::from("MSFT")])
            .await
            .unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].symbol, "AAPL");
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

    #[tokio::test]
    async fn search_symbols_ok_from_mock() {
        let server = MockServer::start().await;
        let body = r#"{
          "quotes": [
            { "symbol": "AAPL", "shortname": "Apple Inc.", "quoteType": "EQUITY", "exchDisp": "NASDAQ" },
            { "symbol": "AAPB", "shortname": "GraniteShares 2x Long AAPL", "quoteType": "EQUITY", "exchDisp": "NASDAQ" }
          ]
        }"#;
        Mock::given(method("GET"))
            .and(path("/v1/finance/search"))
            .and(query_param("q", "AAP"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let provider = YahooProvider::with_base_url(server.uri()).unwrap();
        let hits = provider.search_symbols("AAP", 8).await.unwrap();
        assert!(hits.len() >= 1);
        assert!(hits.iter().any(|h| h.symbol == "AAPL"));
    }
}
