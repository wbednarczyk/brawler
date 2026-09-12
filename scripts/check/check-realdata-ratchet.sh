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
esef_run="$repo_root/scripts/check/realdata-esef-run.sh"
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

# --- honesty profile (default, no --profile flag) ---------------------------

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

# --- esef wrapper: scripts/check/realdata-esef-run.sh (astra r1 finding 16) --

expect_exit_wrapper() {
  local expected="$1" case_name="$2" actual=0
  shift 2
  bash "$esef_run" "$@" >"$work/wrapper-out.txt" 2>&1 || actual=$?
  if [ "$actual" != "$expected" ]; then
    printf "realdata-esef-run self-test FAILED [%s]: expected exit %s, got %s\n" \
      "$case_name" "$expected" "$actual" >&2
    cat "$work/wrapper-out.txt" >&2
    exit 1
  fi
}

# 20. A filter matching zero tests: the fake "harness" never writes metrics
#     -> the wrapper must fail before the ratchet ever sees a missing file.
run_dir_20="$work/run20"
expect_exit_wrapper 1 "zero-selected-test never writes metrics" "$run_dir_20" -- true

# 21. A pre-existing run directory (a stale artifact, or two runs racing on
#     the same nonce) is refused outright, never silently reused.
run_dir_21="$work/run21"
mkdir -p "$run_dir_21"
expect_exit_wrapper 1 "pre-existing run directory is refused" "$run_dir_21" -- true

# 22. The harness actually writes metrics -> the wrapper prints the path.
run_dir_22="$work/run22"
out="$(bash "$esef_run" "$run_dir_22" -- bash -c 'echo "{}" > "$BRAWLER_ESEF_METRICS_OUT"')"
if [ "$out" != "$run_dir_22/realdata-esef-metrics.json" ]; then
  printf "realdata-esef-run self-test FAILED [wrapper prints the metrics path]: got %s\n" "$out" >&2
  exit 1
fi

# --- esef profile (#331 PR-A, ADR 0112) --------------------------------------

esef_baseline="$work/esef-baseline.json"
esef_metrics="$work/esef-metrics.json"
cat >"$esef_baseline" <<'JSON'
{ "profile": "esef", "status": "measured", "measurement_version": 1, "gt_version": "1", "key_map_version": 1, "normalization_version": 1, "registry_hash": "abc123",
  "events": 10, "floor_events": 6, "issuers": 5, "gt_slots": 100, "unverified": 2,
  "matched": 80, "previously_correct_slots_lost": 0, "false_positives": 3, "zero_output_events": 1,
  "availability_all_periods": {"available": 150, "eligible": 200}, "layer1_capture": {"captured": 50, "eligible": 60, "value_correct": 48},
  "labeled_capability": {"matched": 80, "labeled": 110}, "sensitivity": {"matched": 78, "gt_slots": 95, "excluded": 5}, "twin_agreement": {"agree": 20, "compared": 22},
  "replay": {"events": 8, "exercised_prior_check": 4, "exercised_quarantine": 2, "replay_matched": 1} }
JSON

expect_exit_esef() {
  local expected="$1" case_name="$2" actual=0
  shift 2
  node "$ratchet" "$@" >"$work/out.txt" 2>&1 || actual=$?
  if [ "$actual" != "$expected" ]; then
    printf "realdata-ratchet esef self-test FAILED [%s]: expected exit %s, got %s\n" \
      "$case_name" "$expected" "$actual" >&2
    cat "$work/out.txt" >&2
    exit 1
  fi
}

# 11. Metrics identical to the committed baseline — holds, exit 0.
cat >"$esef_metrics" <<'JSON'
{ "profile": "esef", "status": "measured", "measurement_version": 1, "gt_version": "1", "key_map_version": 1, "normalization_version": 1, "registry_hash": "abc123",
  "events": 10, "floor_events": 6, "issuers": 5, "gt_slots": 100, "unverified": 2,
  "matched": 80, "previously_correct_slots_lost": 0, "false_positives": 3, "zero_output_events": 1,
  "availability_all_periods": {"available": 150, "eligible": 200}, "layer1_capture": {"captured": 50, "eligible": 60, "value_correct": 48},
  "labeled_capability": {"matched": 80, "labeled": 110}, "sensitivity": {"matched": 78, "gt_slots": 95, "excluded": 5}, "twin_agreement": {"agree": 20, "compared": 22},
  "replay": {"events": 8, "exercised_prior_check": 4, "exercised_quarantine": 2, "replay_matched": 1} }
JSON
expect_exit_esef 0 "esef holds at the committed bounds" --profile esef --baseline "$esef_baseline" --metrics "$esef_metrics"

# 12. A previously-matched slot regressed — the hard zero-loss ceiling, exit 1.
sed 's/"previously_correct_slots_lost": 0/"previously_correct_slots_lost": 1/' "$esef_metrics" > "$work/m12.json"
expect_exit_esef 1 "esef lost a previously-correct slot" --profile esef --baseline "$esef_baseline" --metrics "$work/m12.json"

# 13. False positives above the committed ceiling — exit 1.
sed 's/"false_positives": 3/"false_positives": 4/' "$esef_metrics" > "$work/m13.json"
expect_exit_esef 1 "esef false positives above the ceiling" --profile esef --baseline "$esef_baseline" --metrics "$work/m13.json"

# 14. Matched fell below the committed floor — exit 1.
sed 's/"matched": 80/"matched": 79/' "$esef_metrics" > "$work/m14.json"
expect_exit_esef 1 "esef matched fell below the floor" --profile esef --baseline "$esef_baseline" --metrics "$work/m14.json"

# 15. An equality field (key_map_version) differs — incomparable, exit 2.
sed 's/"key_map_version": 1/"key_map_version": 2/' "$esef_metrics" > "$work/m15.json"
expect_exit_esef 2 "esef equality-field mismatch (rebaseline required)" --profile esef --baseline "$esef_baseline" --metrics "$work/m15.json"

# 16. The baseline is still the unmeasured placeholder — exit 2.
printf '{ "profile": "esef", "status": "unmeasured" }' > "$work/unmeasured-baseline.json"
expect_exit_esef 2 "esef unmeasured baseline is refused" --profile esef --baseline "$work/unmeasured-baseline.json" --metrics "$esef_metrics"

# 17. Matched improved beyond the raise threshold but the baseline was never
#     promoted — a silent raise would let a looser (stale) baseline stand.
sed 's/"matched": 80/"matched": 81/' "$esef_metrics" > "$work/m17.json"
expect_exit_esef 2 "esef improvement pending promotion" --profile esef --baseline "$esef_baseline" --metrics "$work/m17.json"

# 18. An unknown profile name is a hard failure, never a silent default.
expect_exit_esef 2 "esef unknown profile name" --profile bogus --baseline "$esef_baseline" --metrics "$esef_metrics"

# 19. A negative informational metric (never ratcheted, but must still be a
#     finite, non-negative number) — exit 2.
sed 's/"available": 150/"available": -1/' "$esef_metrics" > "$work/m19.json"
expect_exit_esef 2 "esef negative informational metric" --profile esef --baseline "$esef_baseline" --metrics "$work/m19.json"

# 23. Amendment J / astra r1 finding 17: "hard zero" must be a REAL hard
#     zero -- a baseline that itself carries a nonzero loss is refused
#     outright (the old ceiling comparison let baseline=1/run=1 pass).
sed 's/"previously_correct_slots_lost": 0/"previously_correct_slots_lost": 1/' "$esef_baseline" > "$work/baseline23.json"
expect_exit_esef 2 "esef baseline must itself carry a hard zero" --profile esef --baseline "$work/baseline23.json" --metrics "$esef_metrics"

# 24. Same run-side loss the baseline also (wrongly) carries: the loss still
#     fails -- it is never judged AGAINST the baseline's own value.
expect_exit_esef 2 "esef loss regardless of a (refused) nonzero baseline" --profile esef --baseline "$work/baseline23.json" --metrics "$work/m12.json"

# 25. Amendment H: the renamed replay.replay_matched field is missing ->
#     a schema violation, exit 2 (never silently skipped as "informational").
sed 's/"replay_matched": 1//' "$esef_metrics" | sed 's/"exercised_quarantine": 2,/"exercised_quarantine": 2/' > "$work/m25.json"
expect_exit_esef 2 "esef schema: replay.replay_matched missing" --profile esef --baseline "$esef_baseline" --metrics "$work/m25.json"

# 26. A string where the schema expects a number -- never silently coerced.
sed 's/"matched": 80,/"matched": "80",/' "$esef_metrics" > "$work/m26.json"
expect_exit_esef 2 "esef schema: string in a numeric field" --profile esef --baseline "$esef_baseline" --metrics "$work/m26.json"

# 27. null where the schema expects a number -- never silently treated as 0.
sed 's/"unverified": 2,/"unverified": null,/' "$esef_metrics" > "$work/m27.json"
expect_exit_esef 2 "esef schema: null in a numeric field" --profile esef --baseline "$esef_baseline" --metrics "$work/m27.json"

# 28. An equality field with the wrong TYPE (a numeric gt_version instead of
#     the string contract) fails the schema before equality is even compared.
sed 's/"gt_version": "1",/"gt_version": 1,/' "$esef_metrics" > "$work/m28.json"
expect_exit_esef 2 "esef schema: gt_version must be a string" --profile esef --baseline "$esef_baseline" --metrics "$work/m28.json"

printf "realdata-ratchet self-test: regressions exit 1, stale baseline / unreadable input exit 2, healthy run exits 0 (honesty + esef profiles).\n"
