#!/usr/bin/env node
/**
 * Signed Tauri build + GitHub Release publish (Windows-first).
 *
 * Usage:
 *   npm run release:publish
 *   npm run release:publish -- --skip-build
 *   npm run release:publish -- --dry-run
 *   npm run release:publish -- --notes "Hotfix"
 *
 * Requirements:
 *   - TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH
 *     (falls back to tmp/updater.key if present — never commit that file)
 *   - GitHub auth: GITHUB_TOKEN / GH_TOKEN / GITHUB_PERSONAL_ACCESS_TOKEN,
 *     or `git credential` for github.com (repo scope)
 *   - Version already set in package.json + src-tauri/tauri.conf.json + Cargo.toml
 *
 * See docs/release.md
 */

import { existsSync, readFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import {
  ensureUpdaterBuildConfig,
  githubRepoFromRemote,
  githubRequest,
  readAppVersion,
  resolveGitHubToken,
  resolvePrivateKey,
  root,
  run,
  windowsBundlePaths,
  writeJson,
} from "./lib/release-utils.mjs";

const args = parseArgs(process.argv.slice(2));
const version = readAppVersion();
const tag = args.tag || `v${version}`;
const repo = args.repo || githubRepoFromRemote();
const notes =
  args.notes ||
  `EconomyWarRoom ${version} — signed release with in-app updater manifest.`;

console.log(`Publish release ${tag} for ${repo}`);
console.log(`  skip-build=${Boolean(args.skipBuild)} dry-run=${Boolean(args.dryRun)}`);

const privateKey = resolvePrivateKey();
if (!privateKey) {
  console.error(`
Missing signing key.
Set one of:
  TAURI_SIGNING_PRIVATE_KEY          (key contents)
  TAURI_SIGNING_PRIVATE_KEY_PATH     (path to .key file)
Or place the key at: tmp/updater.key  (gitignored)

Generate a key pair once:
  npx tauri signer generate -w tmp/updater.key
Put the printed public key into src-tauri/tauri.conf.json → plugins.updater.pubkey
`);
  process.exit(1);
}

if (!args.skipBuild) {
  const tauriCmd = process.platform === "win32" ? "npx.cmd" : "npx";
  const configPath = ensureUpdaterBuildConfig();
  console.log("Building signed bundles (createUpdaterArtifacts)...");
  run(
    tauriCmd,
    ["run", "tauri", "--", "build", "--config", configPath],
    {
      TAURI_SIGNING_PRIVATE_KEY: privateKey,
      CI: process.env.CI || "true",
    },
  );
} else {
  console.log("Skipping build; using existing artifacts.");
}

const paths = windowsBundlePaths(version);
const missing = ["nsis", "nsisSig"].filter((k) => !existsSync(paths[k]));
if (missing.length) {
  console.error("Missing required artifacts:", missing.map((k) => paths[k]).join(", "));
  console.error("Run without --skip-build, or build with signing key first.");
  process.exit(1);
}

const nsisName = basename(paths.nsis);
const nsisSigBody = readFileSync(paths.nsisSig, "utf8").trim();
const pubDate = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
const downloadBase = `https://github.com/${repo}/releases/download/${tag}`;

const latest = {
  version,
  notes,
  pub_date: pubDate,
  platforms: {
    "windows-x86_64": {
      signature: nsisSigBody,
      url: `${downloadBase}/${nsisName}`,
    },
  },
};

const latestPath = resolve(root, "tmp", "latest.json");
writeJson(latestPath, latest);
console.log(`Wrote ${latestPath}`);

const assets = [
  { path: paths.nsis, name: nsisName },
  { path: paths.nsisSig, name: `${nsisName}.sig` },
];
if (existsSync(paths.msi) && existsSync(paths.msiSig)) {
  assets.push(
    { path: paths.msi, name: basename(paths.msi) },
    { path: paths.msiSig, name: `${basename(paths.msi)}.sig` },
  );
}
assets.push({ path: latestPath, name: "latest.json" });

if (args.dryRun) {
  console.log("Dry run — would upload:");
  for (const a of assets) console.log(`  - ${a.name} (${a.path})`);
  console.log(`Release notes: ${notes}`);
  console.log(`Updater endpoint: ${downloadBase.replace(`/download/${tag}`, "/latest/download")}/latest.json`);
  process.exit(0);
}

const token = resolveGitHubToken();
if (!token) {
  console.error(`
Missing GitHub token.
Set GITHUB_TOKEN, GH_TOKEN, or GITHUB_PERSONAL_ACCESS_TOKEN (repo scope),
or ensure git credential manager has a github.com password/token.
`);
  process.exit(1);
}

const releaseBody = [
  `## ${version}`,
  "",
  notes,
  "",
  "### Install",
  `- **NSIS:** [${nsisName}](${downloadBase}/${nsisName})`,
  existsSync(paths.msi)
    ? `- **MSI:** [${basename(paths.msi)}](${downloadBase}/${basename(paths.msi)})`
    : null,
  "",
  "### Updater",
  "Clients fetch `latest.json` from this release (`plugins.updater.endpoints`).",
]
  .filter(Boolean)
  .join("\n");

let release;
try {
  release = await githubRequest(
    token,
    "POST",
    `https://api.github.com/repos/${repo}/releases`,
    {
      contentType: "application/json; charset=utf-8",
      body: JSON.stringify({
        tag_name: tag,
        target_commitish: args.target || "main",
        name: `EconomyWarRoom ${tag}`,
        body: releaseBody,
        draft: false,
        prerelease: Boolean(args.prerelease),
        make_latest: "true",
      }),
    },
  );
  console.log(`Created release: ${release.html_url}`);
} catch (err) {
  if (String(err.message).includes("already_exists") || String(err.message).includes("422")) {
    console.log("Release may already exist; fetching by tag...");
    release = await githubRequest(
      token,
      "GET",
      `https://api.github.com/repos/${repo}/releases/tags/${tag}`,
    );
    console.log(`Using existing release: ${release.html_url}`);
    // Update notes / make latest
    await githubRequest(
      token,
      "PATCH",
      `https://api.github.com/repos/${repo}/releases/${release.id}`,
      {
        contentType: "application/json; charset=utf-8",
        body: JSON.stringify({
          body: releaseBody,
          make_latest: "true",
          draft: false,
        }),
      },
    );
    // refresh assets list
    release = await githubRequest(
      token,
      "GET",
      `https://api.github.com/repos/${repo}/releases/${release.id}`,
    );
  } else {
    throw err;
  }
}

const uploadBase = String(release.upload_url).split("{")[0];

for (const asset of assets) {
  const existing = (release.assets || []).find((a) => a.name === asset.name);
  if (existing) {
    console.log(`Deleting existing asset ${asset.name} (id=${existing.id})`);
    await githubRequest(
      token,
      "DELETE",
      `https://api.github.com/repos/${repo}/releases/assets/${existing.id}`,
    );
  }
  const bytes = readFileSync(asset.path);
  console.log(`Uploading ${asset.name} (${bytes.length} bytes)...`);
  const uploaded = await githubRequest(
    token,
    "POST",
    `${uploadBase}?name=${encodeURIComponent(asset.name)}`,
    {
      contentType: "application/octet-stream",
      body: bytes,
    },
  );
  console.log(`  → ${uploaded.browser_download_url}`);
}

const endpoint =
  `https://github.com/${repo}/releases/latest/download/latest.json`;
console.log("\nDone.");
console.log(`  Release:  ${release.html_url}`);
console.log(`  Endpoint: ${endpoint}`);
console.log("  Install a previous signed build, then header ↻ should pick this up.");

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--skip-build") out.skipBuild = true;
    else if (a === "--dry-run") out.dryRun = true;
    else if (a === "--prerelease") out.prerelease = true;
    else if (a === "--notes") out.notes = argv[++i];
    else if (a === "--tag") out.tag = argv[++i];
    else if (a === "--repo") out.repo = argv[++i];
    else if (a === "--target") out.target = argv[++i];
    else if (a === "--help" || a === "-h") {
      console.log(`Usage: node scripts/publish-release.mjs [options]
  --skip-build   Reuse existing bundle under src-tauri/target/release/bundle
  --dry-run      Build (unless skip) + write latest.json; do not call GitHub
  --notes TEXT   Release notes
  --tag TAG      Override tag (default v{version from tauri.conf.json})
  --repo OWNER/NAME
  --target REF   Git ref for the tag (default main)
  --prerelease
`);
      process.exit(0);
    } else {
      console.error(`Unknown arg: ${a}`);
      process.exit(1);
    }
  }
  return out;
}
