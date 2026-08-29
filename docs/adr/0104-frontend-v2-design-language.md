# ADR 0104: Frontend V2 Design Language

Status: Accepted (2026-08-19, owner approval of styleguide round 1; epic #410/F0 #411)

Deciders: maintainer. Area: frontend, UI authoring, design system.

## Context

The F0 UX audit (epic #410) found the recurring defects are systemic, not per-screen: three
saturated accents competing on every screen, no single verb vocabulary ("Zastosuj/Pokaż/Otwórz"
for one action, two buttons starting with "Read…"), dev vocabulary in user copy, empty states
with no next action, filenames as primary titles, a detail pane that repeats the list instead of
adding context. The owner approved a design language that fixes the classes once. Approved
styleguide mockup: `docs/mockups/frontend-v2-styleguide/` (canvas + five artboards).

## Decisions

1. **Accent discipline — color is meaning, never decoration.** The night-neon palette's hues are
   unchanged (identity); their *right of use* changes: cyan `--primary` is the ONLY interface
   accent, in two sanctioned FORMS — **filled cyan = interaction** (primary action, focus, link,
   selection) and **quiet cyan** (tinted chip, dotted thread, source ticket) = the
   `--tone-official` provenance meaning it already carries today. A filled provenance mark or a
   quiet primary action are both defects. Magenta `--secondary` appears solely as `--tone-media`;
   violet `--accent` solely as `--tone-agent` — always in the quiet form. Status tones
   (positive/caution/negative) are semantics, exempt from the accent budget, and never decorate.
   Rule of one glow: at rest, at most one FILLED element per screen (the primary action); quiet
   forms are outside the budget.
2. **Typography: Schibsted Grotesk (UI + headings) and JetBrains Mono (every figure, period id,
   eyebrow, key).** Inter retires. Both faces OFL, bundled locally as woff2 with a PL+EN subset —
   no CDN at runtime (dependency decision; files land in the F0.5 repaint PR). The ADR 0076 type
   scale (10–20 px) and spacing scale are unchanged; mono carries `--line-numeric` contexts, so
   digit columns align by construction.
3. **Verb dictionary (PL/EN), enforced.** Eight verbs cover the app: Otwórz (navigate), Zastosuj
   (apply a preset; persists nothing), Zapisz (persist under a name), Pobierz (fetch something new
   from outside), Przeczytaj (turn a stored report into numbers), Odśwież (ask a source now),
   Oznacz jako… (state change), Dodaj/Usuń (collections). Labels start with the verb and are never
   full sentences. Banned: two verbs for one action, icon without a label or tooltip, English in
   Polish UI, system vocabulary in copy. Enforcement: a command-palette copy test (lands with F3a)
   plus the swap test in review — if two buttons could exchange labels and still "fit", the
   dictionary is broken.
4. **Pattern: an empty state is an invitation** — three beats: what this is → where it comes
   from → one action button. A bare "no data" sentence is a defect (audit class D).
5. **Pattern: detail shrinks to its content** — a detail pane never repeats what the list row
   already said, and its depth varies by item kind; freed space carries company context
   (specified per-screen, first in F1 #413). Closes audit class B.
6. **Pattern: human title first** — a document row leads with the human name ("Raport
   skonsolidowany Q3 2025"); the filename is metadata in mono. Closes audit class G.
7. **Signature: the provenance thread (nić pochodzenia).** A thesis figure or quoted claim sits
   on a dotted underline ending in a source ticket (document · channel · page/time); thread color
   = provenance tone (official cyan / media magenta / agent violet). Only under theses, never
   under auxiliary numbers; a thread must navigate to its source or it is a defect. This is the
   one element the app is remembered by, and it renders the product's core promise — every number
   traces to its source — visible.

### Amendment (owner dogfooding 2026-08-27)

- **Decision 2 — figures/dates/percentages render in the UI face, not mono.** JetBrains Mono spaces punctuation ("15 , 2 mld PLN", "27 . 08 . 2026"), which reads as broken. Every figure/date/percent context uses Schibsted Grotesk with `font-variant-numeric: lining-nums` (the `.num-tabular` class now sets this explicitly); mono stays reserved for identifiers, keys, period ids, eyebrows, and filenames. Proportional lining figures, not tabular: Schibsted's tabular set widens "."/"," to a full digit cell ("1 . 1 B PLN"); right-aligned numeric cells still line up at their end.
- **Decision 3 — destination labels are nouns, not verbs.** The workshop bar and core-card buttons that open a tool (`Tezy`/`Claims`, `Fundamenty`/`Fundamentals`, …) are **destinations**, styled like nav items — noun only, no leading "Otwórz"/"Open". The verb-prefixed form stays exclusively for the ⌘K palette's command entries (`SPOLKA_TOOL_COMMANDS`), where "Otwórz X" names the ACTION a command performs. A destination button and a palette command for the same tool may legitimately carry different labels.

### Amendment (2026-08-28, F4a S1)

- **Decision 3 — five verbs added for the Library screens.** `create` (Create/Utwórz), `rename` (Rename/Zmień nazwę), `pause`/`resume` (Pause/Wstrzymaj, Resume/Wznów) join the dictionary (`src/shared/verbs.ts`). `remove` (Remove/Usuń) is the **only** collection-removal verb across screen copy — the legacy EN key `Delete` is retired from screen copy (destructive confirms keep their own irreversibility framing per ADR 0076 D5, but the button/action label reads "Remove"). `ActionButton` (`src/ui/ActionButton.tsx`) carries `verb: Verb` or `kind: "destination" | "control"` and emits `data-action-kind`/`data-action-verb` so a screen's rendered action inventory is mechanically checkable (F4a contract, `docs/plans/frontend-v2-f4a.md`).

## Foundations review (question-everything doctrine, owner 2026-08-19)

Every foundation gets an explicit verdict; "keep" also needs evidence. Gated studies block their
dependent work.

| Foundation | Verdict | Evidence / gate |
| --- | --- | --- |
| Token/theme system (ADR 0076) | keep, evolved by this ADR | scales survive audit contact; only usage rules change |
| Typography (Inter) | **replace** (dec. 2) | generic face; figures need mono alignment |
| Docking framework (dockview 6) | **study #414** | gates F3b/#216; requirements derive from F3a's model |
| Screen data layer (app-root pattern) | **study #415** | gates F1 implementation; AppStateRoot at ratchet pin |
| Component primitives (`src/ui`) | keep; Radix spike pending (F0) | primitive contract + gallery + a11y suite hold; spike decides headless adoption posture |
| i18n (`text()` + ratchet) | keep | translation guard + completeness ratchet enforce it; audit found copy defects, not mechanism defects |
| React 19 + Tauri shell | keep | no evidence against; replacement cost unjustified |
| IA: left nav + Dziś/Inbox/Dashboard split | open question | F2 (Dziś thesis) and F3a (panel model) must answer it explicitly in their mockups |

## Consequences

- The global repaint (tokens usage, fonts) ships as the dedicated F0.5 PR (#412) with the full
  visual-baseline sweep; screens then migrate per sub-epic.
- Every F1+ mockup and PR is reviewed against decisions 1–7; the styleguide mockup is the visual
  reference, this ADR the normative text.
- ui-authoring.md carries the day-to-day checklist pointer to this ADR.

## Rejected

- **Tailwind migration** — the token system plus stylelint gates already enforce discipline;
  churn without benefit. **Storybook** — PrimitiveGallery + the visual harness cover it.
- **New palette hues** — the identity is good; the defect was usage discipline, not hue choice.
- **Copy tweaks per screen without a dictionary** — treats class A's instances, not the class.
