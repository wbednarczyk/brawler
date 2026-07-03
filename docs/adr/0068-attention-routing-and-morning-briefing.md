# ADR 0068: Attention Routing — Toasts, Alert Rules, Morning Briefing

Status: Accepted

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
