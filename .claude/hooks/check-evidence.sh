#!/usr/bin/env bash
# check-evidence PreToolUse hook — thin wrapper; the logic (and its tests)
# live in check-evidence.mjs. Contract: see that file and DoD §K
# (engineering-workflow.md).
exec node "$(dirname "$0")/check-evidence.mjs"
