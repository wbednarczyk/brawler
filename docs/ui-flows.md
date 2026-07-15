# UI Flows

This document plans Brawler from the user experience inward. It defines the first workflows the app must make feel natural before detailed database schema or implementation scaffolding hardens.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [Product Spec](product-spec.md), [UI Information Architecture](ui-information-architecture.md), and [Contracts](contracts.md).

## UX North Star

Brawler should feel like a personal investor research desk:

- fast to scan each morning or evening
- dense enough for repeated professional use
- calm enough to avoid dashboard noise
- source-first and origin-aware
- easy to turn a report, article, or transcript excerpt into a durable note

The default screen is the investor inbox. The second most important surface is the company notebook.

## Primary Navigation

V1 navigation should use persistent desktop app regions:

- top toolbar for the brand, search, refresh, and source status
- top navigation bar (directly beneath the toolbar) for the primary sections; it wraps to a second line on narrow windows rather than scrolling or hiding items
- central list or workspace for the current task, full-width beneath the chrome
- right detail pane when reviewing a feed item, company, note, or transcript segment

The Inbox workspace splits the feed list and detail pane 50/50 by default; the divider is draggable between 25% and 75% of the row. See [ADR 0047](adr/0047-top-navigation-bar.md) for the navigation-region decision.

Primary sections:

- Inbox
- Companies
- Notebooks
- Transcripts
- Sources
- Settings

## Journey: First Run

Intent: get from empty app to first useful watchlist quickly.

Flow:

1. User opens the app.
2. App starts in dark theme with empty local data.
3. User sees an empty Inbox state with actions to add companies or import a watchlist.
4. User adds a company by exchange-qualified ticker, for example `GPW:CDR`.
5. User creates or selects a watchlist.
6. App confirms the company is tracked and offers manual refresh.

Acceptance criteria:

- First run does not require cloud login.
- Theme defaults to dark.
- User can add at least one company without understanding internal IDs.
- Empty states are useful but not marketing-like.
- Tracking a company offers a **Backfill history** action that captures
  ~3 years of official-source report documents without duplicating data on
  repeated runs ([ADR 0036](adr/0036-report-document-storage-and-backfill.md)).

## Journey: Morning Review And Attention Alerts

Intent: open the app at the start of the day and learn what changed and whether anything needs action, without re-scanning everything (journey **J1**, [ux-journeys.md](ux-journeys.md); ADR 0068, `v0.54.0`).

Flow:

1. User lands on Today.
2. At the top, the **morning briefing** summarizes what changed in the user's companies and what needs doing — new signals, autopilot runs, claims due, upcoming report dates, and fired alerts — as a structured list, or an AI narrative with citation links when a provider is configured. A **Generate briefing** action recomposes it on demand; it also auto-refreshes once per day while the app is open.
3. Below it, the user triages the **attention stream** (autopilot runs, changed reports, claims to verify, upcoming reports, and fired alerts), optionally filtered by a counter tile.
4. **Fired alerts** also raise a **persistent toast** the user can click through to the evidence; the **Today attention list** groups fired events by company, where each is marked seen or dismissed.
5. User opens the 0–2 items that matter into the company workspace, then returns to Today.

Setup (Library → Alerts): the user creates **alert rules** from preset chips — a signal category, an autopilot run completing, or a price condition (*price enters my range* / *52-week low*) — scoped to a company or a watchlist, each enable/disable-able. Fired alerts are reviewable there too.

Acceptance criteria:

- The briefing renders even with no AI provider configured (structured list, never blocked); a narrative only appears when it can cite the composed items.
- A fired alert always traces back to its evidence (signal, run, or quote) via both the toast click-through and the attention list.
- An alert never re-fires for the same evidence, and never phrases a fact as advice.
- The journey stays within its interaction budget ([budgets.json](../tests/browser/journeys/budgets.json)); reading the briefing is a passive scan, not a counted interaction.

## Journey: Daily Inbox Review

Intent: review new company-specific reports and news with minimal friction.

Flow:

1. User opens Inbox.
2. Feed shows newest items first, with a typed-signal badge on classified official filings (e.g. insider transaction, dividend, profit warning).
3. User filters by watchlist, company, item type, signal type, unread, saved, and significance when available.
4. User opens an item in the detail pane.
5. Detail pane shows title, source, publication time, matched companies, source URL, original text or excerpt, the typed signal(s), and AI analysis if available.
6. User marks item read, saves it, opens the original source, or creates a note from it.
7. For a filing typed only by the optional AI fallback, the signal shows as a proposal the user can confirm or reject before it is applied.

Acceptance criteria:

- Feed list remains dense and scannable.
- Source and timestamp are visible without opening the original website.
- Read/unread and saved states are quick actions.
- Note creation from a feed item preserves origin.
- A classified insider transaction is visually distinguishable in the feed and can be filtered to.
- AI-proposed signals are never applied without explicit user confirmation.
- Confirmed signals appear in the company research timeline (filterable as "Signals") and feed the personal digest; a high-signal classification (insider transaction, profit warning) raises a research reminder.

## Journey: Create Note From Feed Item

Intent: turn relevant information into durable company research.

Flow:

1. User opens a feed item.
2. User chooses `Create note`.
3. App opens a note draft for the selected company.
4. Draft includes source origin and optionally prefilled title/body from selected item text.
5. User chooses note kind, tags, event date, and optional follow-up period.
6. User saves the note.
7. Note appears in the company notebook and links back to the feed item.

Acceptance criteria:

- User can edit all generated or copied text before saving.
- Notes support Markdown in v1.
- Claim notes can be marked as open and assigned a future follow-up period.
- Saved note always keeps source origin.

## Journey: Company Notebook Review

Intent: answer "what do I know about this company and what should I check later?"

Flow:

1. User opens a company (from the Companies library, a pinned sidebar entry, a feed item, or global search), landing the curated cockpit dashboard scoped to it ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md)).
2. The dashboard opens with a calm default panel set (Fundamentals, Feed, Claims, Quality, Report documents, Notebook) and stays composable; the company `Notebook` panel is one of them.
3. Notebook panel lists notes newest first, with filters by tag, kind, claim status, and follow-up period.
4. User opens a note in the detail pane.
5. User edits note content, changes claim status, or opens linked source material.

Acceptance criteria:

- Company notebook is reachable from both company navigation and feed item context.
- Open claims and due follow-up periods are visible.
- Notes can be traced back to feed items, reports, or transcript segments.

## Journey: Track Company Fundamentals From A Report

Intent: turn a periodic report's reported numbers into a structured, source-linked fundamentals view.

Flow:

1. User opens a report feed item (e.g. an ESPI/EBI periodic report) in the detail rail.
2. The rail shows a compact AI KPI extraction launcher; the user opens it, which presents a centered modal.
3. In the modal the user picks a report source: an attached PDF, the company's IR reports page, or a pasted PDF URL.
4. The app extracts reported KPIs for the primary period; the modal shows the job status and proposed values with as-reported figures, confidence, and source snippets.
5. The user reviews each proposal — confirm, edit, or reject — or bulk-confirms known KPIs and accepts out-of-taxonomy suggestions as new company KPIs. No value is committed without confirmation.
6. The user opens the company's Fundamentals tab and sees the confirmed values in the KPI-per-period matrix, with trend charts and click-through provenance. Manual entry and custom per-company KPIs cover anything extraction missed.

Acceptance criteria:

- No AI-proposed number becomes a stored fact without explicit confirmation.
- Heavy extraction interaction happens in the modal, not crammed into the fixed-width detail rail.
- Confirmed facts appear in the Fundamentals matrix through the same read model as manually entered facts.
- Values display in their original as-reported scale with localized KPI names, never raw integers or internal ids.
- A document that is a web page rather than a report PDF is rejected with an actionable message, not a misleading partial extraction.
- A fact's detail shows its source-tier and validation badges (ADR 0061); a fact whose layout drifted from the confirmed company profile additionally shows a "Structure changed" label diff (new/missing report lines, reporting-unit change). When the autonomous pipeline (autopilot/assist) carries the same drift signal on a run, the run's notification card on Today shows the identical "Structure changed" block, so drift is visible without opening the company.
- For a company in `autopilot` mode, step 5's confirm/edit/reject modal is bypassed: extraction runs automatically and produced facts land already committed as `auto_unreviewed`. The review point moves to Today/Pulse's Autopilot run card, which offers **Undo** (two-step confirm) instead of confirm/reject — it reverts exactly the facts that run produced, reusing the same supersede/reject mechanics, and the card then shows a "Reverted N facts" state ([ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md) §4). A company in `assist` mode still lands its proposals `pending` and goes through this journey's step 5 as normal.
- A stored periodic financial statement can be diffed section-by-section against the previous same-type filing, from the company workspace and on new-report arrival ([ADR 0052](adr/0052-report-over-report-diff.md)).

## Journey: Track A Management Claim To Verdict

Intent: capture a management promise, then resolve whether it was delivered when the due period's report arrives ([ADR 0040](adr/0040-management-claims-tracker.md)).

Flow:

1. From a report document or a transcript, the user opens the AI claim-extraction launcher (a modal, like KPI extraction). The app proposes candidate claims with the statement, a suggested due period, an optional quantitative target, confidence, and the verbatim source snippet.
2. The user reviews each proposal — confirm (with optional edits), or reject. No claim is created without confirmation. The user can also add a claim manually from the Claims tab.
3. The confirmed claim appears in the company workspace **Claims** tab with verdict `pending` and its due period, source-linked back to the report/transcript.
4. Later, when the due period's report arrives, the claim resurfaces in the **claims to verify** review queue (bucketed due / overdue / upcoming). For a quantitative claim, the matching confirmed financial fact is shown beside the claim.
5. The user sets the verdict (delivered / partially delivered / missed / revised), optionally linking the verifying fact as supporting or contradicting evidence.

Acceptance criteria:

- No AI-proposed claim becomes a tracked claim without explicit confirmation.
- A claim with a due period resurfaces in the review queue when the due-period report arrives, and can be resolved with a verdict linked to evidence (the milestone exit criterion).
- Verdicts are always user-set; the app never assigns a verdict automatically.
- Claims appear in the company research timeline and feed reminders/digests; they are exported with research data.
- Heavy extraction interaction happens in a modal, not crammed into the fixed-width detail rail.
- A claim's follow-up period supports both a quarter (e.g. `2026-Q3`) and an exact date, with quarter selection the most visible control ([ADR 0064](adr/0064-resolved-v1-ux-decisions.md)).

## Journey: Record A Decision In The Journal

Intent: capture a judgment (and why) the moment it is made, so the decision record starts accumulating for later calibration ([ADR 0071](adr/0071-judgment-capture.md); north star NS2). The app records and mirrors the user's own judgments — it never scores them and never gives buy/sell/hold advice.

Flow:

1. From a company's cockpit dashboard, the user opens the **Decision journal** panel (via the panel palette / Add panel — it is not a default dashboard panel).
2. The user records an entry: a decision kind (`buy` / `pass` / `keep_watching` / `sell_note`), a decided-on date, and a Markdown rationale.
3. The entry appears in the company's chronological list (by decided_at) and joins the company research timeline.
4. Selecting an entry, the user links supporting evidence — feed items, notes, claims, or events from the company timeline — to the decision.
5. To correct or revise a past decision, the user records a **Supersede** follow-up (a new entry linked back to the superseded one); the original is never edited or deleted.
6. Across companies, the user reviews the **global decision journal** ("Journal (all companies)") chronologically, filtered by decision kind and company.

Acceptance criteria:

- A recorded entry is immutable: there is no edit or delete affordance; the only correction is a Supersede follow-up linked by `supersededByEntryId`.
- Entries order by their decided_at date, never by insertion order.
- An entry can link typed evidence (feed item / note / claim / event) and surfaces in the company research timeline as a `decision_entry`.
- The app records and reflects the user's judgments back; it produces no recommendation and no automatic score ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md) posture).

## Journey: Prepare For Report Season

Intent: arrive at each tracked company's report date already knowing what to check, record what you expect before it lands, then close the loop when the report lands ([ADR 0044](adr/0044-report-season-cockpit.md), [ADR 0071](adr/0071-judgment-capture.md)).

Flow:

1. The user opens **Report Season** (next to Inbox). The cockpit lists upcoming report dates across the watchlists, ordered by date; a stale-calendar indicator shows when the calendar is out of date.
2. Each upcoming report shows a pre-report card composed from the company's open research questions, unresolved claims (due / overdue / upcoming), last-period confirmed KPIs, and recent evidence.
3. From the card the user can **write expectations** for the occurrence (ADR 0071): a free-text stance plus optional per-metric expectations (a metric picked from the card's last-period KPIs, a comparator, and an expected value); the user picks the fiscal period the report covers. Expectations stay editable — and re-openable via **Edit expectations** — until the period's facts land, when they freeze.
4. The user reviews a card and marks the company **prepared**; from the card they can drill into the company workspace, its research questions, or its claims-review queue.
5. When the report arrives, the user marks it **processed**; the card links to the arrived filing and to KPI extraction for the new report, and the resurfaced claims appear in the claims-review queue. Autopilot's single "what changed" summary nudges the user to review recorded expectations vs actuals.
6. Once facts land, the frozen expectation flips to an **expectation-vs-actual** review: each metric shows expected / actual / a factual outcome (Met / Missed / No data), and the user records their own **verdict**. The app never scores the user's judgment — it mirrors the comparison and stores the user's note ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md) posture).

Acceptance criteria:

- For a watchlist with upcoming reports, the cockpit shows dated cards whose contents match the company workspaces (the milestone exit criterion).
- A company can be taken from upcoming → prepared → processed against a real report cycle.
- Expectations written before a report land as an editable draft, freeze once the period's facts are recorded (freeze checked inside the update transaction; an edit after facts land surfaces the `conflict` envelope as a read-only flip), and expose an expectation-vs-actual review with a user-recorded verdict.
- The cockpit composes existing domains with no duplicated logic and adds no per-company data beyond the prepared/processed workflow state and the recorded expectations.
- The cockpit never auto-fetches or auto-extracts; processing links to the manual KPI-extraction and claims-review entry points.

## Journey: YouTube Conference To Notes

Intent: capture relevant management statements from a press conference.

Flow:

1. User opens Transcripts or a company workspace.
2. User enters a YouTube video link in a field labeled `URL`.
3. User may optionally provide the target company/ticker before transcription.
4. App creates a Gemini-backed transcript job.
5. If no company was provided, app attempts to recognize the company from the video/transcript result.
6. If recognition fails, the transcript remains available as an unlinked transcript.
7. User can optionally link the transcript to a company through the same local lookup used by Companies.
8. User sees job status while processing.
9. Transcript segments appear with timestamps when available.
10. User reviews transcript segments.
11. User selects one or more whole transcript segments.
12. If the transcript is linked to a company, user edits the note draft and saves it to that company notebook.

Acceptance criteria:

- Gemini is used only as the preferred provider for YouTube transcription.
- M10 completion requires real Gemini transcript generation for a supported public YouTube URL; sample transcript output is only for development and automated tests.
- URL is the primary required input; company is optional upfront and optional after transcription.
- Unlinked transcripts remain valid and viewable.
- Company selection uses the same cached company lookup/autocomplete behavior as Companies when the user wants a company notebook note.
- User confirms note content before saving.
- Saved notes link to transcript segment IDs, original YouTube URL, provider/job context, and timestamp ranges when available.
- Provider limits and privacy implications are visible before sending video data to the provider.
- Transcript segments are immutable source output and are not edited directly; only note drafts created from them are editable ([ADR 0064](adr/0064-resolved-v1-ux-decisions.md)).

## Journey: Appearance And Locale Settings

Intent: let the user keep the app comfortable for daily use.

Flow:

1. User opens Settings.
2. User selects dark, light, or system theme.
3. App applies the theme immediately.
4. User can select English or Polish as the app language.
5. App applies app-owned UI copy for the selected language.
6. App persists the selected settings locally.

Acceptance criteria:

- First-run theme is dark.
- First-run locale is English.
- Dark theme uses the `night-neon` palette.
- Light theme preserves the same accent identity.
- Polish locale is available from Settings.
- Locale handling is extensible so future supported languages can be added through locale resources/configuration instead of per-screen rewrites.
- Source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies keep their original or user-entered language.

## Journey: AI Capability Routing

Intent: let the user mix AI providers per capability — e.g. a document-capable provider for KPI extraction and a free open-model host for text analysis — instead of one global provider for everything.

Flow:

1. User opens Settings → AI.
2. User sets the general AI provider/model (the fallback every capability uses unless overridden) and, if using a free/self-hosted open-model host, the OpenAI-compatible base URL.
3. User opens Settings → Credentials and saves the API key for each provider they intend to use (including the OpenAI-compatible provider — see `wiki/ai-provider-presets.md` for host presets and what value to use when a host itself needs no key).
4. In the AI capability routing section, user picks a capability (e.g. KPI extraction) and adds one or more provider/model rows — an ordered list, first row tried first.
5. For an OpenAI-compatible row, user types the model id (freeform, host-specific) instead of picking from a curated list.
6. User reorders rows (move up/down) to set failover priority, or removes a row.
7. App saves the capability's pool immediately with the rest of settings; an empty list falls back to the general AI provider.

Acceptance criteria:

- A capability with no configured rows behaves exactly as before (single global provider) — the feature is opt-in per capability.
- Reordering changes the failover order in the saved pool.
- The model field is a picklist for curated providers and a free-text field for the OpenAI-compatible provider.
- Saving an OpenAI-compatible base URL that is non-empty and not `http://`/`https://` is rejected with a clear error.
- The Credentials tab lists the OpenAI-compatible provider alongside Gemini/Claude/OpenAI, using the same generic save/replace/clear form.

## Journey: Connect An AI Assistant (MCP Server)

Intent: let a local AI assistant read the user's research through the read-only MCP server ([ADR 0078](adr/0078-mcp-external-surface.md)); a journey-independent utility ([ux-journeys](ux-journeys.md)).

Flow:

1. User opens Settings → MCP server.
2. User generates an access token; the plaintext shows **exactly once** in a copyable field with an explicit shown-once note. The user copies it.
3. User turns on **Enable the server**; the status pill flips to Active — or the refusal reason (missing token, port in use) shows inline, never a crash.
4. Optionally the user changes the listen port (committed on blur, clamped to 1024–65535); the new port applies on the next server start ([contracts](contracts.md#external-surface--mcp-server-read-only-mvp)).
5. User copies a connection snippet — Claude Code (HTTP) or the stdio adapter — with the port interpolated live, and pastes the token where the placeholder appears.
6. To cut off access, the user revokes the token behind an inline confirm; regenerating replaces it.

Acceptance criteria:

- The server is off by default and never starts without both the setting enabled and a configured token.
- After the one-time reveal (navigating away or refreshing), the token can only be revoked/regenerated — never shown again; only its configured/storage status renders.
- Enable/disable takes effect live and the returned status renders; failures surface as status text, not crashes.
- The snippets are example wording, marked as such; a snippet embeds the token only during the one-time reveal, otherwise a `<token>` placeholder.

## Journey: Global Search

Intent: find anything stored locally from anywhere in the app.

Flow:

1. User opens global search from the top-toolbar search box (or its keyboard shortcut).
2. User types a query.
3. App shows ranked results grouped by content type (companies, feed items, notes, transcript segments, research briefs, digests), each with a snippet.
4. User selects a result.
5. App navigates to the owning screen/item.

Acceptance criteria:

- Searching a known phrase from feed, note, and transcript content returns it and navigates correctly.
- Results are ranked and grouped by content type with a snippet.
- The per-workspace search/filter inputs (Inbox, Companies, Notebooks) still work independently.
- Search copy is available in English and Polish.

## Journey: Restore From Backup

Intent: recover the local data from an automatic backup after loss or a bad upgrade.

Flow:

1. User opens Developer Diagnostics and the backups section.
2. User reviews backup status and the list of rotating backups and pre-migration snapshots.
3. User chooses a backup and selects restore.
4. App requires explicit confirmation and warns that restore is applied on relaunch.
5. App stages the chosen backup and relaunches to apply it.

Acceptance criteria:

- A backup exists before every data-structure migration; a failed migration leaves the snapshot intact.
- Rotating backups keep the last N and prune the oldest.
- Restore from Diagnostics recovers a verified earlier state after relaunch.
- Restore never performs a hot in-place swap while connections are open.

## Screen Inventory

See [UI Information Architecture](ui-information-architecture.md) for the canonical V1 screen list and deferred-UI pointer.

Resolved V1 UX decisions (company workspace structure, report backfill, report-over-report diff, claim follow-up periods, transcript editability, source status placement) are recorded in [ADR 0064](adr/0064-resolved-v1-ux-decisions.md); current behavior for each is stated in the journeys above.
