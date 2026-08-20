# ADR 0106: Screen Data Layer — Thin Self-Fetch Hooks + Composed Read Models, No Query-Cache Library

Status: Accepted (2026-08-20, owner verdict on the F1-pre study #415; gates F1 implementation, not F1's mockup)

Deciders: maintainer. Area: frontend, architecture, dependencies.

## Context

Today every screen's data flows through one root: `AppStateRoot.tsx` (2227 lines, pinned by the
file-size ratchet; already named a "concentration point" by ADR 0050). `useAppDataController`
fetches app-wide on startup and on refresh (companies, watchlists, feed items, an **unfiltered**
`list_company_signals`, …); screens consume via 9 `createScreenContext` providers whose value
objects are rebuilt unmemoized on every root render. F1's company-context block (notes / claims
due / fresh facts / open questions / report documents per selected feed item) is exactly the load
that either stretches this pattern further or justifies per-screen queries.

Study evidence (2026-08-20):

- **No cache/dedup/query layer exists anywhere** (`src/api` is 1:1 `callCommand` wrappers over
  `invoke`); the only prior art is `useAttentionController`'s bespoke poll-diff-skip hook.
- **No backend→frontend event channel exists** (`listen`/`emit` unused). Invalidation today is:
  the 15s scheduler-mirror poll (allowlisted refetches, ADR 0055), manual `app.refreshDatabase`,
  and local `refresh*` calls after mutations.
- **Per-screen self-fetch is an established pattern already**: `FundamentalsPanel` (4 commands
  keyed on `companyId`), `ReportDiffPanel`, `useCockpitRedFlags`/`useCockpitShortPositions`,
  `useMorningBriefing` — screens/widgets own `useEffect` fetches without the root.
- **Composed backend read models are an established pattern**: `get_company_health`,
  `get_red_flags`, `list_company_timeline` — one command returns a server-computed DTO instead of
  the frontend fanning out N calls.
- **Measured cost of the F1 context-block reads on the maintainer's real database** (124 MB
  snapshot, WSL, sqlite3 `.timer`): every representative per-company query (recent notebook
  entries, open claims, recent facts w/ period join, report documents, research questions,
  company events) runs in **≤2 ms**; the 500-row feed join in ~1 ms. Caveat: this measures the
  SQLite read, not the full Tauri IPC round-trip (serialization + webview bridge, typically
  sub-ms to low-ms for payloads this size); an end-to-end in-app measurement can piggyback on the
  J1 baseline session.
- **Query-cache libraries priced** (web survey 2026-08): TanStack Query v5 has a ~10.4 KB gzip
  irreducible floor (QueryClient not tree-shakeable) and its differentiator is a staleness model
  built for *network uncertainty* — polling/revalidate-on-focus because the client cannot know
  when server data changed. Brawler's backend has ground truth (it knows exactly when a row
  changed), so time-based staleness solves a problem we don't have. SWR ~4.3 KB with a thinner
  feature set. Using query caches over Tauri IPC is template convention in the wild, not a
  Tauri-team recommendation. ADR 0050 dec. 4 already ruled "no new state library unless a child
  task justifies one" — this study is that child task, and it does not.

## Decisions

1. **No query-cache library.** The library's core value (staleness under network uncertainty,
   request dedup at scale) does not apply to ≤2 ms local IPC over a backend with ground truth;
   the ~10 KB floor plus cache-key/staleTime discipline is cost without benefit (conservative-deps
   rule). Re-entry trigger, recorded now: if devtools-grade cache introspection or
   optimistic-update ergonomics become a recurring pain across ≥2 screens, revisit with a new ADR.
2. **Formalize the existing self-fetch pattern as one thin shared hook** in `src/app/state/`:
   `useCommandQuery(key, fetcher)` — in-flight sequencing (stale-response discard),
   loading/error state, `refetch()`, and re-run on key change. It standardizes what
   `FundamentalsPanel`/`ReportDiffPanel`/`useCockpitRedFlags` each hand-roll today (they retrofit
   opportunistically, when a slice touches them — no big-bang rewrite). ~50 lines + tests, no
   dependency.
3. **F1's company-context block is a composed Rust read model** (one command, one DTO,
   server-side assembly), following `get_company_health`/`list_company_timeline` — not a 5-command
   frontend fan-out per selection. Consumed via the decision-2 hook keyed on `companyId`.
4. **Invalidation stays event-less in F1**: key-change refetch (selection change) + explicit
   `refetch()` after local mutations + the existing 15s scheduler-mirror/manual-refresh ambient
   layer. A backend→frontend push channel (Tauri `emit`) is a real future option once a screen
   needs sub-poll freshness — its own decision when the need is evidenced, not smuggled in here.
5. **The root stops growing**: new per-screen/per-entity data (starting with F1's context block)
   must NOT be added to `useAppDataController`'s app-wide fetch set or `AppStateRoot`'s state.
   Existing root-fed state migrates out only opportunistically (ADR 0050 dec. 4 decomposition),
   never as a prerequisite for feature work.

## Rejected

- **Extending the app-root pattern** (fetch F1 context app-wide / thread it through
  `AppStateRoot`): grows a 2227-line pinned file, refetches on root renders it doesn't own, and
  repeats the unfiltered-`list_company_signals` mistake (fetch-everything for a per-selection
  need).
- **TanStack Query / SWR**: see decision 1 — staleness machinery for uncertainty we don't have,
  at a real bundle/discipline cost. Precedent exists in Tauri templates but the fit argument
  fails here.
- **A new backend event channel now**: nothing in F1 needs sub-poll freshness; adding transport
  plus subscription lifecycle to every consumer is scope F1 doesn't carry.
