# Release & in-app updates

**Audience:** maintainers publishing Windows builds that clients can install **and** self-update.  
**Companion:** [windows-dev.md](./windows-dev.md), [HANDOFF.md](./HANDOFF.md).

---

## What “publish” means

In-app **Check for updates** (header **↻**) does **not** read git `main`.  
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
| Users on prior release builds | Header ↻ / startup check installs the new package |

`npm run tauri dev` **skips** startup auto-check (`debug_assertions`). Prefer a **release** install when testing updates.

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
