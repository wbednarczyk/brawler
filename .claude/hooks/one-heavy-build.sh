#!/usr/bin/env bash
# one-heavy-build PreToolUse hook — thin wrapper; the logic (and its tests)
# live in one-heavy-build.mjs (node, no jq: the Nix devshell has none).
# Contract and seams: see that file and testing.md § Resource discipline.
exec node "$(dirname "$0")/one-heavy-build.mjs"
