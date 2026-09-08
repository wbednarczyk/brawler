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
  'cd src-tauri && cargo test',
  'make -j4 check-local',
  'env X=1 cargo build',
  './node_modules/.bin/vitest run',
  'yarn test',
  'make -j 4 check-local',
  'make -C "src-tauri" check',
  'rtk proxy cargo test',
  'make --jobs=4 -C src-tauri check-rust-test',
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
  'git commit -m "note: \\"quoted\\"; cargo test; more"',
  "git commit -m 'msg with ; cargo test ; inside'",
  'echo "make check" > note.txt',
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

const JS_ALIVE = "23456 node /r/node_modules/.bin/vitest run";

test("scoped JS runs are never denied; cargo stacks only on a compile; full suites stack on anything", () => {
  for (const cmd of ["rtk npx vitest run src/test/x.test.ts", "npx playwright test tests/browser/x.spec.ts --project=chromium", "vitest run src/a.test.ts -t name", "npm test -- runtime"]) {
    assert.equal(runHook(cmd, HEAVY_ALIVE), "allow", `scoped JS while cargo alive: ${cmd}`);
  }
  assert.equal(runHook("rtk cargo nextest run storage", JS_ALIVE), "allow", "cargo while only vitest alive");
  assert.equal(runHook("rtk cargo nextest run storage", HEAVY_ALIVE), "deny", "cargo while cargo alive");
  for (const cmd of ["npx vitest run", "vitest", "npx playwright test", "make check-local", "npm run build", "npm test"]) {
    assert.equal(runHook(cmd, JS_ALIVE), "deny", `full suite while vitest alive: ${cmd}`);
  }
});

test("round-3 matrix defects stay closed (option arity, cargo globals, chain precedence, full process list)", () => {
  // option VALUES are not scope; grep/-t and quoted files ARE scope
  for (const cmd of ["npx vitest run --maxWorkers 8", "npx playwright test --workers 8", "vitest run --reporter dot"]) {
    assert.equal(runHook(cmd, JS_ALIVE), "deny", `unscoped despite options: ${cmd}`);
  }
  for (const cmd of ['npx vitest run --grep="one case"', "npx playwright test -g 'x'", 'npx vitest run "src/a b.test.ts"', "npx vitest run -t name"]) {
    assert.equal(runHook(cmd, JS_ALIVE), "allow", `scoped: ${cmd}`);
  }
  // cargo through ordinary syntax
  for (const cmd of ['env CARGO_BUILD_JOBS="8" cargo test', "cargo +stable test", "cargo --manifest-path src-tauri/Cargo.toml test", "CARGO_BUILD_JOBS=8 cargo nextest run x"]) {
    assert.equal(runHook(cmd, HEAVY_ALIVE), "deny", `cargo while compile alive: ${cmd}`);
  }
  // chain precedence: full dominates cargo
  assert.equal(runHook("cargo test && npm test", JS_ALIVE), "deny", "chain with a full suite while vitest alive");
  // a compiler hidden after many JS matches still counts
  const manyJsThenRustc = Array.from({ length: 6 }, (_, i) => `${1000 + i} node /r/node_modules/.bin/vitest run`).concat(["9999 /usr/bin/rustc --crate-name a"]).join("\n");
  assert.equal(runHook("cargo test", manyJsThenRustc), "deny", "rustc beyond the first five lines");
});

test("round-4 bypasses stay closed (redirections are not scope; shell wrappers are inspected)", () => {
  for (const cmd of ["npx vitest run > /tmp/vitest.log", "npx playwright test > /tmp/browser.log 2>&1", "npx vitest run 2>&1 | tee /tmp/v.log", "vitest run >>log"]) {
    assert.equal(runHook(cmd, JS_ALIVE), "deny", `redirected full suite: ${cmd}`);
  }
  assert.equal(runHook("npx vitest run src/a.test.ts > /tmp/v.log", JS_ALIVE), "allow", "scoped + redirect");
  for (const cmd of ['nix develop -c bash -lc "cd src-tauri && cargo test"', "bash -c 'CARGO_BUILD_JOBS=8 cargo nextest run x'", 'sh -ec "rtk cargo clippy --all-targets"']) {
    assert.equal(runHook(cmd, HEAVY_ALIVE), "deny", `wrapped cargo while compile alive: ${cmd}`);
  }
  assert.equal(runHook("bash -c 'npm test'", JS_ALIVE), "deny", "wrapped full suite");
  assert.equal(runHook('sh -c "echo make check"', JS_ALIVE), "allow", "echo inside a wrapper is not a run");
  assert.equal(runHook("bash scripts/x.sh", HEAVY_ALIVE), "allow", "a script file is not inspected (no inline body)");
});

test("round-5 bypasses stay closed (glued redirections; quoted assignments before a wrapper)", () => {
  const RUSTC = "200 /usr/bin/rustc --crate-name brawler";
  for (const cmd of ["npx vitest run>/tmp/vitest.log", "npx playwright test>>log 2>&1", "vitest run</dev/null"]) {
    assert.equal(runHook(cmd, RUSTC), "deny", `glued redirection: ${cmd}`);
  }
  assert.equal(runHook("npx vitest run src/a.test.ts>/tmp/v.log", RUSTC), "allow", "scoped + glued redirect");
  for (const cmd of ['env CARGO_BUILD_JOBS="8" bash -c "cargo test"', 'CARGO_BUILD_JOBS="8" sh -c \'cargo nextest run x\'', 'rtk env RUST_LOG="debug" bash -lc "cd src-tauri && cargo test"']) {
    assert.equal(runHook(cmd, RUSTC), "deny", `quoted assignment + wrapper: ${cmd}`);
  }
});

test("round-6 follow-ups (timeout/nice/time wrappers; npx flags before the runner)", () => {
  const RUSTC = "200 /usr/bin/rustc --crate-name brawler";
  for (const cmd of ["timeout 120s cargo test", "timeout -k 5 300 cargo nextest run x", "nice -n 10 cargo build", "time cargo clippy"]) {
    assert.equal(runHook(cmd, RUSTC), "deny", `wrapped cargo: ${cmd}`);
  }
  for (const cmd of ["npx --no-install vitest run", "npx -y playwright test", "npx --package vitest vitest"]) {
    assert.equal(runHook(cmd, RUSTC), "deny", `npx flags before the runner: ${cmd}`);
  }
  assert.equal(runHook("npx --no-install vitest run src/a.test.ts", RUSTC), "allow", "scoped after npx flags");
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
  const hook = readFileSync(path.join(REPO_ROOT, ".claude/hooks/one-heavy-build.mjs"), "utf8");
  const m = hook.match(/PGREP_PATTERN = "([^"]+)"/);
  assert.ok(m, "hook still declares PGREP_PATTERN");
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
