#!/usr/bin/env bash
# Re-surfaces Brawler's standing rules into agent context at session start and,
# critically, after compaction — so doc-first discipline survives context loss.
# Static text only (no repo commands); the canonical rules live in AGENTS.md + ADR 0031.
cat <<'EOF'
Brawler standing rules (AGENTS.md is imported every session — treat it as read at all times):

1. Token discipline: prefix EVERY shell/file command with `rtk` (rtk git/grep/read/ls/cargo/npm/rad ...). Use `repoctx` for structural lookups. Never bare commands; never `rtk proxy` for normal work.

2. Doc-first (spec-driven repo): before any non-trivial change, OPEN and READ the canonical doc for the area and implement to spec; update that doc in the same change. Never invent or guess architecture, data shapes, field/command names, scopes, or error codes — they are specified. If a spec is missing/ambiguous/contradicted, propose a doc or ADR change and confirm it instead of silently choosing.

Single source of truth: roadmap.md = active + upcoming plan only; CHANGELOG.md + kanban-archive.md = delivered history; Radicle/Radboard (`rad issue list --all`) = live epic/task status; contracts / data-model / product-spec / ui-flows / ui-information-architecture / architecture+ADRs / source-strategy / engineering-workflow / modularization-design each own their domain. See AGENTS.md "Two Always-On Rules" and "Single Source Of Truth", and ADR 0031.

Git: do NOT commit or push unless the user explicitly asks, or you are running the brawler-release skill. Otherwise edit files and stop.
EOF
