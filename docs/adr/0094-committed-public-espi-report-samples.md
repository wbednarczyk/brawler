# ADR 0094: Committed public ESPI/EBI report samples for the real-format CI test tier

Status: Accepted (2026-08-04, epic #40 / #139; owner sign-off in chat — policy + sample selection)

Deciders: maintainer. Area: testing, source policy, repository content.

## Context

Every real-report extraction gate today is `#[ignore]`, env-gated, and owner-machine-only
(`private/realdata/`, ADR 0091 decision 4): CI never parses a real report container. The
ESEF, positional, and report-diff tiers are exercised in CI only on inline string literals
and small aggregator HTML pages — a parser regression on a *real* container (a real ESEF
`.xbri` package, a real PDF) cannot redden a PR. #139 (owner request 2026-07-07) asks for a
small set of real public ESPI report files committed as test samples, with owner-verified
extracted values pinned, so a regression that *changes numbers* fails CI, not the owner's
memory.

ADR 0091 decision 4 states "real data never enters the public repo or default CI". Its
motivation is the maintainer's **personal investment research** (the 121 MB live database:
watchlists, notes, judgments). Official ESPI periodic filings are a different category:
disclosures that issuers are **required by law to publish** (MAR / the Polish transparency
regime), already public on the ESPI system and mirrored by distributors. No existing ADR
records a position on committing such filing bytes; the recorded no-redistribution stances
(ADR 0072/0082/0085/0086) all concern **aggregator/API content** (BiznesRadar, Yahoo), which
stays untouched by this decision.

## Decision

1. **Narrow amendment of ADR 0091 decision 4.** Complete, deliberately selected **official
   ESPI/EBI periodic filing files** (report PDFs, ESEF report packages — both the GPW ESPI
   system and NewConnect's EBI are mandated public-disclosure channels) MAY be committed to the
   public repository as test samples. The carve-out covers only mandated-public issuer
   disclosures — never aggregator content, never anything derived from the maintainer's
   research database (facts, notes, labels, metrics stay under `private/` or as aggregates).
   This is an **owner risk/policy decision** about republishing documents that are already
   mandatorily public; it is not a legal conclusion, and it is revisitable if an issuer or
   distributor objects (removal + history note, no rewrite).
2. **Budget and shape.** Total committed sample budget: **≤ 5 MB** (forever in git history —
   deliberate). Files are committed **complete and unchanged** (truncation would destroy the
   real-format regression value); small filings are selected instead of truncating large ones.
3. **Attribution is mandatory and machine-checked.** Canonical artifact:
   `src-tauri/samples/reports/MANIFEST.json` — per file: issuer, report number, period,
   original filename, source/distributor URL (recorded truthfully — ESPI attachments are
   distributed via Bankier/Bonnier static hosts, not issuer domains), retrieval date, byte
   size, SHA-256, expected container/route, and test purpose. A guard test enforces: total
   ≤ 5 MB, every file manifested, hash/size/container match, and no unmanifested report
   binary under `samples/`.
4. **Golden values are cross-source-verified, not owner-read.** Expected extracted values
   are committed as machine-readable data compared exactly (not regenerable `insta`
   snapshots). Correctness anchors on **corroboration**, not manual reading: pinned values
   are checked against the production-validated facts for the same issuer/period (the
   ADR 0061 "good" gate + the ADR 0085/0086 aggregator-witness regime), and any
   re-pin after an intentional parser change repeats that cross-check. The owner signs off
   the policy and the sample selection — never a number-by-number reading of a report.
   **Boundary:** the corroboration is a maintainer-side process performed against the
   owner's production data *before* values are pinned; CI itself only compares committed
   bytes against the committed expected-values file and never touches private data
   (ADR 0091 decision 4 stands for CI).

## Consequences

- Real-container extraction regressions (ESEF package unzip, iXBRL fact values, PDF text
  extraction, no-text-layer parking) redden every PR in default `make check`.
- The repo grows by ~4.5 MB once; the paths-filter treats sample changes as code (full CI).
- ADR 0091 decision 4 remains in force for everything else — the maintainer's database,
  ground-truth labels, and per-document metrics never enter the repo; public reporting stays
  aggregate-only.
- Source policy pointer: [docs/source-strategy.md](../source-strategy.md) § ESPI/EBI notes
  this carve-out; aggregator no-redistribution stances are unaffected.
