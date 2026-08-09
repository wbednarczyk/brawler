# ADR 0068: Attention Routing — Toasts, Alert Rules, Morning Briefing

Status: Accepted — **amended 2026-07-20 by [ADR 0084](0084-retire-in-app-ai-layer.md)**: the briefing's optional AI narrative is removed (`v0.59.0`); the deterministic composed list (`gather_sources` + `compose_briefing`) becomes the only briefing. **Amended 2026-08-06 by [ADR 0097](0097-toasts-are-action-feedback-only.md)**: attention events no longer raise toasts (persistent or transient) — toasts are action feedback only; ambient awareness moved to the Today sidebar badge. Alert rules and attention events are unchanged.

The app produces high-signal events (typed disclosure signals, autopilot runs, upcoming due dates) but has no attention layer: no toast/snackbar system exists at all, async feedback surfaces inconsistently, and nothing tells the user "this deserves a look" without them scanning feeds. This ADR adds the in-app attention boundary and its first synthesis consumer, the morning briefing.

## Context

- A 2026-07-03 frontend audit found zero toast/notification affordances; success/failure of async actions is inline-only and inconsistent.
- Typed signals (ADR 0034), autopilot run records (ADR 0055), and price data (ADR 0067) give deterministic triggers worth routing to the user.
- Desktop/OS notifications remain out of scope for v1 (product-spec); the Windows taskbar indicator is a separate future roadmap item. The boundary must accommodate those later adapters without rework.

## Decision

1. **`Toast` primitive** in `src/ui` (ADR 0037): app-wide transient feedback for async outcomes, plus a persistent variant for attention events. No screen hand-rolls its own.
2. **Attention-event boundary in the backend**: alert evaluation emits attention events consumed by (a) in-app toasts, (b) the Today/Pulse home, and later (c) OS adapters (taskbar/notifications) behind the same boundary — Inbox/domain code never calls a platform API directly (consistent with the taskbar-indicator architecture note in roadmap).
3. **Alert rules as a user-owned entity**: rule = trigger type (signal category e.g. profit warning/insider transaction, autopilot run completion, price condition: enters my range / 52-week low) + scope (company or watchlist) + enabled flag. Evaluated deterministically on ingestion/refresh via the durable queue; every fired alert links to its evidence.
4. **Morning briefing**: an on-demand/daily job reusing the research-digest contract (provider-routed per ADR 0060) that composes "what changed in my companies + what needs doing" (new signals, autopilot results, claims due, upcoming report dates) with citations, surfaced as a Today card. Deterministic composition of the item list; AI only phrases the narrative — with no provider configured, the briefing renders as a structured list without narrative.
5. Alerts and briefings are **decision support**: they state what happened with links back to sources; never action recommendations.

## Consequences

- Async feedback becomes consistent app-wide (quick-win adoption path: replace inline-only statuses opportunistically).
- New settings surface: alert-rule management (visual-first per docs/ui-authoring.md).
- The v0.54 milestone carries this ADR; price-condition rules activate only once ADR 0067 ships (rule types are additive).
- Journey impact: strengthens "morning review" (docs/ux-journeys.md #1) — the briefing is its entry point.

## Amendment 2026-07-18 (v0.57 fix wave 2 — historical ingest never impersonates the present)

Owner-reported live defect: a report-history backfill re-ingesting years of filings raised a wall of ~19 unseen persistent toasts for a `profit_warning` rule — 12 events for one company dated 2023…2026. Two root causes, both fixed:

1. **No freshness gate.** Rule evaluation fired on any confirmed signal regardless of age. **Fix:** the historical-ingest seam (`classify_and_store_signal`) does not evaluate alert rules for a filing whose **domain** date is older than `SIGNAL_FRESHNESS_DAYS` = **14 days** relative to wall-clock now — a backfill of old filings stores the signals but stays silent; only genuinely new signals alert. A signal with no/unparseable domain date is treated as fresh (never suppress what we cannot prove is old). The gate lives on the ingest seam, **not** inside `evaluate_signal_rules` (`signal_is_stale` in `storage::attention`), so present-detection paths — derived red flags and KNF short-position changes, which raise a signal *now* whose domain date is an old report period — still alert.
2. **`fired_at` was the evidence's DOMAIN date, so the per-rule daily throttle keyed on it never coalesced a backfill.** Each historical filing carried a distinct old date, so the "1 event per rule per day" throttle saw each as a different day and let them all through. **Fix:** `fired_at` is now the **wall-clock firing time**, and the throttle keys on the wall-clock day — so however many distinct pieces of evidence a rule matches in one ingestion pass, it pings at most once that day. The evidence's own domain date lives on its linked signal/quote/run (unchanged); the read model orders by `fired_at` (firing time), which for fresh evidence is ~now.
3. **`trigger_type` is now stamped on every event** (W4), not left NULL for rule-backed rows and derived only via `COALESCE` at read — so a direct read / grouping that does not join `alert_rules` sees the real trigger. The read model keeps the `COALESCE` for defense in depth.

Repairs (forward, idempotent, self-healing migrations): `0096_dismiss_stale_attention_events.sql` dismisses the pre-existing unseen backlog (`fired_at` > 30 days old) so the toast wall clears on update without touching fresh events; `0097_backfill_attention_trigger_type.sql` backfills legacy NULL `trigger_type` from the owning rule. The persistent-toast surface is also capped (3 visible + a "+N więcej" summary) and repositioned clear of the left sidebar nav (W3).
