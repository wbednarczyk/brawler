import { expect, openApp, test } from "./helpers/harness";

// The Modes "Company" nav item opens the Spółka screen (ADR 0107; the docking
// engine is gone, ADR 0108); Research is one of its workshop tools. This spec
// opens the Spółka
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
