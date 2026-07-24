import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

export function run(cmd, args, extraEnv = {}) {
  // Windows .cmd shims (npm.cmd / npx.cmd) need a shell; without it spawnSync
  // often exits immediately with status 1 and no output.
  const useShell = process.platform === "win32";
  const result = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: useShell,
    env: {
      ...process.env,
      ...extraEnv,
    },
  });
  if (result.error) {
    console.error(`spawn failed: ${result.error.message}`);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

/** Resolve minisign private key material for Tauri updater signing. */
export function resolvePrivateKey() {
  if (process.env.TAURI_SIGNING_PRIVATE_KEY) {
    return process.env.TAURI_SIGNING_PRIVATE_KEY.trim();
  }
  const candidates = [
    process.env.TAURI_SIGNING_PRIVATE_KEY_PATH,
    resolve(root, "tmp", "updater.key"),
  ].filter(Boolean);
  for (const keyPath of candidates) {
    if (existsSync(keyPath)) {
      return readFileSync(keyPath, "utf8").trim();
    }
  }
  return null;
}

export function readAppVersion() {
  const confPath = resolve(root, "src-tauri", "tauri.conf.json");
  const conf = JSON.parse(readFileSync(confPath, "utf8"));
  return String(conf.version);
}

export function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

/**
 * Candidate GitHub tokens (env + git credential manager).
 * Callers should try until one authenticates — env vars may be stale.
 */
export function resolveGitHubTokenCandidates() {
  const out = [];
  for (const key of ["GITHUB_TOKEN", "GH_TOKEN", "GITHUB_PERSONAL_ACCESS_TOKEN"]) {
    const v = process.env[key]?.trim();
    if (v) out.push(v);
  }
  const result = spawnSync("git", ["credential", "fill"], {
    cwd: root,
    input: "protocol=https\nhost=github.com\n\n",
    encoding: "utf8",
    shell: false,
  });
  if (result.status === 0 && result.stdout) {
    for (const line of result.stdout.split(/\r?\n/)) {
      if (line.startsWith("password=")) {
        const pw = line.slice("password=".length).trim();
        if (pw && !out.includes(pw)) out.push(pw);
      }
    }
  }
  return out;
}

/** @deprecated use resolveGitHubTokenCandidates + probe */
export function resolveGitHubToken() {
  return resolveGitHubTokenCandidates()[0] ?? null;
}

export function githubRepoFromRemote() {
  const result = spawnSync("git", ["remote", "get-url", "origin"], {
    cwd: root,
    encoding: "utf8",
    shell: false,
  });
  if (result.status !== 0) return "sky64422/EconomyWarRoom";
  const url = (result.stdout || "").trim();
  // https://github.com/owner/repo.git or git@github.com:owner/repo.git
  const m =
    url.match(/github\.com[/:]([^/]+)\/([^/.]+)/i) ||
    url.match(/([^/:]+)\/([^/.]+)\.git$/);
  if (!m) return "sky64422/EconomyWarRoom";
  return `${m[1]}/${m[2]}`;
}

export async function githubRequest(token, method, url, { body, contentType } = {}) {
  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
    "User-Agent": "EconomyWarRoom-publish-release",
  };
  if (contentType) headers["Content-Type"] = contentType;
  const res = await fetch(url, {
    method,
    headers,
    body,
  });
  const text = await res.text();
  let data = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = text;
  }
  if (!res.ok) {
    const msg =
      typeof data === "object" && data?.message
        ? data.message
        : text || res.statusText;
    throw new Error(`GitHub ${method} ${url} → ${res.status}: ${msg}`);
  }
  return data;
}

export function productSlug() {
  return "EconomyWarRoom";
}

/** Paths to Windows NSIS + MSI bundles for a version (if present). */
export function windowsBundlePaths(version) {
  const base = resolve(root, "src-tauri", "target", "release", "bundle");
  const nsis = resolve(base, "nsis", `${productSlug()}_${version}_x64-setup.exe`);
  const msi = resolve(
    base,
    "msi",
    `${productSlug()}_${version}_x64_en-US.msi`,
  );
  return { nsis, msi, nsisSig: `${nsis}.sig`, msiSig: `${msi}.sig` };
}

export function ensureUpdaterBuildConfig() {
  const tmpDir = resolve(root, "tmp");
  mkdirSync(tmpDir, { recursive: true });
  const configPath = resolve(tmpDir, "tauri-updater-build.json");
  writeJson(configPath, { bundle: { createUpdaterArtifacts: true } });
  return configPath;
}
