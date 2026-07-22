import { existsSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const exeName =
  process.platform === "win32" ? "economy-war-room.exe" : "economy-war-room";
const exePath = resolve(root, "src-tauri", "target", "release", exeName);

function run(cmd, args) {
  const result = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: false,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

run(npmCmd, ["run", "tauri", "build"]);

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
