#!/usr/bin/env bash
# repoctx-first PreToolUse hook (issue #163, harvested from Graphify; ADR 0038
# enforcement posture): turn the CLAUDE.md "repoctx over grep for structural
# questions" rule into a mechanical guardrail.
#
# Precision contract (ADR 0045 — never flag legitimate use):
# - Intercepts ONLY the native Grep tool. Bash-level `rtk grep` stays free —
#   it is the sanctioned path for prose/string-literal scans.
# - Denies ONLY single-identifier patterns (one \w+ token, ≥4 chars, no
#   spaces/quotes/regex syntax) targeting code — exactly the queries repoctx
#   answers better (search/definition/callers/impact).
# - Prose targets (docs/, wiki/, *.md) always pass.
set -euo pipefail

input=$(cat)
tool=$(printf '%s' "$input" | jq -r '.tool_name // empty')
[ "$tool" = "Grep" ] || exit 0

pattern=$(printf '%s' "$input" | jq -r '.tool_input.pattern // empty')
target=$(printf '%s' "$input" | jq -r '(.tool_input.path // "") + " " + (.tool_input.glob // "")')

case "$target" in
  *docs*|*wiki*|*.md*) exit 0 ;;
esac

if printf '%s' "$pattern" | grep -Eq '^[A-Za-z_][A-Za-z0-9_]{3,}$'; then
  jq -n --arg p "$pattern" '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: ("repoctx-first (CLAUDE.md token discipline): \"\($p)\" is a structural identifier query — use `repoctx search \($p)` (or definition/callers/impact) instead; it is indexed, resolution-aware and token-cheap. If \($p) is genuinely a prose/string literal, scan it with `rtk grep` via Bash or scope the Grep to docs/*.md.")
    }
  }'
  exit 0
fi

exit 0
