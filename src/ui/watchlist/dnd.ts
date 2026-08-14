export function flipRows(listEl: HTMLElement, mutate: () => void): void {
  const rows = Array.from(listEl.querySelectorAll<HTMLElement>(".watchlist-row"));
  const first = new Map<HTMLElement, DOMRect>();
  for (const r of rows) first.set(r, r.getBoundingClientRect());
  mutate();
  for (const r of rows) {
    if (!r.isConnected || r.classList.contains("is-dragging")) continue;
    const a = first.get(r);
    if (!a) continue;
    const b = r.getBoundingClientRect();
    const dy = a.top - b.top;
    if (Math.abs(dy) < 0.5) continue;
    r.style.transition = "none";
    r.style.transform = `translateY(${dy}px)`;
    void r.offsetHeight;
    r.style.transition = "transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1)";
    r.style.transform = "";
    const clear = () => {
      r.style.transition = "";
      r.removeEventListener("transitionend", clear);
    };
    r.addEventListener("transitionend", clear);
  }
}

export function moveDragHole(
  listEl: HTMLElement,
  source: HTMLElement,
  clientY: number,
): void {
  const others = Array.from(
    listEl.querySelectorAll<HTMLElement>(".watchlist-row:not(.is-dragging)"),
  );
  if (others.length === 0) return;

  let targetIndex = others.length;
  for (let i = 0; i < others.length; i++) {
    const rect = others[i].getBoundingClientRect();
    const mid = rect.top + rect.height / 2;
    if (clientY < mid) {
      targetIndex = i;
      break;
    }
  }

  const allRows = Array.from(listEl.querySelectorAll<HTMLElement>(".watchlist-row"));
  const currentIndex = allRows.indexOf(source);
  if (currentIndex < 0 || currentIndex === targetIndex) return;

  if (targetIndex >= others.length) {
    flipRows(listEl, () => listEl.appendChild(source));
  } else {
    const ref = others[targetIndex];
    flipRows(listEl, () => listEl.insertBefore(source, ref));
  }
}
