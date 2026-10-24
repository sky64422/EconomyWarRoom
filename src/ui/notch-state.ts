import type { Quote } from "./types";

export const CLOSE_DELAY_MS = 180;
export const MAX_CELLS = 6;
export interface NotchState { itemId: string | null; pinned: boolean; settings: boolean; update?: boolean; }
export type NotchEvent =
  | { type: "hover" | "pin"; id: string }
  | { type: "items"; ids: string[] }
  | { type: "settings" | "leave" | "reset" | "update" };
export const initialNotchState = (): NotchState => ({ itemId: null, pinned: false, settings: false, update: false });
export function reduceNotchState(state: NotchState, event: NotchEvent): NotchState {
  switch (event.type) {
    case "items": return state.itemId && !event.ids.includes(state.itemId) ? { ...state, itemId: null, pinned: false } : state;
    case "hover": return state.pinned || state.settings ? state : { ...state, itemId: event.id, update: false };
    case "pin": return state.pinned && state.itemId === event.id ? initialNotchState() : { itemId: event.id, pinned: true, settings: false, update: false };
    case "settings": return { ...state, settings: !state.settings, update: false };
    case "update": return { ...state, update: !state.update, settings: false, itemId: null };
    case "leave": return state.pinned || state.settings ? state : initialNotchState();
    case "reset": return initialNotchState();
  }
}
export function primaryPercent(quote?: Quote): number | null {
  const value = quote?.display?.primary.change;
  return value != null && Number.isFinite(value) ? value : null;
}
export function selectCells(ids: string[], capacity: number) {
  const count = Math.max(1, Math.min(MAX_CELLS, Math.floor(capacity)));
  const overflow = ids.length > count ? ids.length - (count-1) : 0;
  return { ids: ids.slice(0, overflow ? count-1 : count), overflow, empty: ids.length === 0 };
}
