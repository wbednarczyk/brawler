#!/usr/bin/env bash
# Re-surfaces Brawler's standing rules into agent context at session start and,
# critically, after compaction — so doc-first discipline survives context loss.
# Static text only (no repo commands); the canonical rules live in AGENTS.md + ADR 0031.
cat <<'EOF'
Brawler standing rules. MANDATORY every session state — new, resumed, compacted, large-context, any other — above all other docs: read AGENTS.md AND docs/engineering-workflow.md. This is a SPEC-DRIVEN project: implement to spec, never invent. (AGENTS.md is imported every session — treat it as read at all times; docs/engineering-workflow.md carries the build/test/toolchain/validation discipline + Definition of Done, e.g. validate under nix — host "green" is not a verdict.)

1. Token discipline: prefix EVERY shell/file command with `rtk` (rtk git/grep/read/ls/cargo/npm/rad ...). Use `repoctx` for structural lookups. Never bare commands; never `rtk proxy` for normal work.

2. Doc-first (spec-driven repo): before any non-trivial change, OPEN and READ the canonical doc for the area and implement to spec; update that doc in the same change. Never invent or guess architecture, data shapes, field/command names, scopes, or error codes — they are specified. If a spec is missing/ambiguous/contradicted, propose a doc or ADR change and confirm it instead of silently choosing.

Single source of truth: roadmap.md = active + upcoming plan only; CHANGELOG.md + kanban-archive.md = delivered history; Radicle/Radboard (`rad issue list --all`) = live epic/task status; contracts / data-model / product-spec / ui-flows / ui-information-architecture / architecture+ADRs / source-strategy / engineering-workflow / modularization-design each own their domain. See AGENTS.md "Three Always-On Rules" and "Single Source Of Truth", and ADR 0031.

Git: do NOT commit or push unless the user explicitly asks, or you are running the brawler-release skill. Otherwise edit files and stop.

Doc load order — read as the task requires (the two MANDATORY docs above come first):
1. docs/project-brief.md + docs/project-practices.md — product intent + operating rules.
2. rad issue show <hex7> — the active epic/task and its recent comments (docs/kanban.md is the Radicle pointer + label conventions; `rad issue list --all` for the full board).
3. Only the canonical doc(s) for the area being touched: contracts.md · data-model.md · product-spec.md · ui-flows.md · ui-information-architecture.md · architecture.md + docs/adr/* · source-strategy.md · modularization-design.md.
   Building/editing UI → docs/ui-authoring.md (primitive-first, ADR 0037) first.
   Milestone/release closure → .agents/skills/brawler-release.md.
EOF
