# ADR 0097: Toasts Are Action Feedback Only — Ambient Attention Lives in Today

Status: Accepted (2026-08-06, owner sign-off on the #330 plan)

Deciders: maintainer. Area: frontend, attention routing, a11y.

Amends [ADR 0068](0068-attention-routing-and-morning-briefing.md) (the persistent-toast leg),
[ADR 0069](0069-source-reliability-and-disclosure-signals.md) decision 2's 2026-07-15 amendment
(the "toast" outlet), and [ADR 0087](0087-today-attention-home-v2.md) decision 3 (toast policy
v2). Resolves issue #330.

## Context

Owner dogfooding (2026-08-05, screenshot on #330): the bottom-left stack of persistent
attention toasts — reconciliation `espi_only` events arriving in batches from one sweep, capped
at 3 visible plus a "+15 more" overflow row — serves no UX role, annoys, and covers Inbox rows.
ADR 0087's toast policy v2 ("a toast is a pointer, never a store") reduced but did not remove
the mismatch: ambient system events still rendered as interrupting chrome, tripling the
attention surface (toast + Today + briefing) that ADR 0054/0087 deliberately centralized in
Today. The Today stream already solves batch density (same-cause cross-company aggregates,
72h urgent aging — ADR 0087 amendments), so the toasts duplicated a solved problem.

The inventory also found a real defect: a reconciliation attention row's Review action
navigated to the company Feed, which **cannot** contain the missed report (witness items never
enter the feed, ADR 0069) — the witness URL sat unused in the reconciliation ledger, breaking
ADR 0069's "previewable" promise. The "backfilled from {source}" copy overclaimed self-healing
for the same reason.

## Decisions

1. **A toast is feedback for a direct user action — nothing else.** Attention events (any
   severity) never raise a toast. `useAttentionToasts` is deleted. The issue's optional
   severity-gated exception is rejected: `urgent` events already lead the Today stream and light
   the badge (decision 4); interrupting chrome may re-enter only via a future ADR.
2. **The `persistent` toast variant is removed from the primitive.** `ToastInput` loses
   `persistent`/`dismissLabel`/`onDismiss`; the visible-cap, "+N more" overflow row,
   `role="alert"` branch, and the overflow-binding plumbing go with it. Toasts are transient
   only: bottom-left, max 3, auto-dismiss 6s — the region stays, for action feedback.
3. **The class is guarded by an allowlist, not the type system alone**: a static test scans
   production `useToast` consumers against an enumerated action-feedback allowlist (any code
   can still call `toast.show`; the deleted variant proves nothing about future producers), and
   a browser spec asserts that many unseen attention events raise zero toasts.
4. **Ambient awareness = a sidebar badge on Today** (the existing Inbox `nav-badge` idiom):
   the count of **unseen non-routine attention events** (`urgent` + `notable`). Scope is
   deliberately attention-events-only — autopilot runs carry their own `notificationState`,
   claims have no seen flag and are due work, not new events. One **polite** coalesced live
   region announces count increases after initial hydration ("K new important items in Today");
   no per-event announcements, no startup-backlog replay. Dropping `role="alert"` means urgent
   events get no assertive screen-reader interrupt anywhere — a deliberate a11y change:
   discovery is passive, via the badge and the stream.
5. **Seen means "was on screen the last time Today was open".** A new batch command
   (`mark_attention_events_seen`) marks every loaded unseen event when the Today stream
   renders, so the badge clears on visiting Today (aggregates included — members are in the
   loaded set). Previously `seen` only gated toast dedup and was set per-row on evidence
   review; nothing else consumed it. `seen` ≠ `dismissed`: rows and the Archive are unchanged.
6. **One app-level attention controller** owns events, rules, the unseen count, refresh, and
   seen/dismiss mutations (request-generation guarded). Today, Alerts, and the badge consume
   the same state — no per-screen copies drifting apart — and it refreshes on every
   event-producing seam (startup behind the license gate, manual all-source refresh,
   single-source refresh, scheduler cycle completion, which also covers job-failure events).
7. **Reconciliation severity stays `urgent`.** With toasts gone, severity no longer routes any
   toast; demoting to `notable` would buy nothing and break wall collapse (urgent aggregates
   from 2, notable from 4) while removing the only real exerciser of the 72h aging rule.
8. **Reconciliation evidence becomes actually reachable**: the attention read model exposes the
   witness URL (joined from the reconciliation result), and the row's Review opens the missed
   report instead of navigating to a feed that cannot contain it. Copy states what happened
   honestly — "missed by the primary source — caught by {source}" — and the `evidence_detail`
   field docs (attention.rs, contracts.md) are corrected: it names the witness that caught the
   report, not the source that missed it.

## Rejected

- **Demoting reconciliation events to the Diagnostics ledger + a digest line** (#330's
  "self-healed" leg): the premise is false — the missed report never enters the feed, so the
  attention event is the only surface where the investor learns it exists. Hiding it hides a
  real filing.
- **Emitter-level batch aggregation** (one summary event per category per run): destroys
  per-report evidence links, per-company scoping, dedup-on-rerun, and the Archive trail; the
  stream's existing wall collapse already delivers the density.

## Consequences

- The bottom-left region can no longer accumulate chrome: nothing persistent exists to stack.
- Severity's surface behavior simplifies to stream lead + badge inclusion; product-spec
  §Attention Routing drops the toast column.
- `retired-surface` guards the deleted identifiers (`useAttentionToasts`,
  `PERSISTENT_VISIBLE_CAP`, `onPersistentOverflowClick`) against doc resurrection.
