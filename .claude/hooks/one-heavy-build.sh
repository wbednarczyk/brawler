#!/usr/bin/env bash
# one-heavy-build PreToolUse hook (owner 2026-09-07, hard gate G2 of the tests
# audit; ADR 0038 enforcement posture). Turns testing.md § Resource discipline
# ("one cargo/nextest/vitest/playwright at a time — the WSL VM OOM-froze twice")
# into a mechanical boundary for every agent and subagent in this repo.
#
# Precision contract (ADR 0045 — never flag legitimate use):
# - Intercepts ONLY Bash commands that START a heavy build/test run:
#   cargo {build,test,nextest,clippy,llvm-cov,check,mutants}, nextest, vitest,
#   playwright test, make {check*,test,ui-smoke*,coverage*,build,tauri-build,
#   package-*}, npm run {test,check*,build,coverage*}, npx {vitest,playwright}.
# - Denies ONLY while another such run is already alive on this machine
#   (pgrep on cargo/rustc/nextest/vitest/playwright/tsc/vite/esbuild).
# - Everything else (rtk read/grep, repoctx, git, gh, node scripts) passes.
# - Escape hatch for a deliberate second run: BRAWLER_ALLOW_PARALLEL_BUILD=1.
set -euo pipefail

input=$(cat)
tool=$(printf '%s' "$input" | jq -r '.tool_name // empty')
[ "$tool" = "Bash" ] || exit 0
[ "${BRAWLER_ALLOW_PARALLEL_BUILD:-0}" = "1" ] && exit 0

cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty')
heavy='(^|[[:space:];&|(]|rtk[[:space:]]+)(cargo[[:space:]]+(build|test|nextest|clippy|llvm-cov|check|mutants)|cargo-nextest|nextest[[:space:]]+run|vitest|playwright[[:space:]]+test|make[[:space:]]+(-C[[:space:]]+[^[:space:]]+[[:space:]]+)?(check|test|ui-smoke|coverage|build|tauri-build|package-)|npm[[:space:]]+(run[[:space:]]+)?(test|check|build|coverage)|npx[[:space:]]+(vitest|playwright))'
printf '%s' "$cmd" | grep -Eq "$heavy" || exit 0

running=$(pgrep -af '(^|/)(cargo|rustc|cargo-nextest|nextest|vitest|playwright|tsc|vite|esbuild)( |$)' 2>/dev/null | grep -v 'one-heavy-build' | head -5 || true)
[ -n "$running" ] || exit 0

jq -n --arg r "$running" '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    permissionDecision: "deny",
    permissionDecisionReason: ("one-heavy-build (testing.md § Resource discipline, hard gate G2): another build/test run is alive on this machine — stacking cargo/vitest/playwright OOM-freezes the WSL VM. Wait for it (Monitor/poll `pgrep -af cargo|vitest|playwright`), or scope your run smaller. Running now:\n" + $r + "\nDeliberate parallel run: BRAWLER_ALLOW_PARALLEL_BUILD=1.")
  }
}'
