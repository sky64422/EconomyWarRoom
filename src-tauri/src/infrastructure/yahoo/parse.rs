use crate::domain::constants::SparklinePolicy;
use crate::domain::sparkline_math::downsample;
use crate::domain::types::{Quote, Sparkline, SparklinePoint};
use serde_json::Value;

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
    let price = meta
        .get("regularMarketPrice")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "price".to_string())?;
    let currency = meta
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("USD")
        .to_string();
    let prev = meta
        .get("previousClose")
        .or_else(|| meta.get("chartPreviousClose"))
        .and_then(|v| v.as_f64());
    let change_percent = prev.filter(|p| *p != 0.0).map(|p| (price - p) / p * 100.0);
    Ok(Quote {
        symbol,
        price,
        currency,
        change_percent,
        as_of: chrono::Utc::now().to_rfc3339(),
        source: "yahoo".into(),
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
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.symbol, "AAPL");
        assert_eq!(s.points.len(), 3);
        assert_eq!(s.previous_close, Some(188.0));
        assert_eq!(s.points[0].t, 1000);
        assert!((s.points[2].close - 190.5).abs() < 1e-9);
    }
}
