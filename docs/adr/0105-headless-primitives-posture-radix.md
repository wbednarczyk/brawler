# ADR 0105: Headless Primitives Posture — Radix, Narrowly, Inside `src/ui` Only

Status: Accepted (2026-08-19, owner verdict after the plain-language review — the earlier same-day "Accepted at spike completion" was a process error, harvested into engineering-workflow §A: a delegated study's ADR stays Proposed until the owner rules on the outcome)

Deciders: maintainer. Area: frontend, dependencies, accessibility.

## Context

The F0 foundations review asked whether hand-rolled interactive primitives should adopt a
headless a11y library. Timeboxed spike evidence (worktree prototype, 2026-08-19):

- **`src/ui/Modal.tsx` has a real gap**: initial focus + Esc + restore exist, but there is **no
  focus trap** — Tab walks out of an open dialog into the background page.
- **`CommandPalette.tsx` carries zero ARIA semantics** (no combobox/listbox roles) — a second
  real gap, owned by F3a where the palette is redesigned anyway.
- **Native `<select>` (SelectField) is correct as-is** — platform accessibility beats any library
  reimplementation; menus/popovers barely exist in today's UI but v2 designs will add them.
- **Prototype**: `@radix-ui/react-dialog@1.1.23` wrapped inside `Modal.tsx` preserving the public
  API and all CSS classes — typecheck green, `Modal.test.tsx` + primitives a11y suite green with
  **zero test edits**, net −24 lines (outside-click/Esc/restore/trap all come native). React
  19.2.8 peer-clean. Footprint: the small `@radix-ui` transitive tree (single-line package.json
  entry). One documented tension: Radix auto-wires `aria-labelledby` to `Dialog.Title`, which
  outranks our authoritative `ariaLabel` prop — fixed by an explicit
  `aria-labelledby={undefined}` override on `Dialog.Content` (keep the code comment at the site).

## Decisions

1. **Adopt narrowly: Radix headless primitives are allowed ONLY inside `src/ui` wrappers.**
   Screens and shared components never import `@radix-ui/*` directly — the primitive-first
   contract (ADR 0037) stays the single vocabulary, and each primitive has exactly one a11y
   implementation. Gate: ESLint `no-restricted-imports` for `@radix-ui/*` outside `src/ui/`
   (ships with the first consumer PR).
2. **Enumerated retrofit list: `Modal` only.** The prototype's rewrite lands with the first v2
   PR that touches modal behavior (candidate: F1 #413). No other existing primitive has an
   evidenced gap worth the churn.
3. **Adopt-for-new**: when a v2 design introduces a popover, dropdown menu, tooltip or similar
   focus-managing surface, its `src/ui` primitive wraps the Radix counterpart from day one
   (gallery + a11y test per the existing contract) instead of hand-rolling focus/dismiss logic.
4. **Explicit non-adoptions**: `Select` stays native; the command palette's semantics are an F3a
   design decision (hand-rolled ARIA combobox vs a library) made with the palette redesign, not
   pre-empted here.

## Rejected

- **Full retrofit of `src/ui`** — one evidenced gap does not justify rewriting healthy primitives.
- **Rejecting headless entirely** — re-hand-rolling focus traps for the popovers v2 will add is
  the expensive path, and the spike shows the wrap costs less code than it removes.
- **Component libraries with styling opinions** (MUI/Mantine/shadcn-as-styled) — collide with the
  token system and the primitive contract; headless-only keeps ADR 0076 sovereign.
