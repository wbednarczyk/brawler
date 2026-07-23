---
name: brawler-mcp
description: Use when you (an AI agent) are connected to Brawler over its MCP server and need to read the user's investment research or write back into it. Covers tool discovery, the read/act tiers, the mandatory-source (provenance) rules per write family, internal-id conventions, the permanent denylist, and safe read→analyze→write sequences. Load it before your first tools/call against Brawler.
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

Call `tools/list` first. Tools fall in two tiers, tellable by name:

- **Read** — `get_*` / `list_* / search_*`. Always available. Use them freely to
  understand the workspace before doing anything else.
- **Act** — every other verb (`create_*`, `update_*`, `set_*`, `mark_*`,
  `confirm_*`, `trigger_*`, `run_*`, …). These **write**. They are always listed
  even when disabled.

**Act tools may be off.** Writes are gated by a user setting (default OFF). If
you call an act tool while writes are disabled you get a typed
`writes_disabled` error (`isError: true`) — the handler never ran, nothing
changed. **React gracefully:** do not retry, do not work around it. Tell the
user *"writing is turned off — enable Settings → MCP server → Allow write tools
if you want me to record this,"* then continue read-only. You **cannot** enable
it yourself: `update_settings` is not on the MCP surface, by design.

## Provenance: every write must cite a source

Brawler rejects unsourced research. If a write's provenance carrier is empty you
get a typed `provenance_required` error naming the missing field — again nothing
is written. **Gather the citation from read tools first, then write.** Per
family, with a concrete input shape:

| Family | Tools | Carrier + example |
| --- | --- | --- |
| Notes | `create_notebook_entry` | non-empty `origins[]`: `"origins":[{"sourceType":"external_url","sourceUrl":"<filingUrl>","label":"FY2024 annual report"}]` — valid `sourceType`: `feed_item` \| `transcript_segment` \| `ai_analysis` \| `manual` \| `external_url` (a stored report document has no first-class origin type yet — cite its URL or `feed_item`; card `670946e`). Also required: `tags` (may be `[]`) and `kind` ∈ `manual` \| `observation` \| `claim` \| `question` \| `follow_up` |
| Transcript notes | `create_note_from_transcript_selection` | `"transcriptSegmentIds":["<seg1>","<seg2>"]` (the selection is the origin) |
| Claims | `create_management_claim`, `update_management_claim` | `"sourceEvidenceId":"<reportOrTranscriptId>","sourceEvidenceType":"report"` |
| Facts | `create_financial_fact`, `update_financial_fact` | `"sourceDocumentRef":"<reportDocId>"` **or** `"attribution":"FY2024 report, p.42"` |
| Qualitative verdicts | `set_qualitative_verdicts` | every result carries `"citationsJson":"[{\"type\":\"note\",\"id\":\"<noteId>\"}]"` (non-empty typed-evidence array) |

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

## Tool catalog

Machine-generated from the server's frozen `tools/list` snapshot (a drift gate
keeps it exact — do not hand-edit):

<!-- BEGIN GENERATED MCP CATALOG — do not edit; regenerate: node scripts/check/docs-drift.mjs --write-mcp-catalog -->

**Read tools** — always available once the server is on (41):

| Tool | What it does |
| --- | --- |
| `get_company_dossier` | One company's research dossier: identity, fundamentals coverage per fiscal period, confirmed financial facts, and quality-scorecard summaries. Sourced from the user's own research; decision support only. |
| `search_research` | Full-text search across the user's research workspace (notes, report documents, transcripts, claims, facts). Returns ranked matches with snippets. |
| `list_claims_due` | Management claims whose verification period has arrived (due), passed (overdue), or is approaching (upcoming), per company. |
| `get_quality_assessment` | Quality-framework state for one company: the latest stored scorecard evaluation per framework, plus previously-stored qualitative verdicts. The in-app qualitative-assessment writer was retired (ADR 0084) — qualitative criteria are recorded manually now, and this tool reads only stored verdicts (new criteria stay empty until the planned MCP write-tools). Decision support only — never an investment recommendation. |
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

**Act tools** — dispatchable only with *Settings → MCP server → Allow write tools* on (55):

| Tool | What it does |
| --- | --- |
| `create_notebook_entry` | Create a research note for a company. Every note must carry a non-empty `origins` array tracing it to a report/article/transcript (provenance). References the company by its internal id (from list_companies). |
| `create_note_from_transcript_selection` | Create a research note anchored to selected transcript segments (the selection is the note's origin/provenance). |
| `update_notebook_entry` | Update an existing research note (by id): title/body/tags/kind. The note keeps its recorded origins. |
| `create_management_claim` | Record a tracked management claim (guidance/promise). Must anchor to a `sourceEvidenceId` (the report/transcript it was made in). |
| `update_management_claim` | Update a tracked management claim (by id). Must carry its `sourceEvidenceId` provenance. |
| `set_claim_verdict` | Record a verification verdict on a management claim (optionally linking the verifying fact). |
| `create_financial_fact` | Record a financial fact for a company/period/metric. Must carry a citation (`sourceDocumentRef` or `attribution`). Decision support only. |
| `update_financial_fact` | Update a stored financial fact (by id). Must carry its citation provenance. |
| `set_qualitative_verdicts` | Record agent-authored qualitative criterion verdicts for one framework+company as one immutable snapshot. Every result must carry a non-empty `citationsJson` evidence array (provenance). Decision support only — never an investment recommendation. |
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
| `set_alert_rule_enabled` | Enable/disable an alert rule (by id). |
| `trigger_autopilot_run` | Trigger an autopilot run over one company's report document (fail-fast on unknown ids); enqueues the durable pipeline. |
| `generate_morning_briefing` | Enqueue composition of a fresh morning briefing (read the result via get_latest_morning_briefing). |
| `refresh_sources` | Run a source-refresh sweep across all enabled adapters (`trigger`: manual \| scheduler). |
| `refresh_source` | Run a refresh for one source adapter (by `adapterId`; optional `trigger`, `date`). |
| `run_aggregator_fundamentals_pull` | Run the aggregator fundamentals pull across tracked companies. |
| `backfill_company_history` | Run an on-track history backfill for one company (`companyId`); progress via get_backfill_progress. |
| `run_structured_extraction` | Run the deterministic structured-first extraction pipeline over one company report+period (`mode`: autopilot \| assist). |
| `rerun_extraction_outcome` | Re-run the deterministic pipeline for a recorded extraction outcome slot (`outcomeId`). |

<!-- END GENERATED MCP CATALOG -->
