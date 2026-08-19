# ADR 0103: File-Size Ratchet — Architectural Fitness Function For Oversized Modules

Status: Proposed (2026-08-19, epic #402)

Deciders: maintainer. Area: tooling, architecture, agent process.

## Context

CLAUDE.md declares very large source files architecture debt to be paid down as part of feature
slices — the repo's only architectural rule with **no enforcing gate** ([ADR 0038](0038-enforcement-as-guardrails.md):
an unenforced rule rots). Measured effect: `mcp/registry.rs`, `mcp/kpi_ingest_context.rs` and
`storage/kpi_ingest_staging.rs` grew slice-by-slice across epics #384–#399 to 2 900–3 600 lines,
because every slice's minimal-diff discipline locally beat the extraction rule. The fix is settled
industry practice: an **architectural fitness function** (Ford/Parsons, *Building Evolutionary
Architectures*) with a **ratchet baseline** — the same pattern the repo's two coverage gates
already use (`coverage-baseline.json`).

## Decisions

1. **The gate.** `scripts/check/file-size-ratchet.mjs` runs in `make check-docs-gates` (a
   `MANDATORY_SUITES` entry in gate-integrity guards the step) and in `parallel-check` Stage 1, so
   both CI and `make check-local` see it. `file-size-baseline.json` pins every production source
   file at/over `thresholdLines` at its **exact** current line count.
2. **Threshold 1000 lines** (owner 2026-08-19; the epic draft said ~1500, lowered because a
   ~1480-line cluster — `CockpitScreen.tsx`, `report_tagged_facts.rs`, `kpi_manifest.rs` — would
   have been born just under a 1500 gate with free room to grow). Counting = number of `\n`
   (`wc -l` equivalent).
3. **Exact pin, both directions.** Growth of a pinned file fails (extract as part of the change);
   shrinking without re-running `--write` also fails — a stale-high pin is silent regrow headroom.
   Consequence: a pin can never sit above reality, so headroom cannot be pre-bought; a hand-raised
   pin must match the actual count of the very change that grew the file.
4. **`--write` only moves down.** It lowers pins, drops entries that fell under the threshold, and
   **refuses** raises and additions. Raising a pin (or adding one for a new offender) is a hand
   edit to `file-size-baseline.json` — the honest enforcement mechanism in both directions is that
   diff line in front of a reviewer, never an automated bypass. After a rebase, re-run `--write`
   and re-review; baseline conflicts are resolved by regeneration, not manual merge.
5. **Scope: production code, not tests, not data.** `src-tauri/src/**/*.rs` + `src/**/*.{ts,tsx}`,
   excluding dedicated test files/dirs (`storage/tests/`, `**/tests.rs`, `src/test/`,
   `*.test.*`/`*.spec.*`), generated bindings (`src/api/generated/`) and locale resource tables
   (`src/shared/locale/resources/`). Colocated `#[cfg(test)]` modules deliberately **count**
   toward a file's total — extracting tests into a dedicated test file is a legitimate ratchet
   move, not an exemption.
6. **Tail policy.** The initial baseline pins ~40 files (largest: `storage/kpi_ingest_runs.rs`
   6 017). Epic #402 extracts only the three headline offenders; the rest stay frozen and get
   extracted **as part of whichever future slice next touches them** (the ratchet forces the
   moment), not as a dedicated epic.

## Consequences

- Any PR that grows a pinned file reddens until the file is split or the pin is consciously
  hand-raised in the diff. This is the CLAUDE.md extraction rule finally holding under pressure.
- PRs that shrink a pinned file must include the `--write` baseline update (one command).
- Renaming a production file to match an exclusion (e.g. `tests.rs`) would dodge the gate but is
  loudly review-visible; no automated counter-guard is added (ADR 0038: no broad gate that flags
  legitimate code).

## Rejected

- **Tolerance bands / suggest-only shrink** (the coverage-ratchet posture): line counts are exact
  and deterministic, so slack only creates regrow headroom; measurement noise does not exist here.
- **A `--force`/override flag**: the override IS the hand edit, visible in review.
- **Per-function/complexity metrics**: heavier tooling, subjective thresholds; file length is the
  debt actually observed and the cheapest honest proxy.
