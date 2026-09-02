import fs from "node:fs";
import path from "node:path";
import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { ACQUISITION_TOOLS } from "./helpers/acquisitionTools";

// §G live proof for the run-lifecycle tools (#384, ADR 0099): on the REAL
// Windows app running against a DISPOSABLE COPY of the data directory, the
// acquisition bearer drives the full write flow — scoped tools/list (exactly
// the four tools), fresh start on a REAL captured document with full context
// (must reach `extracting` — proves claim + blob pin + set-once attach +
// begin_extracting live), status, bare-runId keepalive (attemptCount frozen),
// cancel — and the content-addressed blob exists on disk.
//
// Env-gated: refuses to run without BRAWLER_T4_DOC_ID (a fetched document in
// the sandbox DB) and BRAWLER_T4_DATA_DIR (the sandbox data dir as a WSL
// path), so an ordinary live sweep never triggers it. Credential strategy
// (sol R2 H-1): the acquisition token is generated fresh in the sandbox app,
// used, and revoked at cleanup; the PRIMARY token is never touched.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("run lifecycle over live MCP: start → status → keepalive → cancel + blob on disk", async () => {
  const documentId = process.env.BRAWLER_T4_DOC_ID;
  const dataDir = process.env.BRAWLER_T4_DATA_DIR;
  test.skip(!documentId || !dataDir, "BRAWLER_T4_DOC_ID/BRAWLER_T4_DATA_DIR not set");
  const { page } = connection;
  test.setTimeout(180_000);

  // Settings → MCP section.
  await page.getByRole("button", { name: /^(Settings|Ustawienia)$/ }).click();
  const region = page.getByLabel(/Application settings|Ustawienia aplikacji/);
  await region.getByRole("button", { name: /MCP/ }).click();

  const serverToggle = region.getByRole("switch", {
    name: /Let assistants connect|Pozwól asystentom się łączyć/,
  });
  const gateToggle = region.getByRole("switch", {
    name: /Allow report-data processing|Pozwól przetwarzać dane raportów/,
  });
  const serverWasOn = await serverToggle.isChecked();
  const gateWasOn = await gateToggle.isChecked();

  // The sandbox copy carries the owner's settings; make sure the listener is
  // up (the primary token lives in the SHARED keychain and is only read,
  // never rotated) and the acquisition gate is open.
  if (!serverWasOn) await serverToggle.click();
  if (!gateWasOn) await gateToggle.click();

  const portField = region.getByLabel(/^Port$/);
  const port = Number.parseInt((await portField.inputValue()) || "8317", 10);

  // Credential pre-check (sol R2 H-1 STOP branch): the OS keychain is SHARED
  // with the real app — generating over an already-configured acquisition
  // token would invalidate the owner's external clients. Refuse loudly and
  // let the owner decide; an empty slot proceeds generate → use → revoke.
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
    // (1) The scoped surface is exactly the ten acquisition workflow tools.
    const listed = await rpc("tools/list");
    const names = (listed.result?.tools ?? []).map((tool) => tool.name);
    expect(names).toEqual([...ACQUISITION_TOOLS]);
    console.log("scoped tools/list on the live app: exactly the 4 lifecycle tools");

    // (2) Fresh start with FULL context on a real captured document → the
    // run must reach `extracting` (attach + begin_extracting proven live).
    const started = await callTool("start_kpi_ingest", {
      documentId,
      profileId: "gpw_ifrs_annual",
      scope: "consolidated",
      dataQuality: "final",
      period: { fiscalYear: 2024, periodType: "FY" },
    });
    expect(started.isError, JSON.stringify(started.payload)).toBe(false);
    expect(started.payload.status).toBe("extracting");
    expect(started.payload.lease.holder).toBe("mcp:kpi_acquisition");
    expect(started.payload.attemptCount).toBe(1);
    const runId = started.payload.runId as string;
    const hash = started.payload.sourceContentHash as string;
    expect(hash).toMatch(/^[0-9a-f]{64}$/);

    // The content-addressed blob exists on disk in the sandbox data dir.
    const blobPath = path.join(dataDir!, "report_snapshots", hash);
    expect(fs.existsSync(blobPath), blobPath).toBe(true);
    console.log(`start → extracting; blob pinned at report_snapshots/${hash}`);

    // (3) Pure-read status agrees.
    const status = await callTool("get_kpi_ingest_status", { runId });
    expect(status.isError).toBe(false);
    expect(status.payload.status).toBe("extracting");

    // (4) Bare-runId keepalive: attemptCount frozen.
    const keepalive = await callTool("start_kpi_ingest", { runId });
    expect(keepalive.isError).toBe(false);
    expect(keepalive.payload.attemptCount).toBe(1);
    console.log("bare-runId keepalive: attemptCount stays 1");

    // (5) The run is visible on the pending list.
    const pending = await callTool("list_pending_kpi_ingests", {});
    expect(pending.isError).toBe(false);
    const runIds = (pending.payload.items as { runId: string }[]).map((item) => item.runId);
    expect(runIds).toContain(runId);

    // (6) Cancel → cancelled, and it leaves the pending list.
    const cancelled = await callTool("cancel_kpi_ingest", { runId });
    expect(cancelled.isError).toBe(false);
    expect(cancelled.payload.status).toBe("cancelled");
    const after = await callTool("list_pending_kpi_ingests", {});
    const afterIds = (after.payload.items as { runId: string }[]).map((item) => item.runId);
    expect(afterIds).not.toContain(runId);
    console.log("cancel → cancelled; run gone from the pending list");

    // (7) A fictional run is a typed not_found in the error envelope.
    const missing = await callTool("get_kpi_ingest_status", {
      runId: "kpiing_ffffffffffffffffffffffffffffffff",
    });
    expect(missing.isError).toBe(true);
    expect(JSON.stringify(missing.payload)).toContain("not_found");
  } finally {
    // Restore: revoke the acquisition token (trigger + inline confirm carry
    // the same label); return the gate and server toggles to how they were.
    const revokeButtons = region.getByRole("button", {
      name: /Remove acquisition token|Usuń token akwizycyjny/,
    });
    await revokeButtons.first().click();
    await revokeButtons.first().click();
    if (gateWasOn !== (await gateToggle.isChecked())) await gateToggle.click();
    if (serverWasOn !== (await serverToggle.isChecked())) await serverToggle.click();
  }
});
