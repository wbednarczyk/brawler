---
name: brawler-mcp
description: Use when you (an AI agent) are connected to Brawler over its MCP server and need to read the user's investment research or write back into it — including acquiring KPIs from an issuer report ("process all pending KPI ingests", staging observations, resuming a pending run, a lease that expired, a validation that failed). Covers tool discovery, the read/act tiers, mandatory-source (provenance) rules, the KPI-ingest run workflow, internal-id conventions, the permanent denylist, and safe read→analyze→write sequences. Load it before your first tools/call against Brawler.
---

# Talking to Brawler over MCP

**Brawler is a local-first investor research workspace.** It holds one user's
tracked public companies and everything they know about them — official filings
(GPW ESPI/EBI first), allowed media, financial facts with trust-ladder
provenance, ownership and insider data, quality-framework scores, a decision
journal, notes, and tracked management claims. Every datum keeps its source.
You are the intelligence layer (BYOA); Brawler is the sourced ground truth.
Everything here is **decision support — never phrase output as buy/sell/hold**
(you cannot be made to; the app forbids it too).

## Discovery: read vs act

Call `tools/list` first. Every tool the server exposes is classified into one of
two tiers by the server itself (the registry, not its name — the tool catalog
below carries the authoritative split); the name is only a hint:

- **Read** — usually `get_*` / `list_*` / `search_*`. Always available. Use them
  freely to understand the workspace before doing anything else.
- **Act** — the writes (typically `create_*`, `update_*`, `set_*`, `mark_*`,
  `confirm_*`, `trigger_*`, `run_*`, …). These **write**. They are always listed
  even when disabled. When a name doesn't fit the usual prefix, trust the
  catalog's tier, not the verb.

**Act tools may be off.** On the **primary (Full-scope) token**, writes are
gated by a user setting (default OFF). If you call an act tool while writes are
disabled you get a typed `writes_disabled` error (`isError: true`) — the handler
never ran, nothing changed. **React gracefully:** do not retry, do not work
around it. Tell the user *"writing is turned off — enable Settings → MCP server →
Allow write tools if you want me to record this,"* then continue read-only. You
**cannot** enable it yourself: `update_settings` is not on the MCP surface, by
design.

**The acquisition scope is different.** If you authenticated with the
**acquisition token**, your surface is exactly the nine KPI-ingest workflow
tools (below), and their act tools are **not** gated by that toggle — the gate
ran once at authentication (`kpiAcquisitionEnabled`; a disabled scope rejects
the token outright). So "writing is off" never applies on the acquisition
scope; a `writes_disabled` reply only happens on the Full-scope token.

## Provenance: every write must cite a source

Brawler rejects unsourced research. If a write's provenance carrier is empty you
get a typed `provenance_required` error naming the missing field — again nothing
is written. **Gather the citation from read tools first, then write.** Per
family, with a concrete input shape:

| Family | Tools | Carrier + example |
| --- | --- | --- |
| Notes | `create_notebook_entry` | non-empty `origins[]`: `"origins":[{"sourceType":"report_document","sourceId":"<reportDocId>","label":"FY2024 annual report"}]` (or `"sourceType":"external_url","sourceUrl":"<filingUrl>"` when you have no stored document) — valid `sourceType`: `feed_item` \| `transcript_segment` \| `ai_analysis` \| `manual` \| `external_url` \| `report_document` (a document you registered via `capture_report_document`, epic #285 T9, closes #111). Also required: `tags` (may be `[]`) and `kind` ∈ `manual` \| `observation` \| `claim` \| `question` \| `follow_up` |
| Transcript notes | `create_note_from_transcript_selection` | `"transcriptSegmentIds":["<seg1>","<seg2>"]` (the selection is the origin) |
| Claims | `create_management_claim`, `update_management_claim` | `"sourceEvidenceId":"<reportOrTranscriptId>","sourceEvidenceType":"report_document"` — valid types: `report_document` \| `transcript_segment` \| `transcript` \| `feed_item` \| `manual` (a bare `"report"` is refused; caught live by the epic #285 T11 dogfooding ritual) |
| Facts | `create_financial_fact`, `update_financial_fact` | `"sourceDocumentRef":"<reportDocId>"` — a non-blank citation, REQUIRED. Never put a citation in `attribution`: that field is the fact's slot dimension (`total` \| `owners_of_parent` \| `nci`), not a citation carrier — prose there is refused, not accepted. Every write through these tools is stamped `source_tier="agent"` provenance honestly (never `manual`). |
| Batch facts | `record_financial_facts` | non-blank `"reportDocumentId":"<reportDocId>"` PLUS a non-blank `citation` on **every** entry of `facts[]` — one blank citation refuses the WHOLE batch before any write (see the ritual below). |
| Qualitative verdicts | `set_qualitative_verdicts` | every result carries `"citationsJson":"[{\"evidenceType\":\"notebook_entry\",\"evidenceId\":\"<noteId>\"}]"` — types: `feed_item` \| `notebook_entry` \| `claim` \| `transcript_segment` \| `company_event` \| `research_question` \| `company_signal` \| `decision_entry`; every ref must resolve to an EXISTING row (#343) — one dead/fabricated ref refuses the whole batch |

`update_notebook_entry` (origins are immutable, set at create) and
`set_claim_verdict` (evidence is the optional `verifyingFactId`) carry no extra
citation requirement.

## Ids: read first, then act

Read tools take a **qualified ticker** (`GPW:CDR`, or a bare ticker when
unambiguous). Act tools take the **internal ids** that read tools return in
their payloads (`Company.id`, a period id, a framework/criterion id, a note id,
…). So the rule is always **list first, then act against the ids you saw** —
never invent or guess an id. E.g. `list_companies` → `companyId`;
`list_financial_periods` → `periodId`; `list_quality_frameworks` →
`frameworkId` + `criterionId`; `list_report_documents_view` → the
report-document id you cite as a fact source.

## Do NOT attempt (permanent denylist)

These are UI-only and absent from the MCP surface — never try to reach them, and
tell the user they are theirs to do in the app:

- **Deletes** of anything.
- **Undo / rollback** of a run or write.
- **Settings, tokens, credentials, MCP self-management** (incl. enabling writes).
- Dev / diagnostic mutations, bulk import/backup.

## Recommended sequences

- **Read → analyze → write-with-citation.** Never write blind. Read the relevant
  surface (dossier, notes, reports, facts), form the judgment, then write citing
  exactly what you read. Read the result back to confirm.
- **Filing triage.** `list_company_signals` (or the M4 unclassified-filings
  read) → confirm the routine via `confirm_company_signal`, reject noise via
  `reject_company_signal`, surface the unusual to the user.
- **Qualitative assessment.** `list_quality_frameworks` +
  `get_company_dossier` + `list_notebook_entries` + `get_report_documents_view`
  → `set_qualitative_verdicts` with a `citationsJson` array per criterion →
  read back via `get_quality_assessment`.
- **Jobs.** Triggers (`trigger_autopilot_run`, `refresh_sources`,
  `run_structured_extraction`, `generate_morning_briefing`) are async: kick off,
  then poll the matching read tool (`list_autopilot_runs`,
  `get_latest_morning_briefing`) for the result.

## KPI acquisition: the run workflow

Ingesting an issuer's financial figures is a **run-based workflow** (ADR 0098 /
ADR 0099), and it is the **only** supported path — never write canonical facts
directly for a normal report (see *Direct fact writes are repair-only* below).
The nine workflow tools are the whole surface the acquisition credential sees;
the Full-scope token sees them too, as a superset. `"process all pending KPI
ingests"` is a sufficient instruction — the loop below is what it means.

<!-- BEGIN ACQUISITION WORKFLOW TOOLS -->
The nine workflow tools, in contract order:

1. `start_kpi_ingest` — claim or resume a run; an idempotent keepalive.
2. `list_pending_kpi_ingests` — the pending-run work queue (paginated).
3. `get_kpi_ingest_context` — catalog, plausibility evidence, profile doctrine.
4. `get_kpi_ingest_document` — the pinned source bytes, in chunks.
5. `stage_kpi_observations` — write the complete revision snapshot.
6. `validate_kpi_ingest` — the typed manifest / repair report.
7. `commit_kpi_ingest` — atomic, idempotent commit of the validated manifest.
8. `get_kpi_ingest_status` — a pure status read.
9. `cancel_kpi_ingest` — abandon a pre-commit run.
<!-- END ACQUISITION WORKFLOW TOOLS -->

### Process all pending KPI ingests

**Drain loop.** Start `list_pending_kpi_ingests` with no cursor; **process every
run on the page**, then follow `nextCursor` to the next page until a page comes
back empty. Then **restart from no cursor** and repeat; stop only when the first
page is empty. (Do NOT re-list before you have processed the runs you already
saw — the same pending rows would reappear and you would loop forever.)

For each pending run:

1. **Claim.** `start_kpi_ingest{ runId }` — claims the lease (or renews it if you
   already hold it). This never re-picks a profile; the profile is fixed at
   creation.
2. **Read the context.** `get_kpi_ingest_context{ runId }` → the `catalog`
   (definition ids, metric keys, labels, units — everything you need to map a
   Polish line to a metric), `plausibility` (per-slot medians and recent history,
   the validator's own evidence), and `profileRules`. When the period, scope, or
   data quality is still unknown, read the source itself:
   `get_kpi_ingest_document{ runId, offset, length }` (length ≤ 262144) chunk by
   chunk until `eof` — the figures and the reporting period live in the document,
   not the run row.
3. **Stage.** `stage_kpi_observations{ runId, observations, missingReasons,
   execution? }`. `observations` is the **complete revision snapshot** — storage
   replaces the whole set each revision, so a repair must resend every retained
   observation, not just the fixed one. `missingReasons` is required; `{}` means
   "nothing deliberately omitted". Cite every observation (below).
4. **Validate.** `validate_kpi_ingest{ runId, revision }` — pass the `revision`
   from the stage response (it pins the exact generation). Outcomes: `ready` →
   commit; `failed` → the returned `manifest` **is** the typed repair report
   (each flagged observation says what to fix); `superseded` → someone else moved
   the run, re-read its status and restart from step 1.
5. **Repair (bounded).** On `failed`, fix the flagged observations, re-stage the
   **complete** snapshot, and re-validate. Cap this at **two repair rounds**; if
   the same diagnostics repeat or you have no new evidence to add, stop and hand
   the manifest diagnostics to the user. This bound is **agent policy**, not a
   server limit — the server will keep accepting re-stages.
6. **Commit.** `commit_kpi_ingest{ runId, manifestHash, revision, execution? }` →
   the receipt: `acceptedCount`, and a per-observation `outcomes` array
   (`created` / `reobserved` / `upgraded` / `divergent`, each with its `factId`).
   Commit is idempotent — a replay returns the same receipt byte-for-byte.
7. **Confirm.** `get_kpi_ingest_status{ runId }` → the terminal status
   (`complete`, or `partial` when `missingReasons` covered a gap).

**Cancel is not on the happy path.** Cancelling a committed run returns
`conflict`. `cancel_kpi_ingest` is only for a run you deliberately abandon
**before** commit (an unusable document, a wrong-company run). A run whose two
repair rounds you exhausted stays `validation_failed` — recoverable later — never
cancel it to "clean up".

### Keeping the lease alive

The lease belongs to the **credential**, not your session; its TTL is **30
minutes**. `start_kpi_ingest{ runId }` is the explicit keepalive: **while your
lease is still live**, it renews it without incrementing `attemptCount`. Reads
(`context`, `status`, `document`) never touch the lease. So while reading a long
document or preparing a large re-stage, **renew at least every ~15 minutes**
(half the TTL), steering by the `lease.expiresAt` in the run status — a renewal
before expiry is free. If you let the lease lapse, the next `stage`/`validate`
returns `run_lease_expired`; you recover with `start(runId)`, but that is a fresh
claim on a lapsed lease and **does increment `attemptCount`** — so keep it alive
rather than reclaiming. If another holder claimed the run after your lease
expired you get `run_taken_over` — abandon it; that is convergence, not a fight
to reclaim.

### Choosing the profile (fresh runs only)

A **fresh** `start_kpi_ingest` requires a `profileId` from the frozen registry; a
**resume by `runId` never reselects** one. Pick by report type:

| Report type | `profileId` |
| --- | --- |
| Annual IFRS / ESEF report | `gpw_ifrs_annual` |
| Interim (final) report | `gpw_interim` |
| Preliminary (wstępne) release | `gpw_preliminary` |
| NewConnect UoR | `nc_uor` |
| Company-characteristic pack | `company_characteristic` |

If the document's metadata does not settle the type, **ask the user — do not
guess**. A `profileId` outside the registry is refused at start.

### Citing a staged observation

Every observation carries a `citation { page (≥1), table, row, quote (≤1024 B) }`
— the exact locator you read the figure from. Always give the `page`; add
`table` / `row` when the figure sits in a statement table; put the declared unit
in the `quote` (and the raw source unit in `rawUnitScale`) so the scaling stays
checkable. A citation with no locator at all fails validation as
`citation.missing` — that is the repair report telling you to cite the source.

### Reading the source document

<!-- EDITOR NOTE (epic #353 DoD): keep this skill FREE of single-document knowledge —
     no specific issuer, report code (e.g. "RB N/YYYY"), or exact figures from one
     filing. Illustrate the doctrine generically; a worked example must not name a
     real document or its numbers. -->

This doctrine is about the SOURCE format and holds regardless of tooling — it
governs the `normalizedValue`, `measureWindow`, and `citation` you stage.

**Unit scaling.** Polish statements declare the unit in the table header:
`(w tys. PLN)` → store the FULL base unit (multiply the printed figure by
1,000); `(w mln PLN)` → ×1,000,000. `normalizedValue` is always base-unit PLN
(or the stated currency), never the printed thousands/millions figure — and set
`unitScale` to the declared scale so the scaling is checkable.

**Parenthesized negatives.** `(419 996)` in a Polish statement is **−419996**
(thousands), not a label or a range. Thousand separators are non-breaking
spaces; the decimal separator is a comma (`1 027 240,50` → `1027240.50`).

**Cumulative-only recording (ADR 0093 dec. 3).** A GPW interim publication
prints discrete-quarter AND cumulative columns side by side — record ONLY the
cumulative column (H1/9M/FY) against the cumulative period; skip the
discrete-quarter column, it's derivable by span arithmetic. When a half-year
table shows a discrete Q2 figure beside the cumulative H1 figure, stage the H1
number against the `H1` period, never the Q2 number.

**Preliminary flag.** A wstępne/preliminary release → start the run with
`dataQuality: "preliminary"`. The later final audited report supersedes it
automatically at commit (ADR 0093 dec. 2) — no follow-up write needed.

**Ambiguous label trap.** The same label can appear twice at different windows
in one publication — e.g. an operating metric printed both as a quarterly figure
and as a half-year figure. Cite the exact table/row you read and pick the column
whose `measureWindow` matches the period you're staging — never the first match.

**Raster-chart caveat.** A value that exists only inside a chart image is not
in the document's text layer — read the page visually before citing it, and
cite the page number. Never estimate a chart value from its axis.

**Image-only scans.** Some filings are page scans with no text layer at all.
`get_kpi_ingest_document` serves the pinned bytes exactly as filed — there is
no server-side OCR, deliberately (ADR 0084: the LLM is the reader). When the
document yields no extractable text, read it with your own vision: render the
pages to images (e.g. `pdftoppm -png -r 150`), read the figures from the
renders, and cite page/table/row as usual. A scanned filing is NOT an unusable
document — the scan→vision path is the normal path for it; cancel the run
only when the pages are genuinely illegible.

### Direct fact writes are repair-only

Normal ingestion goes through the run workflow above. **You MUST NOT** write
canonical facts directly for a normal report: `record_financial_facts`,
`create_financial_fact`, and `update_financial_fact` are **Full-scope, low-level
repair tools** (ADR 0098) — for hand-fixing a fact the workflow cannot express,
never for reporting ingestion. They are absent from the acquisition scope
entirely (calling one there is an unknown tool). `capture_report_document` is
likewise **not** part of acquisition: capturing a document and resolving a URL to
a `documentId` belong to the UI, the Full scope, or the #354 planner — the
acquisition credential discovers and captures nothing; it processes runs that
already exist.

Everything else a report contains routes to a **Full-scope** write, not the
acquisition workflow:

| Report content | Tool | Scope |
| --- | --- | --- |
| P&L / balance-sheet / KPI figures | the run workflow (`stage_kpi_observations`, `metricKeyCandidate`) | acquisition |
| Management guidance with numbers ("costs +30% in 2026") | `create_management_claim` (`sourceEvidenceId` = the document id) | Full-scope |
| One-off narrative events (a KNF penalty, a donation) | `create_notebook_entry` (`report_document` origin) | Full-scope |
| Dividend declarations/payments | `create_company_event` | Full-scope |
| The upcoming final report date | `create_report_expectation` | Full-scope |
| Post-balance-date trading updates ("92.8k new clients in July") | `create_notebook_entry` — **NEVER** a fact (wrong-period discipline) | Full-scope |

## Tool catalog

Result shape: every tool's `structuredContent` is a JSON object — list results
arrive as `{ "items": [...] }`, scalar results as `{ "result": ... }`.

Machine-generated from the server's frozen `tools/list` snapshot (a drift gate
keeps it exact — do not hand-edit):

<!-- BEGIN GENERATED MCP CATALOG — do not edit; regenerate: node scripts/check/docs-drift.mjs --write-mcp-catalog -->

**Read tools** — always available once the server is on (50):

| Tool | What it does |
| --- | --- |
| `get_company_dossier` | One company's research dossier: identity, fundamentals coverage per fiscal period, confirmed financial facts, and quality-scorecard summaries. Sourced from the user's own research; decision support only. |
| `search_research` | Full-text search across the user's research workspace (notes, report documents, transcripts, claims, facts). Returns ranked matches with snippets. |
| `list_claims_due` | Management claims whose verification period has arrived (due), passed (overdue), or is approaching (upcoming), per company. |
| `get_quality_assessment` | Quality-framework state for one company: the latest stored scorecard evaluation per framework, plus stored qualitative verdicts. The in-app qualitative-assessment writer was retired (ADR 0084) — this tool reads only stored verdicts; agents record new verdicts with provenance via the `set_qualitative_verdicts` write-tool (until then a criterion reads as unassessed). Decision support only — never an investment recommendation. |
| `list_companies` | Every company tracked in the user's workspace (identity, exchange, qualified ticker). |
| `get_company_basic_info` | One company's identity card: name, exchange, ticker, ISIN, sector (with its provenance), and latest reported shares outstanding. |
| `list_watchlists` | The user's watchlists (id, name, ordering). |
| `list_watchlist_memberships` | Which companies belong to which watchlist (the membership edges). |
| `list_feed_items` | The unified newsfeed: official filings (ESPI/EBI) and allowed media items with their read/saved state. |
| `list_company_signals` | Typed filing classifications (ESPI/EBI signals) for one company, with their confirmation status. |
| `list_company_events` | One company's calendar events (dividends, general meetings, report dates) and their status. |
| `list_financial_facts` | One company's stored financial facts, each carrying its trust-ladder provenance: sourceTier, validationStatus, and citation. Decision support only. |
| `list_financial_periods` | One company's fiscal periods (year + period type + period-end date). |
| `list_kpi_definitions` | The metric catalog: every KPI/financial-concept definition (id, label, unit) facts are keyed by. |
| `list_flagged_fact_provenance` | Every fact the extraction pipeline flagged for review (a drift or contradiction against another source) — the data-quality review surface. |
| `get_price_context` | One company's price context: latest quote and the recent range, plus derived valuation ratios where computable. |
| `get_kpi_comparison` | Compare one or more canonical KPIs across companies on a shared, aligned period axis (annual or quarterly). Each cell carries the native + PLN-converted value with its FX basis, the evidence link (fact id + validation status), and server-computed QoQ/YoY deltas; gaps and unconvertible currencies are typed flags, never silent. Works for a single company too (the periods×deltas view). Decision support only. |
| `get_sector_percentiles` | Where one company stands against its tracked sector peers: rank-based percentiles for the level-0 market ratios (P/E, P/BV, EV/EBITDA, dividend yield, FCF yield) and selected canonical KPIs, computed from confirmed data only. Always returns the peer count N and flags thin sets (N < 4); a company with no sector returns a typed empty reason. Decision support only. |
| `list_valuation_runs` | One company's append-only comparative-valuation run history (ADR 0089): each stored run's method (P/E, EV/EBITDA, or P/BV multiple), per-share fair-value range (low/base/high), input signature, confidence grade, and data-as-of date, newest first. The compute-and-persist path is the act-tier compute_comparative_valuation. Decision support only. |
| `get_ownership_overview` | One company's shareholder structure: significant holders, holder types, and free float, with change history. |
| `get_insider_overview` | One company's insider-transaction timeline, management holdings, and rolling net-direction aggregates. |
| `list_short_positions` | One company's KNF short-selling register: active positions, change history, aggregate net short %, and the 30-day change. |
| `get_analyst_recommendations` | One company's recorded analyst recommendations and price targets over time. Decision support only — never an investment recommendation. |
| `get_company_health` | One company's deterministic financial-health scores (Piotroski F, Altman Z") per fiscal period, computed from confirmed facts. |
| `get_red_flags` | One company's active red flags (auditor concerns, short spikes, contradictions) plus the acknowledged history. |
| `get_report_documents_view` | One company's stored report documents, each tagged with its fiscal period and whether it is that period's canonical report. |
| `list_report_diff_candidates` | Comparable pairs of successive financial statements for one company — the (older, newer) document pairs get_report_diff can diff. |
| `get_report_diff` | The section-level text diff between two report documents (discover the pair via list_report_diff_candidates). |
| `list_video_transcript_jobs` | Video-transcript jobs (optionally scoped to one company): their source, status, and resolved company. |
| `list_transcript_segments` | One transcript job's ordered segments (timestamped text) — the transcript body itself. |
| `list_notebook_entries` | One company's research notes, each preserving the origin (report/article/transcript) it traces back to. |
| `list_management_claims` | One company's tracked management claims (guidance/promises), with their verification period and verdict. |
| `list_report_expectations` | Pre-report expectations the user recorded (optionally scoped to one company), with any resolution outcome. |
| `list_decision_entries` | The decision journal (optionally scoped to one company): recorded decisions and their rationale. |
| `list_research_questions` | Open and answered research questions across the workspace, with their scope and status. |
| `list_report_season` | The upcoming report-season calendar: which tracked companies report when, with preparation state. |
| `list_attention_events` | Fired attention events (newest first), optionally scoped to one company and optionally including dismissed ones. |
| `get_latest_morning_briefing` | The most recently composed morning briefing (its structured item list plus any narrative), or null when none exists yet. |
| `list_autopilot_runs` | Recent autopilot runs (optionally scoped to one company): what the autonomous pipeline produced and its notification state. |
| `get_autopilot_run` | One autopilot run's full composed result (discover run ids via list_autopilot_runs). |
| `list_quality_frameworks` | The quality-scorecard framework catalog: every framework and its criteria (the rubric get_quality_assessment scores against). |
| `list_alert_rules` | The alert-rule catalog: every configured rule (trigger, scope, enabled state). Fired events are read via list_attention_events. |
| `list_flagged_extraction_outcomes` | One company's extraction-coverage gaps: the fiscal periods where the deterministic pipeline emitted nothing (a flagged/failed outcome). Complements list_flagged_fact_provenance (flagged facts that DID emit) — the coverage-gap review surface. |
| `list_unclassified_filings` | Official filings (ESPI/EBI) the deterministic rule classifier could not place — the explicit unclassified bucket, never guessed at. Optionally scoped to one company. Classify one with classify_filing. |
| `get_report_tagged_fact_coverage` | How much of one company's tagged filings reached Fundamentals, and where the rest went: comparatives, dimensional breakdowns, note-level figures, positions awaiting a name, and conflicts. Every captured number is either projected or has a stated reason it is not. |
| `get_pipeline_reextraction_progress` | Progress of one company's latest re-extraction batch (re-armed runs, how many have terminated, how many failed). A null batch means the company never ran one. |
| `list_pending_kpi_ingests` | List claimable KPI ingest runs (discovered/source_captured/extracting/validation_failed), newest first, keyset-paginated (limit ≤ 50, default 20). |
| `get_kpi_ingest_context` | Everything one report's extraction needs, within hard budgets (≤256 KiB): run status, document metadata, the derived-period hint, the expected+minted KPI catalog, validator-equivalent plausibility evidence per slot, profile doctrine and repair-manifest access. Sections (catalog/plausibility/manifest) paginate via cursors; the manifest is served only via section calls. Pure read. |
| `get_kpi_ingest_document` | Chunked bytes (offset/length ≤ 256 KiB, base64) from the run's content-addressed source blob, verified against the frozen sourceContentHash — the portable document delivery channel. Available once the source is captured. Pure read. |
| `get_kpi_ingest_status` | Full status of one KPI ingest run (state, context, lease, expected KPIs, progress). Pure read — never touches the lease. |

**Act tools** — dispatchable only with *Settings → MCP server → Allow write tools* on (64):

| Tool | What it does |
| --- | --- |
| `create_notebook_entry` | Create a research note for a company. Every note must carry a non-empty `origins` array tracing it to a report/article/transcript (provenance). References the company by its internal id (from list_companies). |
| `create_note_from_transcript_selection` | Create a research note anchored to selected transcript segments (the selection is the note's origin/provenance). |
| `update_notebook_entry` | Update an existing research note (by id): title/body/tags/kind. The note keeps its recorded origins. |
| `create_management_claim` | Record a tracked management claim (guidance/promise). Must anchor to a `sourceEvidenceId` (the report/transcript it was made in). |
| `update_management_claim` | Update a tracked management claim (by id). Must carry its `sourceEvidenceId` provenance. |
| `set_claim_verdict` | Record a verification verdict on a management claim (optionally linking the verifying fact). |
| `create_financial_fact` | Low-level single-fact repair write (ADR 0098) — never report ingestion; once the KPI ingest run-workflow tools are present on this server, use them for ingesting reports. Records a financial fact for a company/period/metric. Must carry a non-blank `sourceDocumentRef` citation (`attribution` is the total/owners_of_parent/nci slot dimension, never a citation carrier). Decision support only. MCP writes are stamped honestly: `source_tier='agent'` provenance, `extraction_method='mcp_agent'`, `validation_status='unreviewed'` — never masquerading as a manual entry. |
| `update_financial_fact` | Low-level single-fact repair write (ADR 0098) — never report ingestion. Updates a stored financial fact (by id). Must carry its non-blank `sourceDocumentRef` citation. Stamps `source_tier='agent'` provenance (honest takeover — never masquerading as manual), even on a previously-manual fact. |
| `capture_report_document` | Register and fetch a report document by URL for a company — the document an agent read before citing facts from it (the fact-write tools need its returned documentId). Always registers under source_type "user_url"; passing a sourceType is refused (unknown field). Gated: https only, private/loopback/link-local network addresses refused (including via redirect), content-type restricted to application/pdf \| text/html \| application/xhtml+xml, 30 MiB size cap. Idempotent on (companyId, url). Returns the document's id, local path, and fetch success/error. |
| `record_financial_facts` | Low-level batch fact write (ADR 0098). If `start_kpi_ingest` is absent from this server's tools, this is the only supported temporary report-ingest route; once the run-workflow tools are present, use them for ingestion and this tool ONLY for manual repair. Records a batch (1-100) of financial facts for one company/period from a document an agent read, with per-fact citations. Ensures the fiscal period, resolves each metricKey against the KPI catalog, judges the set against stored history and same-period accounting identities, and commits every plausible fact under the `agent` source tier (ADR 0093) — never overwriting an issuer-held or manual fact; a disagreement is reported as `divergent`, never silently resolved. Use `dataQuality: "preliminary"` for issuer pre-report releases (e.g. GPW wstępne wyniki) — record CUMULATIVE columns only (H1/9M/FY), never discrete-quarter columns. Decision support only. |
| `set_qualitative_verdicts` | Record agent-authored qualitative criterion verdicts for one framework+company as one immutable snapshot. Every result must carry `citationsJson`: a serialized non-empty array of typed evidence refs `[{"evidenceType":"notebook_entry","evidenceId":"<id>"}]` (types: feed_item \| notebook_entry \| claim \| transcript_segment \| company_event \| research_question \| company_signal \| decision_entry); every ref must resolve to an existing row or the whole batch is refused. Decision support only — never an investment recommendation. |
| `create_research_question` | Open a research question scoped to a company/watchlist/sector. |
| `update_research_question` | Update a research question (title/body/status) by id. |
| `create_evidence_link` | Link two research-graph entities (note/claim/fact/document…) with a typed relation. |
| `create_research_reminder` | Create a personal follow-up reminder scoped to a company/watchlist/sector. |
| `update_research_reminder` | Update a research reminder (status/due/snooze) by id. |
| `create_decision_entry` | Append an immutable decision-journal entry for a company (rationale + decided-at). |
| `create_report_expectation` | Record pre-report expectations for a company's upcoming report event (stance + optional metrics). |
| `update_report_expectation` | Update a company's report expectation (stance/metrics) by its event key. |
| `record_expectation_resolution` | Resolve a report expectation after the report lands (resolution note). |
| `create_company_event` | Add a calendar event (dividend/meeting/report date) for a company. |
| `create_kpi_definition` | Add a KPI/financial-concept definition to the metric catalog. |
| `create_kpi_relevance` | Mark a KPI definition relevant to a company (scorecard editor). |
| `update_kpi_relevance` | Update a company's KPI-relevance row (status/rank) by id. |
| `create_quality_framework` | Create a quality-scorecard framework. |
| `update_quality_framework` | Update a quality framework (name/description) by id. |
| `create_framework_criterion` | Add a criterion to a quality framework. |
| `update_framework_criterion` | Update a framework criterion by id. |
| `create_alert_rule` | Create an alert rule (trigger + scope). Fired events surface via list_attention_events. |
| `update_alert_rule` | Update an alert rule (trigger/scope/enabled) by id. |
| `create_company` | Track a new company (exchange + ticker + display name). On GPW this also enqueues a quote backfill. |
| `create_watchlist` | Create a watchlist. |
| `add_company_to_watchlist` | Add a company to a watchlist (by their internal ids). |
| `remove_company_from_watchlist` | Remove a company from a watchlist (by their internal ids). |
| `update_feed_item_state` | Set a feed item's read/saved flags (by id). |
| `mark_report_prepared` | Mark a company's upcoming report event as prepared. |
| `mark_report_processed` | Mark a company's report event as processed (optionally linking the report document). |
| `mark_research_scope_reviewed` | Set a 'reviewed' checkpoint for a research scope (optionally cascading to its companies). |
| `confirm_company_signal` | Confirm a proposed filing signal (by id). |
| `reject_company_signal` | Reject a proposed filing signal (by id). |
| `classify_filing` | Classify an unclassified official filing (from list_unclassified_filings) into a confirmed signal. Takes `feedItemId` (the evidence anchor) and a `category` key from the seeded taxonomy. Rejects an unknown category, a non-official item, or an already-classified filing. |
| `confirm_derived_event` | Confirm or reject a proposed derived calendar event (`action`: confirm\|reject). |
| `acknowledge_red_flag` | Acknowledge an active red flag (by id). |
| `set_ownership_holder_type` | Relabel a shareholder's holder type for a company; returns the recomputed ownership overview. |
| `mark_attention_event_seen` | Mark an attention event as seen (by id). |
| `dismiss_attention_event` | Dismiss an attention event (by id). |
| `set_autopilot_run_notification_state` | Set an autopilot run's notification state (unread\|read\|dismissed). |
| `evaluate_framework` | Run the deterministic quantitative scorecard engine for a framework+company and persist the evaluation. |
| `compute_comparative_valuation` | Compute the level-1 comparative valuation for one company (peer-multiple implied fair-value ranges for P/E, EV/EBITDA, and P/BV, method-convergence spread, and a deterministic confidence grade) and append a valuation_runs row per method whose input signature changed. Read the history via list_valuation_runs. Decision support only — never buy/sell/hold language. |
| `set_alert_rule_enabled` | Enable/disable an alert rule (by id). |
| `trigger_autopilot_run` | Trigger an autopilot run over one company's report document (fail-fast on unknown ids); enqueues the durable pipeline. |
| `generate_morning_briefing` | Enqueue composition of a fresh morning briefing (read the result via get_latest_morning_briefing). |
| `refresh_sources` | Run a source-refresh sweep across all enabled adapters (`trigger`: manual \| scheduler). |
| `refresh_source` | Run a refresh for one source adapter (by `adapterId`; optional `trigger`, `date`). |
| `run_aggregator_fundamentals_pull` | Run the aggregator fundamentals pull across tracked companies. |
| `backfill_company_history` | Run an on-track history backfill for one company (`companyId`); progress via get_backfill_progress. |
| `run_structured_extraction` | Run the deterministic structured-first extraction pipeline over one company report+period (`mode`: autopilot \| assist). |
| `rerun_extraction_outcome` | Re-run the deterministic pipeline for a recorded extraction outcome slot (`outcomeId`). |
| `run_pipeline_reextraction` | Re-arm one company's landed ESEF runs whose stored pipeline version is stale, so the current extractor reads their filings again. Queues a durable batch; poll `get_pipeline_reextraction_progress`. |
| `start_kpi_ingest` | Start or resume a KPI ingest run (ADR 0099). Fresh: documentId + profileId (+ optional scope/dataQuality/period) creates the run, claims the lease, pins the source bytes, and enters extraction once context is complete. Resume: runId re-claims idempotently (the explicit keepalive) and attaches missing context set-once. Provenance is the run pipeline itself; no citation carrier here. |
| `stage_kpi_observations` | Stage the COMPLETE revision snapshot of extracted observations (1..100, with citations) plus the REQUIRED missingReasons declaration ({} = explicitly none), written in the same transaction. A repair resends every retained observation. Requires the caller's live lease. Provenance is the run pipeline itself. |
| `validate_kpi_ingest` | Validate one staged revision synchronously (generation-pinned). Returns the FULL manifest — a failed manifest is the typed repair report; a raced loser gets outcome=superseded with the current run tuple. |
| `commit_kpi_ingest` | Atomically commit a ready manifest (runId + manifestHash + revision) and return the immutable receipt. Idempotent: replaying a committed tuple returns the stored receipt verbatim; a stale tuple is a typed conflict. |
| `cancel_kpi_ingest` | Cancel a KPI ingest run in any pre-commit state (releases its lease). Refuses `committing` and terminal states. |

<!-- END GENERATED MCP CATALOG -->
