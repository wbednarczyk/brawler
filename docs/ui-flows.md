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
5. Detail pane shows title, source, publication time, matched companies, source URL, original text or excerpt, and the typed signal(s) (a stored pre-retirement AI analysis remains readable as legacy data — [ADR 0084](adr/0084-retire-in-app-ai-layer.md)).
6. User marks item read, saves it, opens the original source, or creates a note from it.
7. Signals are typed by the deterministic rule classifier only ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); an unclassifiable filing simply carries no typed signal.

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

Intent: turn a periodic report's reported numbers into a structured, source-linked fundamentals view — automatically, with no per-report chore.

Flow:

1. The user tracks a company; from that point core KPIs arrive **automatically** — no modal, no AI, no per-report action ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md)). The BiznesRadar aggregator is pulled daily (primary source, every period column its statement pages carry), and issuer filings corroborate as they are ingested: ESEF/iXBRL (annual) and the ESPI cover-note "wybrane dane finansowe" (WDF) table lifted from the periodic-report komunikat at ingest (the structured xHTML/positional parser that once also corroborated visual-only interims is retired, [ADR 0095](adr/0095-retire-html-positional-tier.md)).
2. The user opens the company's **Fundamentals** tab and sees the values in the KPI-per-period matrix, with trend charts and click-through provenance. **Facts are review-free**: each lands already confirmed and honestly labeled by origin (source tier + citation) — nothing waits for ratification.
3. A fact's detail shows its **provenance labels** (source tier, extraction method, citation) and validation state; the user can **edit or remove** any value inline, and **add** a fact manually (inline KPI search, reporting-period selector, value, currency) to cover anything the automatic tiers left absent.
4. Custom per-company KPIs cover metrics outside the seeded taxonomy. Manual values are **untouchable** by every automatic path (a later divergence is logged, never applied).

Acceptance criteria:

- Core KPIs populate without any per-report user action; the automatic tiers write facts labeled by origin, and gaps are shown, never silently absent.
- Facts are review-free: no value ever waits `pending` or needs confirmation; manual add/edit/delete stays an option, never a duty ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md)).
- Facts appear in the Fundamentals matrix through the same read model whether automatically extracted or manually entered.
- Below the matrix, a **positions × periods** view (the N=1 case of the comparison read model, [ADR 0089](adr/0089-cross-company-comparison-and-valuation-l1.md)) shows each KPI's recent aligned periods with inline QoQ (quarterly only) / YoY deltas — % for monetary, p.p. for ratio/percentage — every value ⧉-linked to its fact; gaps and non-positive/sign-flipped bases render as typed flags, never fabricated numbers.
- Values display in their original as-reported scale with localized KPI names, never raw integers or internal ids.
- A fact's detail shows its source-tier and validation labels; where a higher issuer tier and the aggregator disagree, the disagreement is recorded informationally and never overwrites the issuer figure, and a strictly higher tier upgrades a lower tier's slot ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md)).
- For a company in `autopilot` mode, a run's facts land already committed; the review point is Today/Pulse's Autopilot run card, which offers **Undo** (two-step confirm) reverting exactly the facts that run produced, then shows a "Reverted N facts" state ([ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md) §4).
- A stored periodic financial statement can be diffed section-by-section against the previous same-type filing, from the company workspace and on new-report arrival ([ADR 0052](adr/0052-report-over-report-diff.md)).

## Journey: Compare Companies On A KPI

Intent: judge a tracked company's relative position by lining it up against its peers on the same canonical KPI, aligned across periods, with the evidence one click away — input to a buy/pass decision (journey J6, [ADR 0089](adr/0089-cross-company-comparison-and-valuation-l1.md)).

Flow:

1. The user opens **Compare (Porównaj)** from the sidebar (its own Modes item under Dashboard). The empty state is a quiet invite — "pick at least 2 companies with confirmed data" — never a blank screen and never an artificial CTA.
2. The user builds a **zestaw spółek**: "+ Add company…" multi-selects from tracked companies, "From watchlist…" quick-picks a whole list, and a "**+ Peers in sector X**" helper offers the tracked siblings of the first company's sector. Each selection appears as a chip with its series colour dot (the same four colour slots the trend overlay uses); ✕ removes it.
3. The user picks a **canonical KPI** and a **granularity** (annual / quarterly), then presses **Porównaj** (the one primary action for the primed state).
4. The result is a **side-by-side table**: rows are the companies (colour dot + full ticker), columns are the aligned periods plus Δ YoY (Δ QoQ too, quarterly). Every value cell carries an **⧉ evidence link** to the fact's company Fundamentals/report surface; a non-PLN company shows an **EUR→PLN** chip whose tooltip names the FX basis (flow: period average / stock: rate at period end).
5. No value is ever silently absent: a gap, a missing FX rate, or an unknown currency renders a **typed, translated flag** in the cell, not a fabricated number; an undefined percentage change renders "—" with its reason. A read failure shows a **typed error strip** with a per-section **Try again** (never a raw `.message`).
6. Removing a chip (or changing the KPI/granularity) **recomputes immediately**; dropping below two companies falls back to the invite rather than a stale table. Across the supported window range (down to ~960px) the sections stack and the table scrolls inside its own container — no global horizontal scroll.

Acceptance criteria:

- Reaching a two-company comparison takes ≤2 interactions from entry; the empty state invites without CTA-spam and the primed state has exactly one primary action ("Porównaj").
- The table aligns each company's confirmed facts on a shared period axis; deltas are server-computed (% for monetary, p.p. for ratio/percentage KPIs), never re-derived in the UI.
- Every gap carries a typed reason; mixed currency never renders an unconverted number as comparable; percentiles/medians (valuation, later slices) never show without their peer count.
- Each value's ⧉ opens the fact's company workspace (Fundamentals/report surface); the FX-basis and evidence detail stay one interaction away, not on the face of the table.
- All copy is localized (en + pl, "zestaw spółek" — never "kohort"); counts render through the plural helper.

## Journey: Review Company Ownership Structure

Intent: see who owns a tracked company and how stakes moved over time, straight from the Basic info panel, with every disclosure traced to its report ([ADR 0072](adr/0072-ownership-structure.md)). Decision support only — never advice.

Flow:

1. The user opens a company's cockpit dashboard; the **Ownership ("Akcjonariat") section** sits under the Basic info identity facts — no navigation, it is just there. Once the company's periodic reports are fetched, deterministic extraction has already populated it (zero interaction).
2. The populated section shows a donut by holder type with the derived free-float slice, a stakes-over-time chart, and holder rows with type chips; the derived free float also appears as a Basic-info rowline.
3. If nothing is disclosed yet, the section shows an empty state with a **"Wydobądź z raportów"** CTA that force-enqueues deterministic extraction across the company's reports (`backfill_ownership_extraction`), with per-document progress; normally the automatic post-backfill run makes this unnecessary.
4. Holder types are assigned deterministically from a dictionary ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); no type-confirmation chip appears — types come from the dictionary or the user's own edit.
5. The user can manually re-type any holder (its current type shows its source — dictionary / AI (legacy) / manual); a manual label is authoritative and offers an immediate Undo (`set_ownership_holder_type`). Stake history is untouched — only the type label changes.
6. A report the deterministic parser can't read (glyph-encoded font / image table) is reported as an **honest flagged gap** with no run-action ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); the document is named, never guessed, and partial ESPI data stays visible.

Acceptance criteria:

- The section populates automatically from fetched reports with zero user interaction; the backfill CTA only force-kicks the same deterministic job.
- Free float is always derived (`100 − Σ disclosed capital`), rendered as a neutral hatched "uncertain" donut slice and a "Free float (derived)" rowline with an uncertainty hint (the 5% disclosure threshold hides smaller stakes).
- Holder-type colors are fixed per holder TYPE, never cycled; more than four types fold into "Other".
- Holder types are deterministic (dictionary) or the user's own edit — a manual re-type is authoritative and never overwritten by automation ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)).
- A residual (unreadable) document is never fabricated into data — it is disclosed as an honest flagged gap (the Review queue is retired); partial ESPI data stays visible.
- The section stays usable in a ~340px narrow pane (donut over legend, full-width chart, rows wrap without horizontal scroll).

## Journey: Review Company Health

Status: planned (v0.57.0, ADR 0083)

Intent: see at a glance whether a company's published health formulas and disclosure behavior raise concerns, with every number and flag traced to evidence ([ADR 0083](adr/0083-company-health-scores-and-red-flags.md)). Decision support only — never advice.

Flow:

1. The user opens a company's cockpit dashboard; the Quality panel shows Piotroski F and Altman Z″ tiles (band-colored, variant labeled) computed automatically from confirmed facts — or an explicit "insufficient data" state listing the missing inputs, or "not applicable" for financials.
2. Expanding a score tile reveals its per-component breakdown (each of the 9 F signals / 4 Z″ inputs with measured values) and the published-formula citation.
3. The **Red flags panel** lists active flags (auditor red flag, report delay, fund exit, score deterioration, short spike) with severity and an evidence link; acknowledging a flag moves it to history and never re-raises it for the same evidence. No flags = a calm explicit "no active flags" state.
4. In the Ownership section, founder/management holders carry a skin-in-the-game badge; the insider view shows the parsed transaction timeline and rolling 90-day / 12-month net buy−sell (only once ≥ 2 transactions exist).
5. A newly raised flag fires existing alert rules (typed signal) and appears in the morning briefing.

Acceptance criteria:

- A score never renders as a partial or rescaled headline; missing inputs are enumerated, not papered over.
- Every flag links to its evidence (filing, ownership basis, calendar event, or score history); acknowledged flags persist in history.
- No surface phrases advice or composes a single conviction rating (ADR 0042 guardrail).

## Journey: Track A Management Claim To Verdict

Intent: capture a management promise, then resolve whether it was delivered when the due period's report arrives ([ADR 0040](adr/0040-management-claims-tracker.md)).

Flow:

1. Reading a report document or a transcript, the user adds a claim manually from the company workspace **Claims** tab: the statement, a due period, an optional quantitative target, and the source reference. (The in-app AI claim-extraction launcher is retired — [ADR 0084](adr/0084-retire-in-app-ai-layer.md); agent-assisted claim proposals return later through the MCP write path with mandatory provenance.)
2. The claim is the user's own record — nothing is created automatically.
3. The confirmed claim appears in the company workspace **Claims** tab with verdict `pending` and its due period, source-linked back to the report/transcript.
4. Later, when the due period's report arrives, the claim resurfaces in the **claims to verify** review queue (bucketed due / overdue / upcoming). For a quantitative claim, the matching confirmed financial fact is shown beside the claim.
5. The user sets the verdict (delivered / partially delivered / missed / revised), optionally linking the verifying fact as supporting or contradicting evidence.

Acceptance criteria:

- Claims are user-created (manual now; MCP write-tools with provenance later) — nothing lands automatically.
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
5. When the report arrives, the user marks it **processed**; the card links to the arrived filing (facts arrive automatically — BR-primary pull + deterministic extraction, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md)), and the resurfaced claims appear in the claims-review queue. Autopilot's single "what changed" summary nudges the user to review recorded expectations vs actuals.
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

## Journey: AI Capability Routing — retired

Per-capability AI provider routing is retired with the in-app AI analysis layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); the only AI setting left is the transcript provider (Settings → AI, Gemini key for YouTube transcription). Intelligence arrives through the user's own agent over the MCP port (BYOA).

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
