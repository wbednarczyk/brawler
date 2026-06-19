# ADR 0034: Typed ESPI Event Classification (Design)

Status: Accepted

This ADR captures the **design** for typed ESPI/EBI event classification (epic `0e1d6c5`, milestone `v0.40.0`). It records the classification taxonomy, the new `company_signals` model, the signals-vs-events boundary, the rule/AI execution split, and the confirmation policy, so the contracts and data model are decision-complete before implementation.

## Context

ESPI/EBI official filings arrive as undifferentiated `feed_items`: an insider transaction, a dividend recommendation, a profit warning, and a routine administrative notice all look the same in the Inbox. The investor has to open and read each filing to learn what kind of disclosure it is. The goal is to turn the official-report stream into a typed signal stream — insider transactions, dividends, profit warnings/estimates, significant contracts, own-share transactions, guidance changes — so the feed surfaces *what happened* without manual triage.

Constraints from the existing system:

- **The active official-report source is Bankier company-komunikaty, not `gpw-espi-ebi`.** `gpw-espi-ebi` is registered but disabled (see [source-strategy.md](../source-strategy.md)). Bankier exposes the ESPI/EBI category label (`Komunikaty spółek (ESPI)`), report title, body text, and attachments, which is enough to classify. The classifier must therefore read the active Bankier feed items, but must be worded source-neutrally so a future GPW re-enable feeds the same classifier.
- **`feed_items` has no field to carry a classified type today**, and `company_events` is a calendar/forward-looking model (it powers the Events screen and the upcoming report-season cockpit). Most ESPI disclosures are *dated past events*, not future calendar items; forcing every classified filing into `company_events` would pollute the calendar with non-calendar signals.
- The repository's modularity-first posture (see [modularization-design.md](../modularization-design.md), [engineering-workflow.md](../engineering-workflow.md)) favors a typed, extensible registry over a hard-coded enum, and an async, provider-neutral AI boundary (see [ADR 0028](0028-multi-provider-ai-boundary.md)) with confirm-before-commit for AI-derived data (consistent with the fundamentals confirmation model, [ADR 0027](0027-company-fundamentals-scope.md)).

## Decision (proposed)

### 1. New canonical entity: `company_signals`

A typed disclosure signal is its own first-class entity, separate from both `feed_items` (the raw filing) and `company_events` (the calendar). `company_signals` is the canonical output of classification. The data-model fields are specified in [data-model.md](../data-model.md); the classification carries at minimum: owning company, origin `feed_item`, category, confidence, `classified_by` (`rule` | `ai`), `status` (`confirmed` | `proposed`), and a signal date.

### 2. Seeded, extensible category registry

The taxonomy is a typed registry of categories with associated rule definitions, seeded with a fixed set but extensible as data (new categories/markets add rows + rules, not schema rewrites). Seed categories:

- `insider_transaction` — manager transactions under MAR Art. 19
- `dividend` — dividend recommendations and declarations
- `profit_warning` — profit warnings and result estimates (positive or negative)
- `significant_contract` — significant agreements/contracts
- `own_shares` — own-share / treasury-share transactions (purchases **and** sales; generalized from the original `buyback` category, which mis-typed own-share sales — migration 0044)
- `guidance_change` — forecast/guidance changes
- `general_meeting` — general-meeting convocations carrying a meeting date
- `other` — official filing that classified to no specific category

Each category records its matching rules and whether it derives a calendar event (see §4).

### 3. Rule classifier at ingestion; async AI fallback with confirmation

- A **deterministic rule classifier** runs synchronously during ingestion over the filing's ESPI/EBI category label, title, and body. It carries no provider cost and handles the formulaic majority of filings. A confident rule match produces a `confirmed` signal directly.
- Filings the rule classifier cannot place become **`unknown`** and flow to an **async AI fallback** that honors the multi-provider boundary ([ADR 0028](0028-multi-provider-ai-boundary.md)) and is opt-in/disable-able. The AI fallback never auto-commits: it produces a `proposed` signal that requires explicit user confirmation before it becomes `confirmed` (and before any derived event is created). Provider provenance is persisted on the signal.
- An unknown filing is never silently assigned a wrong type; it stays unclassified (or `proposed`) rather than guessed.

### 4. Signals are canonical; calendar events are derived only for dated types

`company_signals` is the source of truth for "what kind of disclosure this is." A `company_events` row is materialized **only** for forward-looking categories that carry a genuine future date (e.g. a dividend record/payment date, a general-meeting date declared in the filing). Past-disclosure signals — insider transaction, profit warning, own-share transaction, significant contract — remain signals and do **not** create calendar events. Derivation is idempotent: re-running ingestion or re-confirming a signal never duplicates the signal or its derived event, and derived events carry origin linkage back to the signal and the originating `feed_item`. **Derivation itself runs in `v0.41.0`** (see Scope boundary): the future date is extracted from the filing body via the report body-fetch path that milestone introduces, so `v0.40.0` only persists the `derived_event_id` wiring.

### 5. Confirmation and provenance policy

- Rule-classified signals are `confirmed` on creation (deterministic, auditable rule).
- AI-classified signals are `proposed` and require user confirmation; on confirmation they become `confirmed` and may derive a calendar event per §4.
- Every signal records `classified_by` and, for AI, the provider/model provenance, so the basis of every typed signal is auditable and reversible.

### 6. Surfacing

Classified signals are surfaced where the investor already looks: type badges and type filters on feed items, type-aware digest grouping (e.g. insider-activity grouping), and reminder hooks for high-signal categories. Surfacing details and copy (en/pl) are specified in [product-spec.md](../product-spec.md) and [ui-flows.md](../ui-flows.md).

Research-workspace integration (the digest/reminder half of surfacing) plugs signals into the existing research-evidence boundary (ADR-tracked M24/M31 model) rather than adding a parallel path:

- **Confirmed** signals become a `company_signal` research **evidence type** in the backend timeline read model (proposed AI signals stay out of research evidence until confirmed). Because the personal digest is generated from collected changed evidence, signals flow into the digest automatically and the digest groups them by type alongside other evidence — no separate digest pipeline.
- A **high-signal** classification (insider transaction, profit warning) generates a research reminder of kind `signal_review` (`source_type = company_signal`), created once when the signal is first classified. Reminder generation is best-effort and never fails ingestion.

## Scope boundary

- In scope (`v0.40.0`): classification of the active official ESPI/EBI feed into `company_signals` (rule classifier + opt-in AI fallback with confirmation), and feed/digest/reminder surfacing.
- **Event derivation deferred to `v0.41.0`** (owner decision at milestone start): materializing a `company_events` row for a dividend/general-meeting signal requires a *future date* that lives only in the filing **body**, not the title. Reliable date extraction depends on the Bankier article/attachment body-fetch path that `v0.41.0` (Report document ingestion & history backfill) introduces. To avoid wrong-dated calendar entries, `v0.40.0` ships signals only; the `company_signals.derived_event_id` column and the §4 derivation contract stay in place as forward-compatible wiring, and derivation runs in `v0.41.0`. This keeps the conservative posture intact (never create a guessed-date event) and is additive — no migration when derivation lands.
- Out of scope: classification of non-official media sources; trading signals or sentiment scoring; ESPI/EBI **attachment ingestion** and **on-track history backfill**, which are part of milestone `v0.41.0` (Report document ingestion & history backfill) because they depend on the Bankier article/attachment fetch path and live verification rather than on classification.

## Consequences

- The feed becomes a typed signal stream while the calendar stays calendar-shaped; past disclosures do not pollute the Events screen.
- A new entity (`company_signals`), a category registry, the rule classifier, an opt-in async AI fallback path, event derivation, and the feed/digest/reminder surfaces are added. The classifier reads the active Bankier feed but is source-neutral for a future GPW re-enable.
- The `status` (`confirmed` | `proposed`) value is additive and forward-compatible with the `v0.49.0` autopilot trust ladder, which adds an auto-confirm provenance state rather than a migration.
- Owner-confirmed decisions (milestone `v0.40.0` start): (a) **event derivation is dividend + general-meeting dates only** — no other category materializes a calendar event, keeping the Events screen calendar-shaped; (b) **the rule classifier is conservative** — only high-confidence formulaic matches produce a `confirmed` rule signal, and any borderline/partial match routes to `unknown` (and thus to confirmation), never to an auto-confirmed guessed type; (c) **the AI fallback is disabled by default** (opt-in) on a fresh install, consistent with the local-first / BYO-key posture, so no provider calls happen until the user enables it and unknown filings stay unclassified until then. The exact per-category rule patterns are still pinned empirically from real Bankier ESPI filings during the classifier task (`64061a4`).
- Related: this is a building block toward the autonomous report pipeline ([roadmap.md](../roadmap.md) North Star); detection of new report publication there can reuse classified signals.
