# ADR 0030: Detail Rail Containment Boundary

Status: Accepted

## Context

The Inbox/company feed detail pane is a fixed-width side rail (one column of a
CSS grid). Over several feature additions it accumulated rich, interactive
content rendered inline: the AI analysis panel (preset prompts, custom-question
textarea, result with source links) and the AI KPI extraction panel (source
buttons with long PDF filenames, a capture-by-URL input row, document and IR
candidate lists, and a per-value proposal review).

This produced a recurring class of layout defect. In CSS intrinsic sizing a
block's content width is the widest descendant's min-content width, so a single
unconstrained element (a long URL, an unbreakable filename, a wide input row)
widened the whole content box and clipped every sibling at the same edge.
Containment was applied per element (`min-width: 0` here, `overflow-wrap` there,
`text-overflow: ellipsis` on one span), so each new descendant was a new chance
to reintroduce the overflow. Fixes alternated between horizontal scroll and a
hard clip without removing the underlying overflow — visible as repeated
"detail pane is clipped / not scrollable" reports, worst at the supported narrow
window range (see the UI scaling requirement in `AGENTS.md`).

This extends ADR 0026 (Reusable UI Foundation), which already calls out
horizontal overflow and long localized labels as regression-prone.

## Decision

The feed detail rail is a containment boundary, not a general content surface.

- The rail hosts compact, read-only summaries and launchers only: title,
  context, summary, actions, attachments, timestamps, a status pill, and — for a
  completed AI analysis — a short significance + summary preview.
- Rich, interactive flows open in the centered `Modal` (a `position: fixed`
  viewport-centered surface, sized independently of the rail), launched by a
  button in the rail. This covers the AI analysis controls/result detail and the
  full AI KPI extraction flow (source selection, capture-by-URL, IR fallback,
  and per-value review/confirmation).
- Containment is a primitive, not a per-element habit. New rail panels render
  inside the `DetailSection` primitive (`src/ui`), and `.detail-pane` establishes
  one subtree-wide contract (`min-width: 0` via a zero-specificity `:where()`
  rule plus default `overflow-wrap`) so no descendant can grow the track. The
  rail scrolls vertically and never horizontally.

## Consequences

- A new wide descendant cannot blow out the rail; the containment guarantee lives
  in one place instead of being re-derived per feature.
- The narrow-window range stays readable; the layout-contract test asserts the
  rail is `overflow-y: auto` / `overflow-x: hidden`, and the browser harness
  asserts no horizontal overflow with a report item selected.
- Heavy flows gain room to lay out (long filenames, multi-step review) in the
  modal instead of fighting a ~300px column.
- The modal becomes the single home for the extraction flow; its prior
  auto-close-on-complete behavior is dropped in favor of an explicit Close, which
  also removes the reopen "blink" failure mode.
- The modal's built-in dismiss control uses the accessible name "Close dialog" so
  it does not collide with a footer "Close" action.
