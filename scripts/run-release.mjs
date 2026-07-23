import { existsSync } from "node:fs";
import { spawn } from "node:child_process";
import { dirname, resolve } from "node:path";
import {
  ensureUpdaterBuildConfig,
  resolvePrivateKey,
  root,
  run,
} from "./lib/release-utils.mjs";

const tauriCmd = process.platform === "win32" ? "npx.cmd" : "npx";
const exeName =
  process.platform === "win32" ? "economy-war-room.exe" : "economy-war-room";
const exePath = resolve(root, "src-tauri", "target", "release", exeName);

const privateKey = resolvePrivateKey();
const hasSigningKey = Boolean(privateKey);
const tauriArgs = ["run", "tauri", "--", "build"];
if (hasSigningKey) {
  tauriArgs.push("--config", ensureUpdaterBuildConfig());
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
