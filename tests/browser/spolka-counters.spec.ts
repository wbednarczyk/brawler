import { test, expect, openApp, journey, expectNoPageOverflow, expectNoA11yViolations } from "./helpers/harness";

// F3a S2 (ADR 0107, plan §8/§11): every Spółka glance-bar counter drills to a
// concrete workshop tool — the counter is never a dead readout. One named
// case per counter (plan § Test-layer plan "Browser … drill liczników").

async function openCdrSpolka(page: Parameters<typeof openApp>[0]) {
  const j = journey(page, "SPOLKA");
  await openApp(page);
  await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
  await expect(page.getByLabel("Companies list")).toBeVisible();
  await j.click(page.locator('[data-company-id="company_gpw_cdr"] .company-row-main'));
  const spolka = page.getByRole("region", { name: "Company view", exact: true });
  await expect(spolka).toBeVisible();
  return spolka;
}

test.describe("Spółka glance-bar counter drills", { tag: "@journey" }, () => {
  test("signals counter opens the signals tool", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    await spolka.getByLabel("Signals counter").click();
    const tool = spolka.getByLabel("Workshop tool");
    await expect(tool).toBeVisible();
    await expect(tool).toHaveAttribute("data-tool", "sygnaly");
    await expectNoPageOverflow(page);
  });

  test("claims counter opens the claims tool", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    await spolka.getByLabel("Claims counter").click();
    const tool = spolka.getByLabel("Workshop tool");
    await expect(tool).toBeVisible();
    await expect(tool).toHaveAttribute("data-tool", "tezy");
    await expectNoPageOverflow(page);
  });

  test("short counter opens akcjonariat's short section", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    await spolka.getByLabel("Shorts counter").click();
    const tool = spolka.getByLabel("Workshop tool");
    await expect(tool).toBeVisible();
    await expect(tool).toHaveAttribute("data-tool", "akcjonariat");
    await expect(tool.locator('[data-section="shorts"]')).toBeVisible();
    await expectNoPageOverflow(page);
  });

  test("events counter opens the company events tool", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    await spolka.getByLabel("Events counter").click();
    const tool = spolka.getByLabel("Workshop tool");
    await expect(tool).toBeVisible();
    await expect(tool).toHaveAttribute("data-tool", "wydarzenia");
    await expectNoPageOverflow(page);
  });

  // Narrow pane (~960–1280px, chromium-quarter-uw): no global horizontal
  // scrollbar AND the tool zone stays inside its own container.
  test("tool zone stays inside its container at the narrow viewport", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    await spolka.getByLabel("Claims counter").click();
    await expect(spolka.getByLabel("Workshop tool")).toBeVisible();
    await expectNoPageOverflow(page);
    const layout = page.locator(".spolka-layout");
    const overflow = await layout.evaluate((el) => el.scrollWidth <= el.clientWidth + 1);
    expect(overflow, ".spolka-layout must not overflow horizontally").toBe(true);
    await expectNoA11yViolations(page, "Spółka with a tool open (narrow)");
  });
});
