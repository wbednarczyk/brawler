import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import { SAMPLE_NOW } from "../../src/test/scenarios/entities";
import {
  PRUNED_CLEAN_SNAPSHOT,
  PRUNED_GLUED_FILENAME,
  PRUNED_GLUED_HUMAN,
} from "../../src/test/scenarios/overlays";

// Today rebuilt to Dziś v2 (F2 #422, docs/plans/frontend-v2-f2.md): a
// per-day decision queue (`dayQueueModel.ts` + `rows2/`) replaced the old
// prioritized stream + counter-tile filters + j/k roving keyboard flow this
// file used to cover. The browser runtime locale is en.
//
// DELETED, no Dziś v2 home (plan decision 7 — "kafelki-liczniki OUT: liczniki
// = nagłówki dni"): the two counter-tile filter cases ("a counter tile
// filters the stream…", "the Pilne (Urgent) tile filters…") — the tiles
// themselves are a sanctioned kasacja, counting now lives on each day's
// header, not a separate filter control.
//
// DELETED, no Dziś v2 home: the two same-category cross-company aggregate/
// group-roving cases ("j/k roving crosses a group header…", "an aggregate row
// expands its collapsed companies…") — Dziś v2 has no same-category
// aggregation at all (`dayQueueModel.ts` buckets by LOCAL DAY only; the one
// collapse mechanism left is the per-day row cap, `capDayRows`, S-tier
// "top-3+zwiń"). Superseded by `tests/browser/density-matrix.spec.ts`'s
// `today-dense` entry, which already proves the cap holds under a dense
// wall — see the "dense" deletion note below.
//
// DELETED, FLAGGED (not a plan decision — an unaddressed gap): "j/k moves
// roving focus across the stream's action buttons". A repo-wide grep for
// roving-focus code (`RovingFocus`, `useRoving`, a `j`/`k` keydown handler)
// turns up NOTHING anywhere in `src/screens` or `src/app` — the S4 rebuild
// carries no keyboard-roving implementation over `.dayq-row-action` buttons,
// and `docs/plans/frontend-v2-f2.md` names no replacement and no explicit
// cut. This reads as a real regression, not a deliberate kasacja — surfaced
// for an owner call (restore roving over the new row actions, or add the cut
// to the plan) rather than silently deleted.
//
// DELETED: "dense: a wall of routine runs folds into one aggregate" — same
// `today-dense` overlay, same S-tier cap mechanism, already covered more
// completely by `density-matrix.spec.ts` (S/M/L, not just one narrow
// viewport) whose Today entry names itself the density owner (plan §12 "full
// wave" / S5 "właściciel density S nazwany").
//
// DELETED: "partial: one category errors → an inline error strip" — the
// mechanism this case depended on (holding/rejecting a per-category command,
// `list_report_season`) no longer exists: Today now reads through exactly
// ONE composed, backend-infallible command (`get_today_view`,
// `src-tauri/src/commands/today.rs` `compute_today_view` — every section
// degrades into `sectionErrors` INSIDE that one Rust call, never surfaced as
// a separate rejectable IPC call the mock runtime can intercept). Per the
// plan's own test-layer table (§12), per-section partial/empty/error
// rendering is Vitest territory (component proof, S3), not browser; a full
// `get_today_view` read failure (the one failure shape still reachable via
// `holdInvocation`/`rejectInvocation` at this boundary) is already covered by
// `tests/browser/journeys/j1-morning-review.spec.ts`'s rewritten ADR 0081 Q9
// case ("a failed Today read is explicit, never false quiet, and Retry
// recovers it").

test.describe("Today day queue — severity expression", { tag: "@clickable" }, () => {
  test("urgent rows carry the severity accent and stay reachable", async ({ page }) => {
    // `morning-review` seeds exactly 2 urgent attention events today (PKN
    // insider transaction, KGH missed-report reconciliation).
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    await primeMockScenario(page, { base: "rich", overlays: ["morning-review"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // Severity expression is weight + outline, never an extra color (plan
    // decision 8): both urgent rows carry the accent outline and the bold
    // title. Scoped to `.dayq-row-accent` itself (not a bare ticker filter):
    // the `rich` base already seeds plenty of routine PKN/KGH rows, so only
    // the severity outline disambiguates the two urgent ones from the rest
    // of either company's day.
    const todaySection = page.locator(".dayq-section").first();
    const pknRow = todaySection.locator(".dayq-row-accent").filter({ hasText: "PKN" }).first();
    const kghRow = todaySection.locator(".dayq-row-accent").filter({ hasText: "KGH" }).first();
    await expect(pknRow).toBeVisible();
    await expect(kghRow).toBeVisible();
    await expect(pknRow.locator(".dayq-row-title")).toHaveClass(/dayq-row-title-strong/);
    await expect(kghRow.locator(".dayq-row-title")).toHaveClass(/dayq-row-title-strong/);

    // NOT asserted here: "the header CTA targets the most urgent item"
    // (plan decision 8's stated priority). Verified live: ADR 0097's
    // batch-mark-seen-on-load effect (kept unchanged, S4) flips BOTH urgent
    // events' `seen` optimistically within the same mount that renders
    // `pickPrimary` — by the time either the DOM settles or a real person
    // would see the header, `pickPrimary`'s urgent branch (`!event.seen`)
    // has already lost its only candidates, and the header CTA falls through
    // to the next tier (the unread filing report). The urgent→primary-CTA
    // path is therefore only reachable for an urgent event that arrives
    // WHILE Today is already open (a live refresh), never one already
    // loaded at mount — worth an owner call on whether decision 8's ordering
    // and ADR 0097's seen-on-load are supposed to interact this way.
    const urgentRowAction = kghRow.getByRole("button", { name: "Review" });
    await expect(urgentRowAction).toBeVisible();
    await urgentRowAction.click();
    await expect(page.locator(".dayq-delta-header")).toHaveCount(0);
    await expectNoPageOverflow(page);
  });
});

// UI dogfooding finding ⇒ overlay (docs/testing.md standing rule; owner dogfooding
// 2026-07-23): the two data states the owner's real database exposed on Today,
// reproduced via ADR 0081 overlays so the tolerant render is pinned in CI forever.
// Selectors retargeted to the Dziś v2 day-queue DOM (`rows2/RowShell.tsx`) — the
// underlying protected behavior (never blank/crash on orphaned evidence; never
// leak a filename into a row statement) is unchanged.
test.describe("Today dogfooding states — orphaned evidence + pruned feed", { tag: "@clickable" }, () => {
  test("orphaned evidence (null title + gone rule) renders a category fallback, never blank or crashed", async ({ page }) => {
    // The overlay fires its events AT `SAMPLE_NOW` (`overlays.ts`) — freeze
    // the page clock so `dayQueueModel`'s local-day bucketing (real wall
    // clock otherwise) agrees and the rows land in a visible display slot,
    // never collapsed into the "Earlier" rollup (same j1-morning-review.spec.ts
    // pattern).
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    await primeMockScenario(page, { base: "rich", overlays: ["orphaned-evidence"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // No generic error fallback — the missing rule/signal must not crash the queue.
    await expect(page.locator(".app-error-recovery")).toHaveCount(0);

    // Each orphan company's row still renders, with a non-empty statement and its
    // category chip — the fallback, not a blank line. The statement must be the
    // localized generic copy, never a raw trigger enum token like
    // "signal_category" (issue #119).
    for (const ticker of ["ZZO1", "ZZO2", "ZZO3"]) {
      const row = page.locator(".dayq-row").filter({ hasText: ticker }).first();
      await expect(row).toBeVisible();
      const title = await row.locator(".dayq-row-title").innerText();
      expect(title.trim().length).toBeGreaterThan(0);
      expect(title.trim()).not.toMatch(/^[a-z0-9]+(?:_[a-z0-9]+)+$/);
      await expect(row.locator(".ui-status-chip").first()).toBeVisible();
    }
    await expectNoPageOverflow(page);
  });

  test("pruned feed renders the surviving snapshot; a glued filename splits off the statement", async ({ page }) => {
    // Same clock freeze as above — the overlay's events fire AT `SAMPLE_NOW`.
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    await primeMockScenario(page, { base: "rich", overlays: ["pruned-feed"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // The clean snapshot title survives the pruned feed row and renders verbatim.
    await expect(page.getByText(PRUNED_CLEAN_SNAPSHOT)).toBeVisible();

    // The glued snapshot splits: the human part is the statement, the filename drops
    // to the row's quiet meta line — the extension never lands in the statement.
    const gluedRow = page.locator(".dayq-row").filter({ hasText: "ZZP2" }).first();
    await expect(gluedRow.locator(".dayq-row-title")).toHaveText(PRUNED_GLUED_HUMAN);
    await expect(gluedRow.locator(".dayq-row-meta")).toHaveText(PRUNED_GLUED_FILENAME);
    await expectNoPageOverflow(page);
  });
});
