import { invoke } from "@tauri-apps/api/core";
import type { UpdateInfo } from "./updates";

export interface UpdateCardController {
  setUpdate(info: UpdateInfo | null): void;
  destroy(): void;
}

export function mountUpdateCard(
  root: HTMLElement,
  options?: {
    onConfirm?: () => void;
    onDismiss?: () => void;
  },
): UpdateCardController {
  root.innerHTML = `
    <div class="update-detail-card">
      <div class="update-card-header">
        <svg class="update-card-star" viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
          <defs>
            <linearGradient id="notch-star-gold-detail" x1="0" y1="0" x2="1" y2="1">
              <stop stop-color="#fff4c2"/>
              <stop offset=".45" stop-color="#ffd043"/>
              <stop offset="1" stop-color="#fa8c16"/>
            </linearGradient>
          </defs>
          <path d="M 9 4.5 C 9 9.1, 10.9 11, 15.5 11 C 10.9 11, 9 12.9, 9 17.5 C 9 12.9, 7.1 11, 2.5 11 C 7.1 11, 9 9.1, 9 4.5 Z" fill="url(#notch-star-gold-detail)"/>
          <circle cx="9" cy="11" r="1.3" fill="#ffffff" opacity="0.95"/>
          <path d="M 18.5 1 C 18.5 3.5, 19.5 4.5, 22 4.5 C 19.5 4.5, 18.5 5.5, 18.5 8 C 18.5 5.5, 17.5 4.5, 15 4.5 C 17.5 4.5, 18.5 3.5, 18.5 1 Z" fill="url(#notch-star-gold-detail)"/>
          <circle cx="18.5" cy="4.5" r="0.8" fill="#ffffff" opacity="0.9"/>
        </svg>
        <span>새 버전 업데이트</span>
        <span class="update-version-tag"></span>
      </div>
      <div class="update-card-actions">
        <button type="button" class="update-action-btn update-btn-cancel">취소</button>
        <button type="button" class="update-action-btn update-btn-confirm">설치</button>
      </div>
    </div>
  `;

  const versionTag = root.querySelector<HTMLElement>(".update-version-tag")!;
  const cancelBtn = root.querySelector<HTMLButtonElement>(".update-btn-cancel")!;
  const confirmBtn = root.querySelector<HTMLButtonElement>(".update-btn-confirm")!;

  cancelBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    options?.onDismiss?.();
  });

  confirmBtn.addEventListener("click", async (e) => {
    e.stopPropagation();
    confirmBtn.disabled = true;
    confirmBtn.textContent = "설치 중…";
    try {
      await invoke("check_for_updates");
      options?.onConfirm?.();
    } catch (err) {
      console.error("Update failed", err);
      confirmBtn.disabled = false;
      confirmBtn.textContent = "설치";
    }
  });

  return {
    setUpdate(info: UpdateInfo | null) {
      if (info) {
        versionTag.textContent = `v${info.version}`;
      }
    },
    destroy() {
      root.innerHTML = "";
    },
  };
}
