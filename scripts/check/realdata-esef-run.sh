#!/usr/bin/env bash
# Wrapper used by `make realdata-esef-check` (#331 PR-A, ADR 0112, amendment
# 1 K; fixes astra r1 finding 16: the old recipe passed `--exact` to cargo
# itself instead of after `--` to libtest, so the target could never reach
# the harness, and its timestamp-based freshness check accepted same-second
# files with no uniqueness guarantee).
#
# Reserves a FRESH run directory (never reused -- refuses if it already
# exists, covering the "pre-existing artifact" case) before running the
# given harness command with BRAWLER_ESEF_METRICS_OUT pointed inside it, then
# refuses to conclude without a metrics file the harness actually wrote
# (covering the "zero-selected-test" case: a test filter matching nothing
# still exits 0, it just never runs the harness body). Prints the metrics
# path to stdout on success -- the only thing the caller should capture.
#
# Usage: realdata-esef-run.sh <run-dir> -- <command...>
# Testable standalone (scripts/check/check-realdata-ratchet.sh) by injecting
# a fake harness command instead of the real `cargo test`.
set -euo pipefail

if [ "$#" -lt 2 ] || [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
  printf "Usage: realdata-esef-run.sh <run-dir> -- <command...>\n" >&2
  exit 64
fi

run_dir="$1"
shift
if [ "$1" != "--" ]; then
  printf "realdata-esef-run: usage: realdata-esef-run.sh <run-dir> -- <command...>\n" >&2
  exit 64
fi
shift

if [ -d "$run_dir" ]; then
  printf "realdata-esef-run: run directory %s already exists -- refusing to reuse a stale/pre-existing artifact\n" "$run_dir" >&2
  exit 1
fi
mkdir -p "$run_dir"

metrics_out="$run_dir/realdata-esef-metrics.json"
BRAWLER_ESEF_METRICS_OUT="$metrics_out" "$@"

if [ ! -f "$metrics_out" ]; then
  printf "realdata-esef-run: no fresh metrics -- the harness did not run (%s missing; a zero-selected-test filter or a stale wrapper never wrote it)\n" "$metrics_out" >&2
  exit 1
fi

printf "%s\n" "$metrics_out"
