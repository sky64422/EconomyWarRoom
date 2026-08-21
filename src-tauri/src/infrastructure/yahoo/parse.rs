use crate::domain::constants::SparklinePolicy;
use crate::domain::sparkline_math::{downsample, stitch_session_close};
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

/// Session windows for `pre` / `regular` / `post`, oldest → newest.
///
/// Yahoo `tradingPeriods.{name}` is a list of per-day arrays; fall back to
/// `currentTradingPeriod.{name}` when that history is missing.
fn session_windows(meta: &Value, name: &str) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = Vec::new();
    if let Some(days) = meta
        .pointer(&format!("/tradingPeriods/{name}"))
        .and_then(|v| v.as_array())
    {
        for day in days {
            let objs: Vec<&Value> = if let Some(arr) = day.as_array() {
                arr.iter().collect()
            } else {
                vec![day]
            };
            for o in objs {
                let Some(start) = o.get("start").and_then(|v| v.as_i64()) else {
                    continue;
                };
                let Some(end) = o.get("end").and_then(|v| v.as_i64()) else {
                    continue;
                };
                if end > start {
                    out.push((start, end));
                }
            }
        }
    }
    if let Some(cur) = trading_period_bounds(meta, name) {
        if !out.contains(&cur) {
            out.push(cur);
        }
    }
    out.sort_by_key(|(start, _)| *start);
    out.dedup();
    out
}

/// Regular-session windows, oldest → newest.
fn regular_session_windows(meta: &Value) -> Vec<(i64, i64)> {
    session_windows(meta, "regular")
}

/// Last after-hours print: today's post window, else the most recent completed post
/// (overnight/weekend `currentTradingPeriod.post` is *next* session and still empty).
fn last_post_close(meta: &Value, result: &Value, now_secs: i64) -> Option<f64> {
    if let Some((s, e)) = trading_period_bounds(meta, "post") {
        if let Some(c) = last_close_in_period(result, s, e) {
            return Some(c);
        }
    }
    let mut posts: Vec<(i64, i64)> = session_windows(meta, "post")
        .into_iter()
        .filter(|(s, _)| *s < now_secs)
        .collect();
    posts.sort_by_key(|(s, _)| *s);
    for (s, e) in posts.into_iter().rev() {
        if let Some(c) = last_close_in_period(result, s, e) {
            return Some(c);
        }
    }
    last_close_after_elapsed_regular(meta, result, now_secs)
}

/// Bars at/after the last regular session `end` (16:00 is post, not RTH).
fn last_close_after_elapsed_regular(
    meta: &Value,
    result: &Value,
    now_secs: i64,
) -> Option<f64> {
    let regs = regular_session_windows(meta);
    if let Some((_, reg_end)) = regs.iter().copied().rev().find(|(_, e)| *e <= now_secs) {
        let next_pre = session_windows(meta, "pre")
            .into_iter()
            .filter(|(s, _)| *s > reg_end)
            .map(|(s, _)| s)
            .min();
        let post_end = next_pre.unwrap_or(reg_end.saturating_add(14_400));
        return last_close_in_period(result, reg_end, post_end);
    }
    // Meta only lists *upcoming* regular; 1d chart still holds yesterday's post bars.
    last_close_in_period(result, i64::MIN / 4, now_secs)
}

fn points_in_window(
    timestamps: &[Value],
    closes: &[Value],
    start: i64,
    end: i64,
) -> Vec<SparklinePoint> {
    let mut points = Vec::new();
    for (i, t) in timestamps.iter().enumerate() {
        let Some(ts) = t.as_i64() else {
            continue;
        };
        // `[start, end)` — `end` is also post-market start; the 16:00 bar is not RTH.
        if ts < start || ts >= end {
            continue;
        }
        if let Some(c) = closes.get(i).and_then(|c| c.as_f64()) {
            points.push(SparklinePoint { t: ts, close: c });
        }
    }
    points
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
        // Last bar is usually incomplete "today" — never treat it as yesterday.
        let last_search = n.saturating_sub(1).max(1);
        for i in (1..last_search).rev() {
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

fn prior_looks_like_previous_close(quote: &Quote) -> bool {
    match (quote.prior_close, quote.previous_close) {
        (Some(prior), Some(pc)) if pc.is_finite() && pc != 0.0 => {
            let tol = (pc.abs() * 0.005).max(0.01);
            (prior - pc).abs() <= tol
        }
        _ => false,
    }
}

/// When meta lacks a true T-2 close / previous-day %, fill from a multi-day daily chart.
/// Yahoo `regularMarketPreviousClose` is T-1 (same as `previousClose`), not T-2.
pub fn enrich_quote_prior_from_daily(quote: &mut Quote, daily_json: &Value) {
    if quote.prior_close.is_some()
        && quote.previous_day_change_percent.is_some()
        && !prior_looks_like_previous_close(quote)
    {
        return;
    }
    let closes = daily_closes_from_chart(daily_json);
    let derived = derive_prior_close(&closes, quote.previous_close);
    let replace_t1_as_t2 = prior_looks_like_previous_close(quote);
    if quote.prior_close.is_none() || replace_t1_as_t2 {
        quote.prior_close = derived;
    }
    if quote.previous_day_change_percent.is_none() || replace_t1_as_t2 {
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
    // `regularMarketPreviousClose` is the official previous session close (T-1),
    // same role as `previousClose` / `chartPreviousClose` — never T-2.
    let previous_close = meta
        .get("previousClose")
        .or_else(|| meta.get("chartPreviousClose"))
        .or_else(|| meta.get("regularMarketPreviousClose"))
        .and_then(|v| v.as_f64());
    let regular_change_percent = previous_close.and_then(|p| change_pct(p, regular_price));
    let prior_close = None;
    let previous_day_change_percent = None;

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
    let candle_post = last_post_close(meta, result, now_secs);

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

    // Keep the after-hours/pre print even when it equals the regular close (0% move).
    // Dropping it caused CLOSED/POST rows to reuse regular-session % on both lines.
    let extended_price = extended_price.filter(|_| is_extended_state(market_state.as_deref()));

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
        display: None,
        sparkline_change_percent: None,
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
        .or_else(|| meta.get("regularMarketPreviousClose"))
        .and_then(|v| v.as_f64());
    let timestamps = result
        .get("timestamp")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "timestamp".to_string())?;
    let closes = result
        .pointer("/indicators/quote/0/close")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "close".to_string())?;
    let windows = regular_session_windows(meta);
    let current_regular = trading_period_bounds(meta, "regular");
    let series_max_t = timestamps.iter().filter_map(|t| t.as_i64()).max();
    let current_session_started = matches!(
        (current_regular, series_max_t),
        (Some((start, _)), Some(t)) if t >= start
    );

    let mut chosen: Option<(i64, i64, Vec<SparklinePoint>)> = None;
    if current_session_started {
        // LIVE/POST: do not keep yesterday's RTH once today's regular window has
        // timestamps (including a 9:30 open with no close yet).
        if let Some((start, end)) = current_regular {
            let pts = points_in_window(timestamps, closes, start, end);
            chosen = Some((start, end, pts));
        }
    } else {
        for (start, end) in windows.iter().rev() {
            let pts = points_in_window(timestamps, closes, *start, *end);
            if !pts.is_empty() {
                chosen = Some((*start, *end, pts));
                break;
            }
        }
    }

    let (mut points, session_start, session_end, used_regular) = if let Some((start, end, pts)) =
        chosen
    {
        (pts, Some(start), Some(end), true)
    } else if windows.is_empty() {
        // Crypto / missing period: keep the full series.
        let mut pts = Vec::new();
        for (i, t) in timestamps.iter().enumerate() {
            let Some(ts) = t.as_i64() else {
                continue;
            };
            if let Some(c) = closes.get(i).and_then(|c| c.as_f64()) {
                pts.push(SparklinePoint { t: ts, close: c });
            }
        }
        (pts, None, None, false)
    } else {
        // Regular windows exist (PRE) but this payload has no RTH bars yet.
        // Do not plot premarket against yesterday's close.
        (Vec::new(), None, None, false)
    };

    points = downsample(&points, SparklinePolicy::TARGET_POINTS);
    if let (Some(end), true) = (session_end, used_regular) {
        let official = meta.get("regularMarketPrice").and_then(|v| v.as_f64());
        stitch_session_close(&mut points, end, official);
    }
    Ok(Sparkline {
        symbol,
        points,
        previous_close: prev,
        as_of: chrono::Utc::now().to_rfc3339(),
        session_start,
        session_end,
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

    fn googl_5d_pre_fixture() -> Value {
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/yahoo_chart_googl_5d_pre.json"
        ));
        serde_json::from_str(raw).expect("parse googl 5d fixture")
    }

    #[test]
    fn recorded_googl_5d_pre_sparkline_is_yesterday_rth() {
        let s = parse_sparkline_from_chart(&googl_5d_pre_fixture()).unwrap();
        assert_eq!(s.session_start, Some(1787146200));
        assert_eq!(s.session_end, Some(1787169600));
        assert!(s.points.iter().all(|p| p.t < 1787212800 || p.t >= 1787232600));
        let last = s.points.last().unwrap();
        assert_eq!(last.t, 1787169600);
        assert!((last.close - 344.72).abs() < 1e-6);
        assert!(last.close > s.previous_close.unwrap());
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
    fn closed_post_equal_to_regular_keeps_zero_extended_change() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "AAPL",
                  "regularMarketPrice": 190.0,
                  "previousClose": 188.0,
                  "postMarketPrice": 190.0,
                  "postMarketChangePercent": 0.0,
                  "marketState": "CLOSED",
                  "currency": "USD"
                }
              }]
            }
        });
        let mut q = parse_quote_from_chart(&v).unwrap();
        assert_eq!(q.extended_price, Some(190.0));
        assert_eq!(q.price, 190.0);
        assert_eq!(q.extended_change_percent, Some(0.0));
        crate::domain::display::attach_display(&mut q, None);
        let d = q.display.expect("display");
        assert_eq!(d.primary.price, Some(190.0));
        assert_eq!(d.primary.change, Some(0.0));
        assert_eq!(d.secondary.price, Some(190.0));
        assert!((d.secondary.change.unwrap() - ((190.0 - 188.0) / 188.0 * 100.0)).abs() < 1e-9);
    }

    #[test]
    fn overnight_closed_uses_prior_session_post_last_print() {
        // Fri 00:09 ET: currentTradingPeriod is *today*, but 1d bars still hold Thu post.
        let thu_reg_start = 1_787_232_600i64;
        let thu_reg_end = 1_787_256_000;
        let thu_post_end = thu_reg_end + 14_400;
        let fri_pre_start = 1_787_299_200;
        let fri_pre_end = 1_787_319_000;
        let fri_reg_end = 1_787_342_400;
        let now = 1_787_285_363; // after Thu post, before Fri pre
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "TSLA",
                  "regularMarketPrice": 345.13,
                  "previousClose": 351.12,
                  "currency": "USD",
                  "currentTradingPeriod": {
                    "pre": { "start": fri_pre_start, "end": fri_pre_end },
                    "regular": { "start": fri_pre_end, "end": fri_reg_end },
                    "post": { "start": fri_reg_end, "end": fri_reg_end + 14_400 }
                  },
                  "tradingPeriods": {
                    "regular": [[{ "start": thu_reg_start, "end": thu_reg_end }]]
                  }
                },
                "timestamp": [thu_reg_end - 60, thu_reg_end + 60, thu_post_end - 60],
                "indicators": { "quote": [{ "close": [345.13, 345.5, 346.3] }] }
              }]
            }
        });
        let mut q = parse_quote_from_chart_at(&v, now).unwrap();
        assert_eq!(q.market_state.as_deref(), Some("closed"));
        assert_eq!(q.regular_price, Some(345.13));
        assert_eq!(q.extended_price, Some(346.3));
        assert_eq!(q.price, 346.3);
        let chg = q.extended_change_percent.unwrap();
        assert!((chg - ((346.3 - 345.13) / 345.13 * 100.0)).abs() < 1e-6);
        crate::domain::display::attach_display(&mut q, None);
        let d = q.display.expect("display");
        assert_eq!(d.primary.price, Some(346.3));
        assert_eq!(d.secondary.price, Some(345.13));
    }

    #[test]
    fn overnight_closed_without_historical_periods_uses_last_chart_print() {
        let thu_reg_end = 1_787_256_000i64;
        let fri_pre_start = 1_787_299_200;
        let fri_pre_end = 1_787_319_000;
        let fri_reg_end = 1_787_342_400;
        let now = 1_787_285_363;
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "NVDA",
                  "regularMarketPrice": 216.85,
                  "previousClose": 217.56,
                  "currency": "USD",
                  "currentTradingPeriod": {
                    "pre": { "start": fri_pre_start, "end": fri_pre_end },
                    "regular": { "start": fri_pre_end, "end": fri_reg_end },
                    "post": { "start": fri_reg_end, "end": fri_reg_end + 14_400 }
                  }
                },
                "timestamp": [thu_reg_end - 60, thu_reg_end + 120],
                "indicators": { "quote": [{ "close": [216.85, 217.05] }] }
              }]
            }
        });
        let q = parse_quote_from_chart_at(&v, now).unwrap();
        assert_eq!(q.market_state.as_deref(), Some("closed"));
        assert_eq!(q.extended_price, Some(217.05));
        assert_eq!(q.price, 217.05);
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
    fn sparkline_keeps_regular_session_only_vs_previous_close() {
        // Premarket sits above yesterday's close; RTH sold off (SPCX-shaped).
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "SPCX",
                  "previousClose": 141.29,
                  "currentTradingPeriod": {
                    "pre": { "start": 1000, "end": 2000 },
                    "regular": { "start": 2000, "end": 3000 },
                    "post": { "start": 3000, "end": 4000 }
                  }
                },
                "timestamp": [1100, 1900, 2000, 2500, 2900],
                "indicators": {
                  "quote": [{ "close": [142.2, 143.0, 141.63, 139.0, 137.02] }]
                }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.previous_close, Some(141.29));
        assert_eq!(s.points.len(), 3);
        assert_eq!(s.points[0].t, 2000);
        assert!((s.points[2].close - 137.02).abs() < 1e-9);
        assert!(s.points.iter().all(|p| p.t >= 2000 && p.t < 3000));
        assert!(s.points.last().unwrap().close < s.previous_close.unwrap());
        assert_eq!(s.session_start, Some(2000));
        assert_eq!(s.session_end, Some(3000));
        let last_x = crate::domain::sparkline_math::session_x(
            s.points.last().unwrap().t,
            s.session_start.unwrap(),
            s.session_end.unwrap(),
        );
        assert!(last_x < 1.0, "RTH tail must stay on-axis, not clipped");
        assert!((last_x - 0.9).abs() < 1e-12);
    }

    #[test]
    fn sparkline_drops_bar_at_regular_end_as_post_and_stitches_official_close() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "GOOGL",
                  "previousClose": 100.0,
                  "regularMarketPrice": 100.15,
                  "currentTradingPeriod": {
                    "regular": { "start": 2000, "end": 3000 },
                    "post": { "start": 3000, "end": 4000 }
                  }
                },
                "timestamp": [2000, 2940, 3000, 3100],
                "indicators": {
                  "quote": [{ "close": [99.5, 99.8, 100.4, 100.5] }]
                }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert!(s.points.iter().all(|p| p.close != 100.4 && p.close != 100.5));
        assert_eq!(s.points.last().unwrap().t, 3000);
        assert!((s.points.last().unwrap().close - 100.15).abs() < 1e-9);
    }

    #[test]
    fn sparkline_stitches_official_close_when_last_bar_is_near_end() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "GOOGL",
                  "previousClose": 100.0,
                  "regularMarketPrice": 100.15,
                  "currentTradingPeriod": {
                    "regular": { "start": 2000, "end": 3000 }
                  }
                },
                "timestamp": [2000, 2500, 2940],
                "indicators": {
                  "quote": [{ "close": [99.5, 99.2, 99.8] }]
                }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.points.last().unwrap().t, 3000);
        assert!((s.points.last().unwrap().close - 100.15).abs() < 1e-9);
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
    fn parse_search_missing_quotes_is_empty() {
        let v = serde_json::json!({ "news": [] });
        assert!(parse_search_results(&v, "aapl", 8).is_empty());
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
    fn quote_type_maps_commodity_and_other() {
        let v: Value = serde_json::json!({
          "quotes": [
            { "symbol": "GC=F", "shortname": "Gold", "quoteType": "FUTURE", "exchDisp": "CME" },
            { "symbol": "ZZZ", "shortname": "Zed", "quoteType": "INDEX", "exchDisp": "X" }
          ]
        });
        let hits = parse_search_results(&v, "", 10);
        assert_eq!(hits[0].asset_kind, AssetKind::Commodity);
        assert_eq!(hits[1].asset_kind, AssetKind::Other);
    }

    #[test]
    fn sparkline_keeps_all_bars_when_regular_period_missing() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": { "symbol": "BTC-USD", "previousClose": 1.0 },
                "timestamp": [1, 2, 3],
                "indicators": { "quote": [{ "close": [1.0, 1.1, 1.2] }] }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.points.len(), 3);
        assert_eq!(s.session_start, None);
        assert_eq!(s.session_end, None);
    }

    #[test]
    fn sparkline_skips_premarket_when_regular_window_has_no_bars() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "X",
                  "chartPreviousClose": 9.0,
                  "currentTradingPeriod": {
                    "regular": { "start": 5000, "end": 6000 }
                  }
                },
                "timestamp": [1, 2],
                "indicators": { "quote": [{ "close": [8.0, 9.0] }] }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.previous_close, Some(9.0));
        assert!(s.points.is_empty());
        assert_eq!(s.session_start, None);
        assert_eq!(s.session_end, None);
    }

    #[test]
    fn sparkline_uses_previous_regular_session_when_today_has_no_rth_bars() {
        // GOOGL PRE: premarket last is below yesterday close; yesterday RTH closed +0.15%.
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "GOOGL",
                  "previousClose": 344.2,
                  "regularMarketPrice": 344.72,
                  "currentTradingPeriod": {
                    "pre": { "start": 8000, "end": 9000 },
                    "regular": { "start": 9000, "end": 10000 }
                  },
                  "tradingPeriods": {
                    "regular": [
                      [{ "start": 2000, "end": 3000 }],
                      [{ "start": 9000, "end": 10000 }]
                    ]
                  }
                },
                "timestamp": [2000, 2500, 2940, 8100, 8500],
                "indicators": {
                  "quote": [{ "close": [344.0, 344.4, 344.5, 343.1, 342.9] }]
                }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.session_start, Some(2000));
        assert_eq!(s.session_end, Some(3000));
        assert!(s.points.iter().all(|p| p.t >= 2000 && p.t <= 3000));
        assert!(!s.points.iter().any(|p| p.close < 343.5));
        assert!(!s.points.iter().any(|p| (p.close - 342.9).abs() < 1e-9));
        let last = s.points.last().unwrap();
        assert_eq!(last.t, 3000);
        assert!((last.close - 344.72).abs() < 1e-9);
        assert!(last.close > s.previous_close.unwrap());
    }

    fn assert_rth_only(s: &Sparkline, pre: (i64, i64), regular: (i64, i64), post: (i64, i64)) {
        assert_eq!(s.session_start, Some(regular.0));
        assert_eq!(s.session_end, Some(regular.1));
        for p in &s.points {
            let in_pre = p.t >= pre.0 && p.t < pre.1;
            let in_post = p.t > post.0 && p.t < post.1;
            assert!(!in_pre, "premarket print leaked t={}", p.t);
            assert!(!in_post, "post print leaked t={}", p.t);
            // t == regular.end is the official-close stitch, not a Yahoo post bar.
            assert!(
                p.t >= regular.0 && p.t <= regular.1,
                "point t={} outside RTH [{}, {}]",
                p.t,
                regular.0,
                regular.1
            );
            if p.t == regular.1 {
                continue;
            }
            assert!(p.t < regular.1, "Yahoo bar at regular.end is post t={}", p.t);
        }
    }

    #[test]
    fn sparkline_live_uses_today_rth_not_yesterday() {
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "GOOGL",
                  "previousClose": 344.72,
                  "regularMarketPrice": 345.0,
                  "currentTradingPeriod": {
                    "pre": { "start": 8000, "end": 9000 },
                    "regular": { "start": 9000, "end": 10000 },
                    "post": { "start": 10000, "end": 11000 }
                  },
                  "tradingPeriods": {
                    "regular": [
                      [{ "start": 2000, "end": 3000 }],
                      [{ "start": 9000, "end": 10000 }]
                    ]
                  }
                },
                "timestamp": [2000, 2940, 8100, 9100, 9200],
                "indicators": {
                  "quote": [{ "close": [344.0, 344.72, 343.0, 344.8, 345.0] }]
                }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_rth_only(&s, (8000, 9000), (9000, 10000), (10000, 11000));
        assert_eq!(s.points[0].t, 9100);
        assert!(s.points.iter().all(|p| p.t != 8100 && p.t != 2000));
    }

    #[test]
    fn sparkline_live_resets_even_when_today_has_no_rth_closes_yet() {
        // First LIVE print: series already has t >= today's open, but no RTH close yet.
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "GOOGL",
                  "previousClose": 344.72,
                  "regularMarketPrice": 344.80,
                  "currentTradingPeriod": {
                    "pre": { "start": 8000, "end": 9000 },
                    "regular": { "start": 9000, "end": 10000 },
                    "post": { "start": 10000, "end": 11000 }
                  },
                  "tradingPeriods": {
                    "regular": [
                      [{ "start": 2000, "end": 3000 }],
                      [{ "start": 9000, "end": 10000 }]
                    ]
                  }
                },
                "timestamp": [2000, 2940, 8100, 9000],
                "indicators": {
                  "quote": [{ "close": [344.0, 344.72, 343.0, null] }]
                }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.session_start, Some(9000));
        assert_eq!(s.session_end, Some(10000));
        assert!(s.points.is_empty());
    }

    #[test]
    fn sparkline_never_plots_yahoo_bar_timestamped_at_regular_end() {
        // Yahoo 5m: regular.end == post.start; the 16:00 bar is the first AH print.
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "GOOGL",
                  "previousClose": 344.2,
                  "regularMarketPrice": 344.72,
                  "currentTradingPeriod": {
                    "pre": { "start": 1000, "end": 2000 },
                    "regular": { "start": 2000, "end": 3000 },
                    "post": { "start": 3000, "end": 4000 }
                  }
                },
                "timestamp": [1500, 2000, 2700, 2940, 3000, 3100],
                "indicators": {
                  "quote": [{ "close": [343.0, 344.1, 344.4, 344.78, 344.83, 344.9] }]
                }
              }]
            }
        });
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_rth_only(&s, (1000, 2000), (2000, 3000), (3000, 4000));
        assert!(!s.points.iter().any(|p| (p.close - 344.83).abs() < 1e-9));
        assert!(!s.points.iter().any(|p| (p.close - 344.9).abs() < 1e-9));
        assert!(!s.points.iter().any(|p| (p.close - 343.0).abs() < 1e-9));
        let last = s.points.last().unwrap();
        assert_eq!(last.t, 3000);
        assert!((last.close - 344.72).abs() < 1e-9);
        assert!(last.close > s.previous_close.unwrap());
    }

    #[test]
    fn derive_prior_close_matches_previous_close_bar() {
        // oldest → newest; last is partial "today"
        let closes = vec![100.0, 110.0, 105.0, 120.0, 118.0];
        assert_eq!(derive_prior_close(&closes, Some(120.0)), Some(105.0));
        assert_eq!(derive_prior_close(&closes, Some(105.0)), Some(110.0));
    }

    #[test]
    fn derive_prior_close_does_not_treat_today_bar_as_yesterday() {
        // LIVE session: last print is near yesterday's close (flat day).
        let closes = vec![327.51, 339.96, 339.50];
        assert_eq!(derive_prior_close(&closes, Some(339.96)), Some(327.51));
    }

    #[test]
    fn parse_does_not_use_regular_market_previous_as_t2() {
        // Yahoo's regularMarketPreviousClose is T-1 (same as previousClose), not T-2.
        let v: Value = serde_json::json!({
            "chart": {
              "result": [{
                "meta": {
                  "symbol": "TSLA",
                  "regularMarketPrice": 334.52,
                  "previousClose": 339.96,
                  "regularMarketPreviousClose": 339.96,
                  "currency": "USD",
                  "marketState": "REGULAR"
                }
              }]
            }
        });
        let q = parse_quote_from_chart(&v).unwrap();
        assert_eq!(q.previous_close, Some(339.96));
        assert_eq!(q.prior_close, None);
        assert_eq!(q.previous_day_change_percent, None);
        let today = q.regular_change_percent.unwrap();
        assert!((today - (-1.60)).abs() < 0.02);
    }

    #[test]
    fn enrich_tsla_yesterday_move_from_daily_bars() {
        let mut q = Quote {
            symbol: "TSLA".into(),
            price: 334.52,
            previous_close: Some(339.96),
            regular_price: Some(334.52),
            regular_change_percent: Some(-1.60),
            prior_close: None,
            previous_day_change_percent: None,
            ..Quote::default()
        };
        let daily = serde_json::json!({
            "chart": {
              "result": [{
                "indicators": {
                  "quote": [{
                    "close": [330.88, 332.81, 327.51, 339.96, 334.52]
                  }]
                }
              }]
            }
        });
        enrich_quote_prior_from_daily(&mut q, &daily);
        assert!((q.prior_close.unwrap() - 327.51).abs() < 1e-6);
        let pct = q.previous_day_change_percent.unwrap();
        assert!(
            (pct - 3.80).abs() < 0.02,
            "yesterday move should be +3.80%, got {pct}"
        );
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

    #[test]
    fn enrich_replaces_t1_copied_as_t2() {
        let mut q = Quote {
            previous_close: Some(339.96),
            prior_close: Some(339.96),
            previous_day_change_percent: Some(0.0),
            ..Quote::default()
        };
        let daily = serde_json::json!({
            "chart": {
              "result": [{
                "indicators": {
                  "quote": [{ "close": [327.51, 339.96, 334.52] }]
                }
              }]
            }
        });
        enrich_quote_prior_from_daily(&mut q, &daily);
        assert!((q.prior_close.unwrap() - 327.51).abs() < 1e-6);
        assert!((q.previous_day_change_percent.unwrap() - 3.80).abs() < 0.02);
    }
}
