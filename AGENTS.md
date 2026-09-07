# Brawler agent contract (pointer)

Canonical contract: [CLAUDE.md](CLAUDE.md) ([ADR 0063](docs/adr/0063-claude-native-context-architecture.md)). Read it first, then `docs/engineering-workflow.md`, then `.claude/skills/investing-domain/SKILL.md` (who the user is — required for any product/UI/analysis judgment).

Codex peers (astra/sol, `astra-peer` skill): same rules — `rtk` prefix, `repoctx` for structure, spec-driven (never invent shapes/commands), read-only unless a worktree is assigned, no git checkout/restore/stash/commit. Repo skills: `.claude/skills/*/SKILL.md`.
