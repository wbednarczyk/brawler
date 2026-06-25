# ADR 0057: Composable Views — dockview Re-Styled, the "+" New-View Model, Curated Company Dashboard

Status: Accepted (2026-06-25)

Amends [ADR 0053](0053-dockview-layout-pilot.md) (dockview scope) and [ADR 0054](0054-mode-based-thesis-centric-shell.md) (the sectioned tabbed Company workspace). Relates to [ADR 0056](0056-per-company-settings-surface.md) (Companies → library + management) and the `cockpit_layouts` persistence shipped with ADR 0053.

## Context

In real owner use the **cockpit in its current form is unusable** — not because freeform/dockview is wrong, but because the *implementation* is heavy: each panel carries a large header/chrome, and the default is a blank/directionless grid, so it goes unused. ADR 0054 reacted to this by demoting the cockpit to an opt-in "advanced layout" behind a **sectioned, click-through tabbed Company workspace** (Overview/Fundamentals/Quality/Claims/Notebook/Transcripts/Metadata).

Two things became clear in design discussion (2026-06-25):

- **The tabbed workspace doesn't earn its keep.** Click-through tabs show one section at a time; once panels exist, maintaining a second way to view a company is incoherent. The maintainer wants **one view that tells them everything about a company**.
- **Freeform is wanted — done right.** dockview itself is excellent (ref: a friend's 6502 IDE on dockview — modular, configurable, **minimal panel chrome**: a thin tab header = name + close). The fix is **lightweight, composable** views, not retiring dockview. (An earlier "retire the cockpit" conclusion was an overshoot and is rejected.)

## Decision

**dockview is retained as the single view engine** and re-shaped into a lightweight, composable model. The click-through tabbed workspace is retired in favor of a **curated company dashboard** (itself a dockview view), and users can build their own **named views**.

### 1. Minimal panel chrome

Re-skin dockview panels to a **minimal header** — tab name + close, thin bar, reduced padding — like the reference IDE. Heavy per-panel chrome is the single biggest reason the cockpit felt unusable; slimming it is the highest-leverage change and lands first.

### 2. The "+" new-view model

A **"+" entry in the Modes sidebar group** (same size as a nav item, below the built-in modes) creates a **new named view**:

1. **Name it** (required) up front.
2. **Pick a grid:** quick visual presets **2×2 / 2×3 / 3×3** (clickable mini-grid icons) **or a custom A×B** — column/row **sliders linked bidirectionally to number inputs** (drag updates the field, typing updates the slider) with a **live grid preview**.
3. An **empty grid** appears; **fill cells from a panel palette** of pre-built panels.
4. Later, add / remove / move / resize panels freely (dockview).

A named view is persisted as a **`cockpit_layout`** (existing table — versioned `panels_json`/`layout_json`, import/export) and appears as a **nav destination in Modes**. The persistence backbone already exists; this is primarily a frontend reshape on top of it.

### 3. Curated company dashboard (replaces the tabbed workspace)

Opening a company (from the Companies/Library, a pinned sidebar entry, or a Today/Pulse "Review") lands a **curated default dashboard** — a **seeded `cockpit_layout` scoped to the company** with a calm, opinionated starting set of panels (e.g. Overview focus + fundamentals + what-changed + claims), **not** a blank grid and **not** click-through tabs. It is the "one view that tells you everything," and it stays composable (add/remove/move panels). This is the curated, guided default ADR 0054's research asked for — now delivered *as* a dockview view rather than as tabs.

### 4. Companies → Library + management

The Companies screen's primary role becomes **browse + manage**: the company list/search plus the per-company settings management surface ([ADR 0056](0056-per-company-settings-surface.md)). The deep-dive is the dashboard (decision 3), not a tabbed panel inside Companies.

### 5. The "Kokpit" nav entry

The current standalone "Kokpit" mode is **replaced by this composable-views model** — user-created named views (decision 2) are the cockpit reborn. There is no separate blank-canvas "Cockpit" destination; the entry point is the "+" (create a view) and the saved named views it produces.

## Consequences

- **Mostly frontend.** The dockview engine, `cockpit_layouts` persistence (versioned, import/export), the selection store, and pop-out already exist (ADR 0053). This ADR reshapes the UX on top: minimal chrome, the "+" creation flow, the grid-picker, the panel palette, the seeded company dashboard, and the Companies demotion.
- The **click-through tabbed Company workspace is retired**; its sections become panels available in the palette / the curated dashboard.
- Build order: **(1) minimal panel chrome** (immediate, visible win) → (2) "+" new-view creation flow (name + grid presets/custom A×B + palette) → (3) curated company dashboard as the company entry point → (4) Companies → library + management, retire tabs.
- Narrow-window constraint still applies — grids and panels must degrade/stack in tall-narrow windows (Playwright viewport matrix).

## Status notes

Accepted 2026-06-25 after a design discussion that (a) rejected an overshoot to "retire the cockpit" — dockview is kept — and (b) converged on lightweight composable views: minimal chrome, a "+" named-view creator with grid presets + custom A×B, a curated company dashboard replacing the tabs, and Companies demoted to a library + settings-management surface. Amends ADR 0053 (dockview is now the one view engine, re-styled) and ADR 0054 (sectioned tabbed workspace → curated dashboard).

### Implementation status (2026-06-25)

- **Decision 1 (minimal chrome)** — done.
- **Decision 2 ("+" new-view creator)** — done: the `+` in the Modes group opens `CreateViewModal` (grid presets + slider↔input custom A×B + live preview), saves an empty named `cockpit_layout`, and activates it on open. *Saved named views are not yet listed as standalone nav destinations in Modes* (the remaining half of this decision) — tracked for follow-up.
- **Decision 3 (curated company dashboard)** — done: opening a company (Companies library / pinned spine / feed item / global search / Today) lands the cockpit scoped to it, loading `dashboard:<companyId>` or seeding the curated default (Fundamentals, Feed, Claims, Quality, Report documents, Notebook). `Feed` and `Notebook` shipped as new self-contained company-scoped panels (`companyFeed`/`companyNotebook`, reusing the extracted `CompanyFeedSection`/`CompanyNotebookSection` with cockpit-owned controllers). `Save dashboard` persists per company.
- **Decision 4 (Companies → library)** — done: the tabbed `CompanyWorkspace` is deleted; Companies is the library + the per-company settings surface. Company metadata folded into the library; Transcripts (a placeholder) dropped.
- **Decision 5 (replace the standalone "Kokpit" nav entry)** — **mostly done.** Saved named views now appear as nav destinations in the Modes group (loaded in `AppStateRoot`, kept in sync via `onLayoutsChanged`); clicking one opens the cockpit with that layout activated. An empty view shows an `Add panel` prompt, and the toolbar carries an explicit `Add panel` button (the palette also lists every `Open panel: …`). *Remaining:* optionally retire the standalone blank "Cockpit" nav button now that views are reachable directly — kept for now as the cockpit render target / first-run entry.
