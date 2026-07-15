import { expect, openApp, test } from "./helpers/harness";

// Dashboard redesign (epic c793ca1): the left-nav "Dashboard" entry opens the ONE
// company-scoped Dashboard directly — never blank. It seeds a default company
// (last-viewed, else a pinned/first company) so the company-overview follow panels
// render immediately, rather than the legacy feed-linked blank cockpit. Amends ADR
// 0057 decision 5 (the "no empty mode" rationale is preserved and strengthened).
test("Dashboard nav entry opens the company-scoped Dashboard (never blank)", async ({ page }) => {
  await openApp(page);

  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await nav.getByRole("button", { name: "Dashboard" }).click();

  // The cockpit renders (not the standalone Research screen) and is scoped to a
  // company: the view company is set and a company-overview follow panel
  // (Fundamentals) is present — proof it is not the blank feed-linked cockpit.
  const cockpit = page.getByLabel("Research cockpit");
  await expect(cockpit).toBeVisible();
  await expect(cockpit).not.toHaveAttribute("data-company-id", "");
  await expect(page.getByLabel("Company fundamentals")).toBeVisible();
});
