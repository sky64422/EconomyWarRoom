import "./styles/tokens.css";
import "./styles/app.css";
import { mountApp } from "./ui/app";

window.addEventListener("DOMContentLoaded", () => {
  const root = document.querySelector("#app");
  if (!root) {
    console.error("#app root missing");
    return;
  }
  void mountApp(root as HTMLElement).catch((err) => {
    console.error("Failed to mount app", err);
    root.innerHTML = `<div class="panel" style="padding:16px;color:var(--text,#fff)">Failed to load: ${String(err)}</div>`;
  });
});
