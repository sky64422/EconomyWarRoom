import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriCmd = process.platform === "win32" ? "npx.cmd" : "npx";
const exeName =
  process.platform === "win32" ? "economy-war-room.exe" : "economy-war-room";
const exePath = resolve(root, "src-tauri", "target", "release", exeName);

function run(cmd, args, extraEnv = {}) {
  const result = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: false,
    env: {
      ...process.env,
      ...extraEnv,
    },
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

const privateKey = resolvePrivateKey();
const hasSigningKey = Boolean(privateKey);
const tauriArgs = ["run", "tauri", "--", "build"];
if (hasSigningKey) {
  const tmpDir = resolve(root, "tmp");
  mkdirSync(tmpDir, { recursive: true });
  const configPath = resolve(tmpDir, "tauri-updater-build.json");
  writeFileSync(
    configPath,
    JSON.stringify({ bundle: { createUpdaterArtifacts: true } }, null, 2),
  );
  tauriArgs.push("--config", configPath);
}

run(tauriCmd, tauriArgs, privateKey ? { TAURI_SIGNING_PRIVATE_KEY: privateKey } : {});

if (!existsSync(exePath)) {
  throw new Error(`release exe not found: ${exePath}`);
}

const child = spawn(exePath, [], {
  cwd: dirname(exePath),
  detached: true,
  stdio: "inherit",
  shell: false,
  windowsHide: false,
});

child.unref();

function resolvePrivateKey() {
  if (process.env.TAURI_SIGNING_PRIVATE_KEY) {
    return process.env.TAURI_SIGNING_PRIVATE_KEY.trim();
  }
  const keyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH;
  if (!keyPath) return null;
  if (!existsSync(keyPath)) {
    throw new Error(`private key not found: ${keyPath}`);
  }
  return readFileSync(keyPath, "utf8").trim();
}
