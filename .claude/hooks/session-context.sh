#!/usr/bin/env bash
# Re-grounds Brawler's always-on rules at session start and after compaction.
# Static text only; the canonical contract lives in CLAUDE.md (ADR 0063).
cat <<'EOF'
Brawler re-grounding (MANDATORY in every session state — new, resumed, compacted): the canonical agent contract is CLAUDE.md (auto-loaded — treat as read) plus docs/engineering-workflow.md (read before acting: test-driven loop + Definition of Done; validate under Nix — host "green" is not a verdict). This is a SPEC-DRIVEN repo: implement to spec, never invent architecture/data shapes/command names.

Three always-on rules (full statements in CLAUDE.md):
1. Token discipline — prefix every shell/file command with `rtk`; repoctx for structural lookups; never bare commands.
2. Doc-first (spec-driven) — read the area's canonical doc before any non-trivial change; update it in the same change; propose + confirm a doc/ADR change when the spec is missing or contradicted.
3. Enforcement is a hard stop — never weaken/skip/--no-verify a failing gate; fix it or surface it.

Git: do NOT commit or push unless the user explicitly asks. Edit, stop, wait.

Load per task: `gh issue view <n>` (active work; `gh issue list` / the "Brawler board" for the board) → only the area's canonical doc(s) from the CLAUDE.md map. UI work → docs/ui-authoring.md first. Epic closure: docs/kanban.md § Epic closure. Packaging → the packaging skill.
EOF
