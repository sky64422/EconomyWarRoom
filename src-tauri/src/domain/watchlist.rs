use super::types::{AssetKind, WatchlistItem};
use uuid::Uuid;

pub fn normalize_symbol(raw: &str) -> String {
    raw.trim().to_uppercase()
}

pub fn next_sort_index(items: &[WatchlistItem]) -> u32 {
    items
        .iter()
        .map(|i| i.sort_index)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0)
}

pub fn add_item(
    items: &mut Vec<WatchlistItem>,
    symbol: &str,
    asset_kind: AssetKind,
    display_name: Option<String>,
) -> Result<WatchlistItem, String> {
    let symbol = normalize_symbol(symbol);
    if symbol.is_empty() {
        return Err("symbol empty".into());
    }
    if items.iter().any(|i| i.symbol == symbol) {
        return Err(format!("duplicate symbol {symbol}"));
    }
    let item = WatchlistItem {
        id: Uuid::new_v4().to_string(),
        symbol,
        display_name,
        asset_kind,
        sort_index: next_sort_index(items),
    };
    items.push(item.clone());
    Ok(item)
}

pub fn remove_item(items: &mut Vec<WatchlistItem>, id: &str) -> bool {
    let before = items.len();
    items.retain(|i| i.id != id);
    if items.len() != before {
        reindex(items);
        true
    } else {
        false
    }
}

/// `ordered_ids` is the full list of ids in the new visual order.
pub fn reorder(items: &mut Vec<WatchlistItem>, ordered_ids: &[String]) -> Result<(), String> {
    if ordered_ids.len() != items.len() {
        return Err("ordered_ids length mismatch".into());
    }
    let mut map: std::collections::HashMap<String, WatchlistItem> =
        items.drain(..).map(|i| (i.id.clone(), i)).collect();
    let mut next = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        let item = map.remove(id).ok_or_else(|| format!("unknown id {id}"))?;
        next.push(item);
    }
    if !map.is_empty() {
        return Err("ordered_ids missing some items".into());
    }
    *items = next;
    reindex(items);
    Ok(())
}

fn reindex(items: &mut [WatchlistItem]) {
    for (idx, item) in items.iter_mut().enumerate() {
        item.sort_index = idx as u32;
    }
}

pub fn sorted_clone(items: &[WatchlistItem]) -> Vec<WatchlistItem> {
    let mut v = items.to_vec();
    v.sort_by_key(|i| i.sort_index);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_appends_at_bottom() {
        let mut items = vec![];
        add_item(&mut items, "aapl", AssetKind::Equity, None).unwrap();
        add_item(&mut items, "btc-usd", AssetKind::Crypto, None).unwrap();
        assert_eq!(items[0].symbol, "AAPL");
        assert_eq!(items[0].sort_index, 0);
        assert_eq!(items[1].symbol, "BTC-USD");
        assert_eq!(items[1].sort_index, 1);
    }

    #[test]
    fn reject_duplicate() {
        let mut items = vec![];
        add_item(&mut items, "MSFT", AssetKind::Equity, None).unwrap();
        assert!(add_item(&mut items, "msft", AssetKind::Equity, None).is_err());
    }

    #[test]
    fn reorder_updates_sort_index() {
        let mut items = vec![];
        let a = add_item(&mut items, "A", AssetKind::Equity, None).unwrap();
        let b = add_item(&mut items, "B", AssetKind::Equity, None).unwrap();
        reorder(&mut items, &[b.id.clone(), a.id.clone()]).unwrap();
        assert_eq!(items[0].symbol, "B");
        assert_eq!(items[0].sort_index, 0);
        assert_eq!(items[1].symbol, "A");
        assert_eq!(items[1].sort_index, 1);
    }

    #[test]
    fn remove_reindexes() {
        let mut items = vec![];
        let a = add_item(&mut items, "A", AssetKind::Equity, None).unwrap();
        add_item(&mut items, "B", AssetKind::Equity, None).unwrap();
        assert!(remove_item(&mut items, &a.id));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].sort_index, 0);
        assert_eq!(items[0].symbol, "B");
    }
}
