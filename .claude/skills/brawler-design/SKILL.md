---
name: brawler-design
description: Use before ANY Brawler UI work — writing/changing JSX, drawing a mockup, styleguide or visual-language work, choosing colors/typography/labels, designing a panel or empty state. Binds the generic design skills to Brawler's normative design language (ADR 0104) and authoring contract.
---

# Brawler design ritual

Thin by design: rules live in the canonical docs; this skill is the load order, the precedence,
and the pre-write ritual. Never restate rule content here — link it (single source of truth).

## Load order (before the first line of JSX or mockup HTML)

1. [ADR 0104](../../../docs/adr/0104-frontend-v2-design-language.md) — the design language
   (accent discipline + form rule, typography, verb dictionary, patterns, provenance thread).
2. [docs/ui-authoring.md](../../../docs/ui-authoring.md) — the authoring contract (primitive-first,
   mockup-first, density contracts, i18n, its v2 checklist section).
3. Visual reference: `docs/mockups/frontend-v2-styleguide/` (approved canvas; gitignored).
4. For charts/tiles: the `dataviz` skill. For aesthetic judgment: the `frontend-design` skill.
   For owner mockup rounds: the `design` skill (canvas) — the approved result is saved back to
   `docs/mockups/` (owner rule 2026-08-18).

## Precedence (on any conflict)

Repo beats generic skill: `src/ui` primitives and ADR 0076 tokens/scales beat whatever a generic
design skill would invent; ADR 0104's dictionary beats any label instinct; product language
(no dev vocabulary) beats cleverness. A generic skill's advice is welcome exactly where the repo
is silent.

## Pre-write ritual (every time)

- Verb check: does every action label start with a dictionary verb (ADR 0104 dec. 3) and predict
  its effect? Run the swap test.
- Color check: is every non-neutral color a MEANING (interaction-filled / provenance-quiet /
  status)? Count filled elements at rest: max one.
- Copy check: user vocabulary only; empty states carry three beats (what → where from → action).
- Structure check: ui-authoring pre-write self-check (primitives, scaffold-from-sibling, no inline
  styles, `text()` both languages, density contract for the pane).
- Figures in mono; human title before filename; provenance thread only under theses.

## Mockup workflow (new panel/screen or redesign)

Mockup BEFORE code (binding): draw on the `design` canvas → owner round(s) → approved artboards
copied to `docs/mockups/<feature>/` → then experience contract/storyboard if the work touches a
journey (ADR 0081 posture per plan) → only then components.
