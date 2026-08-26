# Composable cockpit views (frozen)

**As of F3a (ADR 0107), the freeform view-builder is frozen.** Opening a
company no longer opens a dashboard you assemble — it opens the fixed
[Spółka screen](company-view.md): a glance bar, an always-visible core, and a
one-click workshop of tools. This page describes the older views that still
exist and how they behave now.

## What's gone

Every command that changes a view's **structure** is gone: **"+ New view"**,
**"Add panel"**, applying a grid preset, and closing or dragging a panel.
There is no replacement flow for building a new freeform view — that
capability is frozen until an engine decision (F3a study: it saw zero
adoption, and its "fill it in" state was never actually persisted).

## What still works

A view you already built still **opens** and still **shows its panels**.
Every view (and every legacy per-company dashboard) now carries a
**"Layout frozen until the engine decision"** strip — a reminder that the
*arrangement* is locked, not the data. **Editing inside a panel still saves
normally**: facts, theses, notes, journal entries, quality assessments — none
of that is frozen, only the grid itself.

## Your old per-company dashboards

If you had saved a per-company dashboard before F3a, it didn't disappear: it
shows up in the sidebar's **Views** section as **"Legacy dashboard · TICKER"**
— still openable, still read/write inside its panels, same frozen strip as
any named view. Opening a company from **Companies**, a pinned row, or the
palette no longer lands here, though — it lands on the
[Spółka screen](company-view.md) instead.

## Renaming and deleting views

Hovering a saved view's sidebar row still reveals a **pencil** (rename in
place) and an **X** (delete the saved layout — never your data).

## A note on saved geometry

If a saved view's panel *arrangement* (sizes/positions) came from a newer
app version than the one reading it, that geometry is rejected and the
default layout is used instead — panels and their data are unaffected, only
where they sit on first open.
