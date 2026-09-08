#!/usr/bin/env node
// one-heavy-build PreToolUse hook (owner 2026-09-07, hard gate G2 of the tests
// audit; ADR 0038). Node instead of bash+jq: the Nix devshell (and CI, where
// scripts/check/one-heavy-build.test.mjs drives this file) carries node but
// not jq — a jq-based hook failed OPEN there (review 2026-09-08).
//
// Contract (testing.md § Resource discipline): deny a Bash command that would
// START a heavy build/test run (classification: one-heavy-build-classify.mjs,
// head tokens only) while another such run is alive on the machine.
// Seams: BRAWLER_HEAVY_PS_OVERRIDE (when set, replaces pgrep — hermetic tests),
// BRAWLER_ALLOW_PARALLEL_BUILD=1 (deliberate parallel run).
import { spawnSync } from "node:child_process";
import { classifyCommand } from "./one-heavy-build-classify.mjs";

const PGREP_PATTERN = "(^|/)(cargo|rustc|cargo-nextest|nextest|vitest|playwright)( |$)";

function runningHeavy() {
  if (Object.hasOwn(process.env, "BRAWLER_HEAVY_PS_OVERRIDE")) {
    return process.env.BRAWLER_HEAVY_PS_OVERRIDE.trim();
  }
  const r = spawnSync("pgrep", ["-af", PGREP_PATTERN], { encoding: "utf8" });
  if (r.status !== 0 || !r.stdout) return "";
  // The FULL list decides (a compiler hiding behind five JS matches must
  // still count); only the diagnostic shown to the agent is truncated.
  return r.stdout
    .split("\n")
    .filter((l) => l && !l.includes("one-heavy-build"))
    .join("\n");
}

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (c) => (input += c));
process.stdin.on("end", () => {
  let payload;
  try {
    payload = JSON.parse(input);
  } catch {
    process.exit(0); // malformed input: never block
  }
  if (payload?.tool_name !== "Bash") process.exit(0);
  if (process.env.BRAWLER_ALLOW_PARALLEL_BUILD === "1") process.exit(0);
  const cmd = payload?.tool_input?.command ?? "";
  const klass = classifyCommand(cmd);
  if (!klass) process.exit(0);
  const running = runningHeavy();
  if (!running) process.exit(0);
  // Matrix (owner 2026-09-08): a cargo run stacks a compile — deny only while
  // another cargo/rustc/nextest is alive; a FULL JS suite or composite target
  // denies while anything heavy is alive; scoped JS runs never reach here.
  const compileAlive = /(^|\s|\/)(cargo|rustc|cargo-nextest|nextest)(\s|$)/.test(running);
  if (klass === "cargo" && !compileAlive) process.exit(0);
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason:
          "one-heavy-build (testing.md § Resource discipline, hard gate G2): another build/test run is alive on this machine — stacking cargo/vitest/playwright OOM-freezes the WSL VM. Wait for it (Monitor/poll `pgrep -af cargo|vitest|playwright`), or scope your run smaller. Running now:\n" +
          running +
          "\nDeliberate parallel run: BRAWLER_ALLOW_PARALLEL_BUILD=1.",
      },
    }),
  );
  process.exit(0);
});
