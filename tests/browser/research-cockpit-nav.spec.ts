import { expect, openApp, test } from "./helpers/harness";

// sol R1 finding 5: this spec targeted the removed "Dashboard" nav row and
// the frozen cockpit it used to open. F3a S3 (ADR 0107 amendment) replaced
// that bridge with the Modes "Company" nav item, which opens the Spółka
// screen scoped to a company directly — never blank (last-viewed, else the
// first pinned, else the first tracked company). The "no empty mode"
// rationale this spec protects (amending ADR 0057 decision 5) carries over
// verbatim to the new destination.
test("Company nav entry opens the Spółka screen scoped to a company (never blank)", async ({
  page,
}) => {
  await openApp(page);

  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await nav.getByRole("button", { name: "Company" }).click();

  // The Spółka screen renders scoped to a company: `data-company-id` is set
  // and the glance bar (always-visible core, never a hosted tool) is
  // present — proof it is not a blank/company-less screen.
  const spolka = page.getByRole("region", { name: "Company view" });
  await expect(spolka).toBeVisible();
  await expect(spolka).not.toHaveAttribute("data-company-id", "");
  await expect(page.getByRole("group", { name: "Company glance bar" })).toBeVisible();
});
