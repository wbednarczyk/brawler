import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import { resetMockScenario } from "./helpers/mockRuntime";

// Regression coverage for the live-defect fix (v0.57 fix wave 2, W3): unbounded
// persistent (attention-event) toasts piled up in the bottom-left viewport and
// sat directly on top of the left sidebar nav, blocking every nav click (owner
// saw a 19-toast wall). Fixed by:
//   - Toast.tsx: PERSISTENT_VISIBLE_CAP caps VISIBLE persistent toasts at 3,
//     collapsing the rest into a "+N more" summary row.
//   - feedback.css/shell.css: the toast viewport's `left` offset now clears
//     the sidebar (`--sidebar-width`) instead of sitting at `var(--space-5)`
//     directly on top of it.
//
// Seeding ~20 unseen attention events: the mock runtime has no query-param
// lever for scenario/overlay selection (only locale/theme/palette — see
// installBrowserSmokeRuntime), so this uses the `window.__brawlerMock` bridge
// (`resetMockScenario`, the `attention-overflow` overlay — 20 synthetic unseen
// events). The reset alone does not refresh an already-mounted Today screen
// (no polling/event push — `useTodayPulse` fetches once per mount), so the
// test navigates away and back to Today to remount it: TodayScreen unmounts
// whenever the section changes (see TodayScreen.tsx's `toastedAttentionEventIds`
// comment), which both re-fetches the (now 21-event) attention list AND
// re-runs the persistent-toast effect for every id not already toasted this
// session — the closest faithful reproduction of "many unseen attention
// events" available without a dedicated seeding bridge method.
test.describe("Attention-toast persistent-overflow cap", { tag: "@clickable" }, () => {
  test("~20 unseen attention events still leave the sidebar nav clickable, no page overflow", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    await resetMockScenario(page, { base: "rich", overlays: ["attention-overflow"] });

    const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
    // Navigate away, then back to Today — remounts TodayScreen against the
    // freshly re-seeded (21 unseen) attention events.
    await nav.getByRole("button", { name: "Companies" }).click();
    await expect(page.getByRole("heading", { name: "Companies", exact: true })).toBeVisible();
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const viewport = page.locator(".ui-toast-viewport");
    await expect(viewport).toBeVisible();

    // Capped: at most 3 persistent alert toasts render, plus the "+N more"
    // summary row — never an unbounded wall.
    await expect(viewport.getByRole("alert")).toHaveCount(3);
    const summary = viewport.getByRole("button", { name: /^\+\d+ more$/ });
    await expect(summary).toBeVisible();

    // The regression itself: Playwright's `.click()` fails its actionability
    // check if the target is covered by another element (here, the toast
    // viewport used to sit directly on top of the sidebar). A successful
    // click through to a new screen is proof the sidebar is not obscured.
    await nav.getByRole("button", { name: "Watchlists" }).click();
    await expect(page.getByRole("heading", { name: "Watchlists", exact: true })).toBeVisible();
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    await expectNoPageOverflow(page);

    // Narrow ~1024px tall-narrow window (CLAUDE.md supported range: the
    // viewport matrix's `chromium-quarter-uw-125` project) — the sidebar-width
    // offset must keep the toast viewport (max-width: min(360px, …)) inside
    // the document, not just at the default project's width.
    await page.setViewportSize({ width: 1024, height: 1152 });
    await expectNoPageOverflow(page);
    await expect(viewport.getByRole("alert")).toHaveCount(3);
    await nav.getByRole("button", { name: "Sources" }).click();
    await expect(page.getByRole("heading", { name: "Sources", exact: true })).toBeVisible();
  });

  test("the overflow summary survives a section change and navigates back to Today", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    await resetMockScenario(page, { base: "rich", overlays: ["attention-overflow"] });

    const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
    // Away-and-back remounts Today against the freshly re-seeded events (see
    // the describe-block comment) so the overflow summary actually appears.
    await nav.getByRole("button", { name: "Companies" }).click();
    await expect(page.getByRole("heading", { name: "Companies", exact: true })).toBeVisible();
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const summary = page.locator(".ui-toast-viewport").getByRole("button", { name: /^\+\d+ more$/ });
    await expect(summary).toBeVisible();

    // The Toast surface mounts above AppStateRoot (App.tsx) — it must outlive
    // a section change, unlike the screen that raised the toasts.
    await nav.getByRole("button", { name: "Companies" }).click();
    await expect(page.getByRole("heading", { name: "Companies", exact: true })).toBeVisible();
    await expect(summary).toBeVisible();

    await summary.click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
  });
});
