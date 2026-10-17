import { primaryPercent, selectCells } from "./notch-state";
import { formatChange, changeClass } from "./watchlist/format";
import { markMarkup } from "./stock-marks";
import type { WatchlistItem, Quote, NotchLayout, Rect } from "./types";
export function mountNotch(root: HTMLElement, callbacks: {hover: (id: string) => void; pin: (id: string) => void; more: () => void}) {
  root.setAttribute("aria-label", "관심종목. 드래그로 이동, 우클릭으로 설정.");
  root.innerHTML = `<span class="notch-rest" aria-hidden="true"></span>
    <div class="notch-surface">
      <svg class="notch-shape" aria-hidden="true">
        <path id="notch-outline"/>
        <defs>
          <clipPath id="notch-finish-clip"><use href="#notch-outline"/></clipPath>
          <linearGradient id="notch-glass-rim" x1="0" y1="0" x2="1" y2="1">
            <stop stop-color="#eef6ff" stop-opacity=".9"/>
            <stop offset=".18" stop-color="#f9fcff" stop-opacity=".22"/>
            <stop offset=".36" stop-color="#212a33" stop-opacity=".55"/>
            <stop offset=".48" stop-color="#f4f8ff" stop-opacity=".88"/>
            <stop offset=".61" stop-color="#8393a2" stop-opacity=".28"/>
            <stop offset=".79" stop-color="#eff8ff" stop-opacity=".72"/>
            <stop offset="1" stop-color="#d2e3eb" stop-opacity=".5"/>
          </linearGradient>
          <linearGradient id="notch-liquid-rim" x1="0" y1="0" x2="1" y2="1">
            <stop stop-color="#b4e5ee" stop-opacity=".82"/>
            <stop offset=".28" stop-color="#c1b4ec" stop-opacity=".58"/>
            <stop offset=".52" stop-color="#f1bed8" stop-opacity=".52"/>
            <stop offset=".74" stop-color="#f2d5aa" stop-opacity=".66"/>
            <stop offset="1" stop-color="#b9e5d2" stop-opacity=".64"/>
          </linearGradient>
        </defs>
        <use class="notch-glass-rim" href="#notch-outline"/>
        <use class="notch-liquid-edge" href="#notch-outline"/>
      </svg>
      <div class="notch-finish" aria-hidden="true"></div>
      <div class="notch-cells"></div>
    </div>`;
  const cells = root.querySelector<HTMLElement>(".notch-cells")!;
  const buttons = new Map<string, HTMLButtonElement>();
  let lastItems = "";
  let currentLayout: NotchLayout | undefined;
  let lastShapeKey = "";
  function shape(layout: NotchLayout) {
    const vertical = layout.edge === "left" || layout.edge === "right";
    const length =
      (vertical ? layout.notch.height : layout.notch.width) / layout.scale;
    const shapeKey = `${layout.edge}:${length}:${layout.scale}:${layout.metrics.content_scale ?? 1}`;
    if (shapeKey === lastShapeKey) return;
    lastShapeKey = shapeKey;

    const {
      depth: DEPTH,
      shoulder: SHOULDER,
      inner_radius: INNER,
    } = layout.metrics;
    const currentScale = layout.metrics.content_scale ?? 1;
    root.style.setProperty("--notch-depth", `${layout.metrics.depth}px`);
    root.style.setProperty("--notch-scale", String(currentScale));
    root.style.setProperty("--notch-cell", `${layout.metrics.cell}px`);
    root.style.setProperty("--notch-inset", `${layout.metrics.inset}px`);
    root.style.setProperty("--rest-depth", `${layout.metrics.rest_depth}px`);
    root.style.setProperty("--rest-length", `${layout.metrics.rest_length}px`);
    const svg = root.querySelector("svg")!;
    svg.setAttribute(
      "viewBox",
      `0 0 ${vertical ? DEPTH : length} ${vertical ? length : DEPTH}`,
    );
    const path = svg.querySelector("path")!;
    path.setAttribute(
      "d",
      `M${DEPTH} 0 A${SHOULDER} ${SHOULDER} 0 0 1 ${INNER} ${SHOULDER} A${INNER} ${INNER} 0 0 0 0 ${SHOULDER + INNER} L0 ${length - SHOULDER - INNER} A${INNER} ${INNER} 0 0 0 ${INNER} ${length - SHOULDER} A${SHOULDER} ${SHOULDER} 0 0 1 ${DEPTH} ${length} Z`,
    );
    path.setAttribute(
      "transform",
      layout.edge === "left"
        ? `translate(${DEPTH} 0) scale(-1 1)`
        : layout.edge === "bottom"
          ? "matrix(0 1 1 0 0 0)"
          : layout.edge === "top"
            ? `matrix(0 -1 1 0 0 ${DEPTH})`
            : "",
    );
  }
  return {
    reveal(open: boolean) { root.classList.toggle("is-folded", !open); cells.inert = !open; root.setAttribute("aria-expanded", String(open)); },
    update(items: WatchlistItem[], quotes: Quote[], capacity: number) {
      const selected = selectCells(items.map(i => i.id), capacity);
      const visible = items.filter(i => selected.ids.includes(i.id));
      const showMore=selected.overflow>0;
      const key = JSON.stringify([visible.map(i => [i.id, i.symbol]), selected.overflow, showMore]);
      if (lastItems !== key) {
        const focusedId = (document.activeElement as HTMLElement)?.dataset.id;
        lastItems = key; buttons.clear(); cells.replaceChildren();
        for (const item of visible) {
          const b = document.createElement("button"); b.type = "button"; b.className = "notch-cell"; b.dataset.id = item.id;
          b.innerHTML = `<span class="ring-wrap"><svg viewBox="0 0 44 44" aria-hidden="true"><circle class="ring-track" cx="22" cy="22" r="19"/></svg>${markMarkup(item.symbol)}</span><span class="ring-meta"><span class="ring-pct">—</span></span>`;
          b.addEventListener("pointerenter", () => callbacks.hover(item.id));
          b.addEventListener("click", () => callbacks.pin(item.id));
          const slot=document.createElement('div');slot.className='notch-item';
          slot.append(b);cells.append(slot);buttons.set(item.id,b);
        }
        if (showMore) {
          const b = document.createElement("button"); b.type = "button"; b.className = "notch-cell notch-more"; b.dataset.id = "__more";
          b.innerHTML = `<span class="ring-wrap more-mark" aria-hidden="true">⋯</span><span class="ring-pct">+${selected.overflow}</span>`;
          b.setAttribute("aria-label", `관심종목 ${selected.overflow}개 더 보기`);
          b.addEventListener("click", callbacks.more); cells.append(b); buttons.set("__more", b);
        }
        if (focusedId) (buttons.get(focusedId) ?? buttons.get("__more") ?? root).focus({preventScroll:true});
      }
      for (const item of visible) {
        const b = buttons.get(item.id)!;
        const pct = primaryPercent(quotes.find(q => q.symbol === item.symbol));
        const text=pct === 0 ? "0.00%" : formatChange(pct);
        const label=`${item.display_name || item.symbol}, ${item.symbol}, ${pct === null ? '시세 대기' : formatChange(pct)}`;
        const className=`notch-cell ${changeClass(pct)}`;
        const percent=b.querySelector(".ring-pct")!;
        if(percent.textContent!==text)percent.textContent=text;
        if(b.className!==className)b.className=className;
        if(b.getAttribute("aria-label")!==label)b.setAttribute("aria-label",label);
      }
      if (currentLayout) shape(currentLayout);
    },
    layout(l: NotchLayout) { currentLayout=l; root.dataset.edge=l.edge; positionNotch(root,l.notch,l); shape(l); },
    select(id: string | null, pinned: boolean) { for (const [key,b] of buttons) { b.dataset.detailOpen=String(key===id); b.setAttribute("aria-pressed",String(pinned && key===id)); } },
    focus(id: string | null) { (id ? buttons.get(id) ?? root : root).focus(); },
    target(id: string | null) { const b=id ? buttons.get(id) : undefined; if (!b || !currentLayout) return null;const slot=b.closest<HTMLElement>('.notch-item') ?? b; return currentLayout.edge==='left'||currentLayout.edge==='right' ? slot.offsetTop+slot.offsetHeight/2+currentLayout.metrics.inset : slot.offsetLeft+slot.offsetWidth/2+currentLayout.metrics.inset; },
  };
}
export function positionNotch(el: HTMLElement, r: Rect, l: NotchLayout) {
  const width = `${r.width / l.scale}px`;
  const height = `${r.height / l.scale}px`;
  if (el.style.width !== width) el.style.width = width;
  if (el.style.height !== height) el.style.height = height;

  if (l.anchor_x === "right") {
    const right = `${((l.window.x + l.window.width) - (r.x + r.width)) / l.scale}px`;
    if (el.style.right !== right) el.style.right = right;
    if (el.style.left !== "") el.style.left = "";
  } else {
    const left = `${(r.x - l.window.x) / l.scale}px`;
    if (el.style.left !== left) el.style.left = left;
    if (el.style.right !== "") el.style.right = "";
  }

  if (l.anchor_y === "bottom") {
    const bottom = `${((l.window.y + l.window.height) - (r.y + r.height)) / l.scale}px`;
    if (el.style.bottom !== bottom) el.style.bottom = bottom;
    if (el.style.top !== "") el.style.top = "";
  } else {
    const top = `${(r.y - l.window.y) / l.scale}px`;
    if (el.style.top !== top) el.style.top = top;
    if (el.style.bottom !== "") el.style.bottom = "";
  }
}
export function position(el: HTMLElement, r: Rect, l: NotchLayout) {
  const left = `${(r.x - l.window.x) / l.scale}px`;
  const top = `${(r.y - l.window.y) / l.scale}px`;
  const width = `${r.width / l.scale}px`;
  const height = `${r.height / l.scale}px`;
  if (el.style.left !== left) el.style.left = left;
  if (el.style.top !== top) el.style.top = top;
  if (el.style.width !== width) el.style.width = width;
  if (el.style.height !== height) el.style.height = height;
}
