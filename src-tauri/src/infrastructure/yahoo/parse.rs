use crate::domain::constants::SparklinePolicy;
use crate::domain::sparkline_math::downsample;
use crate::domain::types::{AssetKind, Quote, Sparkline, SparklinePoint, SymbolSuggestion};
use serde_json::Value;

fn quote_type_to_kind(qt: &str) -> AssetKind {
    let u = qt.to_ascii_uppercase();
    if u.contains("CRYPTO") {
        AssetKind::Crypto
    } else if u.contains("EQUITY") || u.contains("ETF") || u.contains("MUTUAL") {
        AssetKind::Equity
    } else if u.contains("FUTURE") || u.contains("COMMODITY") {
        AssetKind::Commodity
    } else {
        AssetKind::Other
    }
}

/// Parse Yahoo `/v1/finance/search` JSON into suggestions (quotes only).
pub fn parse_search_results(json: &Value, query: &str, limit: usize) -> Vec<SymbolSuggestion> {
    let q = query.trim().to_ascii_uppercase();
    let Some(quotes) = json.get("quotes").and_then(|v| v.as_array()) else {
        return vec![];
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in quotes {
        let Some(symbol) = item.get("symbol").and_then(|v| v.as_str()) else {
            continue;
        };
        let symbol = symbol.trim().to_ascii_uppercase();
        if symbol.is_empty() || !seen.insert(symbol.clone()) {
            continue;
        }
        // Prefer substring match on symbol or name when query present.
        let name = item
            .get("shortname")
            .or_else(|| item.get("longname"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if !q.is_empty() {
            let name_u = name.as_deref().unwrap_or("").to_ascii_uppercase();
            if !symbol.contains(&q) && !name_u.contains(&q) {
                continue;
            }
        }
        let qt = item
            .get("quoteType")
            .and_then(|v| v.as_str())
            .unwrap_or("EQUITY");
        let exchange = item
            .get("exchDisp")
            .or_else(|| item.get("exchange"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        out.push(SymbolSuggestion {
            symbol,
            name,
            asset_kind: quote_type_to_kind(qt),
            exchange,
        });
        if out.len() >= limit {
            break;
        }
    }
    out
}

pub fn parse_quote_from_chart(json: &Value) -> Result<Quote, String> {
    let result = json
        .pointer("/chart/result/0")
        .ok_or_else(|| "missing chart.result".to_string())?;
    let meta = result
        .get("meta")
        .ok_or_else(|| "missing meta".to_string())?;
    let symbol = meta
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "symbol".to_string())?
        .to_string();
    let regular_price = meta
        .get("regularMarketPrice")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "price".to_string())?;
    let currency = meta
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("USD")
        .to_string();
    let previous_close = meta
        .get("previousClose")
        .or_else(|| meta.get("chartPreviousClose"))
        .and_then(|v| v.as_f64());
    let prior_close = meta
        .get("regularMarketPreviousClose")
        .and_then(|v| v.as_f64());
    let regular_change_percent = previous_close
        .filter(|p| *p != 0.0)
        .map(|p| (regular_price - p) / p * 100.0);
    let previous_day_change_percent = match (previous_close, prior_close) {
        (Some(pc), Some(prior)) if prior != 0.0 => Some((pc - prior) / prior * 100.0),
        _ => None,
    };
    let market_state = meta
        .get("marketState")
        .or_else(|| meta.get("regularMarketState"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_ascii_lowercase());

    let pre_price = meta.get("preMarketPrice").and_then(|v| v.as_f64());
    let post_price = meta.get("postMarketPrice").and_then(|v| v.as_f64());
    let pre_change = meta
        .get("preMarketChangePercent")
        .and_then(|v| v.as_f64());
    let post_change = meta
        .get("postMarketChangePercent")
        .and_then(|v| v.as_f64());

    let (extended_price, extended_change_percent) = match market_state.as_deref() {
        Some("pre") | Some("prepre") => (pre_price, pre_change),
        Some("post") | Some("postpost") => (post_price, post_change),
        Some("closed") => {
            if post_price.is_some() {
                (post_price, post_change)
            } else {
                (pre_price, pre_change)
            }
        }
        _ => {
            if post_price.is_some() {
                (post_price, post_change)
            } else if pre_price.is_some() {
                (pre_price, pre_change)
            } else {
                (None, None)
            }
        }
    };

    let extended_change_percent = extended_change_percent.or_else(|| {
        extended_price
            .filter(|_| regular_price != 0.0)
            .map(|ext| (ext - regular_price) / regular_price * 100.0)
    });

    let in_extended = extended_price.is_some()
        && matches!(
            market_state.as_deref(),
            Some("pre") | Some("prepre") | Some("post") | Some("postpost") | Some("closed")
        );

    let (price, change_percent) = if in_extended {
        (
            extended_price.unwrap_or(regular_price),
            extended_change_percent.or(regular_change_percent),
        )
    } else {
        (regular_price, regular_change_percent)
    };

    Ok(Quote {
        symbol,
        price,
        currency,
        change_percent,
        as_of: chrono::Utc::now().to_rfc3339(),
        source: "yahoo".into(),
        previous_close,
        regular_price: Some(regular_price),
        regular_change_percent,
        extended_price,
        extended_change_percent,
        prior_close,
        previous_day_change_percent,
        market_state,
    })
}

pub fn parse_sparkline_from_chart(json: &Value) -> Result<Sparkline, String> {
    let result = json
        .pointer("/chart/result/0")
        .ok_or_else(|| "missing chart.result".to_string())?;
    let meta = result
        .get("meta")
        .ok_or_else(|| "missing meta".to_string())?;
    let symbol = meta
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "symbol".to_string())?
        .to_string();
    let prev = meta
        .get("previousClose")
        .or_else(|| meta.get("chartPreviousClose"))
        .and_then(|v| v.as_f64());
    let timestamps = result
        .get("timestamp")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "timestamp".to_string())?;
    let closes = result
        .pointer("/indicators/quote/0/close")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "close".to_string())?;
    let mut points = Vec::new();
    for (i, t) in timestamps.iter().enumerate() {
        let Some(ts) = t.as_i64() else {
            continue;
        };
        let close = closes.get(i).and_then(|c| c.as_f64());
        if let Some(c) = close {
            points.push(SparklinePoint { t: ts, close: c });
        }
    }
    let points = downsample(&points, SparklinePolicy::TARGET_POINTS);
    Ok(Sparkline {
        symbol,
        points,
        previous_close: prev,
        as_of: chrono::Utc::now().to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_json() -> Value {
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/yahoo_chart_aapl.json"
        ));
        serde_json::from_str(raw).expect("parse fixture")
    }

    #[test]
    fn parses_fixture_quote_and_spark() {
        let v = fixture_json();
        let q = parse_quote_from_chart(&v).unwrap();
        assert_eq!(q.symbol, "AAPL");
        assert!((q.price - 190.5).abs() < 1e-9);
        assert!(q.change_percent.unwrap() > 0.0);
        assert_eq!(q.currency, "USD");
        assert_eq!(q.source, "yahoo");
        assert_eq!(q.previous_close, Some(188.0));
        assert_eq!(q.regular_price, Some(190.5));
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.symbol, "AAPL");
        assert_eq!(s.points.len(), 3);
        assert_eq!(s.previous_close, Some(188.0));
        assert_eq!(s.points[0].t, 1000);
        assert!((s.points[2].close - 190.5).abs() < 1e-9);
    }

    #[test]
    fn parses_post_market_quote() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "AAPL",
                  "regularMarketPrice": 190.0,
                  "previousClose": 188.0,
                  "postMarketPrice": 191.5,
                  "postMarketChangePercent": 0.79,
                  "marketState": "POST",
                  "currency": "USD"
                }
              }]
            }
        });
        let q = parse_quote_from_chart(&v).unwrap();
        assert_eq!(q.price, 191.5);
        assert_eq!(q.change_percent, Some(0.79));
        assert_eq!(q.regular_price, Some(190.0));
        assert_eq!(q.extended_price, Some(191.5));
        assert_eq!(q.extended_change_percent, Some(0.79));
        assert_eq!(q.market_state.as_deref(), Some("post"));
    }

    #[test]
    fn computes_extended_change_when_percent_missing() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "AAPL",
                  "regularMarketPrice": 200.0,
                  "previousClose": 195.0,
                  "postMarketPrice": 202.0,
                  "marketState": "POST",
                  "currency": "USD"
                }
              }]
            }
        });
        let q = parse_quote_from_chart(&v).unwrap();
        assert_eq!(q.extended_change_percent, Some(1.0));
    }

    #[test]
    fn missing_result_is_error() {
        let v: Value = serde_json::json!({"chart": {"result": [], "error": null}});
        assert!(parse_quote_from_chart(&v).is_err());
        assert!(parse_sparkline_from_chart(&v).is_err());
    }

    #[test]
    fn missing_price_is_error() {
        let v: Value = serde_json::json!({
            "chart": { "result": [{ "meta": { "symbol": "X" } }] }
        });
        assert!(parse_quote_from_chart(&v).is_err());
    }

    #[test]
    fn previous_close_zero_skips_change_percent() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "Z",
                  "regularMarketPrice": 10.0,
                  "previousClose": 0.0,
                  "currency": "USD"
                }
              }]
            }
        });
        let q = parse_quote_from_chart(&v).unwrap();
        assert!(q.change_percent.is_none());
    }

    #[test]
    fn sparkline_skips_null_closes() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": { "symbol": "N", "previousClose": 1.0 },
                "timestamp": [1, 2, 3],
                "indicators": { "quote": [{ "close": [1.0, null, 3.0] }] }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.points.len(), 2);
        assert_eq!(s.points[1].close, 3.0);
    }

    #[test]
    fn parse_search_filters_substring_and_maps_crypto() {
        let v: Value = serde_json::json!({
          "quotes": [
            { "symbol": "AAPL", "shortname": "Apple Inc.", "quoteType": "EQUITY", "exchDisp": "NASDAQ" },
            { "symbol": "BTC-USD", "shortname": "Bitcoin USD", "quoteType": "CRYPTOCURRENCY", "exchDisp": "CCC" },
            { "symbol": "MSFT", "shortname": "Microsoft", "quoteType": "EQUITY", "exchDisp": "NASDAQ" }
          ]
        });
        let hits = parse_search_results(&v, "btc", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].symbol, "BTC-USD");
        assert_eq!(hits[0].asset_kind, AssetKind::Crypto);

        let apple = parse_search_results(&v, "app", 10);
        assert_eq!(apple.len(), 1);
        assert_eq!(apple[0].symbol, "AAPL");
    }
}
