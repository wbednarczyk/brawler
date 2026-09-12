# ESEF measurement v2 — labeling and blinded adjudication protocol

Protocol version: 1 (ADR 0112, #331). Every artifact this protocol produces stays in the owner's
private corpus directory (`private/realdata/spikes/esef-v2/`, gitignored); only aggregate counts
ever reach the repository (ADR 0091 dec. 4).

## Roles

| Role | Who / what | Sees |
| --- | --- | --- |
| First reader | `label_esef_v2.py` — the stdlib iXBRL reference parser, written blind to the app's ESEF parser | the filing bytes only |
| Second reader | a human offline read, or an external agent the owner runs (BYOA — the app has no in-app AI) | the task file: filing reference, concept, fiscal period, evidence location — **never** a candidate value, the app's value, the first reader's value, or why the task was selected |
| Adjudicator | the owner, with `adjudicate.py compare` | both sealed answers, mechanically compared |

The app's extraction output is never an input to any reader. A reader who has seen an app value for
an event is disqualified for that event.

## Steps (owner order)

1. **Frame** — `build_frame.py` selects events from the pinned snapshot by bytes and filing evidence
   (never from extraction results). Anything it cannot resolve (`basis_scope`, `language`,
   `period_type` = `unknown`) lands in `review-queue.json` and is resolved by reading the filing,
   evidence anchor quoted, before labeling.
2. **First read** — `label_esef_v2.py` emits `occurrences_v2.json` (immutable) and derives
   `ground_truth_v2.json` slots (`verification: machine`). Conflicting duplicate occurrences become
   `unverified` with both values kept in `resolution_ref`.
3. **Task build** — `adjudicate.py prepare --seed <n>`: every machine/`unverified` disagreement class
   plus a **seeded, frozen 10 % sample of agreements** (minimum 10 tasks, or all when fewer) become
   indistinguishable task files (`adjudication/tasks/<id>.json`); `tasks.lock` pins their hashes.
   The sample is frozen before any second read starts.
4. **Second read** — the reader answers every task from the filing only (value, currency, period,
   basis, attribution as read; `unverified` + reason when the filing does not settle it) and the
   answers are **sealed** (`adjudicate.py seal <reader> <answers.json>` — hashed, timestamped, refused
   once a compare has run). `reader.json` records identity/protocol version (human) or model id +
   prompt hash + input allow-list (agent).
5. **Compare** — `adjudicate.py compare` first re-verifies the lock (every locked task file + index + ground-truth/occurrence hashes) and every seal registered in `seals.lock` (a missing, altered or unregistered seal aborts), validates each answer (finite decimal; a three-letter currency for a monetary task unit and `null` for a shares/pure unit; basis/attribution/period domains — an invalid answer fails the whole compare, nothing is written), then compares sealed answers with the machine slots mechanically:
   agree → `second_read`; disagree with filing evidence → `adjudicated` (the evidence anchor is the
   record, the app's value is never consulted); unsettled, or contradicted without filing evidence → `unverified` (a contradicted `machine` label never stays `machine`; excluded from every
   denominator, count pinned in the baseline).
6. **Systematic-error rule** — if the agreement sample reveals a labeling error class (a transform,
   a sign convention, a namespace), the whole affected class is re-read before any floor is pinned.
7. **Freeze** — `gt_version`, `key_map_version`, `normalization_version` and the corpus
   `registry_hash` are frozen together; any change bumps `measurement_version` and requires a new,
   deliberately promoted baseline (`make realdata-esef-promote`).

## Convention-resolved rows

`gt_key_map.json` enumerates the only normalizations applied to BOTH sides (cumulative context →
`flow`, cash-flow outflow sign); each slot records the rule ids it went through (`normalized_by`), and so
does each prediction on the harness side. A basis, window, variant or period-date difference is **never**
normalized — it is scored as a mismatch. The sensitivity score in every report recomputes
recall/precision with the convention-resolved population removed from both sides.

## What a labeling artifact may contain

Private files: everything (tickers, titles, values, evidence quotes). Public files
(`gt_key_map.json`, this protocol, the ratchet baseline, the metrics JSON): counts, versions, hashes
— never a ticker, title, id, filename or value.
