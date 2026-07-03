# ADR 0065: Spec↔Code Drift Gates and the Planned-Section Convention

Status: Accepted

Spec-driven development is only real if the spec provably matches the code. This ADR makes the machine-checkable subset of that promise a hard-fail step of `make check` (`scripts/check/docs-drift.mjs`), and legalizes spec-ahead-of-code via an explicit tag instead of silent divergence.

## Context

A 2026-07-03 drift audit found six live spec↔code divergences in `docs/contracts.md`: the Investor Week Calendar section (ADR 0058, targeted at v0.59.0) documented six commands as if they existed at v0.48.0, `update_feed_item` had been renamed `update_feed_item_state` in code only, and `list_jobs` never existed. No automated check compared docs to code; doc updates relied on per-change discipline (DoD §A), which multiple agents demonstrably skipped. Per [ADR 0038](0038-enforcement-as-guardrails.md), a practice that matters must be a gate, not a convention. A parallel usage audit (25 local session transcripts) confirmed the canonical docs are heavily read by agents — so rot in them actively misleads.

## Decision

1. **`docs-drift` gate** — a hard-fail `make check` step verifying, at minimum:
   - **Commands, two-way**: every command bulleted under a `Commands:` list in `contracts.md` exists as a `#[tauri::command]` (unless its section is tagged planned), and every `#[tauri::command]` in code is mentioned in `contracts.md`.
   - **Screens**: every navigation destination in `src/app/navigation.ts` is represented in `ui-information-architecture.md`.
   - **Settings keys**: every settings key in `src-tauri/src/storage/settings.rs` is documented in `data-model.md` § Settings.
   - **ADR hygiene**: every `docs/adr/*.md` carries a `Status:` line; `docs/adr/INDEX.md` (generated via `node scripts/check/docs-drift.mjs --write-adr-index`) stays in sync.
   - Parsers carry sanity floors (e.g. ≥150 extracted commands) so a silently broken heuristic fails the gate instead of passing vacuously.
2. **Planned-section convention.** A spec section describing not-yet-built scope must carry `Status: planned (vX.Y.Z, ADR NNNN)` on its first lines. The gate skips planned sections' doc→code checks, and **fails when a planned section's commands appear in code** — delivering the scope forces removing the tag, so the spec never claims more *or less* than reality in either direction.
3. **Structural mappings, not per-name exceptions.** When a legitimate pattern trips the checker (e.g. a screen documented as an App Shell bullet rather than a `##` heading), the heuristic or a named structural mapping in the script header is refined; silencing individual drift findings is prohibited (ADR 0038 corollary).
4. **Evidence-based retire policy.** A canonical doc that transcript-usage evidence shows agents never load, and whose content is superseded by ADRs, is retired — deleted outright when nothing live links to it, its historical text routed to the kanban-archive (first case: `ai-analysis-framework.md`). Legal/user-facing artifacts (`dependency-licenses.md`, `wiki/`) are exempt from agent-usage criteria.

## Considered and deferred

- **DB tables ↔ data-model gate**: data-model sections are deliberately grouped (not 1:1 with tables) and the audit's sample found no drift; revisit if a table-level drift instance ever surfaces.
- **Persistent docs-usage telemetry**: local transcript retention is ~3 weeks; a periodic ad-hoc audit (the method used here) suffices.

## Consequences

- Six drift instances are fixed in the same change; the Investor Week section becomes the first tagged planned section.
- Older ADRs receive missing `Status:` lines; supersessions become greppable and INDEX-visible.
- Renaming a command, adding a screen, or adding a settings key now reddens `make check` until the spec is updated — doc-first stops depending on agent memory.
