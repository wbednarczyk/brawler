# Implementation Plans

Per-milestone execution plans authored at planning time (by the owner + the strongest planning model) so implementation sessions can run on a cheaper model without re-deriving design. **Non-normative**: the norm lives in ADRs and the canonical docs (contracts, data-model, product-spec, ui-flows) — if a plan contradicts them, the ADR/doc wins and the plan must be fixed. Live task status lives in Radicle (cards reference plan sections), not here.

## How an implementing agent uses a plan

1. Ground first, always: `CLAUDE.md` (auto-loaded) + `docs/engineering-workflow.md` (test-driven loop, Definition of Done). Then `rad issue show <epic-hex7>` and the plan file named by the card.
2. Work **one task (§T*n*) at a time**, in order unless the plan says tasks are independent. Each task states scope, files, contracts/shapes, the tests that must redden, and acceptance criteria — implement to that, test-first (`superpowers:test-driven-development` where available).
3. **Doc-first still applies**: update the canonical doc(s) the task names in the same change. `make check` green under Nix before claiming a task done; DoD §A/§B/§C boxes per change type.
4. **Tripwires are hard stops.** Every plan lists `STOP-AND-ASK` conditions — reality contradicting a pre-decided shape, a missing spec, a gate you'd be tempted to weaken. Stop, surface to the owner, do not improvise architecture. A defect flagged along the way follows the guardrail-harvest loop.
5. **Plan drift**: when implementation legitimately diverges (better name, extra column), update the plan file in the same change and note it in the task's Radicle card. When it diverges from an **ADR**, stop — propose the ADR change first.
6. Never commit/push unattended; never mark cards solved or close a milestone without explicit owner sign-off. Milestone closure runs DoD §I (spec-conformance audit, journeys check, retro with a UX section) and the `brawler-release` skill — closure review is owner + strongest-model territory, not the implementing session's call.

## Kickoff prompt (owner template)

Paste into a fresh implementation session (one session = one §T task, `/clear` between tasks; fill the two placeholders):

```
Implementujesz zaplanowany task milestone'u wg kontraktu tego repo.

Task: <hex7 karty> — v0.5X T<n>
Plan: docs/plans/<plik>.md — przeczytaj README planów i sekcję §T<n>, potem rad issue show <hex7>.

Zasady tej sesji:
- Wykonujesz TYLKO ten task, dokładnie wg planu. Plan pre-decyduje design — nie projektujesz alternatyw. Konflikt planu z ADR/canonical docs albo trafienie w tripwire = STOP i raport do mnie.
- Test-first; make check zielony pod Nix przed zgłoszeniem "done"; doc-first w tej samej zmianie.
- Bez commit/push. Na koniec: raport co zweryfikowane i jak, co NIE, i czy plan wymagał korekty (jeśli tak — zaktualizuj plik planu i odnotuj w karcie).
```

Operating model: the owner orchestrates (card → session → review), the implementing model works solo (subagents only for read-only exploration and post-task `/code-review`), the strongest model handles gating decisions, spike verdicts, and milestone closure (DoD §I). Don't paste plan content into the prompt — the file is the single copy.

## Experience contracts (ADR 0081)

Non-mechanical UI work — a new panel/screen, functional redesign, changed
cross-screen journey, or new primary user decision — gets an **experience contract**
before component work: copy [`EXPERIENCE-CONTRACT-TEMPLATE.md`](EXPERIENCE-CONTRACT-TEMPLATE.md)'s
11 sections into the task's plan section and fill every field (any `N/A` needs a
written reason). Pair it with a copy of
[`docs/mockups/STORYBOARD-TEMPLATE.html`](../mockups/STORYBOARD-TEMPLATE.html) for the
visual storyboard. Copy/token-only fixes, primitive-preserving mechanical migrations,
and exact regression repairs are exempt unless they change a journey. The textual
contract lives in the plan file (this directory); the storyboard lives under
`docs/mockups/` — never only in a session scratchpad or `test-results/`. Both need
explicit owner approval before they are normative. Full trigger/exemption/approval
flow: [ui-authoring.md](../ui-authoring.md) § Experience contracts, storyboards &
discoverability.

## Files

- Cross-cutting, ready to implement after the active `v0.52` worktree closes:
  [`ux-quality-loop-v2.md`](ux-quality-loop-v2.md) — experience contracts,
  adversarial frontend scenarios, expanded journey evidence, continuous dogfooding,
  and the J1/J2 pilot (Radicle epic `bd1a6af`).
- Detailed (ready to implement): `v0.52` (active milestone: judgment + MCP MVP + cleanup) and `v0.53` … `v0.56` — execution order stays dependency-driven (roadmap order); the heavy-but-ADR-frozen v0.55/v0.56 were planned in full up front deliberately.
- Skeletons (need a planning session before start): `v0.57` — do **not** start implementation from a skeleton.
- Parked: `v0.67-import-export-v2-offsite-backup.md` (deferred 2026-07-11, schema stability first — re-validate at take-up).
