use crate::domain::types::{Quote, Sparkline};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct QuoteCache {
    map: HashMap<String, (Quote, Instant)>,
}

impl QuoteCache {
    pub fn get(&self, symbol: &str) -> Option<&Quote> {
        self.map.get(symbol).map(|(q, _)| q)
    }

    pub fn put(&mut self, quote: Quote) {
        let sym = quote.symbol.clone();
        self.map.insert(sym, (quote, Instant::now()));
    }

    pub fn age(&self, symbol: &str) -> Option<Duration> {
        self.map.get(symbol).map(|(_, t)| t.elapsed())
    }

    pub fn all(&self) -> Vec<Quote> {
        self.map.values().map(|(q, _)| q.clone()).collect()
    }
}

#[derive(Default)]
pub struct SparklineCache {
    map: HashMap<String, (Sparkline, Instant)>,
}

impl SparklineCache {
    pub fn get(&self, symbol: &str) -> Option<&Sparkline> {
        self.map.get(symbol).map(|(s, _)| s)
    }

    pub fn put(&mut self, spark: Sparkline) {
        let sym = spark.symbol.clone();
        self.map.insert(sym, (spark, Instant::now()));
    }

    pub fn age(&self, symbol: &str) -> Option<Duration> {
        self.map.get(symbol).map(|(_, t)| t.elapsed())
    }

    pub fn all(&self) -> Vec<Sparkline> {
        self.map.values().map(|(s, _)| s.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_quote(sym: &str, price: f64) -> Quote {
        Quote {
            symbol: sym.into(),
            price,
            currency: "USD".into(),
            change_percent: Some(1.0),
            as_of: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
        }
    }

    #[test]
    fn quote_cache_get_put_all() {
        let mut cache = QuoteCache::default();
        assert!(cache.get("AAPL").is_none());
        cache.put(sample_quote("AAPL", 100.0));
        assert_eq!(cache.get("AAPL").map(|q| q.price), Some(100.0));
        assert!(cache.age("AAPL").is_some());
        assert_eq!(cache.all().len(), 1);
    }

    #[test]
    fn sparkline_cache_get_put() {
        let mut cache = SparklineCache::default();
        cache.put(Sparkline {
            symbol: "AAPL".into(),
            points: vec![],
            previous_close: Some(99.0),
            as_of: "2026-01-01T00:00:00Z".into(),
        });
        assert!(cache.get("AAPL").is_some());
        assert!(cache.age("AAPL").is_some());
    }
}
