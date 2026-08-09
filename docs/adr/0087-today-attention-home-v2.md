# ADR 0087: Today attention home v2 — grouped stream, typed severity, quiet toasts

Status: Accepted (2026-07-22, owner sign-off at v0.60 planning) — **decision 3 (toast policy v2) superseded 2026-08-06 by [ADR 0097](0097-toasts-are-action-feedback-only.md)**: attention events raise no toasts at all; ambient awareness lives in the Today sidebar badge

Deciders: maintainer. Area: frontend, attention routing, i18n.

Amends [ADR 0068](0068-attention-routing-and-morning-briefing.md) (toast policy, briefing
composition) and [ADR 0076](0076-density-and-design-system.md) U-Rb (stream content policy).
Closes the seam named in [ADR 0084](0084-retire-in-app-ai-layer.md) decision 6 (backend-composed
Today copy). Executes as the full [ADR 0081](0081-ux-quality-loop-v2.md) UX-loop pilot on
journey J1.

## Context

Owner dogfooding (2026-07-19 and 2026-07-22, screenshot evidence on card `abd456e`) shows the
Today home failing its one job — attention hierarchy:

- The morning-briefing card renders **raw token streams and English prose verbatim**
  (`kpi_extraction_unavailable:no_deterministic_tier; report_diff_available — succeeded`):
  `compose_briefing` bakes prose and codes into the `morning_briefing_items.title`/`detail`
  columns and `MorningBriefingCard` displays them untranslated. The attention and autopilot
  surfaces already solved this correctly (typed codes in the DB, frontend renderers translate);
  the briefing is the last un-cut seam — exactly the one ADR 0084 dec. 6 assigned to v0.60.
- **No grouping or dedup exists anywhere**: 8+ near-identical briefing rows (one company twice),
  duplicate autopilot rows per company in the stream, a config-level condition repeated per row
  instead of stated once.
- **Toasts hold the important items instead of the stream leading with them.** Every attention
  event gets the same hardcoded-`caution` persistent toast; insider transactions and
  missed-report reconciliations sit stacked bottom-left (capped 3 + "+N more") while the stream
  leads with routine autopilot rows. Live evidence: the toast stack intercepts pointer events
  over real controls (registry-sector live spec blocked 3 minutes; owner screenshots show list
  rows covered).

The U-Rb shape (single prioritized stream + narrow counters column, J1 budget ≤15 interactions
at 10 new items as the acceptance bar) stands and is not re-litigated here.

## Decisions

1. **Content policy: group, dedup, then rank.** The stream stays a single prioritized list
   (U-Rb), but rows are produced by a deterministic pipeline: (a) **dedup** — one row per
   (company, category, evidence), repeats collapse; (b) **group** — same-category items for one
   company collapse into a single row with a count ("GPW:PAS — 2 runy autopilota") expanding in
   place; (c) **rank** — severity first, then recency. The same dedup applies inside briefing
   composition (one item per company+type+evidence). Exact presentation (row anatomy, expansion,
   counters) is fixed by the D1 experience contract + owner-approved mockup, not by this ADR.
2. **A typed severity taxonomy, mapped in exactly one place.** Three levels: `urgent`
   (leads the stream; the only level that may raise a persistent toast — e.g. insider
   transaction, profit warning, missed-report reconciliation), `notable` (stream + transient
   toast at most — e.g. autopilot failure, fired price alert), `routine` (stream only — e.g.
   successful autopilot run, upcoming report). Severity is derived from `trigger_type` + signal
   category by one backend mapping shipped with typed values on the attention/stream payloads;
   the frontend never re-infers importance from strings. The mapping table lives in
   product-spec §Attention Routing; adding a trigger type without classifying its severity is a
   gate failure (same posture as the MCP registry gate, ADR 0088).
3. **Toast policy v2: a toast is a pointer, never a store.** The stream is the system of
   record for attention; a toast only announces "something urgent landed — it's at the top of
   Today". Persistent toasts are reserved for `urgent` events, transient for `notable`
   completions; nothing requires dismissing a toast to act, and the stack must never block
   interaction with underlying controls (hard layout constraint, guarded by the existing
   toast-cap browser spec extended to the new model). Dismissing the stream row dismisses its
   toast and vice versa (single seen/dismissed state, as today).
4. **The briefing seam is cut on the autopilot pattern.** `compose_briefing` stops writing
   prose: `title`/`detail` carry only typed tokens/codes or verbatim source data (a signal's own
   title), never composed English. The frontend renders items through the existing renderers
   (`renderAutopilotSummaryTokens`, `attentionEventTitleText`, `text()` framing) with a tolerant
   read for legacy prose rows (the `isTokenizedSummary` pattern; no migration). Class guardrail
   (guardrail-harvest of `abd456e`): a test scans the briefing item builders for English
   literals, and the ui-authoring §6 rule is extended: **backend writes codes, the frontend
   translates** — any new backend-composed user-visible string is a defect of this class.
5. **Config-state conditions render once, as a banner.** A condition that is a property of the
   app (a source unreachable, a provider misconfigured) appears as a single dismissible banner
   above the stream, never repeated per row.
6. **Process: mockup-gated, budget-gated.** Implementation follows the ADR 0081 loop —
   experience contract + 7-frame storyboard + HTML mockup approved by the owner before Today
   component code; the J1 journey test (≤15 interactions at 10 new items) is the acceptance
   bar; first-slice/mid/release live checkpoints on the real Windows app.

## Consequences

- The 684-line `TodayScreen.tsx` monolith is decomposed as part of D3 (row components per
  category, the toast effect extracted) — architecture-debt rule, not optional cleanup.
- Raw backend error `.message` strings on Today are replaced by typed codes + translations
  (same class as decision 4).
- `ui-information-architecture.md` §Today is rewritten against the approved mockup in the same
  change as the implementation; its stale AI-narrative briefing sentence is corrected at
  roadmap-reshuffle time (it is already false since ADR 0084).
- Severity values ship through existing payloads (attention events / stream read models);
  the exact wire shape is fixed in contracts.md in the same change as the backend mapping.

## Amendments

### 2026-07-23 — live-checkpoint P1 (owner-approved): a systemic-cause wall of stale urgent rows

The ADR 0081 mid-milestone live checkpoint on the owner's real DB found 14+ week-old
`source_reconciliation` attention events rendering as 14 separate PILNE rows — a new wall of the
exact kind the redesign set out to remove. Two class causes, two amendments:

- **Decision 2 (severity aging).** Urgency did not age: a missed-report reconciliation stayed
  `urgent` forever. Added an **age-based demotion** at read time — an `urgent` attention event that
  has gone **unacted for more than 72h after `fired_at`** demotes to `notable` (nothing is hidden or
  auto-dismissed; it just stops shouting and leading the stream). The rule is **purely age-based**:
  seen/dismissed state does not enter the mapping. It lives in the single severity home
  (`storage::severity`, one authoritative `aged_attention_severity` entry point + a named 72h
  threshold const) and is applied where the read model computes severity (`list_attention_events`).
  Boundary: at **exactly** 72h the event **stays urgent**; only strictly older demotes.
- **Decision 1 (urgent + notable same-cause collapse).** The approved mockup's "urgent never
  collapses" is **superseded for same-cause systemic causes**: when `URGENT_AGGREGATE_MIN` (2) or
  more urgent company-rows share one cause (e.g. many companies each with a `source_reconciliation`
  event), they collapse cross-company into one leading `urgent` aggregate ("PILNE · ×K spółek",
  ranked by its newest member, members expanding in place, each keeping its own Review/Dismiss).
  The **notable** severity collapses the same way (its own `NOTABLE_AGGREGATE_THRESHOLD`, the routine
  "more than N" semantics) into an `Uwaga · ×K spółek` aggregate — so a wall that merely **aged out
  of urgent** (dec. 2) no longer just changes colour, it still folds to one line. Collapse is
  **partitioned by severity first**: a cause's urgent and notable rows never fold together (urgent
  ranks above, notable below). **Different causes never merge** — a day of price alerts across four
  companies collapses **per cause** (a `price_enters_range` line and a `price_week52_low` line stay
  distinct), which is the intended density behaviour, not a defect. The routine cross-company
  aggregate (`ROUTINE_AGGREGATE_THRESHOLD`, category-keyed) is unchanged and keeps its **separate**
  const; urgent keeps its own `URGENT_AGGREGATE_MIN` (2). Attention aggregate/group rows additionally
  carry a two-step **"Dismiss all"** action dispatching the existing per-event dismiss for every
  member (urgent or notable).
