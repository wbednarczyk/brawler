// Guard (ADR 0045 harvest, B3 fix 2026-09-08): one-heavy-build.sh must deny
// ONLY a real heavy build/test invocation while one is already running, and
// must never flag legitimate commands (a commit message that happens to
// mention "vitest", a `rtk read` of a config file, an unrelated dev-server
// process). Drives the hook exactly as Claude Code's PreToolUse machinery
// does: spawn it with JSON on stdin, read its permissionDecision back.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const HOOK_PATH = path.join(REPO_ROOT, ".claude/hooks/one-heavy-build.sh");

// `running` is injected through the hook's BRAWLER_HEAVY_PS_OVERRIDE seam:
// "" = nothing heavy alive; a pgrep-shaped line = a heavy run alive. The real
// pgrep branch is two lines of shell and is exercised by the hook's own use.
function runHook(command, running = "") {
  const input = JSON.stringify({ tool_name: "Bash", tool_input: { command } });
  const result = spawnSync("bash", [HOOK_PATH], {
    input,
    encoding: "utf8",
    cwd: REPO_ROOT,
    env: { ...process.env, BRAWLER_HEAVY_PS_OVERRIDE: running, BRAWLER_ALLOW_PARALLEL_BUILD: "" },
  });
  const stdout = result.stdout.trim();
  if (!stdout) return "allow";
  return JSON.parse(stdout).hookSpecificOutput?.permissionDecision ?? "allow";
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

const HEAVY_ALIVE = "12345 cargo nextest run fake";

test("one-heavy-build denies heavy commands while a heavy run is alive, allows the rest", () => {
  for (const cmd of DENY_COMMANDS) {
    assert.equal(runHook(cmd, HEAVY_ALIVE), "deny", `expected deny for: ${cmd}`);
  }
  for (const cmd of ALLOW_COMMANDS) {
    assert.equal(runHook(cmd, HEAVY_ALIVE), "allow", `expected allow for: ${cmd}`);
  }
});

test("nothing heavy alive: every command is allowed", () => {
  for (const cmd of [...DENY_COMMANDS, ...ALLOW_COMMANDS]) {
    assert.equal(runHook(cmd, ""), "allow", `expected allow when idle for: ${cmd}`);
  }
});

test("the escape hatch BRAWLER_ALLOW_PARALLEL_BUILD=1 allows a deliberate parallel run", () => {
  const input = JSON.stringify({ tool_name: "Bash", tool_input: { command: "rtk cargo nextest run x" } });
  const result = spawnSync("bash", [HOOK_PATH], {
    input,
    encoding: "utf8",
    cwd: REPO_ROOT,
    env: { ...process.env, BRAWLER_HEAVY_PS_OVERRIDE: HEAVY_ALIVE, BRAWLER_ALLOW_PARALLEL_BUILD: "1" },
  });
  assert.equal(result.stdout.trim(), "");
});

// The pgrep alternation itself: a dev-server-shaped cmdline must not count as
// heavy. Exercised through the real branch with a synthetic `ps`-style line is
// impossible (pgrep decides), so pin the pattern instead.
test("the pgrep pattern excludes dev servers, analyzers and MCP shims", () => {
  const hook = readFileSync(HOOK_PATH, "utf8");
  const m = hook.match(/pgrep -af '([^']+)'/);
  assert.ok(m, "hook still uses pgrep -af '<pattern>'");
  const re = new RegExp(m[1]);
  // pgrep -af lines are "<pid> <cmdline>"; the pattern anchors on ^ or a "/" — so
  // real cmdlines (`node …/.bin/vitest`, `…/.bin/playwright test`) match while a
  // bare word inside another command does not.
  for (const alive of ["cargo nextest run x", "/usr/bin/rustc --crate-name a", "node /r/node_modules/.bin/vitest run", "node /r/node_modules/.bin/playwright test", "cargo-nextest nextest run"]) {
    assert.ok(re.test(alive), `heavy: ${alive}`);
  }
  for (const benign of ["vite dev", "tsc --watch", "esbuild --serve", "rust-analyzer", "node playwright-mcp", "npm exec @playwright/mcp@latest"]) {
    assert.ok(!re.test(benign), `benign: ${benign}`);
  }
});
