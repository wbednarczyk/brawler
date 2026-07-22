# ADR 0085: BiznesRadar Fundamentals Page as the Aggregator Witness

Status: Accepted (2026-07-21, owner sign-off)

Realizes decision 4 of [ADR 0061](0061-deterministic-fundamentals-data-gathering.md) ("HTML aggregator as second witness + fallback"), whose text explicitly gates the live binding: *"The adapter (parser) ships now; binding a live BiznesRadar fetch is gated on a source-specific scraping ADR (ToS review, politeness/rate-limit, cache) per the source-strategy rule."* Relates to [ADR 0084](0084-retire-in-app-ai-layer.md) (with the AI tier removed, the witness is now the pipeline's **last** layer, so its value rises), [ADR 0072](0072-ownership-structure.md) and [ADR 0073](0073-analyst-recommendations-tracking.md) (the two existing live BiznesRadar adapters this one follows), and [ADR 0082](0082-market-data-source-selection.md) (which rejected BiznesRadar's *historical-quote* paths for automation).

## Context

The fundamentals pipeline ends at the aggregator witness after ADR 0084. The witness's job is **corroboration, not sourcing**: where an aggregator covers a company, cross-check its structured tables routinely so that agreement between the primary filing and the aggregator yields high confidence, and disagreement raises a diff the user sees. The parser is built and the pipeline already accepts a `witness` input; today `run_structured_extraction` passes `witness: None` (`jobs/structured_extraction.rs:412`) because no policy-approved live fetch exists.

Source policy ([source-strategy.md](../source-strategy.md), CLAUDE.md) requires a source-specific ADR before any fragile/scraped path is bound live.

## Evidence gathered (2026-07-20, live probes)

1. **`robots.txt` (verbatim, fetched 2026-07-20)**:
   ```
   User-agent: *
   Allow: /
   Disallow: /profile-transactions/*,*
   Disallow: /profile-history/*,*
   Disallow: /transakcje/*,*
   Disallow: /notowania-historyczne/*,*
   ```
   The fundamentals report paths (`/raporty-finansowe-*`) are **not** disallowed — the site allows everything except four explicitly listed paths. The two paths ADR 0082 rejected for automation (`/notowania-historyczne/`, `/profile-history/`) are precisely the disallowed ones, so that rejection and this approval are consistent, not contradictory.
2. **Availability probe**: `GET /raporty-finansowe-rachunek-zyskow-i-strat/CDPROJEKT` → `301` → `/raporty-finansowe-rachunek-zyskow-i-strat/CD-PROJEKT` → **HTTP 200, ~222 KB**, containing the Polish statement labels the tier-2 dictionary already knows (`Przychody ze sprzedaży`, `Zysk operacyjny`, `Zysk netto`, `EBITDA`). No anti-bot gate, no proof-of-work challenge (contrast: Stooq, ADR 0082).
3. **Precedent**: two BiznesRadar adapters are already live and healthy under the same posture — `biznesradar-akcjonariat` (`v0.56.0`, ADR 0072) and `biznesradar-rekomendacje` (`v0.58.0`, ADR 0073): public page, one polite fetch per tracked GPW company, daily cadence, `optional` visibility. This ADR adds a third path on the same host under the same rules, not a new scraping relationship.

## Decision

1. **Adopt the BiznesRadar financial-report pages as the fundamentals witness**, as adapter `biznesradar-fundamenty` (`fundamentals` type, `optional` visibility, **`witness` role — never `primary`**), reusing the existing BiznesRadar fetch/politeness infrastructure and slug mapping rather than adding a second HTTP path.
2. **Witness semantics — compare-first, with a last-resort fallback** (*amended 2026-07-21, owner decision — see the amendment note below*). The primary filing is always the source of truth. The witness **never overwrites** a fact any deterministic tier produced, and never silently "correct" one: agreement raises confidence; disagreement produces a **user-visible diff notification** and leaves the primary value in place, flagged. **But where no deterministic tier produced a value at all**, the aggregator **may source the fact** rather than leaving the period empty — see the amendment for the exact conditions and the trust marking.
3. **Politeness and cadence**: at most one page fetch per tracked company per day, sequential per host (the existing per-source serialization), respecting the shared BiznesRadar rate limit; responses cached so a re-run within the cadence window does not refetch. No bulk crawling, no untracked-company sweeps, no historical-path access.
4. **Slug mapping is deterministic and cached.** The probe shows ticker→slug is not identity (`CDPROJEKT` → `CD-PROJEKT`); reuse whatever mapping the two live BiznesRadar adapters already maintain rather than re-deriving it, and treat an unresolvable slug as "no witness available" (a normal, non-error state), never as a failure.
5. **The witness is optional and degradable.** A company with no BiznesRadar coverage is a normal case: the pipeline reports "no witness" rather than an error, and validation still stands on the identities/comparative/structure checks. Losing the witness must never block emission of an otherwise-validated fact.
6. **Every refresh records its outcome** via `record_source_outcome` so Sources shows real freshness (Definition of Done §C guardrail).

## Consequences

- ADR 0061's decision 4 becomes fully live: routine PDF↔aggregator cross-checking, with the "agreement ⇒ ~100% confidence" property that the deterministic-only pipeline now leans on more heavily than when an AI tier existed.
- One additional daily page fetch per tracked company on a host the app already contacts twice daily; no new credentials, no paid API, no cloud dependency.
- A new drift surface: markup changes break the parse. Mitigated the same way as the two live BiznesRadar adapters — a drift-guard test over a stored sample page plus honest source-outcome reporting, so a broken witness shows as "witness unavailable", never as a false agreement.
- **Legal/ToS posture**: read-only access to publicly served pages, robots-compliant, at a volume far below normal human browsing of the same pages, with attribution preserved on any surfaced value. This ADR does not authorize redistribution of BiznesRadar content — witness values are used for corroboration and shown as attributed comparison, consistent with the existing two adapters.

## Alternatives considered

- **Bankier "wyniki finansowe" as the witness instead.** Kept as the documented fallback (ADR 0061 names it): its fetch infrastructure already exists, but BiznesRadar leads on GPW + NewConnect fundamentals coverage, which is the whole point of a breadth witness. Revisit if BiznesRadar coverage or markup proves unstable.
- **StockWatch.** Rejected for now: no policy review, and a third host adds drift surface without adding coverage the first two lack.
- **No witness at all** (validation on identities + comparative checks only). Rejected: after ADR 0084 removed the AI residual, the witness is the only *independent* source of corroboration left; dropping it would make "never silently wrong" rest entirely on internal consistency, which cannot catch a self-consistent misparse.

## Amendment — aggregator as last-resort source (2026-07-21, owner decision)

**Reversal of the original decision 2's "may never create a fact on its own."** Owner's rule, stated
plainly: *"I would rather have untrusted numbers than none at all — if we read nothing from the PDF and
BiznesRadar has them, we take BiznesRadar's."* This also restores the "**+ fallback**" half of
[ADR 0061](0061-deterministic-fundamentals-data-gathering.md) decision 4, which the original text of this
ADR had unintentionally contradicted (conflict surfaced during implementation, 2026-07-21).

Conditions — all must hold before the aggregator sources a fact:

1. **Nothing else produced a value for that slot.** The fallback fires only when every deterministic tier
   (ESEF → structured xHTML → EspiCoverNote → PDF) produced no value for the metric+period. It never
   competes with, overrides, or "corrects" a value another tier produced — that half of decision 2 stands
   unchanged, and the tier-precedence rules are untouched.
2. **The fact is marked as lower-trust and stays reversible.** It carries `source_tier = html_aggregator`,
   a conservative confirmation state (never auto-confirmed), and a citation naming the aggregator page —
   so the investor can always see that this number came from a third-party aggregator rather than the
   issuer's own filing, and can reject it.
3. **The extraction outcome records it as a fallback, not a corroboration.** Agreement, disagreement, and
   fallback-sourcing are three distinct recorded states; a fallback must never be reported as though the
   primary filing had been read successfully.
4. **Attribution stays visible** wherever the value is surfaced, per this ADR's ToS posture.

Rationale: after [ADR 0084](0084-retire-in-app-ai-layer.md) removed the AI residual tier, refusing the
aggregator as a source would leave an unreadable filing with **no numbers at all** — which conflicts with
the epic's "100% automatic" requirement more than a clearly-labelled third-party figure does. The honesty
guarantee is preserved not by withholding the number, but by **naming its provenance and its lower trust**.
Coverage impact will be measured by the `v0.59.0` real-data recall/precision harness, which must report
aggregator-sourced facts separately from filing-sourced ones.

**Clarification — a fallback is never a deterministic emit (2026-07-21, C1/C2 fix).** Three points, so the
distinction the amendment draws in principle is enforced in the code, not just described:

1. **A fallback never counts as an issuer emit.** The extraction result's `emitted` flag — the signal
   autopilot reads to decide "the filing was parsed" — is set from the **issuer** tiers only. Aggregator
   fallback facts still persist (the owner rule above), but they never set `emitted`. So a fallback-only
   run takes autopilot's *not-emitted* path: its `kpi_delta_json` is `extractionAvailable:false` with
   `reason:"witness_fallback"`, **never** `structured:true`/`extractionAvailable:true`.
2. **A fallback-only period stays re-armable.** Because the run records `witness_fallback` (not a false
   success), `witness_fallback` joins the re-arm class in `terminal_run_should_rearm`: once a later parser
   fix makes the document readable, the period is re-extracted with **real issuer data** instead of being
   frozen forever on third-party numbers. A repeat run whose issuer tiers fail again while the aggregator
   slots are already filled re-records `witness_fallback` (never a phantom `emitted`) and creates no
   duplicate facts.
3. **A mixed period keeps the issuer success reason.** When an issuer tier emits *N* facts and the
   aggregator only tops up an empty slot, the **set-level** outcome is the normal `emitted` reason, not
   `witness_fallback` — branding the correctly-parsed issuer facts as aggregator-sourced would misrepresent
   them. Each topped-up fact still carries its own per-fact `html_aggregator`/`accepted_unreviewed`
   provenance, so the two sources stay distinguishable at the fact level.

## Amendment — an empty/zero aggregator cell is never evidence (2026-07-21, BFT H1 2025 incident)

**Incident.** For BFT H1 2025 the aggregator column rendered `net_profit` as a placeholder `0` while the
filed report (and our primary parse) said 242 454 000. The cross-check treated that `0` as a witness value
(`expected = 0`, `actual = 242 454 000`), residual ≫ tolerance, and produced a spurious `witnessDisagreement`
that blocked emission — the correct nine-figure value was never written. The same class blocked the ABE H1
cover-note refills through the ingest seam.

**Rule.** An aggregator cell of **exactly `0`** against a **non-zero** primary parse is a scrape/cache
artifact — an empty, unreported, or wrong-period cell BiznesRadar renders as `0`, never a filed nine-figure
value. **An empty/zero aggregator cell is never evidence.** The compare drops such `Fail`s (logged
`stage=zero_witness_skip`); emission is never blocked by one. Preserved unchanged: genuine disagreements
(both sides non-zero, beyond tolerance) still flag and abstain; a true `0` vs `0` is exact agreement
(residual 0 → `Pass`) and remains a corroboration.

**Where.** A single compare-side guard, `witness_cross_check` (in `source_adapters/biznesradar_fundamentals.rs`),
wraps `cross_check_prior` and applies the drop; both seams call it (the `Pdf` arm in
`fundamentals/extraction/pipeline.rs`, the `EspiCoverNote` arm in `storage/espi_cover_note_facts.rs`). The
guard is keyed on the **aggregator (`expected`) side**, so it is stated generally, not witness-specifically:
it protects any path where a BiznesRadar cell is the value being compared — the witness role today **and** the
core-KPI primary-write path once BiznesRadar is promoted to the primary source (ADR 0086).

## Resolved at sign-off (2026-07-21)

- **Scope of the witness run — decided:** the witness runs where the primary tier is **`Pdf` or `EspiCoverNote`**, and is **skipped where the primary tier is `Esef`**. ESEF facts are tagged by the issuer against IFRS concepts and are already near-certain; spending the one-fetch-per-company-per-day budget re-confirming them buys no confidence. Corroboration is spent where a parse could plausibly be wrong. A future ADR may widen this if measured ESEF error is ever non-zero.

## Amendment — pipeline seam retired, witness promoted to primary (2026-07-22, [ADR 0086](0086-aggregator-primary-fundamentals.md))

ADR 0086 promotes BiznesRadar from witness to the PRIMARY core-KPI source with its own daily
pull and reversed witnessing. Consequences for this ADR's seams:

- The **`Pdf` seam is deleted with the PDF fact arm** (ADR 0086 dec. 1), and with it the whole
  in-pipeline witness path: `resolve_witness`, the pipeline's aggregator-as-last-resort arm, the
  gap-fill fallback (`persist_witness_fallback`) and the `witness_fallback` outcome/flag. Stored
  `witness_fallback` rows/deltas stay readable and re-armable as legacy.
- The **`EspiCoverNote` ingest seam SURVIVES unchanged** (cache-only comparison, `witness_pending`
  deferral, zero-guard) — it is now the only seam this ADR governs.
- The zero-guard (`witness_cross_check`) additionally protects the BR-primary pull's reversed
  witnessing (ADR 0086 dec. 4).

## Implementation status (`v0.59.0`)

Both scoped seams were wired; after the 2026-07-22 amendment only the ingest seam remains:

- **`Pdf` seam** — retired (see amendment above).
- **`EspiCoverNote` seam** — at **ingest time** in `storage/espi_cover_note_facts.rs`, where cover-note facts are produced (outside `run_pipeline`). Because the cover-note hook is a post-commit, best-effort step inside feed ingestion, this seam is **cache-only: it never fetches synchronously** (decision 3 politeness — a synchronous witness fetch in the feed path would couple feed latency and reliability to a third-party host). It reuses the *same* cadence cache (`fundamentals_witness::get_fresh_witness_page`) and the *same* comparison primitive (`validation::cross_check_prior` over `fact_set_for_period` sets) the `Pdf` seam uses. A cache hit resolves the comparison immediately (agreement ⇒ `accepted_via_witness` corroboration outcome; disagreement ⇒ `witness_disagreement` outcome with the diff, cover-note value unchanged); a **miss defers** the comparison — a `witness_pending` diagnostic, no fetch, never a false agreement. The witness page is normally warmed by the daily fundamentals sweep for the same company, so cache hits are the common case at ingest.

## Open questions

- None at sign-off.
