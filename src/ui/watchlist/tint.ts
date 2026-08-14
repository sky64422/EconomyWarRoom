import type { CardTint } from "../types";
import { CARD_TINTS } from "../types";

export function normalizeTint(raw: CardTint | undefined | null): CardTint {
  if (!raw || raw === "none") return "none";
  const ok = CARD_TINTS.some((t) => t.value === raw);
  return ok ? raw : "none";
}

const ADD_TINT_STORAGE_KEY = "ewr.add_card_tint";

export function loadAddCardTint(): CardTint {
  try {
    return normalizeTint(localStorage.getItem(ADD_TINT_STORAGE_KEY) as CardTint | null);
  } catch {
    return "none";
  }
}

export function saveAddCardTint(tint: CardTint): void {
  try {
    if (tint === "none") localStorage.removeItem(ADD_TINT_STORAGE_KEY);
    else localStorage.setItem(ADD_TINT_STORAGE_KEY, tint);
  } catch {
    /* ignore quota / private mode */
  }
}
