//! Primary / secondary quote rows shown in the widget (PRE / LIVE / POST / CLOSED).
//!
//! Policy matches `docs/ARCHITECTURE.md`:
//! - **Primary** = print for the current session (extended when pre/post).
//! - **Secondary** = last completed regular session (not a repeat of primary %).

use crate::domain::types::{PriceRow, PriceRows, Quote};

const PRICE_EPS: f64 = 0.0001;

pub fn resolve_price_rows(q: Option<&Quote>, spark_prev_close: Option<f64>) -> PriceRows {
    let empty = PriceRow::default();
    let Some(q) = q else {
        return PriceRows {
            primary: empty,
            secondary: empty,
        };
    };

    let previous_close = q.previous_close.or(spark_prev_close);
    let regular_price = q.regular_price.unwrap_or(q.price);
    let regular_change = q
        .regular_change_percent
        .or_else(|| pct_change(previous_close, Some(regular_price)))
        .or_else(|| {
            if !is_extended_quote(q) {
                q.change_percent
            } else {
                None
            }
        });
    let prior_change = q
        .previous_day_change_percent
        .or_else(|| pct_change(q.prior_close, previous_close));

    let use_regular_as_secondary = is_extended_session(q.market_state.as_deref())
        && (previous_close.is_none()
            || (regular_price - previous_close.unwrap_or(regular_price)).abs() > PRICE_EPS);

    let secondary = if use_regular_as_secondary {
        PriceRow {
            price: Some(regular_price),
            change: regular_change,
        }
    } else {
        PriceRow {
            price: previous_close,
            change: prior_change,
        }
    };

    if is_extended_quote(q) {
        if let Some(ext) = q.extended_price {
            return PriceRows {
                primary: PriceRow {
                    price: Some(ext),
                    change: extended_change_percent(q),
                },
                secondary,
            };
        }
    }

    // PRE/POST/CLOSED with no distinct after-hours print: last print is the regular close.
    if is_extended_session(q.market_state.as_deref()) {
        return PriceRows {
            primary: PriceRow {
                price: Some(regular_price),
                change: Some(0.0),
            },
            secondary,
        };
    }

    PriceRows {
        primary: PriceRow {
            price: Some(q.price),
            change: q.change_percent.or(regular_change),
        },
        secondary,
    }
}

/// Sparkline color uses regular-session move when the print is extended.
pub fn sparkline_change_percent(q: Option<&Quote>, spark_prev_close: Option<f64>) -> Option<f64> {
    let q = q?;
    let previous_close = q.previous_close.or(spark_prev_close);
    let regular_price = q.regular_price.unwrap_or(q.price);
    if is_extended_quote(q) {
        return q
            .regular_change_percent
            .or_else(|| pct_change(previous_close, Some(regular_price)));
    }
    q.change_percent
        .or(q.regular_change_percent)
        .or_else(|| pct_change(previous_close, Some(regular_price)))
}

pub fn attach_display(quote: &mut Quote, spark_prev_close: Option<f64>) {
    let snapshot = quote.clone();
    quote.display = Some(resolve_price_rows(Some(&snapshot), spark_prev_close));
    quote.sparkline_change_percent = sparkline_change_percent(Some(&snapshot), spark_prev_close);
}

fn is_extended_session(state: Option<&str>) -> bool {
    let Some(s) = state else {
        return false;
    };
    matches!(
        s.to_ascii_lowercase().as_str(),
        "pre" | "prepre" | "post" | "postpost" | "closed"
    )
}

fn is_extended_quote(q: &Quote) -> bool {
    let Some(ext) = q.extended_price else {
        return false;
    };
    if is_extended_session(q.market_state.as_deref()) {
        return true;
    }
    let reg = q.regular_price.unwrap_or(q.price);
    (ext - reg).abs() > PRICE_EPS
}

fn extended_change_percent(q: &Quote) -> Option<f64> {
    if let Some(p) = q.extended_change_percent {
        if p.is_finite() {
            return Some(p);
        }
    }
    let ext = q.extended_price?;
    let reg = q.regular_price.unwrap_or(q.price);
    pct_change(Some(reg), Some(ext))
}

fn pct_change(from: Option<f64>, to: Option<f64>) -> Option<f64> {
    let from = from?;
    let to = to?;
    if !from.is_finite() || !to.is_finite() || from == 0.0 {
        return None;
    }
    Some((to - from) / from * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote() -> Quote {
        Quote {
            symbol: "AAPL".into(),
            price: 200.0,
            change_percent: Some(2.0),
            previous_close: Some(190.0),
            regular_price: Some(200.0),
            regular_change_percent: Some(5.263),
            prior_close: Some(185.0),
            previous_day_change_percent: Some(2.7),
            market_state: Some("regular".into()),
            ..Default::default()
        }
    }

    #[test]
    fn missing_quote_is_empty_rows() {
        let rows = resolve_price_rows(None, None);
        assert_eq!(rows.primary.price, None);
        assert_eq!(rows.secondary.price, None);
    }

    #[test]
    fn live_secondary_is_yesterday_close() {
        let q = quote();
        let rows = resolve_price_rows(Some(&q), None);
        assert_eq!(rows.primary.price, Some(200.0));
        assert_eq!(rows.primary.change, Some(2.0));
        assert_eq!(rows.secondary.price, Some(190.0));
        assert_eq!(rows.secondary.change, Some(2.7));
    }

    #[test]
    fn post_primary_is_extended_secondary_is_regular() {
        let mut q = quote();
        q.market_state = Some("post".into());
        q.extended_price = Some(202.0);
        q.extended_change_percent = Some(1.0);
        q.price = 202.0;
        let rows = resolve_price_rows(Some(&q), None);
        assert_eq!(rows.primary.price, Some(202.0));
        assert_eq!(rows.primary.change, Some(1.0));
        assert_eq!(rows.secondary.price, Some(200.0));
        assert!((rows.secondary.change.unwrap() - 5.263).abs() < 1e-9);
    }

    #[test]
    fn pre_uses_regular_as_secondary_when_print_differs() {
        let mut q = quote();
        q.market_state = Some("pre".into());
        q.extended_price = Some(191.0);
        q.extended_change_percent = Some(-4.5);
        q.regular_price = Some(190.5);
        q.previous_close = Some(188.0);
        let rows = resolve_price_rows(Some(&q), None);
        assert_eq!(rows.primary.price, Some(191.0));
        assert_eq!(rows.secondary.price, Some(190.5));
    }

    #[test]
    fn spark_prev_close_fills_missing_previous_close() {
        let mut q = quote();
        q.previous_close = None;
        q.previous_day_change_percent = None;
        q.prior_close = Some(180.0);
        let rows = resolve_price_rows(Some(&q), Some(190.0));
        assert_eq!(rows.secondary.price, Some(190.0));
        let pct = rows.secondary.change.unwrap();
        assert!((pct - ((190.0 - 180.0) / 180.0 * 100.0)).abs() < 1e-9);
    }

    #[test]
    fn sparkline_percent_uses_regular_when_extended() {
        let mut q = quote();
        q.market_state = Some("post".into());
        q.extended_price = Some(210.0);
        q.change_percent = Some(99.0);
        let pct = sparkline_change_percent(Some(&q), None).unwrap();
        assert!((pct - 5.263).abs() < 1e-9);
    }

    #[test]
    fn attach_display_fills_quote_view_fields() {
        let mut q = quote();
        attach_display(&mut q, None);
        let d = q.display.expect("display");
        assert_eq!(d.primary.price, Some(200.0));
        assert!(q.sparkline_change_percent.is_some());
    }

    #[test]
    fn regular_change_falls_back_to_quote_change_when_not_extended() {
        let mut q = quote();
        q.regular_change_percent = None;
        q.previous_close = None;
        q.change_percent = Some(-1.5);
        let rows = resolve_price_rows(Some(&q), None);
        assert_eq!(rows.primary.change, Some(-1.5));
    }

    #[test]
    fn sparkline_percent_none_without_quote() {
        assert!(sparkline_change_percent(None, Some(1.0)).is_none());
    }

    #[test]
    fn extended_change_computes_when_percent_is_nan() {
        let mut q = quote();
        q.market_state = Some("post".into());
        q.extended_price = Some(202.0);
        q.extended_change_percent = Some(f64::NAN);
        q.regular_price = Some(200.0);
        let rows = resolve_price_rows(Some(&q), None);
        assert!((rows.primary.change.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn closed_without_extended_print_is_zero_vs_close() {
        let mut q = quote();
        q.market_state = Some("closed".into());
        q.extended_price = None;
        q.extended_change_percent = None;
        q.price = 200.0;
        q.change_percent = Some(5.263);
        let rows = resolve_price_rows(Some(&q), None);
        assert_eq!(rows.primary.price, Some(200.0));
        assert_eq!(rows.primary.change, Some(0.0));
        assert_eq!(rows.secondary.price, Some(200.0));
        assert!((rows.secondary.change.unwrap() - 5.263).abs() < 1e-9);
    }

    #[test]
    fn closed_flat_after_hours_shows_zero_on_primary() {
        let mut q = quote();
        q.market_state = Some("closed".into());
        q.extended_price = Some(200.0);
        q.extended_change_percent = Some(0.0);
        q.price = 200.0;
        q.change_percent = Some(0.0);
        let rows = resolve_price_rows(Some(&q), None);
        assert_eq!(rows.primary.price, Some(200.0));
        assert_eq!(rows.primary.change, Some(0.0));
        assert_eq!(rows.secondary.price, Some(200.0));
        assert!((rows.secondary.change.unwrap() - 5.263).abs() < 1e-9);
    }

    #[test]
    fn pct_change_rejects_zero_base() {
        let mut q = quote();
        q.regular_change_percent = None;
        q.change_percent = None;
        q.previous_close = Some(0.0);
        q.regular_price = Some(10.0);
        q.market_state = Some("regular".into());
        q.extended_price = None;
        let pct = sparkline_change_percent(Some(&q), None);
        assert!(pct.is_none());
    }
}
