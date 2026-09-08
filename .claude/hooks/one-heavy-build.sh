#!/usr/bin/env bash
# one-heavy-build PreToolUse hook (owner 2026-09-07, hard gate G2 of the tests
# audit; ADR 0038 enforcement posture). Turns testing.md § Resource discipline
# ("one cargo/nextest/vitest/playwright at a time — the WSL VM OOM-froze twice")
# into a mechanical boundary for every agent and subagent in this repo.
#
# Precision contract (ADR 0045 — never flag legitimate use, B3 fix
# 2026-09-08): classification is delegated to one-heavy-build-classify.mjs,
# which splits the command into `;`/`&&`/`||`/`|`/newline segments, strips
# quoted strings and leading `env VAR=val…`/`rtk`/`nix develop … -c`/`cd
# <dir>` prefixes, and matches only the HEAD TOKENS of each segment (program
# + subcommand) — so a heavy word inside a commit message or --body string,
# or a `rtk read`/`cd x && ls` prefix, never trips it.
# - Denies ONLY while another such run is already alive on this machine
#   (pgrep on cargo/rustc/cargo-nextest/nextest/vitest/playwright — NOT
#   rust-analyzer/playwright-mcp/vite/tsc/esbuild, which share substrings
#   with the real targets but are dev-server/LSP/MCP processes, not builds).
# - Everything else (rtk read/grep, repoctx, git, gh, node scripts) passes.
# - Escape hatch for a deliberate second run: BRAWLER_ALLOW_PARALLEL_BUILD=1.
set -euo pipefail

input=$(cat)
tool=$(printf '%s' "$input" | jq -r '.tool_name // empty')
[ "$tool" = "Bash" ] || exit 0
[ "${BRAWLER_ALLOW_PARALLEL_BUILD:-0}" = "1" ] && exit 0

cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty')
hook_dir=$(dirname "${BASH_SOURCE[0]}")
printf '%s' "$cmd" | node "$hook_dir/one-heavy-build-classify.mjs" || exit 0

# Test seam: BRAWLER_HEAVY_PS_OVERRIDE (when SET, even empty) replaces the
# pgrep result so scripts/check/one-heavy-build.test.mjs is hermetic — it never
# depends on what is really running on the machine.
if [ -n "${BRAWLER_HEAVY_PS_OVERRIDE+x}" ]; then
  running="$BRAWLER_HEAVY_PS_OVERRIDE"
else
  running=$(pgrep -af '(^|/)(cargo|rustc|cargo-nextest|nextest|vitest|playwright)( |$)' 2>/dev/null | grep -v 'one-heavy-build' | head -5 || true)
fi
[ -n "$running" ] || exit 0

jq -n --arg r "$running" '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    permissionDecision: "deny",
    permissionDecisionReason: ("one-heavy-build (testing.md § Resource discipline, hard gate G2): another build/test run is alive on this machine — stacking cargo/vitest/playwright OOM-freezes the WSL VM. Wait for it (Monitor/poll `pgrep -af cargo|vitest|playwright`), or scope your run smaller. Running now:\n" + $r + "\nDeliberate parallel run: BRAWLER_ALLOW_PARALLEL_BUILD=1.")
  }
}'
