# ESEF measurement v2 tooling (#331 PR-A, ADR 0112)

Stdlib-only Python tooling that builds the private ESEF ground-truth corpus
and feeds the `storage::tests::real_data_esef_v2` harness. Data contract:
`private/realdata/spikes/esef-v2/` layout, JSON shapes, outcome classes —
see ADR 0112 and `docs/testing.md` § ESEF measurement v2 (canonical; this
file is usage only).

## Duration → period_type

`build_frame.py`/`label_esef_v2.py` classify a duration context by its
length (day-count based, ±1 month tolerance), not by calendar position.
GPW interim ESEF/iXBRL filings are cumulative year-to-date (the
current-period column IS the YTD column), so 9 months is always Q3:

| Duration | `period_type` |
| --- | --- |
| ~12 months | `FY` |
| ~9 months | `Q3` (cumulative YTD) |
| ~6 months | `H1` |
| ~3 months | `Q1`/`Q2`/`Q3`/`Q4` by end month vs. the fiscal-year start |
| anything else | `unknown` (review-queue.json) |

`duration_months` is recorded on the manifest event's `labeled_period` and
on every derived ground-truth slot. Window is `flow` for every duration
(vs. `point_in_time` for an instant) regardless of `period_type`.

## Public vs private

| Public (this repo) | Private (`$BRAWLER_ESEF_V2_DIR`, gitignored) |
| --- | --- |
| `esef_ixbrl.py`, `build_frame.py`, `label_esef_v2.py`, `import_v1.py`, `adjudicate.py` | `MANIFEST_v2.json`, `occurrences_v2.json`, `ground_truth_v2.json`, `review-queue.json` |
| `gt_key_map.json` (content-free: concept names + version, no filing data) | `corpus/` (the actual filing bytes), `adjudication/` |
| `tests/` (synthetic `ZZZ` fixtures only) | `baseline/keyed-baseline.json`, `scoring-report-v2.json`, `keyed-outcomes.<run>.json` |
| `realdata-esef-baseline.json` (repo root, aggregates only) | — |

Never commit anything under `$BRAWLER_ESEF_V2_DIR` — it holds real issuer
filings and values. The public baseline carries counts/hashes only (ADR 0091
dec. 4).

## Pipeline (owner's order)

```bash
export BRAWLER_ESEF_V2_DIR=private/realdata/spikes/esef-v2   # default shown

# 1. Build the frame: inventory the pinned snapshot's fetched documents by
#    bytes, classify each iXBRL instance, select the floor/twin/warmup panel.
python3 build_frame.py --snapshot <sqlite path> --data-dir <report_documents dir> \
  --issuers <tickers.csv|acceptance-list-file> --out "$BRAWLER_ESEF_V2_DIR" \
  [--select newest-annual-interim] [--pin-language pl]
# Review review-queue.json for anything classified "unknown" before proceeding.

# 2. Label: parse every selected event's file, emit occurrences_v2.json
#    (every concept, no panel filter) and derive ground_truth_v2.json slots.
python3 label_esef_v2.py --esef-v2-dir "$BRAWLER_ESEF_V2_DIR"

# 3. (Optional) Import the v1 (#182) merged ground truth as machine_v1
#    reference slots, excluded from floors until independently re-verified.
python3 import_v1.py --v1-dir private/realdata/spikes/esef-positional-gt \
  --esef-v2-dir "$BRAWLER_ESEF_V2_DIR"

# 4. Blinded adjudication (LABELING.md protocol): resolve machine conflicts
#    and spot-check a frozen 10% agreement sample. `compare` matches a
#    reader's value against the machine's: agree -> second_read, differs ->
#    adjudicated (settles a conflict OR flags a contradicted agreement-sample
#    slot as a systematic-error signal), unsettled -> stays unverified.
python3 adjudicate.py --esef-v2-dir "$BRAWLER_ESEF_V2_DIR" prepare --seed <int>
#   hand adjudication/tasks/*.json to a reader; they answer blind (no value,
#   no hint which task is a disagreement vs. an agreement check)
python3 adjudicate.py --esef-v2-dir "$BRAWLER_ESEF_V2_DIR" seal <reader> <answers.json>
python3 adjudicate.py --esef-v2-dir "$BRAWLER_ESEF_V2_DIR" compare

# 5. Measure: runs the Rust harness against this corpus and judges it
#    against the committed baseline (never `make check` — owner machine only).
make realdata-esef-check ESEF_V2_DIR="$BRAWLER_ESEF_V2_DIR"

# 6. Promote deliberately once the report is reviewed (never automatic).
make realdata-esef-promote RUN=<nonce>
# then hand-update realdata-esef-baseline.json (the public aggregates) in a
# reviewed commit.
```

## Env / Make targets

| Name | Meaning | Default |
| --- | --- | --- |
| `BRAWLER_ESEF_V2_DIR` | corpus dir | `private/realdata/spikes/esef-v2` |
| `BRAWLER_ESEF_KEYED_BASELINE` | promoted keyed baseline | `$BRAWLER_ESEF_V2_DIR/baseline/keyed-baseline.json` |
| `BRAWLER_ESEF_METRICS_OUT` | nonce metrics path (required in required mode) | — |
| `BRAWLER_ESEF_REQUIRED=1` | turn harness SKIPs into panics | — |
| `make realdata-esef-score` | diagnostic harness run, default dirs | |
| `make realdata-esef-check` | reproducible gate: fresh nonce → harness → ratchet | |
| `make realdata-esef-promote RUN=<nonce>` | pin a new private keyed baseline | |

## Tests

`python3 -m unittest discover -s scripts/realdata/esef-v2/tests -p "test_*.py"`
(also wired into `make check-docs-gates`). Synthetic `ZZZ` fixtures only —
no real data, runs anywhere.
