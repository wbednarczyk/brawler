# ADR 0042: Advisory Verdict Port & Open-Core Decision-Support Boundary (Proposed)

Status: Proposed (draft — planning)

This ADR captures the **design intent** for two related things: (1) extending the versioned quality
scorecard with a valuation-aware dimension, and (2) the boundary between Brawler's **open-core,
decision-support** analysis and an **optional, out-of-band prescriptive advisory layer**.

It builds on:

- [ADR 0041](0041-deterministic-valuation-engine.md) — the valuation outputs that feed the scoring
  dimension and any downstream advisory provider.
- The `v0.45.0` quality-frameworks scorecard (rule engine over fundamentals facts, versioned scorecard,
  clonable templates) — this ADR **extends** that scorecard rather than introducing a parallel one.
- [ADR 0039](0039-ports-and-adapters-posture.md) and [ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)
  — ports-and-adapters and capability contracts; the advisory verdict is modelled as a port with a
  swappable adapter.
- [ADR 0017](0017-license-gate.md) — the local entitlement module that gates optional capabilities.

## Context

The product direction is a decision-making augmentor. A natural next step after computed valuation is a
**prescriptive verdict** (e.g. accumulate/hold/avoid with conviction). However:

- `AGENTS.md` standing rule: **"AI output is decision support only. Do not phrase generated analysis as
  buy/sell/hold advice."** Prescriptive output distributed to other people is also regulated investment
  advice in many jurisdictions. The public open-core product must remain decision-support only.
- A roadmap item ("AI Recommendation Guardrail Enforcement") already plans automated rejection of
  buy/sell/hold language in the public product.
- Any feature whose code **and** logic ship in a public, client-side artifact cannot be robustly
  license-gated — a client-side check is trivially patched. Protecting a runtime flag is not a viable
  strategy.

## Decision (intended)

1. **Scorecard extension (open-core).** Add a **valuation dimension** (fed by ADR 0041) and a
   scenario/upside readout to the existing versioned scorecard. Ship a **functional default rubric**.
   All output stays decision-support framed (scores, dimensions, ranges, "what to watch").
2. **Advisory verdict as a port with an empty public default.** Define an `AdvisoryVerdictProvider`
   port. The public artifact ships **only the port + the entitlement check + a default adapter that
   produces no verdict** (decision-support only). There is no prescriptive logic in the public build to
   unlock; bypassing the entitlement check yields an app looking for an adapter that is not present.
3. **Implementation supplied out-of-band.** A prescriptive verdict adapter is provided outside the
   public artifact — never published in open-core source. (The concrete delivery mechanism — a private
   local module, or a future managed/remote provider under real auth — is an operational detail kept
   out of this public ADR.)
4. **Public posture preserved.** Because the public default emits no prescriptive output, the
   `AGENTS.md` decision-support rule remains literally true for the open-core product, and the planned
   recommendation-guardrail enforcement applies to the public default; the out-of-band adapter is the
   documented exception, not a loophole in the public build.

## Consequences

- The open-core analysis engine (valuation + decision-support scorecard) is fully usable without any
  entitlement; the prescriptive layer is strictly additive and absent by default.
- The guardrail-enforcement work targets the public default surface; tests assert the open-core build
  emits no prescriptive verdict.
- The port is shaped so a future managed/remote advisory provider could plug in without changing the
  open-core seam (would require its own cloud-boundary ADR).

## Open questions

- Exact port shape and the inputs an advisory adapter receives (scores, scenarios, coverage state).
- Test strategy that proves the public default never emits prescriptive output (ties to the guardrail
  item).
- Confirmation that the public default rubric weights are distinct from any private/refined rubric.
