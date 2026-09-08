#!/usr/bin/env bash
# git-boundaries PreToolUse hook — thin wrapper; the logic (and its tests)
# live in git-boundaries.mjs (node, same protocol as one-heavy-build.sh).
# Contract and seams: see that file and CLAUDE.md § Working Rules / T1.
exec node "$(dirname "$0")/git-boundaries.mjs"
