import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// §G live proof for the acquisition-scoped MCP credential (#382, ADR 0099
// dec. 2): on the REAL Windows app, the Settings → MCP section renders the
// acquisition toggle + token section; enabling the gate and generating a
// token yields a bearer that authenticates against the live listener with an
// EMPTY tools list (the allowlist ships empty until #384–#386), and turning
// the gate off makes the same bearer indistinguishable from an unknown one
// (401). State is restored: the token is revoked and the gate turned off;
// the primary credential and every other setting are never touched.
//
// The scoped curl runs only when the live MCP server is actually running
// (the owner's setting) — the UI assertions always run.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("acquisition credential: UI renders, gate gates, scoped list is empty", async () => {
  const { page } = connection;
  test.setTimeout(120_000);

  // Settings → MCP server section.
  await page.getByRole("button", { name: /^(Settings|Ustawienia)$/ }).click();
  const region = page.getByLabel(/Application settings|Ustawienia aplikacji/);
  await region.getByRole("button", { name: /MCP/ }).click();

  const gateToggle = region.getByRole("switch", {
    name: /Allow acquisition access|Zezwól na dostęp akwizycyjny/,
  });
  await expect(gateToggle).toBeVisible();
  await expect(
    region.getByText(/Acquisition token|Token akwizycyjny/).first(),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/live/mcp-acquisition-section.png",
    fullPage: true,
  });

  const serverRunning = await region
    .getByRole("switch", { name: /Enable the server|Włącz serwer/ })
    .isChecked();
  const gateWasOn = await gateToggle.isChecked();

  if (!serverRunning) {
    console.log(
      "live MCP server is off — UI assertions done, scoped curl skipped",
    );
    return;
  }

  const portField = region.getByLabel(/Listen port|Port nasłuchu/);
  const port = Number.parseInt((await portField.inputValue()) || "8317", 10);

  // Enable the gate, mint an acquisition token, capture the one-time reveal.
  if (!gateWasOn) await gateToggle.click();
  await region
    .getByRole("button", {
      name: /Generate acquisition token|Wygeneruj token akwizycyjny/,
    })
    .click();
  const reveal = region.getByLabel(/Acquisition token|Token akwizycyjny/);
  await expect(reveal).toBeVisible();
  const token = await reveal.inputValue();
  expect(token.length).toBeGreaterThan(0);

  const call = (bearer: string) =>
    fetch(`http://127.0.0.1:${port}/mcp`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${bearer}`,
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

  try {
    // Gate ON: the scoped bearer authenticates and sees the EMPTY allowlist.
    const on = await call(token);
    expect(on.status).toBe(200);
    const body = (await on.json()) as {
      result: { tools: unknown[] };
    };
    expect(body.result.tools).toEqual([]);
    console.log("scoped tools/list on the live app: 200 with 0 tools — as frozen");

    // Gate OFF: the same bearer is rejected like an unknown token.
    await gateToggle.click();
    const off = await call(token);
    expect(off.status).toBe(401);
    console.log("gate off: same bearer → 401 — kill switch works live");
  } finally {
    // Restore: revoke the acquisition token (trigger + inline confirm carry
    // the same label) and leave the gate exactly as found (off unless it was
    // on before the test).
    const revokeButtons = region.getByRole("button", {
      name: /Revoke acquisition token|Unieważnij token akwizycyjny/,
    });
    await revokeButtons.first().click();
    await revokeButtons.first().click();
    if (gateWasOn !== (await gateToggle.isChecked())) await gateToggle.click();
  }
});
