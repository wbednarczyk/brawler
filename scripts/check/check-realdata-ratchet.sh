#!/usr/bin/env bash
# Self-test for the real-data honesty ratchet (epic #40 S4/S5, ADR 0091 dec. 4-5).
# Idiom: scripts/release/check-commit-message-validator.sh — the ratchet is the
# only thing standing between a silent honesty regression and a green gate, so
# its verdicts are themselves tested.
#
# Fully synthetic: it feeds the ratchet hand-written metric/baseline JSON in a
# temp dir. NO real data, no real database — runs anywhere, needs no secrets.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ratchet="$repo_root/scripts/check/realdata-ratchet.mjs"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

baseline="$work/baseline.json"
metrics="$work/metrics.json"
cat >"$baseline" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 27, "filename_as_statement": 0, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON

# Runs the ratchet and asserts its exit code; prints its output on mismatch.
expect_exit() {
  local expected="$1" case_name="$2" actual=0
  node "$ratchet" --baseline "$baseline" --metrics "$metrics" >"$work/out.txt" 2>&1 || actual=$?
  if [ "$actual" != "$expected" ]; then
    printf "realdata-ratchet self-test FAILED [%s]: expected exit %s, got %s\n" \
      "$case_name" "$expected" "$actual" >&2
    cat "$work/out.txt" >&2
    exit 1
  fi
}

# 1. Metrics on the committed bounds (and inside tolerance) — green.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.6, "orphaned_evidence": 27, "filename_as_statement": 0, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON
expect_exit 0 "holds at the committed bounds"

# 2. Specificity fell past tolerance — an honesty regression.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 58.0, "orphaned_evidence": 27, "filename_as_statement": 0, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON
expect_exit 1 "specificity below the floor"

# 3. More orphaned evidence than the committed ceiling — a regression.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 28, "filename_as_statement": 0, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON
expect_exit 1 "orphaned evidence above the ceiling"

# 4. A filename reached a row statement — the hard bound, no tolerance.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 27, "filename_as_statement": 1, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON
expect_exit 1 "filename-as-statement is never tolerated"

# 5. Honesty improved but the baseline was never tightened — a silent raise
#    leaves the ratchet judging an old, looser app.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 75.0, "orphaned_evidence": 27, "filename_as_statement": 0, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON
expect_exit 2 "uncommitted specificity improvement"

cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 12, "filename_as_statement": 0, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON
expect_exit 2 "uncommitted orphaned-evidence improvement"

# 6. A metric silently dropped from the harness output — cannot conclude.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 27, "zero_effect_successes": 82, "silent_missing_metrics": 0 }
JSON
expect_exit 2 "metric missing from the harness output"

# 7. Unreadable input — cannot conclude (never a false green).
printf 'not json' >"$metrics"
expect_exit 2 "malformed metrics file"


# 8. Epic #40 S5 — a success that produced nothing and claims an emission is a
#    regression the moment there is one more of them than the committed ceiling.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 27, "filename_as_statement": 0, "zero_effect_successes": 83, "silent_missing_metrics": 0 }
JSON
expect_exit 1 "zero-effect successes above the ceiling"

# 9. A health read model started reporting a missing number without naming what
#    is missing — the committed ceiling is 0, so one is a regression.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 27, "filename_as_statement": 0, "zero_effect_successes": 82, "silent_missing_metrics": 1 }
JSON
expect_exit 1 "silent missing metric above the ceiling"

# 10. The stale-baseline rule applies to the S5 ceiling too: re-extraction heals
#     the stored rows, and an uncommitted improvement leaves the ratchet judging
#     a looser app than the one that shipped.
cat >"$metrics" <<'JSON'
{ "specificity_pct": 60.9, "orphaned_evidence": 27, "filename_as_statement": 0, "zero_effect_successes": 40, "silent_missing_metrics": 0 }
JSON
expect_exit 2 "uncommitted zero-effect-success improvement"

printf "realdata-ratchet self-test: regressions exit 1, stale baseline / unreadable input exit 2, healthy run exits 0.\n"
