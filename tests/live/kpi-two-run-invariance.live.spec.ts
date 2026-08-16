import { test, expect } from "@playwright/test";
import fs from "node:fs";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { ACQUISITION_TOOLS } from "./helpers/acquisitionTools";
import { canonicalizeManifest } from "./helpers/manifestCanonical";

// §G live proof for #389 (T9), the automated leg. HONEST scope: this is
// CROSS-DRIVER SERVER-INVARIANCE, not two real MCP clients (a distinct
// `execution.client` is only diagnostic metadata, ADR 0099 dec. 8). The genuine
// Claude+Codex dogfood is Noga F (owner/agent-driven, separate). Here two
// drivers share one acquisition token (so the SAME lease holder — cooperative,
// never adversarial) over the same raw fetch(/mcp, Bearer) channel and prove:
//   1. the scoped surface is exactly the nine acquisition tools, ≤16 KiB;
//   2. no write capability leaks onto the scoped token (raw JSON-RPC -32602);
//   3. SERVER INVARIANCE: two SEQUENTIAL runs of the same document, staged with
//      the SAME observations against the SAME (empty) fact store, produce a
//      byte-identical manifest after canonicalizing run identity;
//   4. cross-run COMMIT determinism: a second committed run reobserves the first
//      run's facts — no `created`/`upgraded`, identical factIds (no duplicates);
//   5. cooperative resume: a bare start(runId) keepalive renews without
//      incrementing attemptCount while the lease is live;
//   6. context sufficiency: the source is readable via chunked
//      get_kpi_ingest_document to EOF (no other read needed).
//
// Env-gated on BRAWLER_T6_DOC_ID + BRAWLER_T6_PAYLOAD (a gitignored real
// observations JSON — a public spec cannot guess real document content). Runs
// on a DISPOSABLE COPY of the data dir; the acquisition token is generated →
// used → revoked; the primary token is never touched.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("two-run server-invariance: identical manifest + reobserve + cooperative resume", async () => {
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

  // Credential STOP branch: the OS keychain is SHARED with the real app.
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
  // A raw JSON-RPC call: returns the whole envelope (result AND error), needed
  // for the no-write assertion (an unknown tool answers with a top-level
  // `error: -32602`, never a `result`).
  const rpc = async (method: string, params?: unknown) => {
    const response = await fetch(`http://127.0.0.1:${port}/mcp`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
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
      error?: { code: number; message: string };
    };
  };
  // A tool call that expects a domain result. `client` labels the driver via
  // execution metadata (diagnostic only) so "alpha"/"beta" appear in cost_json.
  const callTool = async (name: string, args: unknown) => {
    const body = await rpc("tools/call", { name, arguments: args });
    const result = body.result;
    expect(result, `result for ${name}`).toBeTruthy();
    const structured =
      result?.structuredContent ??
      (result?.content?.[0]?.text ? JSON.parse(result.content[0].text) : undefined);
    return { isError: result?.isError === true, payload: structured as Record<string, any> };
  };

  // start_kpi_ingest carries no `execution` field (contract: only stage/commit
  // do) — the driver label rides on those calls instead.
  const startFresh = async () => {
    const started = await callTool("start_kpi_ingest", {
      documentId,
      profileId: payload.profileId,
      scope: payload.scope,
      dataQuality: payload.dataQuality,
      period: payload.period,
    });
    expect(started.isError, JSON.stringify(started.payload)).toBe(false);
    expect(started.payload.status).toBe("extracting");
    return started.payload.runId as string;
  };
  const stage = async (runId: string, client: string) => {
    const staged = await callTool("stage_kpi_observations", {
      runId,
      observations: payload.observations,
      missingReasons: payload.missingReasons,
      execution: { client },
    });
    expect(staged.isError, JSON.stringify(staged.payload)).toBe(false);
    return staged.payload.revision as number;
  };
  const validateReady = async (runId: string, revision: number) => {
    const validated = await callTool("validate_kpi_ingest", { runId, revision });
    expect(validated.isError, JSON.stringify(validated.payload)).toBe(false);
    expect(validated.payload.outcome, JSON.stringify(validated.payload.manifest)).toBe("ready");
    return validated.payload as { manifest: unknown; manifestHash: string };
  };

  try {
    // (1) Scoped surface: exactly the nine tools, ≤16 KiB.
    const rawList = await fetch(`http://127.0.0.1:${port}/mcp`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcId, method: "tools/list" }),
    });
    const rawBody = await rawList.text();
    expect(Buffer.byteLength(rawBody)).toBeLessThanOrEqual(16 * 1024);
    const names = (JSON.parse(rawBody).result?.tools ?? []).map((t: { name: string }) => t.name);
    expect(names).toEqual([...ACQUISITION_TOOLS]);
    console.log("scoped tools/list: nine tools, contract order, ≤16 KiB");

    // (2) No write capability leaks onto the scoped token: a non-allowlisted
    //     tool is UNKNOWN — a top-level JSON-RPC -32602, never a result.
    const forbidden = await rpc("tools/call", {
      name: "create_notebook_entry",
      arguments: { text: "x" },
    });
    expect(forbidden.result, "a write tool must not exist on the scoped surface").toBeFalsy();
    expect(forbidden.error?.code).toBe(-32602);
    console.log("no-write-caps: create_notebook_entry → -32602 unknown tool");

    // (3) SERVER INVARIANCE — two runs cancelled without committing, so both
    //     validate against the SAME (empty) fact store; identical observations
    //     ⇒ byte-identical canonicalized manifest.
    const runA = await startFresh();
    // (6) Context sufficiency: read the source only via chunked document reads.
    const context = await callTool("get_kpi_ingest_context", { runId: runA });
    expect(context.isError).toBe(false);
    let offset = 0;
    let eof = false;
    let totalBytes = Infinity;
    let chunks = 0;
    // Drive the loop by the document's own totalBytes (reports can exceed the
    // 16 MiB a fixed 64-chunk cap would allow); keep a generous safety cap.
    while (!eof && offset < totalBytes && chunks < 512) {
      const chunk = await callTool("get_kpi_ingest_document", {
        runId: runA,
        offset,
        length: 262144,
      });
      expect(chunk.isError).toBe(false);
      totalBytes = chunk.payload.totalBytes as number;
      offset += chunk.payload.length as number;
      eof = chunk.payload.eof === true;
      chunks += 1;
    }
    expect(eof, `document readable via chunked reads to EOF (${offset}/${totalBytes} B)`).toBe(true);
    const revA = await stage(runA, "alpha");
    const manifestA = (await validateReady(runA, revA)).manifest;
    await callTool("cancel_kpi_ingest", { runId: runA });

    const runB = await startFresh();
    const revB = await stage(runB, "beta");
    const manifestB = (await validateReady(runB, revB)).manifest;
    await callTool("cancel_kpi_ingest", { runId: runB });

    expect(
      canonicalizeManifest(manifestB as never),
      "two runs of the same document, same observations, same store → identical manifest",
    ).toEqual(canonicalizeManifest(manifestA as never));
    console.log("server invariance: canonicalized manifests are byte-identical across runs");

    // (4) CROSS-RUN COMMIT determinism — run C creates the facts, run D commits
    //     the same observations and must reobserve them (no new canonical facts).
    const runC = await startFresh();
    const revC = await stage(runC, "alpha");
    const readyC = await validateReady(runC, revC);
    const receiptC = await callTool("commit_kpi_ingest", {
      runId: runC,
      manifestHash: readyC.manifestHash,
      revision: revC,
      execution: { client: "alpha" },
    });
    expect(receiptC.isError).toBe(false);
    expect(receiptC.payload.acceptedCount as number).toBeGreaterThan(0);
    const factIdByKey = new Map<string, string>();
    for (const outcome of receiptC.payload.outcomes as {
      metricKey: string;
      factId?: string;
    }[]) {
      if (outcome.factId) factIdByKey.set(outcome.metricKey, outcome.factId);
    }

    const runD = await startFresh();
    const revD = await stage(runD, "beta");
    const readyD = await validateReady(runD, revD);
    const receiptD = await callTool("commit_kpi_ingest", {
      runId: runD,
      manifestHash: readyD.manifestHash,
      revision: revD,
      execution: { client: "beta" },
    });
    expect(receiptD.isError).toBe(false);
    expect(receiptD.payload.acceptedCount as number).toBeGreaterThan(0);
    for (const outcome of receiptD.payload.outcomes as {
      metricKey: string;
      factId?: string;
      outcome: string;
    }[]) {
      expect(
        ["created", "upgraded"].includes(outcome.outcome),
        `run D must not create/upgrade canonical facts, got ${outcome.outcome} for ${outcome.metricKey}`,
      ).toBe(false);
      if (outcome.outcome === "reobserved") {
        expect(outcome.factId, `reobserved ${outcome.metricKey} carries a factId`).toBeTruthy();
        const priorFactId = factIdByKey.get(outcome.metricKey);
        if (priorFactId) {
          expect(outcome.factId, `run D reobserves run C's fact for ${outcome.metricKey}`).toBe(
            priorFactId,
          );
        }
      }
    }
    console.log("cross-run commit: run D reobserved run C's facts, no created/upgraded");

    // (5) Cooperative resume — a fresh run's bare start(runId) keepalive renews
    //     the LIVE lease without incrementing attemptCount.
    const runE = await startFresh();
    const before = await callTool("get_kpi_ingest_status", { runId: runE });
    const attemptsBefore = before.payload.attemptCount as number;
    const resumed = await callTool("start_kpi_ingest", { runId: runE });
    expect(resumed.isError, JSON.stringify(resumed.payload)).toBe(false);
    const after = await callTool("get_kpi_ingest_status", { runId: runE });
    expect(
      after.payload.attemptCount,
      "a live keepalive by a second driver must not increment attemptCount",
    ).toBe(attemptsBefore);
    await callTool("cancel_kpi_ingest", { runId: runE });
    console.log("cooperative resume: bare start(runId) keepalive froze attemptCount");
  } finally {
    const revokeButtons = region.getByRole("button", {
      name: /Revoke acquisition token|Unieważnij token akwizycyjny/,
    });
    await revokeButtons.first().click();
    await revokeButtons.first().click();
    if (gateWasOn !== (await gateToggle.isChecked())) await gateToggle.click();
    if (serverWasOn !== (await serverToggle.isChecked())) await serverToggle.click();
  }
});
