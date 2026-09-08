// Guard (ADR 0045 harvest, B3 fix 2026-09-08): one-heavy-build.sh must deny
// ONLY a real heavy build/test invocation while one is already running, and
// must never flag legitimate commands (a commit message that happens to
// mention "vitest", a `rtk read` of a config file, an unrelated dev-server
// process). Drives the hook exactly as Claude Code's PreToolUse machinery
// does: spawn it with JSON on stdin, read its permissionDecision back.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const HOOK_PATH = path.join(REPO_ROOT, ".claude/hooks/one-heavy-build.sh");

function runHook(command) {
  const input = JSON.stringify({ tool_name: "Bash", tool_input: { command } });
  const result = spawnSync("bash", [HOOK_PATH], {
    input,
    encoding: "utf8",
    cwd: REPO_ROOT,
  });
  const stdout = result.stdout.trim();
  if (!stdout) return "allow";
  return JSON.parse(stdout).hookSpecificOutput?.permissionDecision ?? "allow";
}

/** Spawn a process whose reconstructed cmdline (argv[0] renamed via `exec -a`) is `fakeCmdline`. */
function spawnFake(fakeCmdline) {
  return spawn("bash", ["-c", `exec -a ${JSON.stringify(fakeCmdline)} sleep 30`], { stdio: "ignore" });
}

/** Poll pgrep -f until `needle` shows up in a running process's cmdline, or time out. */
async function waitForFake(needle, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const found = spawnSync("pgrep", ["-f", needle], { encoding: "utf8" });
    if (found.status === 0) return;
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(`fake process matching ${JSON.stringify(needle)} never appeared under pgrep`);
}

const DENY_COMMANDS = [
  'rtk cargo nextest run x',
  'npm test -- runtime',
  'cd src-tauri && cargo test',
  'make -j4 check-local',
  'env X=1 cargo build',
  './node_modules/.bin/vitest run',
  'yarn test',
];

const ALLOW_COMMANDS = [
  'git commit -m "fix vitest flake"',
  'rtk read vitest.config.ts',
  'gh pr create --body "ran make check"',
  'cargo metadata',
  'cargo fmt --check',
  'npm run dev',
  'npm run lint',
  'node scripts/check/x.mjs',
  'make types',
];

test("one-heavy-build denies heavy commands while a heavy run is alive, allows the rest", async () => {
  const fake = spawnFake("cargo nextest run fake");
  try {
    await waitForFake("cargo nextest run fake");
    for (const cmd of DENY_COMMANDS) {
      assert.equal(runHook(cmd), "deny", `expected deny for: ${cmd}`);
    }
    for (const cmd of ALLOW_COMMANDS) {
      assert.equal(runHook(cmd), "allow", `expected allow for: ${cmd}`);
    }
  } finally {
    fake.kill("SIGKILL");
  }
});

test("a running dev-server-shaped process (vite) never blocks a heavy command", async () => {
  const fake = spawnFake("vite dev");
  try {
    await waitForFake("vite dev");
    assert.equal(runHook("rtk cargo nextest run x"), "allow");
  } finally {
    fake.kill("SIGKILL");
  }
});
