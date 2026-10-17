# Windows development guide

**Audience:** Developers (and coding agents) setting up EconomyWarRoom on **Windows 10/11**.  
**Companion:** [ARCHITECTURE.md](./ARCHITECTURE.md), [TODO.md](./TODO.md).

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

- Node **24+** (built-in TypeScript UI test runner) and npm
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
npm run run:exe
```

**Expected:** a screen-edge instrument notch, **no taskbar button**, system **tray** icon, seed symbols **AAPL** and **BTC-USD**, percentages filling after network calls. Click an icon for its sparkline and two-row quote detail.
Right-click the notch for **모양 / 종목 / 일반** settings. Appearance includes Dark/Glass/Liquid and opacity. General retains refresh, autostart, version/update, diagnostics and quit. The tray offers Show and Quit. App-specific keyboard controls are removed. Debug builds never modify OS autostart registrations.

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
If you are preparing a release that should support in-app updates, set
`TAURI_SIGNING_PRIVATE_KEY_PATH` or `TAURI_SIGNING_PRIVATE_KEY` before the build
so the updater artifacts and signatures are generated too.

**Full publish (signed build + GitHub Release + `latest.json`):** see **[release.md](./release.md)** and:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH="C:\path\to\updater.key"
npm run release:publish
```

To build and launch the release exe in one step (no GitHub upload):

```powershell
npm run run:exe
```

---

## 3. Day-to-day commands

| Goal | Command |
|------|---------|
| Dev app | `npm run tauri dev` |
| Build + launch exe | `npm run run:exe` |
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

## 5. Current native runtime checklist

- Four edges, mouse drag/dock, restart placement persistence.
- Instrument icon plus latest primary percentage; click opens that instrument's existing sparkline and primary/secondary grid.
- First settings opening after a short card; equal height across all three tabs; body-only scrolling when constrained.
- Add/remove/reorder/multi-select/tint and overflow instrument access.
- Detail column ratios persist across restart.
- Opacity readability, Dark/Glass/Liquid; foreground remains sharp.
- Transparent corners pass input through; hover never activates the window.
- Fold keeps polling and leaves an edge tab; explicit hide pauses new polling.
- Glass/Liquid capture stops on fold/hide; display affinity restores on theme change/reload/hide.
- Tray show/hide/quit, global shortcut, update progress/restart, diagnostics, release autostart.

Recorded evidence and reproducible smoke commands: [testing.md](testing.md).
Native hardware tests and Rust geometry tests are separate evidence.

---

## 6. Troubleshooting

| Symptom | What to try |
|---------|-------------|
| `link.exe` / MSVC not found | Install VS Build Tools C++ workload; open **x64 Native Tools** prompt or ensure PATH |
| WebView2 errors | Install WebView2 Evergreen Runtime |
| `tauri` not found | `npm install` from repo root; use `npm run tauri dev` not bare `tauri` |
| Hotkey does nothing | Another app owns `Ctrl+Shift+Space`; check console for register errors; temporarily close conflicting tools |
| Window “disappeared” / no taskbar entry | Expected tray-only mode — use tray left-click, tray Show, or hotkey |
| Transparent window black/opaque | GPU driver update; Windows “Transparency effects” on; still usable with higher opacity |
| Yahoo quotes empty / errors | Network/firewall; 429 backoff is normal under load; check logs; offline = last cache only |
| Antivirus blocks first run | Allow `target/debug/economy-war-room.exe` or project folder |
| npm scripts fail on `cd` | Use PowerShell 7+ or run `cargo` commands inside `src-tauri` manually |
| CRLF noise in git | `git config core.autocrlf true` is common on Windows; avoid reformatting whole tree |
| Slow first `tauri dev` | Normal — compiles all Rust deps once |
| `npm.ps1` / execution policy blocked | `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` (or call `npm.cmd`) |
| `rustc` / `cargo` not found after install | Open a **new** terminal so PATH picks up `%USERPROFILE%\.cargo\bin` |
| `link.exe` not on PATH | Expected; cargo finds MSVC via Build Tools registry — install **Desktop development with C++** if link fails during build |

---

## 7. Project layout (quick)

```text
EconomyWarRoom/
  src/                 # Vite + TS UI
  src-tauri/           # Rust + Tauri
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

- [ARCHITECTURE.md](./ARCHITECTURE.md) — module layout  
- [TODO.md](./TODO.md) — remaining checklist  
- [Tauri Windows prerequisites](https://tauri.app/start/prerequisites/#windows)  

---

## 10. Defect reporting & agent visibility

### Two modes

| Mode | Who starts the app | How the agent sees problems |
|------|--------------------|-----------------------------|
| **A — agent-run** | Agent runs `npm run tauri dev` (or tests) | Live stdout/stderr in the agent session |
| **B — user-run** | You run dev/release exe | **Settings → Copy diagnostics** → paste into chat |

The agent does **not** auto-attach to an app you started yourself. Hard process crashes (no UI left) still need Mode A logs or a future file logger.

### Mode A — agent-run terminal

```powershell
cd C:\dev\EconomyWarRoom
npm run tauri dev 2>&1 | Tee-Object -FilePath "$env:TEMP\ewr-tauri-dev.log"
```

Agent watches the process output while you click the UI. Optional: keep the Tee log for later.

### Mode B — Copy diagnostics (in-app)

While the widget is still running:

1. Open **Settings**.  
2. Click **Copy diagnostics** (label becomes `Copied`).  
3. Paste into chat (optionally add steps / expected / actual / screenshot).

The dump includes version, OS, visibility, settings, watchlist, quote cache summary, scheduler backoff/last error, and up to **100** recent in-process events (command failures, hotkey/autostart issues, throttled provider errors, etc.). Identical scheduler errors are throttled (≈30s) so the ring is not flooded by 429 storms. It is **not** a native crash minidump and is **not** written to disk by default.

### Other artifacts (optional)

| Artifact | When |
|----------|------|
| WebView `console.error` | UI/layout/invoke-only glitches |
| `%APPDATA%\com.economywarroom.app\economy-war-room-state.json` | Persist/reorder/theme wrong after restart |
| Tee log from Mode A | Compile/link errors, raw `eprintln` |

### Bug packet template

```text
### Bug
- Steps:
- Expected:
- Actual:
- Repro: always / sometimes
- Mode: A (agent tauri dev) | B (user exe)
### Artifacts
- Diagnostics paste: (Mode B — from Copy diagnostics)
- Terminal log: (Mode A — optional path/paste)
- Screenshot: (optional)
```

### Future (not in v1)

- Rolling file log under app data (survives hard crash)  
- `EWR_LOG=debug` verbose scheduler tracing

## 11. Isolated notch preview

Keep the installed app's settings separate. Create a local ignored config from the current base:

```powershell
New-Item -ItemType Directory -Force tmp | Out-Null
$previewConfig = Get-Content -Raw src-tauri/tauri.conf.json | ConvertFrom-Json
$previewConfig.identifier = 'com.economywarroom.notch-preview'
$previewConfig.productName = 'EconomyWarRoom Preview'
$previewConfig.build.beforeDevCommand = ''
$previewConfig.build.devUrl = 'http://localhost:1521'
$previewConfig.app.windows[0].title = 'Economy War Room Preview'
$previewJson = $previewConfig | ConvertTo-Json -Depth 30
[IO.File]::WriteAllText((Join-Path $PWD 'tmp/native-preview.json'), $previewJson, (New-Object Text.UTF8Encoding $false))
```

Run Vite on 1521 separately. Then:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9251'
npm run tauri dev -- --config tmp/native-preview.json
```

Use the browser/native scripts listed in [testing.md](testing.md). Native scripts assert the preview identifier before changing fixture preferences. They never target the normal application identifier. Global shortcuts are no longer registered; the legacy JSON hotkey value is ignored.

The former freely resizable list/content-hug checklist was superseded by the 2026-10-17 notch design. Legacy window coordinates remain persisted for migration, while Rust's notch controller owns current placement.
