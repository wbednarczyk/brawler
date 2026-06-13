# Radicle/Radboard Tracking

Active planning has moved from this file to Radicle issues rendered by Radboard.

- Radicle repository: `rad:z3yTYrLFsFx5qcPtV3XiFYFBpQWuh`
- Visibility: public
- Public seed: `seed.mikolajczyk.org:8776`
- Public seeding policy: owner node syncs releases to the public seed
- Radboard milestones: app version labels such as `milestone:v0.25.0`
- Radboard epics: major capability issues labeled `epic`
- Radboard tasks: reviewable work-slice issues linked to epics with `parent:<epic-hex7>`
- Radboard bug issues: deferred bugs labeled `bug`
- Shared research-workspace label: `area:research-workspace`
- Shared release-workflow label: `area:release-workflow`
- Shared packaging label: `area:packaging`
- Shared fundamentals label: `area:fundamentals`

Current milestone epics:

- `eca2082` - epic: Financial facts foundation (`milestone:v0.34.0`, completed in `0.34.0`; ESPI/EBI attachment ingestion and backfill deferred to `v0.39.0`)
- `fb20c2f` - epic: Multi-provider AI (Claude + OpenAI, BYO key) (`milestone:v0.35.0`, completed in `0.35.0`)
- `9879941` - epic: AI KPI extraction with confirmation (`milestone:v0.36.0`)
- `8505350` - epic: Fundamentals panel and KPI charts (`milestone:v0.37.0`)
- `2cc8bd6` - epic: Search and data safety hardening (`milestone:v0.38.0`)
- `0e1d6c5` - epic: Typed ESPI event classification (`milestone:v0.39.0`)
- `cbf6999` - epic: Management claims tracker (`milestone:v0.40.0`)
- `75001e4` - epic: Report-season cockpit (`milestone:v0.41.0`)
- `b7a54ba` - epic: Cross-company KPI comparison (`milestone:v0.42.0`)
- `287d0b4` - epic: Quality frameworks — quantitative checks (`milestone:v0.43.0`)
- `7cba98b` - epic: Story clustering across sources (`milestone:v0.44.0`)
- `db6be22` - epic: Report-over-report diff (`milestone:v0.45.0`)
- `df178f4` - epic: Feed triage mode and command palette (`milestone:v0.46.0`)
- `9a607da` - epic: Autonomous report pipeline (`milestone:v0.47.0`)
- `5835112` - epic: Quality frameworks — qualitative assessment (`milestone:v0.48.0`)
- `ebbcb29` - epic: Re-invent the notebook panel (`milestone:v0.49.0`)

Current fundamentals sequence:

- v0.34.0 (completed in `0.34.0`) establishes the financial facts data model, report document persistence (user-supplied PDF URLs), URL evidence capture, and manual KPI entry. ESPI/EBI attachment ingestion and backfill were deferred to v0.39.0.
- v0.35.0 (completed in `0.35.0`) adds Claude and OpenAI as BYO-key AI providers alongside Gemini (async provider boundary, provider registry, per-provider keychain credentials, model selection, and a document-input abstraction on the provider trait), drawn before extraction so the report-document path is designed multi-provider.
- v0.36.0 adds AI KPI extraction through the provider boundary with mandatory per-fact user confirmation.
- v0.37.0 ships the fundamentals panel, custom per-company KPIs, KPI trend charts via shared SVG primitives, and export/import.

Current research-leverage sequence:

- v0.38.0 hardens the corpus: FTS5 full-text search across all stored content and automatic local database backups with restore.
- v0.39.0 classifies ESPI/EBI filings into typed company events (insider transactions, dividends, profit warnings, contracts, buybacks).
- v0.40.0 tracks management claims from reports and transcripts with due periods, verdicts, and KPI-backed verification.
- v0.41.0 ships the report-season cockpit: upcoming report dates with pre-report cards built from questions, claims, KPIs, and evidence.
- v0.42.0 adds cross-company KPI comparison with side-by-side tables and multi-series trend charts.

Current time-saver sequence:

- v0.44.0 clusters near-duplicate multi-source coverage into single stories with the official source ranked first.
- v0.45.0 diffs consecutive periodic reports section by section with a cited AI delta summary.
- v0.46.0 ships keyboard feed triage and a global command palette over search, navigation, and actions.
- v0.47.0 composes the building blocks into an autonomous report pipeline: detect publication, auto-fetch, auto-extract, and notify with cross-references, behind a per-company trust ladder (confirm-before-commit stays the default; auto-confirmed facts are flagged unreviewed, reversible, and cited).
- Company history backfill on track ships earlier, inside v0.34.0.

Current quality-frameworks sequence (Kroeze-style checklists, user-owned + app templates):

- v0.43.0 adds quantitative checks: a rule engine evaluates user frameworks against the fundamentals facts and produces a versioned scorecard; ships clonable templates incl. a Kroeze-style quality template. Depends only on facts (v0.37); resequenceable.
- v0.48.0 adds qualitative agent-assessed criteria (moat, pricing power, recurring revenue, capital allocation) with citations, composed into the scorecard and re-evaluated by autopilot.

The fundamentals schema was validated against ~37 GPW companies (IT/SaaS, retail, developers, construction, manufacturing, banks, insurer, debt-purchase, gaming, space); findings are recorded in ADR 0027 (statement-type packs, generalized unit model, fact variants, period model).

Backlog items folded into milestones: README polish into v0.34.0; Claude and ChatGPT providers promoted to the dedicated v0.35.0 multi-provider AI epic; AI waiting animation into v0.36.0; feed retention policy design into v0.38.0; Investing.com RSS assessment into v0.39.0. The portalanaliz.pl source study (`69d5cc0`) was returned to the unscheduled backlog (no milestone). Deliberately unscheduled: X.com and Google Finance studies, Perplexity provider, terminal interface, mobile clients, and the Windows taskbar indicator remain plain backlog until product pressure appears.

Current research-workspace sequence:

- `feaf0ea` - epic: AI research briefs (`milestone:v0.30.0`, completed in `0.30.0`)
- `0f17877` - epic: Event-aware reminders and research digest (`milestone:v0.31.0`, completed in `0.31.0`)

Use `rad issue list --all` or Radboard for active status. Create a Radicle issue for every reported or discovered bug that will not be fixed immediately. Historical completed-card context remains in [Kanban Archive](kanban-archive.md).
