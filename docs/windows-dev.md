# Windows development guide

**Audience:** Developers (and coding agents) setting up EconomyWarRoom on **Windows 10/11**.  
**Companion:** [HANDOFF.md](./HANDOFF.md) (project context), [ARCHITECTURE.md](./ARCHITECTURE.md).

---

## 1. Prerequisites

Install in roughly this order:

### 1.1 Visual Studio Build Tools (MSVC)

- Download: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)  
- Workload: **Desktop development with C++**  
- Needed for `x86_64-pc-windows-msvc` Rust crates and Tauri link step  

### 1.2 Rust

```powershell
# https://rustup.rs — choose MSVC toolchain
rustup default stable-x86_64-pc-windows-msvc
rustc -V
cargo -V
```

### 1.3 Node.js

- Node **18+** (LTS recommended) and npm  
- Verify: `node -v`, `npm -v`

### 1.4 WebView2

- Usually preinstalled on Windows 10/11  
- If `tauri dev` complains about WebView2, install [Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)

### 1.5 Git

- Git for Windows  
- Optional but useful: **Git Bash** (for `scripts/coverage.sh`)

### 1.6 Tauri official checklist

If anything fails, cross-check:  
https://tauri.app/start/prerequisites/#windows

---

## 2. Clone and first run

```powershell
git clone https://github.com/sky64422/EconomyWarRoom.git
cd EconomyWarRoom
git checkout main
git pull origin main

npm install
npm run tauri dev
```

**Expected:** frameless tall window, glass panel, seed symbols **AAPL** and **BTC-USD**, quotes filling after network calls.

### Frontend only (no native window)

```powershell
npm run dev
```

Vite on `http://localhost:1420` — Tauri `invoke` will fail outside the shell; useful only for CSS/layout.

### Release build

```powershell
npm run tauri build
```

Installer/artifacts under `src-tauri/target/release/` (and bundle output per Tauri config).

---

## 3. Day-to-day commands

| Goal | Command |
|------|---------|
| Dev app | `npm run tauri dev` |
| Typecheck + web build | `npm run build` |
| Unit + integration + risk tests | `npm test` |
| Cargo unit only | `cd src-tauri; cargo test --lib` |
| Integration / risk | `cd src-tauri; cargo test --test integration_e2e --test risk_scenarios` |
| Coverage ≥85% | Git Bash: `npm run test:coverage` or `bash scripts/coverage.sh` |

### Coverage without bash

```powershell
cd src-tauri
cargo install cargo-tarpaulin   # once
cargo tarpaulin --lib --tests --timeout 180 --fail-under 85 `
  --exclude-files "src/main.rs" `
  --exclude-files "src/lib.rs" `
  --exclude-files "src/infrastructure/window_ctl.rs" `
  --exclude-files "src/commands.rs" `
  --out Html --output-dir target/coverage
```

---

## 4. App data location (Windows)

State JSON is under Tauri app data, roughly:

```text
%APPDATA%\com.economywarroom.app\
  economy-war-room-state.json
```

(Exact folder may follow `identifier` / product name; if missing, search `%APPDATA%` for `economy-war-room-state.json`.)

- Delete the file to reset to seed watchlist (AAPL, BTC-USD).  
- Corrupt JSON should fall back to defaults (covered by tests).

---

## 5. Runtime checklist (first Windows session)

Do these once after first successful launch — update [TODO.md](./TODO.md) when done:

- [ ] Window always on top, frameless, tall  
- [ ] Drag move; resize; restart → geometry restored  
- [ ] Theme: light / dark / system  
- [ ] Opacity slider changes panel transparency  
- [ ] Seed quotes + sparklines for AAPL / BTC-USD  
- [ ] **+** add symbol (e.g. `MSFT`); appears at bottom  
- [ ] Drag reorder; restart → order kept  
- [ ] Remove symbol  
- [ ] Header hide; `Ctrl+Shift+Space` shows again; no network thrash while hidden  
- [ ] Settings → Quit fully exits  
- [ ] Autostart: enable, reboot or check Task Manager / Startup apps  

---

## 6. Troubleshooting

| Symptom | What to try |
|---------|-------------|
| `link.exe` / MSVC not found | Install VS Build Tools C++ workload; open **x64 Native Tools** prompt or ensure PATH |
| WebView2 errors | Install WebView2 Evergreen Runtime |
| `tauri` not found | `npm install` from repo root; use `npm run tauri dev` not bare `tauri` |
| Hotkey does nothing | Another app owns `Ctrl+Shift+Space`; check console for register errors; temporarily close conflicting tools |
| Transparent window black/opaque | GPU driver update; Windows “Transparency effects” on; still usable with higher opacity |
| Yahoo quotes empty / errors | Network/firewall; 429 backoff is normal under load; check logs; offline = last cache only |
| Antivirus blocks first run | Allow `target/debug/economy-war-room.exe` or project folder |
| npm scripts fail on `cd` | Use PowerShell 7+ or run `cargo` commands inside `src-tauri` manually |
| CRLF noise in git | `git config core.autocrlf true` is common on Windows; avoid reformatting whole tree |
| Slow first `tauri dev` | Normal — compiles all Rust deps once |

---

## 7. Project layout (quick)

```text
EconomyWarRoom/
  src/                 # Vite + TS UI
  src-tauri/           # Rust + Tauri
  docs/HANDOFF.md      # Start here for AI/human sync
  docs/windows-dev.md  # This file
  docs/ARCHITECTURE.md
  docs/testing.md
  docs/TODO.md
  scripts/coverage.sh
  package.json
```

---

## 8. Agent notes specific to Windows

1. Prefer **native Windows** `npm run tauri dev` for UI bugs; do not assume WSL GUI.  
2. Business-logic fixes: still write **Rust tests** under `src-tauri` (same on all OS).  
3. Path separators: use `Path`/`PathBuf` in Rust (already); avoid hardcoding `/` in new Windows-facing scripts.  
4. Do not require bash for core workflows; document bash-only tools as optional.  
5. After fixing a Windows-only issue, add a row to §6 Troubleshooting in this file.

---

## 9. Related

- [HANDOFF.md](./HANDOFF.md) — product + agent starter prompt  
- [Tauri Windows prerequisites](https://tauri.app/start/prerequisites/#windows)  
