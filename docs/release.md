# Release & in-app updates

**Audience:** maintainers publishing Windows builds that clients can install **and** self-update.  
**Companion:** [windows-dev.md](./windows-dev.md).  
**Current release:** **v0.4.11** on GitHub Releases.

## 0.4.11 — 2026-10-31

### Fixed

- Improve Liquid opacity response and bright-background text contrast without an added dark tint.
- Prevent unblurred edges and flashing when detail sizes change.
- Restore the notch's release animation after dragging.

## 0.4.10 — 2026-10-31

### Fixed

- Liquid opacity changes background clarity and frosted glass intensity across the full slider range.

## 0.4.9 — 2026-10-25

### Added

- In-notch twin sparkle update indicator with Quiet Luxury detail card, fast startup check (500ms), and session dismissal persistence.
- Liquid theme contrast responsiveness test case validating prompt adaptation over alternating white/black backdrops across random short and long intervals.

### Fixed

- Preserved desktop capture recovery on window focus/blur with 1000ms static heartbeat and 800ms recovery timer.

## 0.4.2 — 2026-10-24

### Fixed

- Fix drag capture conflict: release pointer capture immediately on window blur so Snipping Tool can be cancelled with ESC; prevent recursive hall-of-mirrors blur by holding stable capture lease during drag.

## 0.4.1 — 2026-10-24

### Fixed

- Liquid/Glass capture disappearance fix: restored per-frame RAII display affinity, eliminated continuous pointermove hover loop, and reduced settling timeout so widgets remain fully capturable during mouse interaction.

## 0.4.0 — 2026-10-24

### Added

- Korean equity search & Hangul decomposition: searching in Korean (`ㅅㅏㅁㅅㅓㅇ` or `삼성`) seamlessly suggests and resolves domestic symbols (e.g. `005930.KS`) and Korean aliases for global assets.

### Fixed

- Preserved desktop capture and fixed screenshot exclusion (`Win + Shift + S`, Snipping Tool):
  - Preserved GDI surfaces and frame cache across lease release instead of clearing on idle, preventing infinite 30 FPS polling loops.
  - Settle-to-sleep: once the backdrop stabilizes, the loop transitions to sleep (`interval = Infinity`), restoring `SetWindowDisplayAffinity(hwnd, 0)` so the widget is 100% visible in screenshots.
  - Immediate zero-latency interaction wake-up on pointer hover, movement, clicks, or dragging, plus an instant `blur` safety net.
- Fixed notch bottom silhouette narrowing by constraining outline dimensions and using uniform border-box scaling.

## 0.3.2 — 2026-10-20

### Fixed

- Restored original brand colors for instrument marks in both the notch and instrument settings, removing monochrome brightness filters while maintaining adaptive contrast across themes.

## 0.3.1 — 2026-10-19

### Fixed

- Restored live desktop capture and canvas blur rendering for Liquid and Glass backdrops on Windows unfocused overlay (`WS_EX_NOACTIVATE`), fixing the opaque dark gray silhouette.
- Background capture uses `WDA_EXCLUDEFROMCAPTURE` leasing with 750ms automatic idle release.

## 0.3.0 — 2026-10-19

### Added

- Adaptive foreground contrast sampling for Liquid material: samples narrow 3px empty margins serialized every 250ms, dynamically switching text and market gain/loss colors between light and dark backgrounds.

### Changed

- Liquid material now uses native same-HWND Windows Composition backdrops with clear prism tint, remaining visible in screenshots and screen sharing.
- Background materials and contrast probes cleanly halt on hide and fold.

## 0.2.0 — 2026-10-17

### Added

- Screen-edge watchlist notch with instrument icons, latest-price changes and inward sparkline detail cards.
- Shared-height appearance, instrument and general settings with mouse-first watchlist management.

### Changed

- Smoked Glass and zero-refraction Liquid materials with mirrored rims. Adaptive, cached desktop capture reduces static-background work while preserving movement.
- Smaller price/change typography, finer sparklines with a visible baseline and subtle fill, and restrained animations.

### Removed

- Keyboard controls, tray hiding, notch add/remove buttons, percentage background badges and settings save notices.

### Fixed

- Delayed startup state could leave an expanded-looking notch unresponsive to the mouse.
- Continuous watchlist reordering, first-open settings sizing and theme-switch position jitter.

---

## What “publish” means

In-app **Check for updates** (Settings → Version download icon) does **not** read git `main`.  
It downloads:

```text
https://github.com/sky64422/EconomyWarRoom/releases/latest/download/latest.json
```

That file must list a **higher semver** than the installed app, a signed installer URL, and a matching signature.

| Step | Purpose |
|------|---------|
| Bump version | `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (and commit) |
| Signed `tauri build` | Produces NSIS/MSI + `.sig` (`createUpdaterArtifacts`) |
| GitHub Release | Hosts installer + **`latest.json`** as release assets |
| Users on prior release builds | Startup check **badges** the settings gear when newer; Settings download icon installs + restarts |

`npm run tauri dev` **skips** startup update check (`debug_assertions`). Prefer a **release** install when testing updates.

### Client UX (Electron-like)

1. ~30s after launch (release): check `latest.json`.  
2. If newer → blue light on the settings gear + **background download** (`update-available` → progress → `update-ready`).  
3. When ready → Settings → Version tooltip `Restart to install x.y.z`; click the download icon installs cached package + restarts.  
4. Manual download-icon click with nothing pending: check → download → install in one shot.

---

## One-time: signing keys

Generate once (keep private key offline / local only):

```powershell
npx tauri signer generate -w tmp/updater.key
```

- **Private:** `tmp/updater.key` — never commit (repo treats `tmp/` as local).  
- **Public:** printed / `tmp/updater.key.pub` — must match  
  `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.

Environment for builds:

| Variable | Meaning |
|----------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Key file **contents** (preferred by CLI) |
| `TAURI_SIGNING_PRIVATE_KEY_PATH` | Path to key file (script also accepts this) |
| (fallback) | `tmp/updater.key` if present |

Optional password: `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key was generated with one.

---

## Publish with the script (recommended)

```powershell
cd C:\dev\EconomyWarRoom

# 1) Bump version in package.json + src-tauri/tauri.conf.json + Cargo.toml
# 2) Commit & push main (so the release tag points at the right commit)

# Auth: either set a token with `repo` scope...
# $env:GITHUB_TOKEN = "ghp_..."
# ...or rely on Git Credential Manager (git push to GitHub already works)

# Key: path or contents
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "C:\dev\EconomyWarRoom\tmp\updater.key"
# or:  $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -Raw .\tmp\updater.key).Trim()

npm run release:publish
```

### Script options

```text
npm run release:publish -- --dry-run       # build + write tmp/latest.json, no GitHub
npm run release:publish -- --skip-build    # reuse existing bundle/ + .sig
npm run release:publish -- --notes "..."   # release body
npm run release:publish -- --tag v0.1.2    # override tag (default v{version})
npm run release:publish -- --help
```

Script path: [`scripts/publish-release.mjs`](../scripts/publish-release.mjs)  
Shared helpers: [`scripts/lib/release-utils.mjs`](../scripts/lib/release-utils.mjs)

What it does:

1. Reads version from `src-tauri/tauri.conf.json`  
2. Runs `tauri build` with `createUpdaterArtifacts: true` and signing env  
3. Writes `tmp/latest.json` for `windows-x86_64` (NSIS setup + signature)  
4. Creates/updates GitHub Release `v{version}` and uploads:  
   setup exe, `.sig`, MSI (+ sig if present), **`latest.json`**

Endpoint used by the app (configured in `tauri.conf.json`):

```text
plugins.updater.endpoints → .../releases/latest/download/latest.json
```

---

## Manual checklist (if not using the script)

1. Bump versions and commit to `main`.  
2. Build:

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -Raw .\tmp\updater.key).Trim()
   npx tauri build --config tmp/tauri-updater-build.json
   ```

   where `tmp/tauri-updater-build.json` is:

   ```json
   { "bundle": { "createUpdaterArtifacts": true } }
   ```

3. Artifacts (example for `0.1.1`):

   ```text
   src-tauri/target/release/bundle/nsis/EconomyWarRoom_0.1.1_x64-setup.exe
   src-tauri/target/release/bundle/nsis/EconomyWarRoom_0.1.1_x64-setup.exe.sig
   src-tauri/target/release/bundle/msi/...
   ```

4. Create GitHub Release tag `v0.1.1`, attach those files + a `latest.json` shaped as:

   ```json
   {
     "version": "0.1.1",
     "notes": "…",
     "pub_date": "2026-07-23T13:00:00Z",
     "platforms": {
       "windows-x86_64": {
         "signature": "<contents of .exe.sig>",
         "url": "https://github.com/sky64422/EconomyWarRoom/releases/download/v0.1.1/EconomyWarRoom_0.1.1_x64-setup.exe"
       }
     }
   }
   ```

5. Verify:

   ```powershell
   Invoke-WebRequest https://github.com/sky64422/EconomyWarRoom/releases/latest/download/latest.json
   ```

---

## Local release exe (no GitHub)

Build + launch without publishing:

```powershell
# With key → also produces updater signatures
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "C:\dev\EconomyWarRoom\tmp\updater.key"
npm run run:exe
```

See [`scripts/run-release.mjs`](../scripts/run-release.mjs).

---

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| “A public key has been found, but no private key” | Set `TAURI_SIGNING_PRIVATE_KEY` to **file contents**, not only PATH (some CLI versions) |
| Update check: no update | Installed version ≥ release; or still on `tauri dev` |
| Signature verify failed | Wrong private key vs `plugins.updater.pubkey` |
| `latest.json` 404 | Asset name must be exactly `latest.json` on the **latest** release |
| GitHub 401 | Token lacks `repo`, or expired; use GCM token that can push |
| Users never update | They must first install a **signed** release build that contains the updater plugin config |

---

## Security

- Never commit `tmp/updater.key` or paste private keys into issues/PRs.  
- Rotate keys only if you also ship a new build with the new **pubkey** in `tauri.conf.json` (old clients keep the old pubkey until reinstalled).  
- Prefer fine-scoped tokens for CI; for local publish, GCM is fine.
