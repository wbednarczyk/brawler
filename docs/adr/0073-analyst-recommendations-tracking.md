# ADR 0073: Analyst Recommendations Tracking

Status: Accepted

Sell-side recommendations (ratings, target prices, consensus shifts) are third-party opinions with market impact; tracking them — strictly as attributed opinions, never as advice — adds context the feed and valuation views lack. This ADR executes the roadmap's "Analyst Recommendations Usage" study (owner study 2026-07-02) per its own decision criteria: an ADR + source/data-model proposal before code.

## Context

- Revision history is the valuable part (who raised/lowered, when, from what to what) and — like the decision journal (ADR 0071) — it cannot be backfilled: it accumulates only from when ingestion starts.
- Candidate access paths: Bankier/BiznesRadar publish recommendation items; some brokerages expose RSS. Each undergoes the standard source-policy review; no scraping beyond policy without a follow-up source decision (ADR 0014 precedent). Paid consensus feeds stay out (roadmap's deferred analyst-consensus adapter note stands for aggregate consensus data).
- The decision-support rule is the hard constraint: the app may display *that firm X said Y about company Z on date D* — it may never adopt, aggregate into, or phrase its own actionable stance.

## Decision

1. **`analyst_recommendations` entity**: company, issuing firm, analyst (optional), rating before/after (source vocabulary preserved verbatim + a coarse normalized direction: upgrade/downgrade/initiate/reiterate), target price before/after with currency, publication date, source attribution + URL, provenance. Append-only history.
2. **Ingestion** via a dedicated adapter per approved source (implementing the ADR 0069 `Fetcher` trait), matched to companies by ticker/ISIN through the existing registry matching.
3. **Consensus-shift signal**: a typed signal on new/changed recommendations (extends ADR 0034 taxonomy) so revisions appear in the feed, Today home, digests, and alert rules (ADR 0068).
4. **Presentation as third-party opinion**: a distinct company-workspace section, always naming firm + date, visually separated from the app's own factual data; a "vs target price" readout may appear beside market data (ADR 0067) as attribution-carrying context. No blending into scorecards or valuation outputs in the open core.
5. **Follow-on (not this milestone)**: per-firm historical accuracy tracking — recorded as a candidate, needs price history (ADR 0067 ✓) plus careful framing.

## Consequences

- The roadmap's Future Study section is resolved into scheduled scope (v0.58).
- Source-strategy gains the recommendation-source review entries; each concrete source is enabled only after its policy check passes.
- Decision-support enforcement note: recommendation *display* is exempt from the future AI-recommendation-language guardrail (it quotes attributed third parties), while app-generated text about them is not.
