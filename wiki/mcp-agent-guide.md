# Connecting an AI agent to Brawler (MCP how-to)

This is the hands-on guide to wiring an AI assistant (Claude Code, Claude
Desktop, or any MCP client) into Brawler so it can **read your research and —
when you allow it — write back into it**, always with a source. For the
what-and-why, security posture, and troubleshooting, see the reference page:
**[The MCP server](mcp-server.md)**.

Brawler is **BYOA** — Bring Your Own Agent. The app itself no longer runs AI
analysis; the intelligence lives in whatever assistant you connect here, and
Brawler is its sourced, local workspace.

## The two tiers, in one breath

- **Read tools** are live the moment the server is on. The agent can look up
  anything you can see: companies, feed, facts with provenance, ownership,
  claims, notes, quality scores, briefings — the whole workspace.
- **Act tools** (write anything) are **off by default**. They only work after
  you turn on **Settings → MCP server → Allow write tools**. Deletes, undo,
  and settings/credentials stay **UI-only, forever** — no agent can reach them.

## Step 1 — turn the server on

In **Settings → MCP server**: **generate a token** (copy it — shown once),
then **enable** the server. The status pill shows *running* and the port
(default **8317**). Full walk-through incl. the Windows-vs-WSL loopback caveat:
[the reference page](mcp-server.md#turning-it-on-settings--mcp-server).

## Step 2 — connect your assistant

You need the **port** and the **token** from step 1.

### Claude Code (HTTP — simplest)

```
claude mcp add --transport http brawler http://127.0.0.1:8317/mcp \
  --header "Authorization: Bearer <your-token>"
```

### Claude Desktop (stdio adapter)

Claude Desktop launches a process and talks over stdin/stdout, so point it at
the bundled bridge `brawler-mcp-stdio` (it sits next to `brawler.exe` in the
portable folder). Edit `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "brawler": {
      "command": "D:\\Brawler\\Builds\\latest\\brawler-mcp-stdio.exe",
      "env": {
        "BRAWLER_MCP_PORT": "8317",
        "BRAWLER_MCP_TOKEN": "<your-token>"
      }
    }
  }
}
```

The bridge does no thinking — it forwards each request to the same local HTTP
server, so **Brawler must be open with the server enabled**. (Claude Code can
use the same bridge with `claude mcp add brawler -- <path>\brawler-mcp-stdio.exe
--port 8317 --token <your-token>` if you prefer stdio there too.)

Now ask, e.g. *"use the brawler tools to pull the dossier for CDR"*.

## Step 3 — enabling writes (optional)

Flip **Settings → MCP server → Allow write tools** on to let the agent record
research **for you**. What it unlocks and what it never touches:

| The agent CAN (act tier, once writes are on) | The agent CANNOT (UI-only, always) |
| --- | --- |
| Create/update notes, claims, facts, expectations, journal, questions, reminders | Delete anything |
| Record qualitative verdicts, claim verdicts, evidence links | Undo / roll back a run |
| Manage watchlists, mark read/saved, dismiss attention, confirm/reject signals | Change settings, tokens, or credentials |
| Trigger jobs (autopilot run, source refresh, extraction, briefing) | Enable its own writes (the toggle is UI-only) |

Because `update_settings` is off-limits to the agent, **an agent can never turn
its own writes on** — only you can, here. That is the safety net behind
default-OFF. Turn it back off any time; act calls then return a clean
*"writes disabled"* refusal.

### Every write must cite a source

Brawler refuses to store an unsourced claim: **if the agent doesn't cite where
a fact came from, the write is rejected** (a typed `provenance_required` error
naming the missing field — nothing is written). The required citation per
write family:

| Write family | Tools | Must carry |
| --- | --- | --- |
| Research notes | `create_notebook_entry` | a non-empty `origins[]` tracing to a source — `sourceType` one of `feed_item` / `transcript_segment` / `ai_analysis` / `manual` / `external_url` (plus required `tags`, may be empty, and `kind`: `manual`/`observation`/`claim`/`question`/`follow_up`) |
| Notes from a transcript | `create_note_from_transcript_selection` | the selected `transcriptSegmentIds` (the selection *is* the origin) |
| Management claims | `create_management_claim`, `update_management_claim` | `sourceEvidenceId` (the filing/transcript the claim was made in) |
| Financial facts | `create_financial_fact`, `update_financial_fact` | `sourceDocumentRef` — a non-blank citation. Never `attribution`: that field is the fact's slot dimension (`total`/`owners_of_parent`/`nci`), not a citation carrier |
| Batch financial facts | `record_financial_facts` | a non-blank `reportDocumentId` PLUS a non-blank `citation` on **every** entry of `facts` — one blank citation refuses the whole batch before any write |
| Qualitative verdicts | `set_qualitative_verdicts` | every `results[].citationsJson` non-empty (typed evidence array) |

Updates that carry no new sourced datum (`update_notebook_entry` keeps its
original origins, `set_claim_verdict`'s evidence is the optional
`verifyingFactId`) have no extra citation requirement — integrity is enforced
at create time.

## Ingesting an issuer publication (writes on)

**Which path (ADR 0098):** if `start_kpi_ingest` is absent from the server's
`tools/list`, the legacy `capture_report_document` → `record_financial_facts`
ritual below is the only supported temporary report-ingest route. Once the
run-workflow tools are present, ingestion goes through start → stage →
validate → commit; the direct fact tools are then repair-only.

This is the ritual for turning a report — an ESPI/EBI filing, a preliminary
results PDF — into cited facts, claims, notes, and events (ADR 0093, the
scenario epic #285 was built for).

1. **Capture** the document — `capture_report_document` with its URL. Keep
   the returned `documentId`; it's the citation anchor for every step below.
2. **Read/extract** — read the document and note, for every figure you plan
   to record, which page and table/row it came from.
3. **Mint any missing metrics** — the catalog doesn't have every
   issuer-specific KPI (broker client counts, CFD lots, net deposits, …).
   `create_kpi_definition` with a snake_case ASCII `metric_key` and
   `scope: "company"`; the definition's `origin` is stamped `agent`
   automatically.
4. **One batch write per fiscal period** — `record_financial_facts`, citing
   the page + row label per fact (e.g. `"p.12, tab. 3, row 'Zysk netto'"`).
5. **Route everything else** through the mapping table below.

**Unit scaling.** Polish statements state the unit in the table header:
`(w tys. PLN)` means store the FULL base unit — multiply the printed number
by 1,000; `(w mln PLN)` → ×1,000,000. `valueNumeric` is always base-unit PLN
(or the stated currency), never the printed thousands/millions figure. Cite
the declared unit in the citation text so the scaling is checkable later.

**Parenthesized negatives.** `(419 996)` in a Polish statement means
**−419996** (thousands) — it's a sign convention, not a label or a range.
Thousand separators are non-breaking spaces; the decimal separator is a
comma (`1 027 240,50` → `1027240.50`).

**The cumulative-only rule (ADR 0093 dec. 3).** GPW interim publications
print discrete-quarter AND cumulative columns side by side. Record ONLY the
cumulative column (H1/9M/FY) into the cumulative period — the discrete
quarter is skipped, it's derivable by span arithmetic. Worked example (XTB RB
18/2026): the table shows Q2 net profit `492 198` tys. right next to H1
`1 027 240` tys. — the correct write is `1027240000` against the H1 period,
never the Q2 figure. `periodType` for a half year is `"H1"`, `periodEnd`
`YYYY-06-30`.

**Preliminary releases.** A wstępne/preliminary publication →
`dataQuality: "preliminary"` on the batch. When the final audited report
lands later, its `final` write supersedes the preliminary one automatically
at creation time — nothing else to do.

**Watch for ambiguous labels.** The same label can appear twice in one
publication at different windows (XTB defines "Liczba aktywnych klientów"
both quarterly and half-yearly). Cite the exact table/row you read, and pick
the column whose window matches the period you're recording, not the first
match you find.

**Chart-only values.** A figure that exists only inside a chart image isn't
in the document's text layer — read the page visually before citing it, and
cite the page number. Never estimate a chart value from its axis.

**Refusals during the ritual.** `provenance_required` means a citation is
missing from the input — fix it, don't retry unchanged. `writes_disabled`
means the owner hasn't flipped *Settings → MCP server → Allow write tools* —
ask them, don't try to work around it.

### Mapping doctrine: report content → tool

| Report content | Tool | Notes |
| --- | --- | --- |
| P&L / balance-sheet / KPI figures | `record_financial_facts` | per-fact citation (page + row label); legacy/low-level — repair-only once the run-workflow tools are present |
| Management guidance with numbers ("costs +30% in 2026", "marketing +50%") | `create_management_claim` | structured target fields (`targetMetricKey`/comparator/value, due year/period) + `sourceEvidenceId` = the captured document id |
| One-off narrative events (a KNF penalty, a donation) | `create_notebook_entry` | `report_document` origin |
| Dividend declarations/payments | `create_company_event` | |
| The upcoming final report date | `create_report_expectation` | feeds the report-season calendar |
| Post-balance-date trading updates ("92.8k new clients in July") | `create_notebook_entry` | **never** `record_financial_facts` — the datum belongs to a period not yet closed |

## The full tool catalog

Grouped by domain at a glance — **companies & watchlists · feed & signals ·
facts, periods & KPIs · quotes, ownership & insiders · health & red flags ·
reports, diffs & transcripts · notes, claims, journal, questions & reminders ·
quality frameworks · calendar & report season · attention, alerts & briefing ·
autopilot · source & extraction jobs**. The exact, machine-generated list (kept
in lock-step with the server by a drift gate — never edit it by hand) follows.

Result shape: every tool's `structuredContent` is a JSON object — tools whose
natural result is a list return `{ "items": [...] }`, scalar results arrive as
`{ "result": ... }`.

<!-- BEGIN GENERATED MCP CATALOG — do not edit; regenerate: node scripts/check/docs-drift.mjs --write-mcp-catalog -->

**Read tools** — always available once the server is on (48):

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
| `list_pending_kpi_ingests` | List claimable KPI ingest runs (discovered/source_captured/extracting/validation_failed), newest first, keyset-paginated (limit ≤ 50, default 20). |
| `get_kpi_ingest_context` | Everything one report's extraction needs, within hard budgets (≤256 KiB): run status, document metadata, the derived-period hint, the expected+minted KPI catalog, validator-equivalent plausibility evidence per slot, profile doctrine and repair-manifest access. Sections (catalog/plausibility/manifest) paginate via cursors; the manifest is served only via section calls. Pure read. |
| `get_kpi_ingest_document` | Chunked bytes (offset/length ≤ 256 KiB, base64) from the run's content-addressed source blob, verified against the frozen sourceContentHash — the portable document delivery channel. Available once the source is captured. Pure read. |
| `get_kpi_ingest_status` | Full status of one KPI ingest run (state, context, lease, expected KPIs, progress). Pure read — never touches the lease. |

**Act tools** — dispatchable only with *Settings → MCP server → Allow write tools* on (63):

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
| `start_kpi_ingest` | Start or resume a KPI ingest run (ADR 0099). Fresh: documentId + profileId (+ optional scope/dataQuality/period) creates the run, claims the lease, pins the source bytes, and enters extraction once context is complete. Resume: runId re-claims idempotently (the explicit keepalive) and attaches missing context set-once. Provenance is the run pipeline itself; no citation carrier here. |
| `stage_kpi_observations` | Stage the COMPLETE revision snapshot of extracted observations (1..100, with citations) plus the REQUIRED missingReasons declaration ({} = explicitly none), written in the same transaction. A repair resends every retained observation. Requires the caller's live lease. Provenance is the run pipeline itself. |
| `validate_kpi_ingest` | Validate one staged revision synchronously (generation-pinned). Returns the FULL manifest — a failed manifest is the typed repair report; a raced loser gets outcome=superseded with the current run tuple. |
| `commit_kpi_ingest` | Atomically commit a ready manifest (runId + manifestHash + revision) and return the immutable receipt. Idempotent: replaying a committed tuple returns the stored receipt verbatim; a stale tuple is a typed conflict. |
| `cancel_kpi_ingest` | Cancel a KPI ingest run in any pre-commit state (releases its lease). Refuses `committing` and terminal states. |

<!-- END GENERATED MCP CATALOG -->

## Three example workflows

**1 · Morning research pass (read-only).** *"Read my latest morning briefing
and the attention events; for anything flagged, pull the company dossier, red
flags, and claims due, and summarise what needs my attention today."* The agent
calls `get_latest_morning_briefing` → `list_attention_events` →
`get_company_dossier` / `get_red_flags` / `list_claims_due`, and hands you a
prioritised list — no writes, no toggle needed.

**2 · Classify backlog filings (writes on).** *"Go through unconfirmed filing
signals and confirm the routine ones, flag anything unusual for me."* The agent
lists signals via `list_company_signals`, then `confirm_company_signal` /
`reject_company_signal` on each. (A dedicated unclassified-filings triage read +
`classify_filing` write land with the M4 slice.)

**3 · Write qualitative verdicts with citations (writes on).** *"Assess CDR
against my 'Moat' framework from its latest report and my notes, and record the
verdicts."* The agent reads `list_quality_frameworks`, `get_company_dossier`,
`list_notebook_entries`, `get_report_documents_view`, then calls
`set_qualitative_verdicts` with a `citationsJson` evidence array on every
criterion. Read it back in the app's Quality tab or via `get_quality_assessment`.
Verdicts are decision support — the agent describes evidence, never "buy/sell".

## See also

- **[The MCP server](mcp-server.md)** — reference: enabling, security, the
  Windows/WSL loopback rule, troubleshooting.
- For agent authors: the repo skill `.claude/skills/brawler-mcp/SKILL.md`
  instructs a client agent on discovery, provenance, and safe sequences.
