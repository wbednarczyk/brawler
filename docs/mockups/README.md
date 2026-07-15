# Approved UI mockups

Owner-approved HTML mockups for new panels/redesigns (mockup-first rule,
[ui-authoring.md](../ui-authoring.md)). Commit the approved mockup here BEFORE
implementation; the file is the normative scope record for the task.

## Storyboards (ADR 0081)

For non-mechanical UI work (new panel/screen, functional redesign, changed
cross-screen journey, or new primary user decision — copy/token-only fixes and
primitive-preserving migrations are exempt unless they change a journey), the visual
half of the experience contract is a **storyboard**: copy
[`STORYBOARD-TEMPLATE.html`](STORYBOARD-TEMPLATE.html) to `<task>-storyboard.html` and
fill its 7 required frames (entry, before action, loading/in-flight, success, error,
undo/recovery, narrow pane). It uses the same self-contained HTML convention as every
mockup here — no runtime dependency, opens standalone in a browser.

**Storage and approval:** the completed storyboard is committed here (never left only
in a session scratchpad or `test-results/`) and reviewed by the owner alongside the
textual experience contract (`docs/plans/EXPERIENCE-CONTRACT-TEMPLATE.md`, filled in
under the task's plan section) **before** it is normative — a copied template is a
draft, not an approval. See [ui-authoring.md](../ui-authoring.md) § Experience
contracts, storyboards & discoverability for the full trigger/exemption/approval flow.
