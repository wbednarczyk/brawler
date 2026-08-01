# ADR 0093: Agent acquisition tier and the preliminary-data lifecycle

Status: Proposed (2026-07-31, epic #285 T1; owner decisions ratified at plan approval)

Deciders: maintainer. Area: fundamentals, MCP port, data trust.

## Context

ADR 0084 retired the in-app AI layer with a promise: intelligence returns through the MCP
port, "writing back with mandatory provenance". The port now has 100 tools (ADR 0088/0089),
but the first real acquisition scenario — an external agent reads an issuer's
preliminary-results publication (XTB RB 18/2026, a 2.3 MB PDF) and records what it found —
exposed structural gaps, verified against the code and the maintainer's database:

- An MCP `create_financial_fact` write produces **no provenance row** and defaults
  `extraction_method='manual'`, `confirmation_state='confirmed'`: the agent's figure
  masquerades as an owner-entered fact, sits at the top of the trust ladder (absence of
  provenance *is* the manual-untouchability guarantee, `outranked_stored_tier_of`), and
  permanently blocks the issuer's later audited filing from correcting the slot.
- The mandatory-citation gate (`ProvenanceRequirement::FactCitation`) accepts a non-empty
  `attribution` — but `attribution` is a **slot dimension** (`total | owners_of_parent |
  nci`) hashed into fact identity, and the agent docs teach putting prose there, which
  mints a phantom uniqueness slot instead of recording a citation.
- The agent can neither create the fiscal period (preliminary results precede any period
  row) nor register the document it read (`capture_report_document` unexposed; the fetcher
  has no security gates and a 1 MiB cap).
- `data_quality` — the slot axis designed (ADR 0027, migration 0034) so "estimate and
  final coexist" — has never been used: 100% of production facts are `final`, the
  vocabulary is inconsistent across docs/tests, `supersedes_id` is write-dead, and several
  readers are quality-blind (coverage double-counts; the cross-check prior and the
  plausibility history would silently absorb preliminary values; the UI fact matrix
  last-wins would show a preliminary over its final sibling).

The owner wants this workflow **often** (preliminary releases are GPW standard), so the
answer is a durable acquisition tier, not a one-off import.

## Decision

### 1. `SourceTier::Agent` — a real rung on the trust ladder

A new tier token `agent`, ordered **below every issuer tier** (`esef`,
`structured_xhtml`, `espi_cover_note`, `pdf`) and **above `html_aggregator`**:

- An issuer tier landing on an agent-held slot **upgrades** it in place (value +
  provenance rewritten, upgrade evidence recorded) — exactly the existing
  `Reobserved`/`Upgraded` machinery; a mis-extracted agent figure can never block the
  audited correction.
- The agent tier never overwrites an issuer-held or manual (no-provenance) slot — a
  disagreement is a `Divergent` outcome, reported, never resolved silently.
- The agent tier fills over `html_aggregator` (an agent reads the issuer's own document;
  the aggregator is third-party).
- Manual stays highest, structurally: absence of a provenance row still parses to no tier.

**Witnessing semantics.** Wherever aggregator precedence asks "is the stored tier an
issuer tier?" the operative question becomes "**does the stored tier outrank
`html_aggregator`?**" — the agent tier qualifies: the BiznesRadar pull never overwrites an
agent slot and records `witness_disagreement`/corroboration against it exactly as against
issuer slots. (Rationale: witnessing exists to cross-check *whoever holds the slot with
more authority than the witness*; membership-in-the-issuer-set was an implementation
detail, not the semantic.)

**Honesty rule** (ADR 0088 `classified_by='agent'` lineage): every MCP-originated fact
write carries `source_tier='agent'` provenance, `extraction_method='mcp_agent'`, and the
validation gate's real verdict in `validation_status` (`passed`/`unreviewed`/`flagged`) —
an agent write must never masquerade as `manual`. `confirmation_state` stays `confirmed`
like every automatic writer: it is the frozen compatibility column (ADR 0086 decision 5 —
facts are review-free; origin truth lives in `source_tier` + `extraction_method` +
`validation_status`, never in a confirmation to-do). The UI manual-entry path is untouched
and stays highest.

No migration: `financial_fact_provenance.source_tier` is TEXT without CHECK (0057); the
tier is code-level (`SourceTier` enum + ordering + parse/as_str).

### 2. `data_quality` canonical vocabulary and the preliminary lifecycle

Vocabulary: **`final | preliminary | estimated`** — `final` is audited-or-reported-final
(the default), `preliminary` is issuer-published pre-report figures (wstępne wyniki),
`estimated` is third-party/derived estimates. Enforced by `normalize_data_quality` at the
storage write boundary (the `normalize_currency` pattern): synonyms normalized
(`estimate` → `estimated`, empty → `final`), unknown tokens are a typed error. No CHECK
migration — all 10 984 existing rows are `final`; a write-boundary guard is the
established, cheaper pattern.

Lifecycle:

- Preliminary and final **coexist** in the slot (the UNIQUE index already includes
  `data_quality` — 0034's design, now actually exercised). Every reader prefers `final`
  and counts a slot once; a preliminary-only slot still counts as covered but stays
  distinguishable.
- **Supersession is stamped in the storage write path**, not a background pass: when a
  `final` fact is **created** into a slot whose sibling — same dimensions except
  `data_quality` — is `preliminary` (or, lacking one, `estimated`; issuer-published beats
  third-party estimate), the final fact's `supersedes_id` points at that row. A tier
  *upgrade* of an existing final slot never needs a fresh stamp — the stamp was made at
  the final fact's creation, and a non-final sibling arriving *after* the final row is
  the degenerate late case every reader already handles by preferring `final`. This revives the write-dead column with race-free semantics (the write
  that creates the successor is the only place that knows both rows). Rejected: a
  startup/post-extraction sweep (raceable, and a second bookkeeping pass for something the
  write already knows).
- The UI shows a preliminary badge; editing a preliminary fact preserves its quality
  (today the controller silently promotes to `final`).

### 3. Cumulative-only recording for GPW interim publications

GPW interim reporting is cumulative (no Q2/Q4 period rows exist — ADR 0092/data-model
window semantics), but preliminary publications print **both** discrete-quarter and
cumulative columns side by side (XTB: Q2 net profit 492.2 M next to H1 1 027.2 M — a
1-column mistake writes a half-year figure ~2× off). Rule: **agents record cumulative
columns only** (H1/9M/FY), into the cumulative period, default `measure_window='flow'`;
discrete-quarter columns are skipped — they are derivable by span arithmetic (the ADR
0086-era TTM machinery already does this). Rejected for now: extending the
`measure_window` vocabulary with a discrete-quarter token — real demand may revive it via
the roadmap, but this epic ships the smallest surface that kills the trap.

### 4. KPI minting by agents

Agents may create KPI definitions for issuer-characteristic metrics the catalog lacks
(broker client counts, CFD lots, net deposits…): **company-scoped** (`scope='company'`),
snake_case ASCII `metric_key`, and a durable origin marker — `kpi_definitions.origin`
(`seed | user | agent`, append-only migration) — so agent-minted definitions are
reviewable and the #272 characteristic-KPI UI can surface them honestly. Minted
definitions are extras, never completeness-denominator entries (kpi_relevance stays
governed by ADR 0092's layers).

### 5. Capture security posture

`capture_report_document` becomes an exposed MCP act so the agent can register the
document it read — behind fetcher gates that land in the same change: **https-only**,
DNS-resolved address must not be private/loopback/link-local (SSRF guard), content-type
allowlist (`application/pdf`, `text/html`, `application/xhtml+xml`), **30 MiB** cap on
this path (the 1 MiB default rejects real preliminary PDFs). Rejected: a per-company
IR-domain allowlist — issuer IR hosting is unbounded and the URL is recorded on the
document row for audit; the gates above bound the damage class (internal-network reach,
oversized/foreign payloads) without a maintenance treadmill.

### 6. Batch write with per-fact verdicts

A preliminary release is a table, not a fact: the port gains one batch act
(`record_financial_facts`) over the battle-tested `record_structured_fact` primitive —
period ensured (deterministic `finper_` ids), definitions resolved by `metric_key`,
document reference FK-checked, **citation required per fact**, the reusable validation
service (identity checks, history plausibility with explicit thin-history abstention,
completeness) run before commit, and **typed per-fact outcomes** (`created | reobserved |
upgraded | divergent | no_definition | implausible`) — a divergence is reported to the
agent, never silently resolved. `create_financial_period` stays unexposed: the batch tool
ensures periods as a side effect of a cited write, which is the only period-creation an
agent needs.

## Consequences

- The trust ladder gains a rung whose entire purpose is *correctability*: agent speed now,
  issuer authority later, with the takeover recorded. Divergences between the two become
  visible workflow (outcome rows), not silent state.
- Preliminary figures stop being unrepresentable: the owner sees them badged the day they
  are published and watches them get superseded when the audited report lands.
- The attribution-gate defect closes (citation ≠ slot dimension), and the existing
  `create_financial_fact`/`update_financial_fact` MCP path adopts the same honest
  provenance — the "agent fact masquerading as manual" hole is shut.
- The agent-facing docs (brawler-mcp skill + wiki guide) carry the extraction ritual (unit
  scaling, parenthesized negatives, the cumulative-only rule, capture-then-cite) — the
  doctrine is versioned with the repo, not folklore in a chat log.
- Epic #285 implements this ADR; the acceptance is a real agent session ingesting XTB RB
  18/2026 into the real app (ADR 0088 dogfooding ritual).
