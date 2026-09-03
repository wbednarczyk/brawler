import { test, expect, openApp, journey, expectNoPageOverflow } from "../helpers/harness";

// J8 — Keyboard-only company review (docs/ux-journeys.md, F3c #197, contract
// docs/plans/frontend-v2-f3c.md). Trigger: an investor with the hands on the
// keyboard opens a company and walks its workshop without the mouse. Path:
// palette → company → Ctrl+. to the workshop bar → arrows to Claims → Enter →
// L to the next tool (Notebook) → Escape back to Overview with focus on the
// closed tool's entry. Done well: zero pointer events, focus never lost.
//
// Keyboard activation (Enter/Space on a button) never emits pointerdown/
// mousedown, so a page-level counter of those two events proves the journey
// stayed on the keyboard (sol R1 finding 7).

test.describe("J8 — keyboard-only company review", { tag: "@journey" }, () => {
  test("company → claims → notebook → overview without the mouse", async ({ page }) => {
    await page.addInitScript(() => {
      (window as unknown as { __pointerEvents: number }).__pointerEvents = 0;
      for (const type of ["pointerdown", "mousedown"]) {
        window.addEventListener(type, () => {
          (window as unknown as { __pointerEvents: number }).__pointerEvents += 1;
        }, true);
      }
    });
    const j = journey(page, "J8");
    await openApp(page);
    await j.markScreen("Today");

    await j.press(page, "Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await expect(palette).toBeVisible();
    await j.fill(palette.getByRole("combobox", { name: "Search commands" }), "Open company: CDR");
    await j.press(page, "Enter");
    const spolka = page.getByRole("region", { name: "Company view" });
    await expect(spolka).toBeVisible();
    await j.markScreen("Company workspace");
    await j.preserveContext("company_gpw_cdr");

    // Ctrl+. lands on the workshop toolbar's single tab stop (Overview at
    // rest); five ArrowRights reach Claims (Overview, Fundamentals, Feed,
    // Coverage, Recommendations, Claims).
    await j.press(page, "Control+.");
    const bar = spolka.getByRole("toolbar", { name: "Workshop" });
    await expect(bar.getByRole("button", { name: "Overview" })).toBeFocused();
    for (let i = 0; i < 5; i += 1) await j.press(page, "ArrowRight");
    await expect(bar.getByRole("button", { name: "Claims", exact: true })).toBeFocused();
    await j.press(page, "Enter");
    const tool = spolka.getByRole("group", { name: "Workshop tool" });
    await expect(tool).toHaveAttribute("data-tool", "tezy");
    await expect(tool.getByRole("heading", { level: 2, name: "Claims" })).toBeFocused();

    // L = next workshop tool (Claims → Notebook); focus lands on its heading.
    await j.press(page, "l");
    await expect(tool).toHaveAttribute("data-tool", "notatnik");
    await expect(tool.getByRole("heading", { level: 2, name: "Notebook" })).toBeFocused();

    // Escape closes the tool; focus returns to the closed tool's bar entry.
    await j.press(page, "Escape");
    await expect(tool).toBeHidden();
    await expect(bar.getByRole("button", { name: "Notebook" })).toBeFocused();
    await expect(bar.getByRole("button", { name: "Overview" })).toHaveAttribute("aria-pressed", "true");

    await expectNoPageOverflow(page);
    expect(await page.evaluate(() => (window as unknown as { __pointerEvents: number }).__pointerEvents)).toBe(0);
    await j.assertBudget();
  });
});
