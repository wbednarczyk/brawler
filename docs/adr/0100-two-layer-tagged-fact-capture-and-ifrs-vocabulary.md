# ADR 0100: Two-layer tagged-fact capture and the IFRS-anchored metric vocabulary

Status: Proposed (2026-08-17, owner decision at epic #398 planning; 3 adversarial review rounds). Amends ADR 0086 (aggregator primacy for periods a tagged filing covers). Implementation: epic #398.

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

**Issuer extensions are issuer-qualified and company-scoped.** The qualifier is a curated stable issuer token (or a namespace-URI digest) — never the QName prefix, which is a document-local alias. Cross-company comparison matches on `metric_key` (`comparison_facts`), so distinct issuer-qualified keys can never be falsely aligned; by the same rule two issuers' extensions are *not* comparable until a crosswalk entry proves them equivalent and maps both to a shared canonical key.

### 3. The primary-statement selector is the presentation linkbase, not dimensionlessness

"Dimensionless" means only "no XBRL dimension members" — it includes note totals and maturity disclosures. The package's presentation linkbase (`*_pre.xml`) groups concepts by statement, in two role families across the corpus: standard IFRS role numbers (`ias_1_role-210000` financial position, `320000`/`410000` income, `ias_7_role-520000` cash flows, `ias_1_role-610000` changes in equity) and vendor-generated Polish role names shared by several filers (`SprawozdanieZSytuacjiFinansowej`, `WynikFinansowy`, `SprawozdanieZPrzeplywowPienieznych`). An unrecognised role classifies as `other` — explicitly, never by guess.

Roles are stored as a fact↔role **relation**: a concept participates in several roles, so a scalar column cannot distinguish the income-statement occurrence from the cash-flow-reconciliation one.

### 4. Projection resolves duplicates per full slot, before any existing collapse

A concept is legitimately tagged **2–3 times at an identical (concept, context)** in one filing — measured: XTB `ProfitLoss` 3×, `ProfitLossBeforeTax` 2×, `OtherComprehensiveIncome` 2×; 13 such concepts, the same pattern at LPP. Net profit appears in the income statement, in comprehensive income, and in the indirect-method cash-flow reconciliation, where the sign convention may be inverted.

Projection therefore applies statement precedence (income statement over comprehensive income over the cash-flow reconciliation for a profit-and-loss concept), then resolves per **full write slot**: identical value, currency, period, basis, window and quality is a deterministic **repeat** retaining links to every raw row; differing normalized content is a **typed conflict**. This runs *before* the metric-key-only collapses in the existing pipeline, which would otherwise erase the second candidate and let XML order decide which occurrence was stored.

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

Consequence: the Full-scope frozen tool count rises by three, so its snapshot and count assertions move with this change — deliberately, as evidence of the widening.

## Consequences

- **ADR 0086 is amended for periods a tagged filing covers**: the issuer package, not the aggregator, is the breadth source there. The precedence ladder is unchanged (`esef` already outranks `html_aggregator`); `docs/product-spec.md`'s "issuer filings corroborate" wording changes with it.
- `every_emittable_metric_key_has_a_canonical_definition` keeps its label-dictionary and cover-note `classify` scans unchanged; only its ESEF portion becomes iteration over the crosswalk, and it becomes scope-aware — standard concepts resolve to seeded global definitions, issuer extensions to their exact seeded company definition.
- `STOCK_METRIC_KEYS` and `every_stock_metric_key_names_a_seeded_definition` retire into the `period_nature` column and a backfill gate.
- A company's Fundamentals matrix grows from ~16 rows to ~150, so the information architecture's all-groups-expanded default no longer holds and changes with a mockup.
- Layer 1 must be reachable, not merely stored: a report-level read model reports raw, dimensional, mapped, projected, refused and uncurated counts. Dimensional facts are stored but not projected — `financial_facts` has no dimension axis, and inventing one is out of scope.
- Existing tagged filings do not widen by themselves: a terminal-successful extraction is never re-armed, and "emitted" excludes an all-reobservation rerun. An explicit version-aware re-extraction, reachable from the Coverage panel, is part of the epic.
- The agent path for untagged documents (#399) maps into the vocabulary this ADR establishes; it does not duplicate it.
