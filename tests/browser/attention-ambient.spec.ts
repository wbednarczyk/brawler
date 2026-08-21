import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import { primeMockScenario, resetMockScenario } from "./helpers/mockRuntime";
import { SAMPLE_NOW } from "../../src/test/scenarios/entities";

// Class guard for ADR 0097: ambient/system attention NEVER renders as toasts —
// it lives in the Today day queue plus the sidebar Today badge (Dziś v2, F2
// #422 — badge semantics UNCHANGED, only the queue's own DOM moved to
// `rows2/`). Supersedes the persistent-overflow-cap spec (the capped toast
// wall this guarded against no longer exists as a surface at all). The
// `attention-overflow` overlay seeds ~20 unseen urgent events, firing AT
// `SAMPLE_NOW` — the exact batch shape that used to raise the bottom-left
// toast wall (owner screenshot, issue #330).
test.describe("Ambient attention — no toasts, Today badge (ADR 0097)", { tag: "@clickable" }, () => {
  test("a ~20-event unseen batch raises ZERO toasts; the stream renders and the sidebar stays clickable", async ({ page }) => {
    // Freeze the page clock so `dayQueueModel`'s local-day bucketing agrees
    // with the overlay's `firedAt: SAMPLE_NOW` and the batch lands in a
    // visible display slot (same j1-morning-review.spec.ts pattern).
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    // Seeded BEFORE boot so the very first mount faces the full batch.
    await primeMockScenario(page, { base: "rich", overlays: ["attention-overflow"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // The events land as day-queue rows — this is genuinely "many unseen
    // events", not an empty screen…
    const todaySection = page.locator(".dayq-section").first();
    await expect(todaySection.locator(".dayq-row").first()).toBeVisible();
    expect(await todaySection.locator(".dayq-row").count()).toBeGreaterThanOrEqual(15);

    // …and NOT ONE of them surfaces as a toast, of any kind. The viewport
    // renders nothing at all (no toast exists to mount it), and no assertive
    // alert region exists anywhere.
    await expect(page.locator(".ui-toast")).toHaveCount(0);
    await expect(page.locator('.ui-toast-viewport [role="alert"]')).toHaveCount(0);

    // The original regression this surface kept causing: chrome covering the
    // sidebar nav. With no stack, nav must stay clickable.
    const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
    await nav.getByRole("button", { name: "Sources" }).click();
    await expect(page.getByRole("heading", { name: "Sources", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);

    // Narrow tall window (the supported quarter-ultrawide range): still no
    // overflow, still no toasts.
    await page.setViewportSize({ width: 1024, height: 1152 });
    await expectNoPageOverflow(page);
    await expect(page.locator(".ui-toast")).toHaveCount(0);
  });

  test("ambient awareness: fresh events light the Today badge; visiting Today clears it (seen = was on screen)", async ({ page }) => {
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    await openApp(page);
    const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
    // Land on Today first: the base scenario's own unseen event gets batch-marked
    // seen, so the badge starts from a clean zero.
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    const todayNav = nav.getByRole("button", { name: "Today" });
    await expect(todayNav.locator(".nav-badge")).toHaveCount(0);

    // Move off Today, reseed a fresh unseen batch, and pull it in through a real
    // refresh seam (the topbar source refresh reloads the attention state,
    // ADR 0097 dec. 6).
    await nav.getByRole("button", { name: "Companies" }).click();
    await expect(page.getByRole("heading", { name: "Companies", exact: true })).toBeVisible();
    await resetMockScenario(page, { base: "rich", overlays: ["attention-overflow"] });
    await page.getByRole("button", { name: "Refresh sources" }).click();

    // The batch arrives as an unobtrusive badge count — and STILL zero toasts
    // beyond the transient "Sources refreshed" action feedback, which
    // auto-dismisses and is never an alert region.
    const badge = todayNav.locator(".nav-badge");
    await expect(badge).toBeVisible();
    await expect(page.locator('.ui-toast-viewport [role="alert"]')).toHaveCount(0);

    // Visiting Today marks the loaded events seen — the badge clears.
    await todayNav.click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await expect(page.locator(".dayq-row").first()).toBeVisible();
    await expect(todayNav.locator(".nav-badge")).toHaveCount(0);
  });
});
