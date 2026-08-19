# ADR 0100: Two-layer tagged-fact capture and the IFRS-anchored metric vocabulary

Status: Accepted (2026-08-19, housekeeping at epic #399 closure — epic #398 was delivered and closed 2026-08-18 with the status flip missed; proposed 2026-08-17). Amends ADR 0086 (aggregator primacy for periods a tagged filing covers). Implementation: epic #398.

Deciders: maintainer. Area: fundamentals, data model, extraction.

## Context

EU issuers file the annual report twice: a PDF for humans and an **ESEF/iXBRL package for machines**, in which every disclosed number carries an IFRS taxonomy concept label. Brawler already routes those packages to the `esef` tier, but `concept_to_metric_key` (`fundamentals/extraction/esef.rs`) maps **22 concepts by design** — "kept deliberately small and explicit; company extension concepts and note-level disclosures are intentionally absent".

Measured on the maintainer's corpus and database (2026-08-17; corpus figures are not reproducible from this tree):

- 45 packages carry an iXBRL report instance; **19 202 tagged facts, mean 426 per package**.
- Per filing: 90–170 dimensionless facts, 89–126 distinct `ifrs-full` concepts. Across 8 issuers: **347 distinct concepts — 76 used by ≥4 issuers, 168 by exactly one**.
- Realised yield: **878 `esef` facts across 45 companies ≈ 16 per annual report**. Fact *breadth* is ~20–30 metrics per tier (`html_aggregator` 20, `esef` 22, `espi_cover_note` 31, `agent` 21).

So the naming convention this repo needed already exists, is maintained by the IFRS Foundation, and is already on disk — and roughly 85 % of each tagged filing is discarded on read.

Three defects surfaced while establishing the above, and are fixed by this epic rather than tracked:

1. **Every one of the 11 341 stored facts carries `measure_window='flow'`**, including 4 902 on balance-sheet keys (4 484 of them from the aggregator). Every writer passes `measure_window: None` and the slot-write boundary defaults to `flow` — even though the ESEF parser already knows instant-vs-duration and discards it.
2. **Catalog fragmentation predates any agent**: `inventory` (0 facts) coexists with `inventories` (771); 22 `wdf_*` keys, several with Polish-language names, were minted canonical by this repo's own cover-note mapper.
3. **The package reader loses whole filings**: it assumes a `reports/` layout (one issuer files at the archive root, another under Polish-named folders) and keeps only the largest XHTML although packages carry several instances.

## Decisions

### 1. Capture is separated from naming: a raw tagged-fact layer under the curated catalog

Requiring a curated catalog entry before a number may be stored means only pre-curated concepts can ever be captured — and **168 of 347 observed concepts appear at exactly one issuer**, so that rule silently reduces "capture everything" to "capture what we already know".

**Layer 1 — `report_tagged_facts`.** Every tagged fact in an instance is written 1:1: expanded namespace URI and local name, context reference, period start/end and `instant|duration`, unit, raw and normalized value, `scale`/`sign`/`decimals`, dimension members, presentation roles, package entry path, and a fact identity. No naming decision, no interpretation. A concept with no crosswalk entry is a **row**, not a diagnostic that evaporates.

**Layer 2 — projection.** A pure function over Layer 1 produces canonical facts: primary-statement role selection → crosswalk resolution → full-slot duplicate resolution → the existing deterministic validation gate → `record_structured_fact`. **Raw capture is unconditional; projection is not.**

Rejected: minting catalog definitions at extraction time (breaks the app-owned `canonical` scope and the bare-id invariant migrations 0129/0130 rely on); curating every concept before shipping (defers capture behind human throughput and still fails for the next filer's extensions).

### 2. The curated crosswalk is the single naming authority, seeded by migration

`ifrs_crosswalk` maps a taxonomy concept to `metric_key` + `label` + `value_kind` + `statement_group` + `period_nature`. It is curated in-repo and seeded by migration; nothing is minted at runtime. It grows through a **concept-harvest command** that reads Layer 1, ranks unmapped concepts by issuer count, and emits a seed-migration draft.

**Reuse before mint.** A concept maps onto an existing key whenever one already holds facts, however unattractive that key's name: `EquityAttributableToOwnersOfParent` → the existing `wdf_equity_parent` (932 facts, 51 companies), `ProfitLossAttributableToOwnersOfParent` → `wdf_net_profit_parent` (883, 51). Minting twins would create precisely the fragmentation this epic exists to remove. Renaming those keys is a separate, evidence-backed change, never a side effect of the crosswalk landing.

**Concept plurality on one key is a deliberate, bounded exception — `revenue` only.** `Revenue` and `RevenueFromContractsWithCustomers` both resolve to `revenue`: GPW filers tag EITHER concept for the same top line, and splitting the key would fragment every cross-company revenue series — the exact harm this ADR exists to prevent. The bound: when one filing tags BOTH with diverging values, the slot is a **typed conflict and neither is written** — never a silent pick, because IFRS 15 contract revenue is not definitionally the whole of `Revenue` (sol round 2 objection, accepted as a real risk). A dedicated split (own key + read-time fallback) is the designed escape hatch, taken only on real-data evidence of a filing that tags both divergently — none observed in the corpus to date.

**Issuer extensions are issuer-qualified and company-scoped.** The qualifier is a curated stable issuer token (or a namespace-URI digest) — never the QName prefix, which is a document-local alias. Cross-company comparison matches on `metric_key` (`comparison_facts`), so distinct issuer-qualified keys can never be falsely aligned; by the same rule two issuers' extensions are *not* comparable until a crosswalk entry proves them equivalent and maps both to a shared canonical key.

### 3. The primary-statement selector is the presentation linkbase, not dimensionlessness

"Dimensionless" means only "no XBRL dimension members" — it includes note totals and maturity disclosures. The package's presentation linkbase (`*_pre.xml`) groups concepts by statement, in two role families across the corpus: standard IFRS role numbers (`ias_1_role-210000` financial position, `320000`/`410000` income, `ias_7_role-520000` cash flows, `ias_1_role-610000` changes in equity) and vendor-generated Polish role names shared by several filers (`SprawozdanieZSytuacjiFinansowej`, `WynikFinansowy`, `SprawozdanieZPrzeplywowPienieznych`). An unrecognised role classifies as `other` — explicitly, never by guess.

Roles are stored as a fact↔role **relation**: a concept participates in several roles, so a scalar column cannot hold its role membership.

**Amended (2026-08-18, sol review finding 2): the selector is CONCEPT-level, and can be nothing stronger.** A presentation linkbase relates concepts to roles, so the parser attaches one identical role set to every occurrence of a concept — the income-statement occurrence of `ProfitLoss` and the cash-flow reconciliation's opening line are indistinguishable by role. The selector answers "does this concept belong to a primary statement", never "which statement is this occurrence printed in". The original text implied a per-occurrence ranking; that is unimplementable from a presentation linkbase, and the revision that claimed to implement it was proven only by a test fabricating per-occurrence role vectors the parser cannot produce.

### 4. Projection resolves duplicates per full slot, before any existing collapse

A concept is legitimately tagged **2–3 times at an identical (concept, context)** in one filing — measured: XTB `ProfitLoss` 3×, `ProfitLossBeforeTax` 2×, `OtherComprehensiveIncome` 2×; 13 such concepts, the same pattern at LPP. Net profit appears in the income statement, in comprehensive income, and in the indirect-method cash-flow reconciliation, where the sign convention may be inverted.

Projection resolves per **full write slot**, grouped by (statement basis, metric key) — the two dimensions this tier actually varies on (amended per sol finding 3; statement precedence between occurrences is impossible, see decision 3):

- **Statement basis is derived per instance** from the package entry path (TXT ships standalone and consolidated filings in one package under Polish-named folders) — a standalone filing is never silently stamped consolidated, and the two bases never conflict with each other.
- **Duration windows sharing an end date resolve to the longest span**: a Q3 filing tags the 3-month quarter and the cumulative 9-month figure with the same end date; GPW interim reporting is cumulative, so the longest window is the reported figure (parity with the retired parser's `dedup_longest_duration`) and the shorter ones are counted, never conflated into a conflict.
- Only then does value equality decide: identical value and currency is a deterministic **repeat** retaining links to every raw row; differing normalized content is a **typed conflict** — the honest outcome for a filing that contradicts itself, since no concept-level evidence can choose a side.

This runs *before* the metric-key-only collapses in the existing pipeline, which would otherwise erase the second candidate and let XML order decide which occurrence was stored.

### 5. Projected facts pass the existing validation gate

Widening capture must not widen trust. `record_structured_fact` only resolves a definition and applies slot precedence; it does not run balance identities, comparative cross-checks, completeness or history plausibility. Projected candidates pass the current `validate_tier` acceptance machinery before any write, so the `esef` tier's standing in the trust ladder (ADR 0098 dec. 7) is unchanged.

### 6. The catalog gains `period_nature`, distinct from trailing-window eligibility

`is_flow_key` currently conflates two questions: whether a figure is an instant or a duration, and whether it may be summed into a trailing twelve months. A ratio is duration-reported yet TTM-ineligible.

The axis is derivable mechanically, not by judgement: across the 347 concepts observed in the 8 sampled filings, **every one reports a consistent instant-or-duration nature across all issuers** — zero conflicts. So the crosswalk's `period_nature` is generated from the corpus by the harvest command rather than hand-assigned, and a future conflict is a signal worth surfacing rather than a case to adjudicate. `kpi_definitions.period_nature` (`instant|duration`) becomes the storage truth, replacing `STOCK_METRIC_KEYS` (whose own comment anticipated this), and two pure functions replace the overloaded predicate: `measure_window_for(period_nature, profile)` for manifest window validation and ingest-context candidates, `is_ttm_eligible(period_nature, value_kind)` for trailing windows. The no-definition fallback keeps today's behaviour.

`measure_window` at the slot-write boundary derives from `period_nature` instead of defaulting to `flow`. A tagged context that disagrees with the catalog is a **typed validation error**, never a silent override — otherwise one metric acquires two window classes. Existing balance-sheet facts are repaired by a forward migration; the repair and the writer change ship together, or the next aggregator pull re-creates what the migration moved.

### 7. Comparative periods are captured, not yet projected

A later filing restating an earlier period cannot be expressed by the existing supersession machinery: `supersedes_id` links a `final` fact only to a `preliminary`/`estimated` sibling, and two `final` facts collide on the slot uniqueness index, so a restatement reaches equal-tier divergence before a second row can exist.

Layer 1 stores **every** period from every filing, so nothing is lost. Layer 2 continues to project only the filing's own declared period — which is today's behaviour, so this is not a regression. Whether a restatement supersedes, coexists under a slot dimension, or is surfaced as a disagreement is a decision to be made **on Layer 1 evidence**, when both values are in the database, and recorded as an amendment here.

### 8. Freshness is `(source_content_hash, extractor_version)` with atomic replace

`extractor_version` alone is insufficient: recapture replaces a document's bytes without changing its id — the reason migration `0140` added `content_hash` to the derived-period cache. Layer 1 follows the report-sections pattern (migration `0053`): a source hash plus an extractor version, an extraction record carrying state and counts, and rebuild by delete-and-replace **in one transaction**, so a failed rebuild never leaves a half-generation. Layer 1 joins the document-bytes retention protection: a raw-only package whose bytes were pruned could never be rebuilt.

### 9. Nothing is dropped silently

The parser today materialises only `ix:nonFraction`, and only after concept mapping, numeric parsing, context resolution and the dimensionless filter — each failed step discards the occurrence. Layer 1 defines treatment of `ix:nonFraction`, `ix:fraction`, nil facts, continuations and non-numeric tags, and stores **every supported occurrence even when normalization fails**, with a nullable normalized value and a typed parse status. The epic's ship gate asserts `encountered = stored`, not "successfully normalized", alongside zero unexplained package instances and zero uncurated in-scope primary-statement concepts.

**Amended (2026-08-18, sol review finding 4)** — two ways "nothing dropped" could hold vacuously are closed: a numeric fact carrying `continuedAt` never parses its local fragment (`parse_status = "unsupported_continuation"`, no value — the fragment can parse cleanly while being only part of the disclosed number), and a mid-document reader error or a skipped package entry marks the whole extraction `truncated`, never `extracted` — "encountered = stored" is only meaningful over what was actually walked, and the record now says when that was not everything.

### 10. The owner may promote a captured position into Fundamentals; a machine still may not

Decision 2 bans runtime minting so that no automated path can invent a name. That ban is about **machines guessing**, and it must not be read as banning the **owner deciding**: a captured concept the catalog has no name for is exactly the long tail this epic exists to preserve, and the investor is the only party who can say "this line matters to me".

So the Layer-1 view carries one action — promote a captured position into that company's Fundamentals — under three constraints that keep decision 2 intact:

- **Company-scoped only.** A promoted position becomes a `scope='company'` definition for that issuer, never a canonical key. It therefore cannot silently align with another issuer's similarly-named line in cross-company comparison, which matches on `metric_key`.
- **The name comes from the report, not from us.** For an issuer's own position the label is the one the issuer published in the package's label linkbase; for a standard taxonomy concept with no curated name yet, the technical concept name is shown as-is and stated to be untranslated. Nothing is invented at any point.
- **Promotion is a curation signal, not a curation.** A promoted concept appears in the harvest output so a later seed migration can lift it into the shared vocabulary deliberately — the same evidence-driven path decision 2 already prescribes.

Rejected: keeping the tail read-only (the view degrades to a receipt nobody opens twice); promoting straight to canonical (re-creates exactly the cross-issuer false alignment decision 2 prevents).

### 11. Re-extraction and its read models are exposed over MCP; promotion is not

Widening the MCP surface is a deliberate decision in this repo, never a side effect (ADR 0099 dec. 3). Three commands are exposed (owner decision, 2026-08-18):

- `get_report_tagged_fact_coverage` (read) — the capture funnel for a company;
- `get_pipeline_reextraction_progress` (read) — batch progress;
- `run_pipeline_reextraction` (act, therefore gated by `mcpWritesEnabled` like every act).

The motivation is concrete: re-reading already-stored reports is exactly the kind of bulk operation an agent should be able to run and then measure, and requiring the owner to click it by hand made the epic's own real-data verification depend on manual steps.

`promote_uncrosswalked_concept` stays **excluded**. Decision 10 reserved promotion as the owner's own authority precisely so that no automated path names a metric; exposing it over MCP would route around that decision rather than honour it. The asymmetry is the point: an agent may re-read and measure, only the owner may name.

Consequence: the Full-scope frozen tool count rises by three, so its snapshot and count assertions move with this change — deliberately, as evidence of the widening. `run_pipeline_reextraction` is **idempotent while a batch is in flight** (sol review finding 12): while the company's latest batch is `queued`/`running`, every further call returns that batch — an agent looping the act cannot mint unbounded durable batches or queue jobs.

### 12. A dead catalog key is redirected by a curated alias, never left to rot

Decision 2 stops the ESEF path from minting duplicates. It does not repair the duplicates that already exist: `inventory` (migration `0048`) and `inventories` (`0084`) are both canonical rows for the same figure, and only the second has ever held a fact (771 across 44 companies). The cost was not theoretical — the seeded `quick_ratio` formula read `inventory`, so the metric evaluated to unavailable for **every company since it was seeded**, indistinguishable from an issuer that never reported it.

A curated alias table (`fundamentals::kpi_aliases`) names each dead key and the live key it means. `resolve_kpi_definition` consults it and redirects **only while the source key holds zero facts for the company being written** — per company, never database-global (one company's legacy series must not flip routing for every other company) — the one-sidedness rule is enforced at runtime, per write, not merely asserted by the curation. On a database where the source key already carries a series (an import, an older schema, a manual entry), the redirect never fires and the write lands on the source key exactly as before.

The table is **one-sided and evidence-proven**: an alias source must hold no facts. A pair where both sides carry facts is a *merge*, which needs its own migration and its own evidence — never an entry here. That asymmetry — checked live, not assumed — is what separates an alias from a repaint (ADR 0077 dec. 8): nothing is renamed and no two real series are joined; a key that was always empty simply stops being a place a fact can land, and a key that turns out not to be empty is left alone.

The seed migrations' `INSERT OR IGNORE` has a known, documented residual risk (sol round 2, finding 10): the 0125-style self-healing preamble re-keys the one malformed shape post-0125 code can no longer produce, but a hostile import carrying a canonical row under a foreign id, or a bare id over a different metric key, can still suppress a seed silently — SQL cannot assert a postcondition. The compensating gate is `every_emittable_metric_key_has_a_canonical_definition`, which resolves every crosswalk entry against the migrated catalog on every test run; a suppressed seed reddens there on a clean database, and a hostile-import database is by definition outside the seed's contract.

Two gates keep it closed. `no_derived_formula_references_an_alias_source` refuses a formula that reads a dead key — the check that would have caught `quick_ratio`, which the existing computability gate could not, because it seeds a fact for every definition including the dead ones. `every_alias_names_two_seeded_catalog_keys` refuses an alias naming a key the catalog does not have. Repairing an already-broken formula is a forward migration (`0147`), matched on the exact seeded text so an owner-edited row is never rewritten.

## Consequences

- **ADR 0086 is amended for periods a tagged filing covers**: the issuer package, not the aggregator, is the breadth source there. The precedence ladder is unchanged (`esef` already outranks `html_aggregator`); `docs/product-spec.md`'s "issuer filings corroborate" wording changes with it.
- `every_emittable_metric_key_has_a_canonical_definition` keeps its label-dictionary and cover-note `classify` scans unchanged; only its ESEF portion becomes iteration over the crosswalk, and it becomes scope-aware — standard concepts resolve to seeded global definitions, issuer extensions to their exact seeded company definition.
- `STOCK_METRIC_KEYS` and `every_stock_metric_key_names_a_seeded_definition` retire into the `period_nature` column and a backfill gate.
- A company's Fundamentals matrix grows from ~16 rows to ~150, so the information architecture's all-groups-expanded default no longer holds and changes with a mockup.
- Layer 1 must be reachable, not merely stored: a report-level read model reports raw, dimensional, mapped, projected, refused and uncurated counts. Dimensional facts are stored but not projected — `financial_facts` has no dimension axis, and inventing one is out of scope.
- Existing tagged filings do not widen by themselves: a terminal-successful extraction is never re-armed, and "emitted" excludes an all-reobservation rerun. An explicit version-aware re-extraction, reachable from the Coverage panel, is part of the epic.
- The agent path for untagged documents (#399) maps into the vocabulary this ADR establishes; it does not duplicate it.
