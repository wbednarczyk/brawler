#!/usr/bin/env bash
# ts-export-reminder PostToolUse hook — thin wrapper; the logic (and its
# tests) live in ts-export-reminder.mjs. Contract: see that file and
# testing.md § Resource discipline.
exec node "$(dirname "$0")/ts-export-reminder.mjs"
