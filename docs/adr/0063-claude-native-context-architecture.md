# ADR 0063: Claude-Native Context Architecture and Lean-Docs Layering

Status: Accepted

Brawler's agent-facing documentation moves from an agent-neutral `AGENTS.md` contract to a **Claude-native context architecture** (`CLAUDE.md` + `.claude/skills/` + session hooks), with an explicit layering policy and enforced byte budgets that keep the always-loaded context lean without weakening what is enforced at session start and after compaction.

## Context

Every session paid a fixed ~80 KB (~20k-token) mandatory-context tax: `AGENTS.md` (34.6 KB, imported by `CLAUDE.md` each session) + `docs/engineering-workflow.md` (42.8 KB, mandatory read) + the session hook + `CLAUDE.md`. An audit (2026-07-02) found the core rules (rtk discipline, doc-first, the ADR 0062 gate, nix-vs-host) restated 6–10× across that stack, ~9 KB of packaging mechanics inside the mandatory-read workflow doc, triple-stated command inventories, scaffold-era aspirational sections, and five drift bugs (knip/Playwright still described as "opt-in/periodic" after ADR 0062 made them mandatory). The spec-docs corpus (342 KB) carried ~55–65 KB of milestone narration, investigation logs, retired sections, and entity rules duplicated between `contracts.md` and `data-model.md`. Duplication is what causes drift: several of the drift bugs sat precisely in the duplicated statements.

The maintainer decided to adopt the Claude Code ecosystem natively (CLAUDE.md, on-demand skills, hooks, plugins) rather than maintain the agent-neutral abstraction.

## Decision

### 1. Context layers with enforced budgets

| Layer | Content | Budget (gate-enforced) |
| --- | --- | --- |
| L0 always-loaded | `CLAUDE.md` (canonical agent contract: Three Always-On Rules, SSOT map, working-rules digest, pointer index) + `.claude/hooks/session-context.sh` | CLAUDE.md ≤ 18 KB, hook ≤ 2.5 KB, `AGENTS.md` stub ≤ 1 KB |
| L1 mandatory read | `docs/engineering-workflow.md` (TDD loop, Definition of Done, day-to-day loop, one command-reference table) | ≤ 26 KB |
| L2 on-demand skills | `.claude/skills/{brawler-release,guardrail-harvest,packaging}/SKILL.md` — loaded via frontmatter description only when relevant | — |
| L3 spec docs | contracts / data-model / product-spec / ui-* / source-strategy / roadmap — targeted reads | — |

`scripts/check/gate-integrity.mjs` enforces the L0/L1 byte budgets and enforcement-parity markers (see Decision 4). Budgets have headroom; raising one is a deliberate, reviewed act, not a workaround.

### 2. `CLAUDE.md` is canonical; `AGENTS.md` is a stub

The full agent contract lives in `CLAUDE.md` (auto-loaded by Claude Code — one hop less than the previous `@AGENTS.md` import, same guarantee). `AGENTS.md` remains as a ≤ 1 KB stub pointing to `CLAUDE.md`, so historical ADR links and other-tool conventions do not rot. Repository-owned agent workflows live in `.claude/skills/*/SKILL.md`; `.agents/skills/*` remain as one-line stubs and `.codex/` is removed.

### 3. Lean-docs layering policy (every fact has one home)

- **Rule = one canonical home + pointers.** A rule is stated in full exactly once; every other mention is a one-line pointer. The session hook is the only permitted short restatement (its job is compaction survival).
- **History routing:** content whose removal would change how it is permissible to build next (rationale, rejected options, evidence for a decision — e.g. source studies) → an **ADR** (normative). Pure execution chronicle ("what we shipped in M24") → **CHANGELOG/kanban-archive** (never normative).
- **War stories** in canonical docs collapse to one rule sentence + a link to the ADR/testing.md entry that owns the narrative.
- **Future scope** lives in `roadmap.md` only; **deferred/not-in-v1** has one home: roadmap *Not In V1*. Other docs point.
- **Zero-delete:** slimming routes content to its one home; only duplicates and stale/scaffold-era statements are cut outright.

### 4. Enforcement parity

Everything enforced at session start/resume/clear/compact before this ADR stays enforced after it:

| Enforced today | Before | After |
| --- | --- | --- |
| Agent contract in context every session | `CLAUDE.md` `@AGENTS.md` import | `CLAUDE.md` is the contract (auto-load) |
| Re-grounding after compaction | hook on startup/resume/clear/compact | same matchers, slimmer text |
| Three Always-On Rules (rtk / doc-first / enforcement) | AGENTS.md + hook restatement | CLAUDE.md + hook restatement |
| Mandatory engineering-workflow.md read | AGENTS.md + hook | CLAUDE.md + hook |
| Commit/push prohibition | AGENTS.md + hook | CLAUDE.md + hook |
| SSOT map / doc load order | AGENTS.md + hook (2×) | CLAUDE.md (1×), hook points |

Gate-integrity asserts the parity markers: the hook contains `rtk`, `CLAUDE.md`, `engineering-workflow.md`, `spec-driven`; `CLAUDE.md` contains the Three Always-On Rules, Single Source Of Truth, and Required Reading sections; `.claude/settings.json` registers the hook on all four matchers.

## Consequences

- The always-loaded stack drops from ~80 KB to ~45 KB; on-demand content (release, packaging, guardrail-harvest mechanics) costs nothing until invoked.
- Other agents (Codex etc.) reading `AGENTS.md` by convention get a pointer, not the contract. This is accepted: the project standardizes on Claude Code.
- The "Standing Agent Guidance" agent-neutral posture in the old AGENTS.md is superseded; durable rules still live in the repo (CLAUDE.md/ADRs), not in private memory — that principle is unchanged.
- Doc slimming per this policy is tracked as a two-slice epic (hot path, then spec docs); the audit's findings are the work list.
- Spec↔code drift enforcement is specified separately in [ADR 0065](0065-spec-code-drift-gates.md).
