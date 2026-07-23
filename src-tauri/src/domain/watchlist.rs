use super::types::{AssetKind, CardTint, WatchlistItem};
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
        card_tint: CardTint::None,
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

/// Remove many items by id; returns how many were removed. Reindexes once.
pub fn remove_items(items: &mut Vec<WatchlistItem>, ids: &[String]) -> usize {
    if ids.is_empty() {
        return 0;
    }
    let drop: std::collections::HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
    let before = items.len();
    items.retain(|i| !drop.contains(i.id.as_str()));
    let removed = before - items.len();
    if removed > 0 {
        reindex(items);
    }
    removed
}

pub fn set_card_tint(items: &mut [WatchlistItem], id: &str, tint: CardTint) -> bool {
    if let Some(item) = items.iter_mut().find(|i| i.id == id) {
        item.card_tint = tint;
        true
    } else {
        false
    }
}

/// `ordered_ids` is the full list of ids in the new visual order.
/// On error, `items` is left unchanged (atomic).
pub fn reorder(items: &mut Vec<WatchlistItem>, ordered_ids: &[String]) -> Result<(), String> {
    if ordered_ids.len() != items.len() {
        return Err("ordered_ids length mismatch".into());
    }
    let map: std::collections::HashMap<&str, &WatchlistItem> =
        items.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut next = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        let item = map
            .get(id.as_str())
            .ok_or_else(|| format!("unknown id {id}"))?;
        next.push((*item).clone());
    }
    // Detect duplicates in ordered_ids (same id twice ⇒ map size mismatch).
    let unique: std::collections::HashSet<&str> = ordered_ids.iter().map(|s| s.as_str()).collect();
    if unique.len() != ordered_ids.len() {
        return Err("ordered_ids contains duplicates".into());
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

    #[test]
    fn reject_empty_and_whitespace_symbol() {
        let mut items = vec![];
        assert!(add_item(&mut items, "   ", AssetKind::Equity, None).is_err());
        assert!(add_item(&mut items, "", AssetKind::Equity, None).is_err());
    }

    #[test]
    fn remove_unknown_returns_false() {
        let mut items = vec![];
        add_item(&mut items, "A", AssetKind::Equity, None).unwrap();
        assert!(!remove_item(&mut items, "missing"));
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn reorder_rejects_length_mismatch_and_unknown_id_atomically() {
        let mut items = vec![];
        let a = add_item(&mut items, "A", AssetKind::Equity, None).unwrap();
        let b = add_item(&mut items, "B", AssetKind::Equity, None).unwrap();
        assert!(reorder(&mut items, &[a.id.clone()]).is_err());
        assert_eq!(items.len(), 2);
        assert!(reorder(&mut items, &[a.id.clone(), "x".into()]).is_err());
        assert_eq!(items.len(), 2);
        assert!(reorder(&mut items, &[a.id.clone(), a.id.clone()]).is_err());
        assert_eq!(items.len(), 2);
        reorder(&mut items, &[b.id.clone(), a.id.clone()]).unwrap();
        assert_eq!(sorted_clone(&items)[0].symbol, "B");
    }

    #[test]
    fn next_sort_index_empty_is_zero() {
        assert_eq!(next_sort_index(&[]), 0);
    }

    #[test]
    fn normalize_symbol_trims_and_uppercases() {
        assert_eq!(normalize_symbol("  btc-usd "), "BTC-USD");
    }

    #[test]
    fn set_card_tint_and_remove_items() {
        let mut items = vec![];
        let a = add_item(&mut items, "A", AssetKind::Equity, None).unwrap();
        let b = add_item(&mut items, "B", AssetKind::Equity, None).unwrap();
        assert!(set_card_tint(&mut items, &a.id, CardTint::Mint));
        assert_eq!(items[0].card_tint, CardTint::Mint);
        assert_eq!(remove_items(&mut items, &[a.id.clone(), b.id.clone()]), 2);
        assert!(items.is_empty());
    }
}
