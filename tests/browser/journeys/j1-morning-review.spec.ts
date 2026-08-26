import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
  setPaneSize,
} from "../helpers/harness";
import { expectPrimaryActionCount } from "../helpers/interactionContracts";
import {
  holdInvocation,
  primeMockScenario,
  rejectInvocation,
} from "../helpers/mockRuntime";
import { SAMPLE_NOW } from "../../../src/test/scenarios/entities";

// `recentFeedItems`/`companies` are fetched exactly once at app bootstrap (not
// re-fetched on in-app navigation), so overlay content must be seeded BEFORE
// boot via `primeMockScenario` (helpers/mockRuntime) — a post-`openApp` reset
// raced and was observed flaky under worker load.

// J1 — Morning review, REWRITTEN for Dziś v2 (F2 S4, #422, ADR 0068 amendment
// 2026-08-20): MorningBriefingStrip is retired from Today — the delta header
// ("what arrived since your last visit") is the journey's entry point
// (docs/ux-journeys.md J1). The per-day decision queue (`dayQueueModel.ts`,
// F2 S3) replaces the severity-ranked stream; row actions name their
// destination and land ON the item (plan decision 6, #422/3) instead of
// "somewhere in the workspace".
//
// `page.clock.setFixedTime(SAMPLE_NOW)` freezes the browser's `Date.now()` at
// the mock's own anchor: `dayQueueModel`'s "Today"/"Yesterday" labels are
// computed against the REAL wall clock (there is no test-injected clock at
// runtime, only in its own unit tests), while `get_today_view`'s mock handler
// (runtime.ts) anchors its "today" on the fixed `SAMPLE_NOW` constant — without
// freezing the page clock the two would disagree and the DZIŚ/WCZORAJ pattern
// could never render in a browser run.
//
// The seed is the `morning-review` overlay (src/test/scenarios/overlays.ts):
// a PZU filing notice published AT `SAMPLE_NOW` ("today") and a CD Projekt
// report published one day earlier ("yesterday"), with `todayLastVisitAt` set
// between the two so the delta header has a non-empty sentence.

test.describe("J1 — morning review", { tag: "@journey" }, () => {
  // Canonical J1 (ux-journeys.md, sol R1 finding 9): the case previously
  // stopped at the Inbox anchor — a TRUNCATED journey, budgeted as if the
  // morning review ended there. The real journey (contract §1 first red row,
  // plan §12) continues: back to Today, the Claims leg ("Otwórz tezę" → the
  // claim highlighted in its company's Claims panel, the `openCompanyClaims`
  // seam / DockLayout `activatePanelId`, fix wave B finding 2), then closes
  // the loop with "Oznacz dzień jako przejrzany".
  test("delta leads, day sections render, Otwórz komunikat/tezę anchor the exact item, and the day closes", async ({
    page,
  }) => {
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    await primeMockScenario(page, { base: "rich", overlays: ["morning-review"] });

    const j = journey(page, "J1");
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    // Key screen: Today is the day's landing — axe-clean, no overflow, before
    // any interaction.
    await expectNoA11yViolations(page, "Today (morning review)");
    await expectNoPageOverflow(page);

    // The delta header leads (contract §6/§10 Entry frame): an eyebrow + a
    // sentence naming what arrived since the last visit (Main.dc.html).
    const deltaHeader = page.locator(".dayq-delta-header");
    await expect(deltaHeader).toBeVisible();
    await expect(deltaHeader).toContainText("Since your last visit");
    // Contract §6: exactly one screen-wide primary action (the header CTA) —
    // every row action stays quiet (plan decision 9).
    await expectPrimaryActionCount(page.locator(".today-screen"), { max: 1 });

    // Day sections render in the DZIŚ/WCZORAJ pattern (plan decision 5): the
    // seeded PZU filing lands "Today", the CD Projekt report "Yesterday".
    await expect(page.getByText("Today", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Yesterday", { exact: true })).toBeVisible();

    // "Otwórz komunikat" on the EXACT PZU filing row lands on THAT item in the
    // Inbox detail pane — never the silent first-row fallback (#422/3, plan
    // decision 6). `data-dayq-row-id` carries the feed item id (RowShell).
    const filingRow = page.locator('[data-dayq-row-id="feed_overlay_mr_filing"]');
    await expect(filingRow).toBeVisible();
    const openFiling = filingRow.getByRole("button", { name: "Open filing" });
    await j.click(openFiling);

    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await j.markScreen("Inbox");
    const detail = page.locator(".detail-pane");
    await expect(detail).toContainText("Powołanie Członka Zarządu PZU SA");

    // The S-overlay (`inboxDetailOpen`) is raised from OUTSIDE (the
    // `inboxDetailActivationToken` seam, useFeedController/InboxScreen S3),
    // never only from a row click — the flag is unconditional on viewport.
    const detailPane = page.getByLabel("Feed item details");
    await expect(detailPane).toHaveAttribute("data-detail-open", "true");

    // At the compact (S) pane tier (ADR 0076 D6, `density-matrix.spec.ts`
    // precedent — the named `.workspace` size container, forced regardless of
    // the actual viewport), the raised overlay visually covers the feed row
    // list rather than sitting side-by-side with it.
    await setPaneSize(page, { width: 380, height: 700, pane: page.locator(".workspace").first() });
    const [listBox, detailBox] = await Promise.all([
      page.locator("[data-feed-row='true']").first().boundingBox(),
      detailPane.boundingBox(),
    ]);
    expect(listBox).not.toBeNull();
    expect(detailBox).not.toBeNull();
    // The overlay's left edge reaches at least as far left as the row it
    // covers — a side-by-side (non-overlay) layout would sit strictly to the
    // right of the list instead.
    expect(detailBox!.x).toBeLessThanOrEqual(listBox!.x + listBox!.width / 2);

    await expectNoPageOverflow(page);

    // Back to Today — closing the Inbox leg of the loop.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    // The Claims leg: `data.toVerify` is the BASE "rich" scenario's bulk
    // claims-to-verify bucket (`claimsToVerify.due[0]`, `runtime.ts`
    // `get_today_view`), deterministically the CD Projekt claim — no overlay
    // seed extension needed (checked: the morning-review overlay never touches
    // `claimsToVerify`, and the base scenario already seeds a non-empty one).
    const claimStatement = "CD Projekt targets revenue above 1.0bn next year";
    const claimRow = page.locator(".dayq-row").filter({ hasText: claimStatement });
    await expect(claimRow).toBeVisible();
    await j.click(claimRow.getByRole("button", { name: "Open thesis" }));

    // F3a S1/S3 (ADR 0107 decision 2 mapping "Claims/highlightClaimId→
    // {t:'tezy', claimId}"): the seam no longer routes through the cockpit —
    // it lands the Spółka screen with the claims tool raised.
    const company = page.getByRole("region", { name: "Company view" });
    await expect(company).toBeVisible();
    await j.markScreen("Company workspace");

    // Browser-proof of the activation seam (sol R1 finding 2/9): the claims
    // tool is RAISED (`data-tool="tezy"`), not just primed behind the core.
    const tool = company.getByRole("group", { name: "Workshop tool" });
    await expect(tool).toHaveAttribute("data-tool", "tezy");
    // ...and the claim itself is highlighted (`CompanyClaimsPanel`'s
    // `highlightClaimId` seam) — `.first()` because the mock's
    // `list_claims_to_verify` returns the whole bulk bucket unfiltered, so the
    // claim can render both in the company's own claims list and the review
    // queue, both correctly highlighted.
    const highlightedClaim = tool
      .locator('[data-claim-id="claim_sample_cdr"].claim-row-highlighted')
      .first();
    await expect(highlightedClaim).toBeVisible();
    await expect(highlightedClaim).toContainText(claimStatement);

    // Back to Today, then close the day (contract §7 exit path).
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    const todaySection = page
      .locator(".dayq-section")
      .filter({ has: page.locator(".dayq-day-label", { hasText: /^Today$/ }) });
    await j.click(todaySection.getByRole("button", { name: "Mark day reviewed" }));
    await expect(todaySection.locator(".dayq-day-header-collapsed")).toBeVisible();

    await expectNoPageOverflow(page);
    await j.assertBudget();
  });

  // J1b (F1 S5, contract §1 FIRST RED): the Inbox leg of the morning review as
  // its OWN budgeted journey — J1's floors were already saturated by the
  // above anchoring flow, so this leg gets its own gate instead of a loosened
  // shared one (sol round-1 finding 3, variant B). Independent of Today's own
  // DOM/data flow — unaffected by the Dziś v2 rebuild.
  test("J1b — inbox filing review fits its own budget", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["morning-review"] });

    const j = journey(page, "J1b");
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Inbox" }));
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await j.markScreen("Inbox");

    const filingRow = page.locator('[data-feed-item-id="feed_overlay_mr_filing"]');
    await expect(filingRow).toBeVisible();
    await j.click(filingRow);
    const detail = page.locator(".detail-pane");
    await expect(detail.getByText("ESPI notice")).toBeVisible();
    await expect(page.getByText("Komunikat ESPI/EBI")).toHaveCount(0);
    // Contract §6: exactly one marked primary action per rendered detail kind.
    await expectPrimaryActionCount(detail, { max: 1 });
    await expectNoPageOverflow(page);

    // Act: mark it read (the ≤10s decision the experience contract names —
    // "deal with it now or mark read and move on").
    await j.click(detail.getByRole("button", { name: "Mark read" }));
    await expect(detail.getByRole("button", { name: "Mark unread" })).toBeVisible();

    // Return to Today, closing the loop.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });

  // ADR 0081 Q9: hostile/adversarial content must render legibly and stay
  // navigable — a long mixed-script title/attachment URL is real content
  // shape (GPW ESPI communiqués are not English-only, ASCII-only, or short).
  test("hostile day-queue row stays legible and its action reachable", async ({ page }) => {
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    // The tall-narrow band (CLAUDE.md Testing Expectations) is where a long
    // unbreakable title/URL is most likely to blow out the layout.
    await page.setViewportSize({ width: 1366, height: 768 });
    await primeMockScenario(page, { base: "rich", overlays: ["hostile-content", "mixed-locale"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);

    // The hostile company (ZZZH) is a fixed report-kind row (F2 S4: it is not
    // grouped/aggregated — Dziś v2 day-buckets every item individually) —
    // matched by its fixed feed item id, never the brittle mixed-script title.
    const hostileRow = page.locator('[data-dayq-row-id="feed_overlay_hostile_1"]');
    await expect(hostileRow).toBeVisible();
    // The long unbreakable title/URL never blows out the layout, even in the
    // tall-narrow band.
    await expectNoPageOverflow(page);

    // The row's action stays reachable (plan decision 9: row actions are
    // quiet — only the header CTA carries `data-ux-primary-action`).
    await expect(hostileRow.getByRole("button", { name: "Read report" })).toBeVisible();
    await expectNoA11yViolations(page, "Today (hostile day queue)");
  });

  // ADR 0081 Q9: a failed Today read must be explicit, never silently folded
  // into the quiet/empty state. Dziś v2 composes ONE read (`get_today_view`,
  // F2 S1) instead of the four independent per-category reads the old stream
  // held/rejected separately — the equivalent Q9 case now holds/rejects that
  // single command. (The out-of-order/stale-response race guard the old spec
  // proved per-category now lives in the shared `useCommandQuery` hook,
  // already covered generically by `src/shared/state/useCommandQuery.test.ts`
  // — F2 S4 does not duplicate that coverage here.)
  test("a failed Today read is explicit, never false quiet, and Retry recovers it", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // `window.__brawlerMock` only exists once the app has booted, so the hold
    // is registered after the first mount, then Today is remounted (a
    // navigate-away/back round trip) so it captures THAT mount's own
    // `get_today_view` read. Registered TWICE: React StrictMode (dev) double-
    // invokes mount effects, so the remount fires it twice — a discarded
    // "phantom" call, then the real one the rendered component keeps.
    const nav = page.getByLabel(/Primary navigation/);
    await nav.getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    const phantomId = await holdInvocation(page, { command: "get_today_view", phase: "before-handler" });
    const readId = await holdInvocation(page, { command: "get_today_view", phase: "before-handler" });
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const failure = {
      code: "internal" as const,
      message: "Sample today-view fetch failure (Q9 controlled-async case)",
    };
    await rejectInvocation(page, phantomId, failure);
    await rejectInvocation(page, readId, failure);

    // A typed, translated error — NEVER the raw backend `.message` — with a
    // scoped Retry (state matrix "Error" row).
    await expect(page.getByText("Couldn't load your Today view.")).toBeVisible();
    await expect(page.getByText(failure.message)).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);
  });
});
