import crypto from "node:crypto";
import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { ACQUISITION_TOOLS } from "./helpers/acquisitionTools";

// §G live proof for the context read model + document delivery (#385, ADR
// 0099): on the REAL Windows app running against a DISPOSABLE COPY of the
// data directory, the acquisition bearer proves the two-phase foundation —
// scoped tools/list (exactly six tools, contract order), a fresh start
// WITHOUT scope/quality/period stopping at `source_captured`, a full default
// context (catalog/plausibility/profileRules/budgets), the REAL document
// read back in ≤256 KiB chunks and reassembled to the exact frozen sha256
// (the portable byte channel), then a resume with full context reaching
// `extracting`, a paginated section call, and cancel.
//
// Env-gated: refuses to run without BRAWLER_T5_DOC_ID (a fetched document in
// the sandbox DB, ideally >256 KiB so the loop takes >1 chunk) and
// BRAWLER_T5_DATA_DIR. Credential strategy (T4 precedent): the acquisition
// token is generated fresh in the sandbox app, used, revoked at cleanup; the
// PRIMARY token is never touched.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("context + chunked document over live MCP: start → source_captured → context → reassemble → resume → cancel", async () => {
  const documentId = process.env.BRAWLER_T5_DOC_ID;
  const dataDir = process.env.BRAWLER_T5_DATA_DIR;
  test.skip(!documentId || !dataDir, "BRAWLER_T5_DOC_ID/BRAWLER_T5_DATA_DIR not set");
  const { page } = connection;
  test.setTimeout(300_000);

  // Settings → MCP section.
  await page.getByRole("button", { name: /^(Settings|Ustawienia)$/ }).click();
  const region = page.getByLabel(/Application settings|Ustawienia aplikacji/);
  await region.getByRole("button", { name: /MCP/ }).click();

  const serverToggle = region.getByRole("switch", {
    name: /Enable the server|Włącz serwer/,
  });
  const gateToggle = region.getByRole("switch", {
    name: /Allow acquisition access|Zezwól na dostęp akwizycyjny/,
  });
  const serverWasOn = await serverToggle.isChecked();
  const gateWasOn = await gateToggle.isChecked();
  if (!serverWasOn) await serverToggle.click();
  if (!gateWasOn) await gateToggle.click();

  const portField = region.getByLabel(/Listen port|Port nasłuchu/);
  const port = Number.parseInt((await portField.inputValue()) || "8317", 10);

  // Credential pre-check (STOP branch): the OS keychain is SHARED with the
  // real app — an already-configured acquisition slot needs owner consent.
  await expect(
    region.getByText(/Brak tokenu akwizycyjnego|No acquisition token yet/),
    "acquisition slot must be EMPTY before §G — a configured token needs owner consent",
  ).toBeVisible();

  await region
    .getByRole("button", {
      name: /Generate acquisition token|Wygeneruj token akwizycyjny/,
    })
    .click();
  const reveal = region.getByLabel(/Acquisition token|Token akwizycyjny/);
  await expect(reveal).toBeVisible();
  const token = await reveal.inputValue();
  expect(token.length).toBeGreaterThan(0);

  let rpcId = 0;
  const rpc = async (method: string, params?: unknown) => {
    const response = await fetch(`http://127.0.0.1:${port}/mcp`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcId, method, params }),
    });
    expect(response.status).toBe(200);
    return (await response.json()) as {
      result?: {
        tools?: { name: string }[];
        isError?: boolean;
        structuredContent?: Record<string, unknown>;
        content?: { type: string; text: string }[];
      };
    };
  };
  const callTool = async (name: string, args: unknown) => {
    const body = await rpc("tools/call", { name, arguments: args });
    const result = body.result;
    expect(result, `result for ${name}`).toBeTruthy();
    const payload =
      result?.structuredContent ??
      (result?.content?.[0]?.text ? JSON.parse(result.content[0].text) : undefined);
    return { isError: result?.isError === true, payload: payload as any };
  };

  try {
    // (1) The scoped surface is exactly the nine acquisition workflow tools.
    const listed = await rpc("tools/list");
    const names = (listed.result?.tools ?? []).map((tool) => tool.name);
    expect(names).toEqual([...ACQUISITION_TOOLS]);
    console.log("scoped tools/list: exactly the 9 acquisition tools in contract order");

    // (0) Preflight on the copy: no active run for this (document, profile).
    const pending = await callTool("list_pending_kpi_ingests", {});
    expect(pending.isError).toBe(false);
    const conflicting = (pending.payload.items as { documentId: string }[]).filter(
      (item) => item.documentId === documentId,
    );
    expect(conflicting, "no pre-existing active run for the chosen document").toEqual([]);

    // (2) Two-phase fresh start: NO scope/quality/period → source_captured.
    const started = await callTool("start_kpi_ingest", {
      documentId,
      profileId: "gpw_ifrs_annual",
    });
    expect(started.isError, JSON.stringify(started.payload)).toBe(false);
    expect(started.payload.status).toBe("source_captured");
    const runId = started.payload.runId as string;
    const frozenHash = started.payload.sourceContentHash as string;
    expect(frozenHash).toMatch(/^[0-9a-f]{64}$/);
    console.log(`start without context → source_captured; frozen hash ${frozenHash}`);

    // (3) Default context: every section present, budgets held.
    const context = await callTool("get_kpi_ingest_context", { runId });
    expect(context.isError, JSON.stringify(context.payload)).toBe(false);
    expect(context.payload.run.runId).toBe(runId);
    expect(context.payload.document.byteSize).toBeGreaterThan(0);
    expect(Array.isArray(context.payload.catalog)).toBe(true);
    expect((context.payload.catalog as unknown[]).length).toBeGreaterThan(0);
    expect(Array.isArray(context.payload.plausibility)).toBe(true);
    expect((context.payload.profileRules as string[]).length).toBeGreaterThan(0);
    const contextBytes = Buffer.byteLength(JSON.stringify(context.payload));
    expect(contextBytes).toBeLessThanOrEqual(262144);
    console.log(
      `default context: catalog=${(context.payload.catalog as unknown[]).length}, ` +
        `plausibility=${(context.payload.plausibility as unknown[]).length}, ` +
        `${contextBytes} bytes (≤256 KiB)`,
    );

    // (4) The portable byte channel: chunk loop → reassemble → hash-verify.
    const chunks: Buffer[] = [];
    let offset = 0;
    let totalBytes = 0;
    for (;;) {
      const chunk = await callTool("get_kpi_ingest_document", {
        runId,
        offset,
        length: 262144,
      });
      expect(chunk.isError, JSON.stringify(chunk.payload)).toBe(false);
      expect(chunk.payload.sha256).toBe(frozenHash);
      totalBytes = chunk.payload.totalBytes as number;
      chunks.push(Buffer.from(chunk.payload.bytesBase64 as string, "base64"));
      offset += chunk.payload.length as number;
      if (chunk.payload.eof) break;
    }
    const reassembled = Buffer.concat(chunks);
    expect(reassembled.length).toBe(totalBytes);
    const digest = crypto.createHash("sha256").update(reassembled).digest("hex");
    expect(digest).toBe(frozenHash);
    console.log(
      `document reassembled from ${chunks.length} chunk(s), ${totalBytes} bytes — ` +
        `sha256 matches the frozen sourceContentHash`,
    );

    // (5) Resume with the discovered context → extracting (two-phase closed).
    const resumed = await callTool("start_kpi_ingest", {
      runId,
      scope: "consolidated",
      dataQuality: "final",
      period: { fiscalYear: 2024, periodType: "FY" },
    });
    expect(resumed.isError, JSON.stringify(resumed.payload)).toBe(false);
    expect(resumed.payload.status).toBe("extracting");
    console.log("resume with full context → extracting");

    // (6) A section call pages the catalog (limit below the cap).
    const sectionPage = await callTool("get_kpi_ingest_context", {
      runId,
      section: "catalog",
      limit: 2,
    });
    expect(sectionPage.isError).toBe(false);
    expect(sectionPage.payload.section).toBe("catalog");
    expect((sectionPage.payload.catalog as unknown[]).length).toBeGreaterThan(0);

    // (7) Cancel and verify the run left the pending list.
    const cancelled = await callTool("cancel_kpi_ingest", { runId });
    expect(cancelled.isError).toBe(false);
    expect(cancelled.payload.status).toBe("cancelled");
    const after = await callTool("list_pending_kpi_ingests", {});
    const afterIds = (after.payload.items as { runId: string }[]).map((item) => item.runId);
    expect(afterIds).not.toContain(runId);
    console.log("cancel → cancelled; run gone from the pending list");
  } finally {
    // Restore: revoke the acquisition token; return toggles to how they were.
    const revokeButtons = region.getByRole("button", {
      name: /Revoke acquisition token|Unieważnij token akwizycyjny/,
    });
    await revokeButtons.first().click();
    await revokeButtons.first().click();
    if (gateWasOn !== (await gateToggle.isChecked())) await gateToggle.click();
    if (serverWasOn !== (await serverToggle.isChecked())) await serverToggle.click();
  }
});
