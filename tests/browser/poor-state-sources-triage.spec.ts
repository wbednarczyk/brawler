import { test, expect, openApp } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import {
  DEGRADED_REPORTS_DETAIL_WARNING,
  DEGRADED_REPORTS_SOURCE_NAME,
} from "../../src/test/scenarios/overlays";

// Epic #40 S2 (ADR 0091) — the sources triage walk on POOR state. The
// `degraded-sources` overlay seeds adapters carrying a real `last_error` (plus
// the failed-detail counters and the detail warning a degraded fetch leaves
// behind), so the source-health surface must report an ATTENTION state, not a
// silently stale-but-green one.
//
// What must hold at each step: the failure is NAMED — the topbar pill counts
// real issues, the triage entry point lands on the offending adapter, and the
// row states WHICH source is degraded and WHAT its last fetch left behind.

// Both strings come from the overlay that seeds them — never re-typed here.
const DEGRADED_SOURCE = DEGRADED_REPORTS_SOURCE_NAME;
const DETAIL_WARNING = DEGRADED_REPORTS_DETAIL_WARNING;

test.describe("poor state — sources triage with degraded adapters", { tag: "@clickable" }, () => {
  test("the degraded adapter row shows its error state", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["degraded-sources"] });
    await openApp(page);

    // The topbar source pill is the triage entry point: a degraded adapter turns
    // it into a counted issue, never a quiet "all ready".
    const pill = page.getByLabel("Open source status");
    await expect(pill).toBeVisible();
    await expect(pill).toHaveClass(/source-status-pill-danger/);
    await expect(pill).toContainText(/issue/);

    // Clicking it opens Sources on the FIRST adapter with a `lastError` — the
    // triage flow lands directly on the offender.
    await pill.click();
    const row = page.locator(".source-row-block", { hasText: DEGRADED_SOURCE });
    await expect(row).toHaveCount(1);
    // The row names the source and states its health.
    await expect(row.getByRole("heading", { name: DEGRADED_SOURCE })).toBeVisible();
    await expect(row.locator(".source-health")).toHaveText("Needs attention");

    // ...and the opened detail panel carries the concrete, source-supplied
    // failure text — a named cause, not a bare status colour. NOTE (epic #40 S3):
    // `SourceAdapter.lastError` itself has no rendered surface today; the detail
    // warning is the only concrete failure string the Sources screen shows.
    const detail = row.getByLabel("Source details");
    await expect(detail).toBeVisible();
    await expect(detail.getByText(DETAIL_WARNING)).toBeVisible();
  });
});
