# ADR 0031: Spec-Driven Documentation Discipline

Status: Accepted

## Context

Brawler is run as a spec-driven project: the docs and contracts define intent before implementation, and `AGENTS.md` states this. In practice agents (including the assistant) repeatedly:

- implemented from the code alone without consulting the canonical doc for the area, and so invented values the spec already fixed (e.g. querying KPI definitions with a non-existent `scope:"global"` when the data-model/contracts specify `canonical`/`sector`/`company`);
- let docs drift (a fundamentals tab missing from ui-flows/ui-information-architecture; the roadmap mixing completed and future milestones);
- duplicated the same information across files (the milestone/epic plan lived in both `roadmap.md` and `kanban.md`, the latter growing large and disagreeing with the former).

`AGENTS.md` is already loaded into context every session (project `CLAUDE.md` imports it) and already lists the area→doc map. The failures were therefore behavioral and structural, not a lack of information: nothing forced consultation, and several facts had more than one home, so they drifted.

## Decision

Make documentation discipline structural rather than dependent on agent attention.

1. **Doc-first is an always-on rule.** `AGENTS.md` carries two always-on rules at the top — token discipline and doc-first — stated as applying in every session. Before any non-trivial change, the agent opens and reads the canonical doc(s) for the area, implements to spec, and updates those doc(s) in the same change. No inventing or guessing architecture, scope, data shapes, names, or error codes; if a spec is missing/ambiguous/contradicted, propose a doc/ADR change and confirm it rather than silently choosing.

2. **Single source of truth.** Every fact has exactly one canonical home (enumerated in `AGENTS.md` → Required Reading → Single Source Of Truth). Notably:
   - `docs/roadmap.md` is forward-looking only (active + upcoming milestones + unscheduled future); it does not restate delivered work.
   - Delivered/release history lives in `CHANGELOG.md` (authoritative per-version) and `docs/kanban-archive.md` (completed-card detail).
   - Live epic/task status and IDs live in Radicle/Radboard; `docs/kanban.md` is only the thin pointer + label conventions, with no milestone narrative or epic list.
   - Commands→contracts, data shapes→data-model, behavior→product-spec, flows/IA→ui-flows/ui-information-architecture, boundaries/decisions→architecture/ADRs, source policy→source-strategy, build/test→engineering-workflow, modules→modularization-design.

3. **Planning updates docs.** Every epic/milestone planning step updates all relevant docs (and adds/updates an ADR for durable decisions) as part of completing the planning, before implementation.

4. **Deterministic enforcement (planned).** Agent attention is not a control. A Claude Code `SessionStart`/`PreToolUse` hook should re-surface the two always-on rules and the doc map at session start and before edits, so the discipline survives context compaction and new sessions. The hook is the enforcement layer; `AGENTS.md` remains the agent-neutral contract that also binds non-Claude agents.

## Consequences

- Less drift: deduplicating roadmap/kanban and fixing single ownership removes the places where the same fact could disagree. `roadmap.md` shrank to the forward plan; `kanban.md` shrank to the Radicle pointer.
- Bugs caused by guessing specified values are prevented at the source by reading the canonical doc first.
- The docs stay loadable: a smaller forward-looking roadmap and a thin kanban are cheap to read in a fresh or compacted context.
- Some history moved out of `roadmap.md`; it remains recoverable in git history and is captured in `CHANGELOG.md`, the ADRs, and `kanban-archive.md`.
- The enforcement hook is Claude-Code-specific; the agent-neutral guarantees live in `AGENTS.md` so Codex/ChatGPT and other agents share them.
