import { test, expect } from "@playwright/test";
import fs from "node:fs";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// §G live proof for the FULL write loop (#386, ADR 0099 dec. 1) — the crown of
// epic #353: on the REAL Windows app against a DISPOSABLE COPY of the data
// directory, the acquisition bearer drives start → context → a DELIBERATELY
// broken stage (one observation without any citation locator →
// `citation.missing`, Flagged ⇒ outcome REQUIRED `failed`) → the typed repair
// report → a COMPLETE re-staged snapshot with the citation fixed → `ready` →
// commit (acceptedCount > 0) → verbatim replay. The forced failure makes the
// repair loop a guaranteed part of the evidence, never a lucky green.
//
// Env-gated: BRAWLER_T6_DOC_ID (a fetched document in the sandbox DB) and
// BRAWLER_T6_PAYLOAD (path to a gitignored JSON file: { "profileId",
// "scope", "dataQuality", "period": {fiscalYear, periodType},
// "observations": [ObservationInput...], "missingReasons": {...} } — a public
// spec cannot guess real document content). Credential strategy (T4/T5
// precedent): the acquisition slot must be EMPTY (STOP branch otherwise);
// generate → use → revoke; the primary token is never touched. Fact-level DB
// verification happens OUTSIDE this spec, after the sandbox process stops
// (never a cross-OS-open database read).

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("full loop over live MCP: start → stage(broken) → failed → repair → ready → commit → replay", async () => {
  const documentId = process.env.BRAWLER_T6_DOC_ID;
  const payloadPath = process.env.BRAWLER_T6_PAYLOAD;
  test.skip(!documentId || !payloadPath, "BRAWLER_T6_DOC_ID/BRAWLER_T6_PAYLOAD not set");
  const payload = JSON.parse(fs.readFileSync(payloadPath!, "utf8")) as {
    profileId: string;
    scope: string;
    dataQuality: string;
    period: { fiscalYear: number; periodType: string };
    observations: Record<string, unknown>[];
    missingReasons: Record<string, string>;
  };
  expect(payload.observations.length).toBeGreaterThan(0);
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

  // Credential STOP branch: the OS keychain is SHARED with the real app — an
  // already-configured acquisition slot needs owner consent.
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
    // (1) The scoped surface is COMPLETE: nine tools, contract order, and the
    // raw response honours the 16 KiB compact-surface gate.
    const rawList = await fetch(`http://127.0.0.1:${port}/mcp`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcId, method: "tools/list" }),
    });
    const rawBody = await rawList.text();
    expect(Buffer.byteLength(rawBody)).toBeLessThanOrEqual(16 * 1024);
    const names = (JSON.parse(rawBody).result?.tools ?? []).map(
      (tool: { name: string }) => tool.name,
    );
    expect(names).toEqual([
      "start_kpi_ingest",
      "list_pending_kpi_ingests",
      "get_kpi_ingest_context",
      "get_kpi_ingest_document",
      "stage_kpi_observations",
      "validate_kpi_ingest",
      "commit_kpi_ingest",
      "get_kpi_ingest_status",
      "cancel_kpi_ingest",
    ]);
    console.log("scoped tools/list: all NINE tools, contract order, ≤16 KiB");

    // (2) Start fresh with full context → extracting.
    const started = await callTool("start_kpi_ingest", {
      documentId,
      profileId: payload.profileId,
      scope: payload.scope,
      dataQuality: payload.dataQuality,
      period: payload.period,
    });
    expect(started.isError, JSON.stringify(started.payload)).toBe(false);
    expect(started.payload.status).toBe("extracting");
    const runId = started.payload.runId as string;

    // (3) Context: catalog + plausibility inform the extraction.
    const context = await callTool("get_kpi_ingest_context", { runId });
    expect(context.isError).toBe(false);
    expect((context.payload.catalog as unknown[]).length).toBeGreaterThan(0);
    console.log(
      `context: catalog=${(context.payload.catalog as unknown[]).length}, ` +
        `profileRules=${(context.payload.profileRules as unknown[]).length}`,
    );

    // (4) Stage revision 1 DELIBERATELY broken: strip every citation from the
    // first observation → citation.missing → validation must FAIL.
    const broken = payload.observations.map((observation, index) => {
      if (index === 0) {
        const { citation: _citation, ...rest } = observation as {
          citation?: unknown;
        };
        return rest;
      }
      return observation;
    });
    const staged1 = await callTool("stage_kpi_observations", {
      runId,
      observations: broken,
      missingReasons: payload.missingReasons,
      execution: { client: "live-spec", tokensIn: 100 },
    });
    expect(staged1.isError, JSON.stringify(staged1.payload)).toBe(false);
    expect(staged1.payload.revision).toBe(1);

    const failed = await callTool("validate_kpi_ingest", { runId, revision: 1 });
    expect(failed.isError, JSON.stringify(failed.payload)).toBe(false);
    expect(failed.payload.outcome, "revision 1 must FAIL — the forced repair").toBe("failed");
    expect(JSON.stringify(failed.payload.manifest)).toContain("citation.missing");
    console.log("validate(1) → failed with citation.missing — the typed repair report");

    // (5) Repair: revision 2 is the COMPLETE snapshot with citations intact.
    const staged2 = await callTool("stage_kpi_observations", {
      runId,
      observations: payload.observations,
      missingReasons: payload.missingReasons,
      execution: { client: "live-spec", repairRounds: 1 },
    });
    expect(staged2.isError, JSON.stringify(staged2.payload)).toBe(false);
    expect(staged2.payload.revision).toBe(2);

    const ready = await callTool("validate_kpi_ingest", { runId, revision: 2 });
    expect(ready.isError, JSON.stringify(ready.payload)).toBe(false);
    expect(ready.payload.outcome, JSON.stringify(ready.payload.manifest)).toBe("ready");
    const manifestHash = ready.payload.manifestHash as string;
    console.log("re-stage complete snapshot → validate(2) → ready");

    // (6) Commit → receipt with real accepted facts.
    const receipt = await callTool("commit_kpi_ingest", {
      runId,
      manifestHash,
      revision: 2,
      execution: { client: "live-spec", costUsd: 0.01 },
    });
    expect(receipt.isError, JSON.stringify(receipt.payload)).toBe(false);
    expect(receipt.payload.acceptedCount as number).toBeGreaterThan(0);
    const factIds = (receipt.payload.outcomes as { factId?: string }[])
      .map((outcome) => outcome.factId)
      .filter(Boolean);
    console.log(
      `commit → ${receipt.payload.terminalStatus}, accepted=${receipt.payload.acceptedCount}, ` +
        `factIds=${JSON.stringify(factIds)}`,
    );

    // (7) Terminal status + merged execution metadata.
    const status = await callTool("get_kpi_ingest_status", { runId });
    expect(["complete", "partial"]).toContain(status.payload.status);
    expect(status.payload.execution.client).toBe("live-spec");

    // (8) Replay is verbatim — byte-equal payloads.
    const replay = await callTool("commit_kpi_ingest", {
      runId,
      manifestHash,
      revision: 2,
    });
    expect(JSON.stringify(replay.payload)).toBe(JSON.stringify(receipt.payload));
    console.log("commit replay: receipt verbatim");

    // Hand the fact ids to the post-spec DB verification (runs after the
    // sandbox process stops — never against a cross-OS-open database).
    fs.writeFileSync(
      "/tmp/claude-1000/t6-fact-ids.json",
      JSON.stringify({ runId, factIds }, null, 2),
    );
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
