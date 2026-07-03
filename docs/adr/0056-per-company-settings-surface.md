# ADR 0056: Per-Company Settings Surface (Master-Detail, Scalable)

Status: Accepted (2026-06-25)

Relates to: [ADR 0054](0054-mode-based-thesis-centric-shell.md) (mode-based shell; Company workspace), [ADR 0055](0055-autonomous-report-pipeline-trust-ladder.md) (per-company autopilot mode — the first setting that needed this), [ADR 0037](0037-ui-component-framework-and-authoring-contract.md) (primitive-first UI). IA propagated to [ui-information-architecture.md](../ui-information-architecture.md) and [ui-flows.md](../ui-flows.md).

## Context

Per-company settings have accumulated and are **scattered** across the UI: the autopilot trust-ladder mode (Fundamentals, [ADR 0055](0055-autonomous-report-pipeline-trust-ladder.md)), pinned/favorite (sidebar), watchlist membership (add/remove flows), the IR reports page URL (Fundamentals). A user who wants to set automation across several companies has no single, fast place to do it; the v0.49 autopilot in particular shipped with only a single-company toggle and **no way to manage it across companies** (the gap that prompted this).

The obvious fix — a **companies × settings grid** (one column per setting) — does not scale. At 5–10 settings it overflows horizontally, cramps columns, and directly violates Brawler's hard constraint of remaining usable in **tall, narrow windows (~960–1280px effective width)** ([AGENTS.md](../../AGENTS.md) Testing Expectations / the ultrawide-quarter case): a wide grid is exactly the layout that breaks there. Adding settings must not make the surface worse.

## Decision

Adopt a **master-detail per-company settings surface** as the scalable home for *all* per-company settings, entered as a **"Manage settings" mode within the Companies screen**:

- **Master (left):** the company list with **multi-select** (checkboxes), plus "select all" and **select-by-watchlist** (scope a bulk change to one watchlist's members). This realizes the per-company **and** watchlist-scope bulk requirement.
- **Detail (right):** a settings panel for the **current selection**, organized into **named groups** (e.g. *Automation* → autopilot; *Organization* → pinned, watchlists; *Sources* → IR URL). Changing a control applies to **every selected company**. When a setting differs across the selection it shows a **mixed/"—" indicator** and only writes on an explicit change.

**Why master-detail over a grid:** it **grows vertically, not horizontally** — a new setting is another grouped row in the detail panel, never another column — so it scales to many settings and **collapses to a single column in narrow windows** like the rest of the app. Grouping keeps it legible as settings accumulate. This is the durable pattern: **future per-company settings are added as groups/rows here, not as new scattered controls or grid columns.**

**Progressive disclosure (optional):** the master list may show **1–2 highest-value settings inline** (e.g. an autopilot badge + pin state) for at-a-glance scanning; the full set lives in the detail panel so the list never becomes a wide grid.

**The in-context single-company controls stay.** The Fundamentals autopilot toggle and IR URL field remain for quick edits while looking at one company; the management surface is the *bulk/cross-company* home. Both write the same per-company state.

### v1 scope

The surface ships with the **Automation group → autopilot mode** (bulk set across the selection) and **watchlist-scope selection** (select all of a watchlist's members, then apply) — the urgent need that prompted this. **Pinned and watchlist-membership editing are the immediate next groups** the surface is built to take (they reuse `onTogglePinnedCompany` / the watchlist add-remove commands; pinned needs `pinnedCompanyIds` threaded into this surface). IR URL stays a per-row/Fundamentals text field (text, not a quick toggle). Backend for autopilot: a bulk read (`list_company_autopilot_modes`) and a bulk write (`set_companies_autopilot(companyIds, mode)`). A mixed value across the selection renders as "—" and only writes on an explicit choice.

## Consequences

- New Companies-screen mode (master-detail), primitive-first ([ADR 0037](0037-ui-component-framework-and-authoring-contract.md)); narrow-window verified against the Playwright viewport matrix.
- New bulk autopilot commands (contracts/data-model updated); pinned/watchlist reuse existing contracts.
- The scattered single-company controls remain as quick-edit affordances; this is the cross-company home.
- **Standing rule:** a new per-company setting is added as a **group/row in this surface** (and may add an inline badge), not as a new scattered control — keeping the management surface the one place and preserving narrow-window scalability.

## Alternatives rejected

- **Companies × settings grid:** does not scale past a few settings; overflows/cramps in tall-narrow windows (the hard constraint). Rejected as the primary surface.
- **Per-setting views only** (one screen per setting): scales in settings but loses the "configure everything for one company / a selection" view; kept only as the in-context single-company affordances.
- **Modal:** a pop-up works for autopilot-only but is too small for a growing, grouped, multi-select surface; the Companies-screen mode gives room and is the natural home.

## Status notes

Accepted 2026-06-25 after a design discussion that surfaced the scalability concern (what happens at 5–10 settings) and the tall-narrow-window constraint. Master-detail chosen for vertical scalability + narrow-window friendliness. Built incrementally; v1 = autopilot + pinned + watchlists.
