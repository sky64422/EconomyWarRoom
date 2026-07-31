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

/// `[start, end)` bounds for a named period under `meta.currentTradingPeriod`.
fn trading_period_bounds(meta: &Value, name: &str) -> Option<(i64, i64)> {
    let p = meta.pointer(&format!("/currentTradingPeriod/{name}"))?;
    let start = p.get("start")?.as_i64()?;
    let end = p.get("end")?.as_i64()?;
    if end > start {
        Some((start, end))
    } else {
        None
    }
}

/// Last non-null close whose bar time falls in `[start, end)`.
fn last_close_in_period(result: &Value, start: i64, end: i64) -> Option<f64> {
    let timestamps = result.get("timestamp")?.as_array()?;
    let closes = result.pointer("/indicators/quote/0/close")?.as_array()?;
    let mut last = None;
    for (i, t) in timestamps.iter().enumerate() {
        let Some(ts) = t.as_i64() else {
            continue;
        };
        if ts < start || ts >= end {
            continue;
        }
        if let Some(c) = closes.get(i).and_then(|c| c.as_f64()) {
            last = Some(c);
        }
    }
    last
}

/// Prefer Yahoo `marketState`; else derive from `currentTradingPeriod` vs `now_secs`.
fn resolve_market_state(meta: &Value, now_secs: i64) -> Option<String> {
    if let Some(s) = meta
        .get("marketState")
        .or_else(|| meta.get("regularMarketState"))
        .and_then(|v| v.as_str())
    {
        return Some(s.to_ascii_lowercase());
    }

    let pre = trading_period_bounds(meta, "pre");
    let regular = trading_period_bounds(meta, "regular");
    let post = trading_period_bounds(meta, "post");
    if pre.is_none() && regular.is_none() && post.is_none() {
        return None;
    }

    if let Some((s, e)) = pre {
        if now_secs >= s && now_secs < e {
            return Some("pre".into());
        }
    }
    if let Some((s, e)) = regular {
        if now_secs >= s && now_secs < e {
            return Some("regular".into());
        }
    }
    if let Some((s, e)) = post {
        if now_secs >= s && now_secs < e {
            return Some("post".into());
        }
    }
    // Outside all windows (overnight / weekend) — treat as closed for extended last print.
    Some("closed".into())
}

fn is_extended_state(state: Option<&str>) -> bool {
    matches!(
        state,
        Some("pre") | Some("prepre") | Some("post") | Some("postpost") | Some("closed")
    )
}

fn change_pct(from: f64, to: f64) -> Option<f64> {
    if from == 0.0 || !from.is_finite() || !to.is_finite() {
        None
    } else {
        Some((to - from) / from * 100.0)
    }
}

/// Non-null finite daily closes from a chart payload (oldest → newest).
fn daily_closes_from_chart(json: &Value) -> Vec<f64> {
    let Some(result) = json.pointer("/chart/result/0") else {
        return vec![];
    };
    let Some(closes) = result
        .pointer("/indicators/quote/0/close")
        .and_then(|v| v.as_array())
    else {
        return vec![];
    };
    closes
        .iter()
        .filter_map(|c| c.as_f64())
        .filter(|c| c.is_finite())
        .collect()
}

/// Pick the session close immediately before `previous_close` from daily bars.
///
/// Crypto chart meta often omits `regularMarketPreviousClose`. Daily bars from
/// `range=5d&interval=1d` still carry multi-day closes so we can recover it.
pub fn derive_prior_close(daily_closes: &[f64], previous_close: Option<f64>) -> Option<f64> {
    let n = daily_closes.len();
    if n < 2 {
        return None;
    }

    if let Some(pc) = previous_close.filter(|p| p.is_finite() && *p != 0.0) {
        let tol = (pc.abs() * 0.005).max(0.01);
        // Prefer matching from the end (most recent completed days).
        for i in (1..n).rev() {
            if (daily_closes[i] - pc).abs() <= tol {
                return Some(daily_closes[i - 1]);
            }
        }
    }

    // Fallback: last bar is usually partial "today", previous is n-2, prior is n-3.
    if n >= 3 {
        return Some(daily_closes[n - 3]);
    }
    Some(daily_closes[0])
}

/// When meta lacks `prior_close` / previous-day %, fill from a multi-day daily chart.
/// Used for crypto (and any asset Yahoo omits `regularMarketPreviousClose` for).
pub fn enrich_quote_prior_from_daily(quote: &mut Quote, daily_json: &Value) {
    if quote.prior_close.is_some() && quote.previous_day_change_percent.is_some() {
        return;
    }
    let closes = daily_closes_from_chart(daily_json);
    let prior = derive_prior_close(&closes, quote.previous_close);
    if quote.prior_close.is_none() {
        quote.prior_close = prior;
    }
    if quote.previous_day_change_percent.is_none() {
        if let (Some(pc), Some(prior)) = (quote.previous_close, quote.prior_close) {
            quote.previous_day_change_percent = change_pct(prior, pc);
        }
    }
}

pub fn parse_quote_from_chart(json: &Value) -> Result<Quote, String> {
    parse_quote_from_chart_at(json, chrono::Utc::now().timestamp())
}

/// Same as [`parse_quote_from_chart`] but with an injectable clock (tests + session inference).
pub fn parse_quote_from_chart_at(json: &Value, now_secs: i64) -> Result<Quote, String> {
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
    let regular_change_percent = previous_close.and_then(|p| change_pct(p, regular_price));
    let previous_day_change_percent = match (previous_close, prior_close) {
        (Some(pc), Some(prior)) => change_pct(prior, pc),
        _ => None,
    };

    let market_state = resolve_market_state(meta, now_secs);

    // Meta fields are often missing on chart API — fall back to last bar in the period.
    let meta_pre = meta.get("preMarketPrice").and_then(|v| v.as_f64());
    let meta_post = meta.get("postMarketPrice").and_then(|v| v.as_f64());
    let meta_pre_chg = meta.get("preMarketChangePercent").and_then(|v| v.as_f64());
    let meta_post_chg = meta
        .get("postMarketChangePercent")
        .and_then(|v| v.as_f64());

    let candle_pre = trading_period_bounds(meta, "pre")
        .and_then(|(s, e)| last_close_in_period(result, s, e));
    let candle_post = trading_period_bounds(meta, "post")
        .and_then(|(s, e)| last_close_in_period(result, s, e));

    let pre_price = meta_pre.or(candle_pre);
    let post_price = meta_post.or(candle_post);

    let (extended_price, extended_change_meta) = match market_state.as_deref() {
        Some("pre") | Some("prepre") => (pre_price, meta_pre_chg),
        Some("post") | Some("postpost") => (post_price, meta_post_chg),
        Some("closed") => {
            // After hours print preferred; else last pre if that's all we have.
            if post_price.is_some() {
                (post_price, meta_post_chg)
            } else {
                (pre_price, meta_pre_chg)
            }
        }
        // Regular / open / unknown with no session: do not surface stale AH as live.
        _ => (None, None),
    };

    // Only treat as extended when session is non-regular *and* we have a print that is not
    // identical noise to the regular mark (allows candle fallback during pre/post).
    let extended_price = extended_price.filter(|ext| {
        is_extended_state(market_state.as_deref())
            && (ext - regular_price).abs() > 1e-6
    });

    let extended_change_percent = extended_change_meta
        .filter(|c| c.is_finite())
        .or_else(|| extended_price.and_then(|ext| change_pct(regular_price, ext)));

    let in_extended = extended_price.is_some();

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
    fn pre_market_from_candles_when_meta_fields_missing() {
        // Chart API often omits marketState / preMarketPrice; only period + bars remain.
        let pre_start = 1_000_000i64;
        let pre_end = 1_003_600;
        let reg_start = pre_end;
        let reg_end = reg_start + 23_400;
        let post_start = reg_end;
        let post_end = post_start + 14_400;
        let now = pre_start + 1_800; // mid pre (strictly before pre_end)
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "TSLA",
                  "regularMarketPrice": 298.32,
                  "previousClose": 307.44,
                  "currency": "USD",
                  "currentTradingPeriod": {
                    "pre": { "start": pre_start, "end": pre_end },
                    "regular": { "start": reg_start, "end": reg_end },
                    "post": { "start": post_start, "end": post_end }
                  }
                },
                "timestamp": [pre_start + 60, pre_start + 120, pre_start + 180],
                "indicators": {
                  "quote": [{ "close": [300.0, 302.5, 304.28] }]
                }
              }]
            }
        });
        let q = parse_quote_from_chart_at(&v, now).unwrap();
        assert_eq!(q.market_state.as_deref(), Some("pre"));
        assert!((q.price - 304.28).abs() < 1e-9);
        assert_eq!(q.extended_price, Some(304.28));
        assert_eq!(q.regular_price, Some(298.32));
        // Extended % vs regular close
        let ext_chg = q.extended_change_percent.unwrap();
        assert!((ext_chg - ((304.28 - 298.32) / 298.32 * 100.0)).abs() < 1e-6);
        // Regular session % vs previous close
        let reg_chg = q.regular_change_percent.unwrap();
        assert!((reg_chg - ((298.32 - 307.44) / 307.44 * 100.0)).abs() < 1e-6);
        assert!((reg_chg - (-2.97)).abs() < 0.02);
    }

    #[test]
    fn regular_session_does_not_surface_stale_post_print() {
        let reg_start = 2_000_000i64;
        let reg_end = reg_start + 23_400;
        let now = reg_start + 3_600;
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "AAPL",
                  "regularMarketPrice": 190.0,
                  "previousClose": 188.0,
                  "postMarketPrice": 191.5,
                  "currency": "USD",
                  "currentTradingPeriod": {
                    "pre": { "start": reg_start - 5_400, "end": reg_start },
                    "regular": { "start": reg_start, "end": reg_end },
                    "post": { "start": reg_end, "end": reg_end + 14_400 }
                  }
                },
                "timestamp": [reg_start + 60],
                "indicators": { "quote": [{ "close": [190.0] }] }
              }]
            }
        });
        let q = parse_quote_from_chart_at(&v, now).unwrap();
        assert_eq!(q.market_state.as_deref(), Some("regular"));
        assert_eq!(q.extended_price, None);
        assert_eq!(q.price, 190.0);
        assert!(q.regular_change_percent.unwrap() > 0.0);
    }

    #[test]
    fn post_market_last_candle_preferred_over_missing_meta() {
        let post_start = 3_000_000i64;
        let post_end = post_start + 14_400;
        let reg_end = post_start;
        let reg_start = reg_end - 23_400;
        let now = post_start + 600;
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "MSFT",
                  "regularMarketPrice": 400.0,
                  "previousClose": 395.0,
                  "currency": "USD",
                  "currentTradingPeriod": {
                    "pre": { "start": reg_start - 5_400, "end": reg_start },
                    "regular": { "start": reg_start, "end": reg_end },
                    "post": { "start": post_start, "end": post_end }
                  }
                },
                "timestamp": [post_start + 60, post_start + 300],
                "indicators": { "quote": [{ "close": [401.0, 402.5] }] }
              }]
            }
        });
        let q = parse_quote_from_chart_at(&v, now).unwrap();
        assert_eq!(q.market_state.as_deref(), Some("post"));
        assert_eq!(q.extended_price, Some(402.5));
        assert_eq!(q.price, 402.5);
        let chg = q.extended_change_percent.unwrap();
        assert!((chg - 0.625).abs() < 1e-6);
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

    #[test]
    fn derive_prior_close_matches_previous_close_bar() {
        // oldest → newest; last is partial "today"
        let closes = vec![100.0, 110.0, 105.0, 120.0, 118.0];
        assert_eq!(derive_prior_close(&closes, Some(120.0)), Some(105.0));
        assert_eq!(derive_prior_close(&closes, Some(105.0)), Some(110.0));
    }

    #[test]
    fn derive_prior_close_fallback_without_match() {
        let closes = vec![100.0, 110.0, 105.0];
        // No bar near previous_close → assume last is today, prior is n-3
        assert_eq!(derive_prior_close(&closes, Some(999.0)), Some(100.0));
        assert_eq!(derive_prior_close(&closes, None), Some(100.0));
        assert_eq!(derive_prior_close(&[50.0, 55.0], None), Some(50.0));
        assert!(derive_prior_close(&[50.0], Some(50.0)).is_none());
    }

    #[test]
    fn enrich_quote_prior_from_daily_fills_crypto_gap() {
        let mut q = Quote {
            symbol: "BTC-USD".into(),
            price: 63820.0,
            previous_close: Some(64725.0),
            prior_close: None,
            previous_day_change_percent: None,
            ..Quote::default()
        };
        let daily = serde_json::json!({
            "chart": {
              "result": [{
                "meta": { "symbol": "BTC-USD" },
                "indicators": {
                  "quote": [{
                    "close": [63724.9, 63871.4, 63908.2, 64725.3, 63820.4]
                  }]
                }
              }]
            }
        });
        enrich_quote_prior_from_daily(&mut q, &daily);
        assert!((q.prior_close.unwrap() - 63908.2).abs() < 1e-6);
        let pct = q.previous_day_change_percent.unwrap();
        // (64725 - 63908.2) / 63908.2 * 100 ≈ 1.278
        assert!((pct - 1.278).abs() < 0.02);
    }

    #[test]
    fn enrich_quote_prior_skips_when_already_present() {
        let mut q = Quote {
            previous_close: Some(100.0),
            prior_close: Some(90.0),
            previous_day_change_percent: Some(11.11),
            ..Quote::default()
        };
        let daily = serde_json::json!({
            "chart": {
              "result": [{
                "indicators": { "quote": [{ "close": [1.0, 2.0, 3.0] }] }
              }]
            }
        });
        enrich_quote_prior_from_daily(&mut q, &daily);
        assert_eq!(q.prior_close, Some(90.0));
        assert!((q.previous_day_change_percent.unwrap() - 11.11).abs() < 1e-9);
    }
}
