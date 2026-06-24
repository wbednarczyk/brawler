import {
  test,
  expect,
  openInbox,
  expectNoHorizontalOverflow,
  expectNoPageOverflow,
} from "./helpers/harness";

// End-to-end user journeys driven against the real React app in browser-smoke
// mode (mocked Tauri IPC with stateful fundamentals/extraction). These assert
// outcomes at each step — the gap that let wiring, state-machine, and
// visibility bugs through when the harness only captured screenshots. They run
// across the full viewport matrix (see playwright.config.ts).

test.describe("user journeys", () => {
  test("extract → review → confirm a reported KPI", async ({ page }) => {
    await openInbox(page);

    // Open a report feed item; its detail rail offers the KPI extractor.
    await page.getByLabel(/Select feed item: CD PROJEKT/).first().click();
    const launcher = page.getByLabel("AI KPI extraction");
    await expect(launcher).toBeVisible();

    // The heavy flow opens in a modal (the rail stays compact).
    await launcher.getByRole("button", { name: "Extract KPIs" }).click();
    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();
    await expectNoHorizontalOverflow(dialog);

    // Run the extraction; status and proposals must appear inside the modal
    // (not only behind it on the rail).
    await dialog.getByRole("button", { name: /Extract from attachment/ }).click();
    await expect(dialog.getByText("succeeded")).toBeVisible();
    await expect(dialog.getByText("Revenue")).toBeVisible();
    await expectNoHorizontalOverflow(dialog);

    // Confirm the first known KPI; it transitions to confirmed in place.
    await dialog.getByRole("button", { name: "Confirm" }).first().click();
    await expect(dialog.getByText("confirmed").first()).toBeVisible();
  });

  test("open company from a feed item focuses the workspace without moving the app", async ({
    page,
  }) => {
    await openInbox(page);

    await page.getByLabel(/Select feed item: CD PROJEKT/).first().click();
    await page.getByRole("button", { name: "Open company", exact: true }).click();

    // The workspace must be visible, in view, and focused — not requiring a
    // manual scroll to find (the reported defect).
    const workspace = page.getByRole("region", { name: "Company workspace", exact: true });
    await expect(workspace).toBeVisible();
    await expect(workspace).toBeInViewport();
    await expect(workspace).toBeFocused();

    // ...and the app shell itself must not have scrolled (the "moves the whole
    // app" defect): the page stays pinned, only the internal list scrolls.
    const pageScroll = await page.evaluate(() => ({
      y: window.scrollY,
      top: document.documentElement.scrollTop,
    }));
    expect(pageScroll.y).toBe(0);
    expect(pageScroll.top).toBe(0);
    await expectNoPageOverflow(page);
  });

  test("confirmed KPI surfaces in the company fundamentals matrix", async ({ page }) => {
    await openInbox(page);

    // Confirm a KPI from the report feed item first.
    await page.getByLabel(/Select feed item: CD PROJEKT/).first().click();
    await page.getByLabel("AI KPI extraction").getByRole("button", { name: "Extract KPIs" }).click();
    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await dialog.getByRole("button", { name: /Extract from attachment/ }).click();
    await expect(dialog.getByText("Revenue")).toBeVisible();
    await dialog.getByRole("button", { name: "Confirm" }).first().click();
    await expect(dialog.getByText("confirmed").first()).toBeVisible();
    await dialog.getByRole("button", { name: "Close dialog" }).click();

    // Then open the company's Fundamentals tab and assert the matrix renders
    // (the confirmed fact is wired through to the fundamentals read model).
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
    await page.getByRole("button", { name: "Fundamentals", exact: true }).click();

    const panel = page.getByLabel("Company fundamentals");
    await expect(panel).toBeVisible();
    await expect(page.getByLabel("Financial facts matrix")).toBeVisible();
    await expectNoHorizontalOverflow(panel);

    // Open a fact and assert the danger "Remove" button renders as a text button
    // — it previously used the icon-only variant (fixed 40px square), clipping
    // its label.
    await page.getByRole("button", { name: /^Revenue, / }).first().click();
    const factDetail = page.getByLabel("Financial fact detail");
    await expect(factDetail).toBeVisible();
    const removeButton = factDetail.getByRole("button", { name: "Remove" });
    await expect(removeButton).toBeVisible();
    const remove = await removeButton.evaluate((el) => ({
      clipped: el.scrollWidth > el.clientWidth + 1,
      width: el.clientWidth,
    }));
    expect(remove.clipped, "Remove button label is clipped").toBe(false);
    expect(remove.width, "Remove button is squeezed to icon width").toBeGreaterThan(56);
  });
});
