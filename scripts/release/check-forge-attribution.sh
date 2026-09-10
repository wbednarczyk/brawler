#!/usr/bin/env bash
# Forge hygiene (CLAUDE.md § Claude-Native Ecosystem): no AI/agent attribution
# anywhere on the forge — history stays authored by the human maintainer.
# Scans ONE text file (a full commit message, a PR body) and fails on an
# AI co-author/trailer or a "generated with <agent>" footer. Precise on
# purpose: a human `Co-authored-by:` trailer passes.
set -euo pipefail

file="${1:?usage: $0 <text-file>}"
agents='claude|anthropic|codex|openai|chatgpt|gpt-[0-9]|copilot|gemini|cursor|devin|aider'
hits="$(grep -n -i -E \
  "^(co-authored-by|signed-off-by|generated-by|assisted-by|reviewed-by):.*(${agents})|generated with .*(${agents})|claude code|noreply@anthropic\.com" \
  "$file" || true)"
if [ -n "$hits" ]; then
  printf "AI/agent attribution is not allowed on the forge (CLAUDE.md § Claude-Native Ecosystem):\n%s\n" "$hits" >&2
  exit 1
fi
