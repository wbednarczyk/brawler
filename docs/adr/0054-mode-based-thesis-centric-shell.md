# ADR 0054: Mode-Based, Thesis-Centric Application Shell

Status: Accepted (2026-06-23)

> **Update (2026-07-11, v0.52.0 audit):** two reality notes. (1) **Drift recorded:** in the delivered shell, dockview is the *default* pane host of the cockpit (`DockLayout` renders unconditionally as the cockpit body) — not an "opt-in advanced layout"; no default-vs-advanced switch exists. The framing below is amended to match reality: dockview is the cockpit's pane engine, load-bearing by default. (2) The never-validated **OS-window pop-out** path (dockview `addPopoutGroup` via `window.open` + the `core:webview:allow-create-webview-window` capability) is **removed** in `v0.52.0` ([ADR 0080](0080-retire-embedding-model.md) decision 5); in-app floating groups stay. A future OS pop-out returns only with the Tauri `WebviewWindow` validation sub-spike ADR 0053 called for.

Amends [ADR 0053](0053-dockview-layout-pilot.md) (dockview re-scoped from the app-wide grid to the workspace engine of two modes — see its Revision note) and continues the supersession of [ADR 0047](0047-top-navigation-bar.md). Relates to: [ADR 0044](0044-report-season-cockpit.md) (report season), [ADR 0040](0040-management-claims-tracker.md) (claims), [ADR 0043](0043-investment-thesis-and-decision-journal.md) (thesis + decision journal), [ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md) (decision-support boundary), the v0.48 command palette + `SemanticSearch`, and the v0.49 autonomous report pipeline (North Star). Roadmap: [roadmap.md](../roadmap.md).

## Context

Brawler is becoming far more than an investor inbox. The roadmap composes toward a **decision-support augmentor**: the autonomous report pipeline (North Star, v0.49) detects a new report, extracts it, says *what changed*, and cross-references it against open claims / questions / evidence; the valuation & decision arc (v0.53–v0.58) adds cross-company comparison, deterministic valuation, quality scoring, an investment **thesis workbench + decision journal**, and a **living thesis fed by the newsfeed**. That is an enormous, growing capability surface.

[ADR 0053](0053-dockview-layout-pilot.md) adopted dockview as the **app-wide shell** — one freeform docking canvas, every screen a co-equal dockable panel, replacing the top-nav. In real owner use on Windows the result feels **overwhelming and directionless** (maintainer feedback, 2026-06-23): a generic 2×2 grid with no focal point, full screens crammed into small cells, flat visual hierarchy, and the app's actual superpowers (what-changed, claims-to-verify, AI analysis, linked selection) buried in panels/tabs/modals with no narrative.

Diagnosis (UX + investor lens):

- **A freeform canvas is a power-user tool.** It offloads *layout design* onto the user and answers "what can I arrange," not "what should I do." Professional terminals get away with dense freeform layouts because their users are trained and have muscle memory; a non-professional individual investor needs the shell to **lead**.
- **The investor's jobs have different shapes.** Daily *triage* wants a fast, opinionated, glanceable flow. *Deep-dive on one company* genuinely wants many panels side by side. *Reading a long report diff* or *writing a thesis* wants full-attention focus. *Comparison* wants a structured grid. One generic grid serves all of them poorly.
- **The product is still inbox-centric** (a chronological feed is the hero) while the North Star and the living thesis imply the real hero is **the company and the conviction state of your thesis**, with the feed as *input* to that thesis.

dockview itself is the right engine; it is **over-applied** as the whole shell.

## Decision

Adopt a **mode-based, thesis-centric application shell**: the app is organized around a few **modes** matched to the investor's jobs, each with its own ideal shape and a strong opinionated default, with customization as opt-in. The organizing unit shifts from "feed items" to **companies and their thesis/conviction state**; the feed becomes an input stream feeding that state.

### 1. Four modes (not one freeform grid)

1. **🏠 Today / Pulse — the home.** A **narrative briefing**, not a grid. It answers "what needs my attention?" by leading with the app's superpowers: *what changed* (new reports/diffs, autonomous-pipeline output), *items to verify* (due claims), *stale theses* (living-thesis signals), upcoming report dates (report season), and a per-company **conviction/health** overview. The chronological feed is present but secondary. This is the home of the North Star "one notification."
2. **🔬 Company workspace — deep-dive on ONE company.** Presented by default as **fixed, modular, progressively-disclosed sections** (fundamentals/KPIs, quality scorecard, valuation, report diff, claims, notebook, thesis) — a stable hierarchy the user *navigates*, not a blank grid they *assemble* (the dominant retail-research pattern; see Evidence). dockview's free arrangement is the **opt-in advanced layout** within this mode, with **task presets** (e.g. *Earnings*, *Valuation*, *Thesis review*) as the named starting points. Linked selection is implicit — everything is *this* company — which removes the "what is linked to what" confusion of an app-wide canvas.
3. **⚖️ Compare — cross-company.** A structured comparison grid (v0.53): canonical KPIs across watchlist peers, period-aligned, unit-normalized, every value linked to evidence; extends to the screener/leaderboard (v0.58).
4. **📖 Focus modes — reader/writer.** Full-screen, distraction-free surfaces invoked from anywhere for deep reading (a long report-over-report diff) and long-form writing (a thesis or notebook note), with `Esc` back to the denser workspace.

Navigation is a **persistent left-sidebar IA spine** with named sections plus **pinned/favorite companies** for progressive disclosure (the dominant retail-research pattern — Koyfin), with the four modes as its top-level destinations. The screens that exist today become content *inside* a mode (e.g. Watchlists/Research/Events/Report-season surface within Today and the workspace), not co-equal flat nav items. A blank/freeform workspace is never the entry point.

### 2. Thesis-centric information architecture

The durable spine of the product is the **company + its thesis/conviction state**, surfaced as a glanceable **per-company composite** (the Simply-Wall-St "Snowflake" model: one multi-axis visual rolled up from a fixed set of automated checks) that **decomposes into a few named factors** (the Ensemble-Capital model: e.g. moat / management / forecastability, each on a small fixed scale, averaged to one conviction readout), plus a **watchlist-level rollup** (the "Portfolio Health" pattern). The feed, signals, report diffs, and events are **inputs** that move that status (the living-thesis model of [ADR 0043](0043-investment-thesis-and-decision-journal.md) / v0.57). Today/Pulse and the Company workspace both render this status; it is the thing that makes a large feature set legible. It stays evidence-linked and decision-support framed — never a prescriptive rating ([ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md)).

### 3. Command palette + global semantic search complement the sidebar spine

The sidebar IA spine (decision 1) is the **primary** navigator. The v0.48 command palette and `SemanticSearch` are a **fast complement** for power users — jump to any company/mode/action, "compare X vs Y", "verify claims", "value X", "find by meaning across everything" — and the global search field ("ask/search anything") is a first-class session entry point, not decoration. They make power reachable without *replacing* the visible spine (the research did not support a command-palette-as-sole-spine; visible sidebar IA is the load-bearing pattern, with the palette as an accelerator). Avoid pro-terminal command-line / ticker-code navigation that assumes trained-user muscle memory.

### 4. dockview is re-scoped to an opt-in advanced layout, not removed

dockview becomes the **opt-in "advanced workspace"** within the **Company workspace** and **Compare** modes — where free multi-panel arrangement earns its keep for a power user — *behind* a curated sectioned default, never as the entry point. It keeps the full investment already made (panels, task presets, saved layouts in `cockpit_layouts`, pop-out, the `CockpitSelectionContext` engine, accessible chrome). It is **not** the app-wide shell, **not** the home, and **not** the default even within a mode. Any cross-entity panel linking must be **explicit and named** (the Bloomberg-Launchpad "Groups" pattern), never implicit/global. [ADR 0053](0053-dockview-layout-pilot.md) is amended accordingly (see its Revision note).

### 5. Design principles (the compass for every future epic)

- **Lead with the job, not the canvas.** Strong opinionated defaults per task; customization opt-in.
- **Progressive disclosure.** Glance (Today) → drill (workspace) → deep work (Focus).
- **Make the magic legible.** "What changed", "to verify", "thesis stale", conviction status are narrative, first-class surfaces — never buried.
- **Design for density, don't cram.** Earn density through typography, scannable rows, and reduced panel chrome — not by shrinking full screens into cells.
- **Every new capability has an obvious home** in a mode (see mapping below), so the shell does not accrete panels.

## How planned features map (cross-assessment)

| Milestone / feature | Home mode |
| --- | --- |
| v0.48 feed triage + command palette + `SemanticSearch` | spine + Today/Pulse triage |
| v0.49 autonomous report pipeline (North Star, "what changed") | **Today/Pulse is designed around this** |
| v0.50 quality (qualitative) | Company workspace |
| v0.51 re-invent notebook | Focus/Writer + Company workspace |
| v0.53 cross-company KPI comparison | Compare |
| v0.54 deterministic valuation | Company workspace + Compare |
| v0.55 valuation-aware scoring | Company workspace status |
| v0.56 thesis workbench + decision journal | Focus/Writer + a Journal surface |
| v0.57 living thesis (newsfeed-as-input) | **Today/Pulse "stale thesis" + per-company conviction status** |
| v0.58 watchlist screener / leaderboard | Compare |

Without this model each epic bolts another panel onto the grid and the shell grows more crowded and confusing. With it, each epic slots into a mode.

## Evidence (UX research, 2026-06-23)

A cited deep-research pass (27 sources → 117 claims → 25 adversarially verified, 20 confirmed / 5 refuted) across professional terminals, modern retail-research apps, and IDE/PKM tools converged on this direction and sharpened it:

- **Lead with a visible IA spine + curated defaults; freeform is opt-in.** Modern retail tools default to a **left-sidebar of named sections with pinned Favorites**; their widget canvas is *contained within* the spine and offered as blank *or* template — Koyfin ([nav](https://www.koyfin.com/help/release-notes/customizable-left-navigation/), [dashboards](https://www.koyfin.com/help/mydashboards-myd/)). Pro terminals ship **20+ task/instrument-shaped preset layouts**; blank-canvas is the deliberate opt-in — IBKR TWS ([library](https://www.interactivebrokers.com/en/trading/tws-workspace-layout-library.php)). Even the most freeform pro shell (Bloomberg **Launchpad**) ships pre-canned sector Views and links panels only via **explicit named Groups**, not implicit global linking ([guide](https://my.lerner.udel.edu/wp-content/uploads/BB-Getting-Started-in-Launchpad.pdf)).
- **Single-stock deep-dive = fixed modular sections, not a canvas.** Simply Wall St navigates stable tabs (Overview/Valuation/Future/Past/Health/Dividend/Management/Ownership) with progressive disclosure ([model](https://github.com/SimplyWallSt/Company-Analysis-Model)).
- **Conviction = one glanceable composite + per-factor + portfolio rollup.** SWS "Snowflake" rolls 30 binary checks into a 5-axis radar ([how it works](https://support.simplywall.st/hc/en-us/articles/360001740916-How-does-the-Snowflake-work)); Ensemble Capital decomposes conviction into named factors on a 0–3 scale averaged to one score ([method](https://intrinsicinvesting.com/2019/12/12/position-sizing-how-we-assess-conviction/)); Stock Unlock ships a portfolio-level "Health" rollup ([site](https://stockunlock.com/)).
- **Attention home = a "Triage" queue.** Linear's **Triage** is a populated review inbox of newly-arrived items needing an explicit accept/decline/snooze decision before entering the workflow — the model for "what changed / to verify / stale" ([docs](https://linear.app/docs/triage)).
- **Thesis tools remember "why I invested" and keep it current** ([usethesis](https://www.usethesis.com/)).

**Avoid (explicitly):** dropping users into an empty freeform canvas; implicit/global panel linking; trained-user command-line/ticker navigation.

**Guardrails from refuted claims (do not over-claim):** the canvas is not *strictly* gated behind a library in pro tools (blank-canvas coexists); Koyfin dashboards are a widget grid within a spine, not a docking canvas; SWS "Narratives" is **not** confirmed as the thesis mechanism; do not model thesis-keeping primarily around discrete events.

**Under-evidenced (reason from analogs, research before building):** command-palette-as-spine specifics; focus/reader/writer modes (lean on VS Code Zen Mode, Obsidian/iA-Writer focus, NN/g progressive-disclosure); how investor tools surface "what changed" on the home beyond the Linear-Triage analog; and — for a *single-user local-first* app — how much dockview freedom and custom-preset saving to expose (cited tools are multi-user/cloud and may over-invest in customization).

## Consequences

- The cockpit epic ([ADR 0053](0053-dockview-layout-pilot.md), Radicle `0077edd`) is **re-scoped**: phases already built (selection store, layout persistence, command palette, per-panel wiring, the section-gated-data fix) carry forward into the Company-workspace mode rather than an app-wide grid. The "cockpit as default shell + slimmed nav" already shipped becomes the interim state while the mode-based shell is designed and built.
- Wireframes for each mode accompany this ADR (the maintainer-facing mockups, 2026-06-23); they are direction, not final visual design.
- A follow-up will land the mode shell incrementally (Today/Pulse first, then Company-workspace re-scope, then Compare), each keeping the app working — consistent with the gradual-migration posture of [ADR 0053](0053-dockview-layout-pilot.md).
- Decision-support boundary unchanged: the conviction/thesis status is decision *support*, never prescriptive advice ([ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md)).

## Risks and mitigations

- **Big direction change mid-epic.** Mitigated by re-scoping (not discarding) the dockview work and shipping the mode shell gradually.
- **Modes could fragment navigation.** Mitigated by the command palette + global search spine making every destination reachable from one place.
- **"Conviction status" risks looking like a rating/advice.** Keep it a transparent, evidence-linked composition of facts (claims/valuation/what-changed), framed as decision support, with the recommendation guardrail in mind.
- **Accepted after the UX research pass** (terminals + modern retail-investor research apps + IDE/PKM mode-vs-freeform evidence) was **integrated** (see Evidence) and strengthened the "lead with curated defaults; freeform opt-in" position. A sequenced implementation plan (sidebar spine → Triage home → sectioned company workspace with dockview opt-in → conviction status → Compare → Focus) is tracked in Radicle under epic `0077edd`.

## Status notes

Accepted 2026-06-23 after maintainer sign-off; the UX research pass is integrated (see Evidence), [ADR 0053](0053-dockview-layout-pilot.md) is amended, and the direction is propagated into [roadmap.md](../roadmap.md) and [ui-information-architecture.md](../ui-information-architecture.md). The shell is built incrementally (sidebar spine → Triage home → sectioned company workspace with dockview opt-in → conviction status → Compare → Focus), each step keeping the app working — consistent with the gradual-migration posture of [ADR 0053](0053-dockview-layout-pilot.md).

**Implementation status (2026-06-24, uncommitted working tree).** Four of the six steps have landed and pass the full gate (`make check`: frontend + Rust): (1) **left-sidebar IA spine + pinned companies** — sidebar groups (Modes/Library/Utilities) + pinned/favorite companies persisted in `UserSettings.pinnedCompanyIds` (settings KV, no migration); (2) **Today/Pulse attention home**, now the **default landing** — what-changed / to-verify (claims for pinned companies) / upcoming reports / conviction rollup placeholder / secondary feed, each with a Review action; (3) **sectioned Company workspace + dockview opt-in** — an *Advanced layout* button opens the dockview cockpit scoped to the company (all cockpit work carries forward); (6) **Focus reader/writer** — a reusable full-screen `FocusOverlay` (Esc to exit) for the report-diff (reader) and notebook notes (writer). **Deferred by maintainer decision (2026-06-24):** (4) **per-company conviction status** and (5) **Compare** stay shell-level placeholders/scaffolds (a neutral conviction dot + a Compare destination that names what's coming) rather than partial builds, because the real composite depends on **valuation (v0.54) / quality (v0.50) / thesis (v0.56)** and the real KPI comparison is **v0.53** — building them fully when those land avoids a partial conviction that could read as a rating (the [ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md) guardrail) and an empty Compare. The shell hooks are in place for both.
