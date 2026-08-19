import { test, expect, type Page } from "@playwright/test";
import fs from "node:fs";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// §G live proof for epic #399 S8 (full-capture doctrine, ADR 0101/0102): on the
// REAL Windows app against a DISPOSABLE COPY of the data directory, the
// acquisition bearer drives the profile @v2 doctrine end to end — propose a
// missing catalog key, capture an excluded (deliberately unmapped, reasoned)
// observation alongside mapped ones, validate a bounded summary, commit a
// bounded summary, and read the full excluded ledger back via the paged
// receipt — plus the chunked-draft path at report scale (open → append →
// idempotent replay → finalize → validate), used for an interim report whose
// observation count exceeds the 100-per-call cap.
//
// Env-gated (two independent payloads, run one per test):
// - BRAWLER_S8_RB_DOC_ID / BRAWLER_S8_RB_PAYLOAD — test 1, `gpw_preliminary`.
// - BRAWLER_S8_Q3_DOC_ID / BRAWLER_S8_Q3_PAYLOAD — test 2, `gpw_interim`.
// Payload JSON shape: `{ period, dataQuality, scope, observations:
// ObservationInput[], missingReasons, proposals?: [{metricKey, label, unit?,
// statementGroup, description?}] }` — observations are ALREADY wire-shaped (a
// public spec cannot guess real document content). Credential strategy (T4-T6
// precedent): the acquisition slot must be EMPTY (STOP branch otherwise);
// generate → use → revoke per test; the primary token is never touched.

type ObservationInput = Record<string, unknown> & {
  mappingStatus?: string;
  rawLabel?: string;
  exclusionReason?: string;
};

type ProposalInput = {
  metricKey: string;
  label: string;
  unit?: string;
  statementGroup: string;
  description?: string;
};

type S8Payload = {
  period: { fiscalYear: number; periodType: string };
  dataQuality: string;
  scope: string;
  observations: ObservationInput[];
  missingReasons: Record<string, string>;
  proposals?: ProposalInput[];
};

function loadPayload(envVar: string): S8Payload {
  return JSON.parse(fs.readFileSync(process.env[envVar]!, "utf8")) as S8Payload;
}

type RpcResponse = {
  result?: {
    tools?: { name: string }[];
    isError?: boolean;
    structuredContent?: Record<string, unknown>;
    content?: { type: string; text: string }[];
  };
};

type ToolCall = (name: string, args: unknown) => Promise<{ isError: boolean; payload: any }>;

/** Settings → MCP → generate-token ritual shared by every acquisition live spec. */
async function openAcquisitionSession(page: Page) {
  const region = page.getByLabel(/Application settings|Ustawienia aplikacji/);
  // A previous test in this file leaves the app on the Settings screen — the
  // nav button is only clickable from outside Settings.
  if (!(await region.isVisible().catch(() => false))) {
    await page.getByRole("button", { name: /^(Settings|Ustawienia)$/ }).click();
  }
  await region.getByRole("button", { name: /MCP/ }).click();

  const serverToggle = region.getByRole("switch", { name: /Enable the server|Włącz serwer/ });
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
    .getByRole("button", { name: /Generate acquisition token|Wygeneruj token akwizycyjny/ })
    .click();
  const reveal = region.getByLabel(/Acquisition token|Token akwizycyjny/);
  await expect(reveal).toBeVisible();
  const token = await reveal.inputValue();
  expect(token.length).toBeGreaterThan(0);

  let rpcId = 0;
  const rpc = async (method: string, params?: unknown) => {
    const response = await fetch(`http://127.0.0.1:${port}/mcp`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcId, method, params }),
      // A silent hang here previously ate the whole test timeout — fail typed.
      // 120s: the startup re-extraction sweep over a full real-data copy can
      // hold the writer for minutes; the claim then queues behind it (§G
      // finding, ADR 0059 fairness — logged in the S8 report).
      signal: AbortSignal.timeout(120_000),
    });
    expect(response.status).toBe(200);
    return (await response.json()) as RpcResponse;
  };

  // The server toggle may have just flipped off→on (a prior test's close()
  // restores original states) — wait until the socket actually answers.
  await expect
    .poll(
      async () => {
        try {
          const r = await fetch(`http://127.0.0.1:${port}/mcp`, {
            method: "POST",
            headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
            body: JSON.stringify({ jsonrpc: "2.0", id: 0, method: "tools/list" }),
            signal: AbortSignal.timeout(2_000),
          });
          return r.status;
        } catch {
          return 0;
        }
      },
      { message: "MCP server did not come up on the configured port", timeout: 30_000 },
    )
    .toBe(200);
  const callTool: ToolCall = async (name, args) => {
    const body = await rpc("tools/call", { name, arguments: args });
    const result = body.result;
    expect(result, `result for ${name}`).toBeTruthy();
    const payload =
      result?.structuredContent ??
      (result?.content?.[0]?.text ? JSON.parse(result.content[0].text) : undefined);
    return { isError: result?.isError === true, payload: payload as any };
  };

  const close = async () => {
    const revokeButtons = region.getByRole("button", {
      name: /Revoke acquisition token|Unieważnij token akwizycyjny/,
    });
    await revokeButtons.first().click();
    // The second click is the confirm step — and the slot must actually read
    // empty afterwards, or the next test's STOP branch trips on our leftover.
    await revokeButtons.first().click();
    await expect(
      region.getByText(/Brak tokenu akwizycyjnego|No acquisition token yet/),
      "revocation must leave the acquisition slot empty",
    ).toBeVisible();
    if (gateWasOn !== (await gateToggle.isChecked())) await gateToggle.click();
    if (serverWasOn !== (await serverToggle.isChecked())) await serverToggle.click();
  };

  return { callTool, close };
}

/** Pages a `get_kpi_ingest_context` section (catalog/manifest/receipt) to exhaustion. */
async function pageSection<T>(
  callTool: ToolCall,
  runId: string,
  section: "catalog" | "manifest" | "receipt",
  extract: (payload: any) => T[],
): Promise<T[]> {
  const items: T[] = [];
  let cursor: string | undefined;
  for (;;) {
    const args: Record<string, unknown> = { runId, section };
    if (cursor !== undefined) args.cursor = cursor;
    const page = await callTool("get_kpi_ingest_context", args);
    expect(page.isError, JSON.stringify(page.payload)).toBe(false);
    items.push(...extract(page.payload));
    const next = page.payload.nextCursor as string | null;
    if (!next) return items;
    cursor = next;
  }
}

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("preliminary@v2 full capture with propose and excluded", async () => {
  const documentId = process.env.BRAWLER_S8_RB_DOC_ID;
  const payloadPath = process.env.BRAWLER_S8_RB_PAYLOAD;
  test.skip(!documentId || !payloadPath, "BRAWLER_S8_RB_DOC_ID/BRAWLER_S8_RB_PAYLOAD not set");
  const payload = loadPayload("BRAWLER_S8_RB_PAYLOAD");
  expect(payload.observations.length).toBeGreaterThan(0);
  test.setTimeout(300_000);

  const session = await openAcquisitionSession(connection.page);
  const { callTool } = session;
  try {
    // (1) Start on the RB doc under `gpw_preliminary` — resolved to the @v2
    // cutover (ADR 0102 dec. 13): full-capture doctrine, not the @v1
    // narrower-mapping instruction.
    const started = await callTool("start_kpi_ingest", {
      documentId,
      profileId: "gpw_preliminary",
      scope: payload.scope,
      dataQuality: payload.dataQuality,
      period: payload.period,
    });
    expect(started.isError, JSON.stringify(started.payload)).toBe(false);
    expect(started.payload.profileVersion).toBe("gpw_preliminary@v2");
    const runId = started.payload.runId as string;
    console.log(`start → ${started.payload.status}, profileVersion=gpw_preliminary@v2, runId=${runId}`);

    // (2) Default context carries `notRequested`; page `catalog` to
    // exhaustion (ADR 0101 dec. 7 doctrine: page before proposing).
    const context = await callTool("get_kpi_ingest_context", { runId });
    expect(context.isError, JSON.stringify(context.payload)).toBe(false);
    expect(Array.isArray(context.payload.notRequested)).toBe(true);
    const expectedKeys = (context.payload.run as { expectedKpis: { keys: string[] } }).expectedKpis
      .keys;

    const catalogEntries = await pageSection<{ metricKey: string }>(
      callTool,
      runId,
      "catalog",
      (p) => p.catalog ?? [],
    );
    const catalogMetricKeys = new Set(catalogEntries.map((entry) => entry.metricKey));
    console.log(
      `context: notRequested=${(context.payload.notRequested as unknown[]).length}, ` +
        `catalog paged to exhaustion=${catalogEntries.length} entries (${catalogMetricKeys.size} unique keys)`,
    );

    // (3) Propose every payload proposal; each response is created:true,
    // created:false (reuse), or a typed synonym_redirect refusal.
    const proposalOutcomes: string[] = [];
    for (const proposal of payload.proposals ?? []) {
      const response = await callTool("propose_kpi_definition", { runId, ...proposal });
      if (response.isError) {
        expect(JSON.stringify(response.payload)).toContain("synonym_redirect");
        proposalOutcomes.push(`${proposal.metricKey}=synonym_redirect`);
      } else {
        expect(typeof response.payload.created).toBe("boolean");
        proposalOutcomes.push(
          `${proposal.metricKey}=${response.payload.created ? "created" : "reused"}`,
        );
      }
    }
    console.log(`propose_kpi_definition: ${proposalOutcomes.join(", ") || "(no proposals in payload)"}`);

    // (4) Single-call stage — the complete revision-1 snapshot.
    const staged = await callTool("stage_kpi_observations", {
      runId,
      observations: payload.observations,
      missingReasons: payload.missingReasons,
      execution: { client: "live-spec-s8-rb" },
    });
    expect(staged.isError, JSON.stringify(staged.payload)).toBe(false);
    expect(staged.payload.revision).toBe(1);

    // (5) Validate — bounded summary, no inline `manifest`, must reach ready.
    const validated = await callTool("validate_kpi_ingest", { runId, revision: 1 });
    expect(validated.isError, JSON.stringify(validated.payload)).toBe(false);
    expect(validated.payload).not.toHaveProperty("manifest");
    expect(validated.payload.outcome, JSON.stringify(validated.payload.severityCounts)).toBe(
      "ready",
    );
    console.log(`validate → ready, severityCounts=${JSON.stringify(validated.payload.severityCounts)}`);

    // (6) Manifest section: the excluded observations carry `mapping.excluded`
    // (Info severity — never forces `flagged`); no observation is flagged.
    const manifestObservations = await pageSection<{
      excluded: boolean;
      validationState: string;
      codes: { code: string }[];
    }>(callTool, runId, "manifest", (p) => p.manifest?.observations ?? []);
    const excludedInManifest = manifestObservations.filter((o) => o.excluded === true);
    expect(excludedInManifest.length).toBeGreaterThan(0);
    for (const observation of excludedInManifest) {
      expect(observation.codes.some((c) => c.code === "mapping.excluded")).toBe(true);
      expect(observation.validationState).not.toBe("flagged");
    }
    expect(manifestObservations.some((o) => o.validationState === "flagged")).toBe(false);
    console.log(`manifest: ${excludedInManifest.length} excluded observations, none flagged`);

    // (7) Commit — bounded summary, no inline `outcomes`; terminalStatus is
    // derived from whether the RUN'S OWN expected-keys union has a
    // missingReasons entry, never assumed from a hardcoded key name.
    const anyExpectedMissing = expectedKeys.some((key) => key in payload.missingReasons);
    const expectedTerminalStatus = anyExpectedMissing ? "partial" : "complete";
    console.log(
      `expectedKpis.keys=${JSON.stringify(expectedKeys)}, ` +
        `missingReasons=${JSON.stringify(Object.keys(payload.missingReasons))} → ` +
        `expect terminalStatus=${expectedTerminalStatus}`,
    );

    const committed = await callTool("commit_kpi_ingest", {
      runId,
      manifestHash: validated.payload.manifestHash,
      revision: 1,
      execution: { client: "live-spec-s8-rb" },
    });
    expect(committed.isError, JSON.stringify(committed.payload)).toBe(false);
    expect(committed.payload).not.toHaveProperty("outcomes");
    expect(committed.payload.terminalStatus).toBe(expectedTerminalStatus);

    // (8) Receipt section: the full excluded ledger, cross-checked against
    // the payload's own excluded rows.
    const excludedRows = payload.observations.filter((o) => o.mappingStatus === "excluded");
    expect(excludedRows.length).toBeGreaterThanOrEqual(2);
    expect(committed.payload.counts.excludedCount).toBe(excludedRows.length);

    const receiptOutcomes = await pageSection<{
      outcome: string;
      detail?: { label: string; reason: string };
    }>(callTool, runId, "receipt", (p) => p.receipt?.outcomes ?? []);
    const excludedLedger = receiptOutcomes
      .filter((o) => o.outcome === "excluded")
      .map((o) => ({ label: o.detail!.label, reason: o.detail!.reason }))
      .sort((a, b) => a.label.localeCompare(b.label));
    const expectedLedger = excludedRows
      .map((o) => ({ label: o.rawLabel as string, reason: o.exclusionReason as string }))
      .sort((a, b) => a.label.localeCompare(b.label));
    expect(excludedLedger).toEqual(expectedLedger);
    console.log(
      `receipt: excludedCount=${excludedRows.length}, ledger matches payload's excluded rows`,
    );
  } finally {
    await session.close();
  }
});

test("interim chunked draft at real scale", async () => {
  const documentId = process.env.BRAWLER_S8_Q3_DOC_ID;
  const payloadPath = process.env.BRAWLER_S8_Q3_PAYLOAD;
  test.skip(!documentId || !payloadPath, "BRAWLER_S8_Q3_DOC_ID/BRAWLER_S8_Q3_PAYLOAD not set");
  const payload = loadPayload("BRAWLER_S8_Q3_PAYLOAD");
  expect(payload.observations.length).toBeGreaterThan(0);
  test.setTimeout(300_000);

  const session = await openAcquisitionSession(connection.page);
  const { callTool } = session;
  try {
    // (1) Start on the Q3 doc under `gpw_interim` — no `@v2` yet (data-model
    // § Extraction-profile registry): resolves to `@v1`.
    const started = await callTool("start_kpi_ingest", {
      documentId,
      profileId: "gpw_interim",
      scope: payload.scope,
      dataQuality: payload.dataQuality,
      period: payload.period,
    });
    expect(started.isError, JSON.stringify(started.payload)).toBe(false);
    expect(started.payload.profileVersion).toBe("gpw_interim@v1");
    const runId = started.payload.runId as string;
    const total = payload.observations.length;
    console.log(`start → ${started.payload.status}, profileVersion=gpw_interim@v1, runId=${runId}`);

    // (2) Open a chunked draft for the whole report.
    const opened = await callTool("stage_kpi_observations", {
      runId,
      draft: { open: true, expectedObservations: total },
    });
    expect(opened.isError, JSON.stringify(opened.payload)).toBe(false);
    expect(opened.payload.status).toBe("draft_open");
    const draftId = opened.payload.draftId as string;

    // (3) Append in chunks of ≤100; replay the first chunk verbatim →
    // idempotent ack (same acknowledgment shape, no error).
    const CHUNK_SIZE = 100;
    const chunks: ObservationInput[][] = [];
    for (let i = 0; i < total; i += CHUNK_SIZE) chunks.push(payload.observations.slice(i, i + CHUNK_SIZE));

    let firstAck: unknown;
    for (let chunkIndex = 0; chunkIndex < chunks.length; chunkIndex++) {
      const appended = await callTool("stage_kpi_observations", {
        runId,
        draft: { draftId, chunkIndex },
        observations: chunks[chunkIndex],
      });
      expect(appended.isError, JSON.stringify(appended.payload)).toBe(false);
      expect(appended.payload.status).toBe("draft_appended");
      expect(appended.payload.chunkIndex).toBe(chunkIndex);
      if (chunkIndex === 0) firstAck = appended.payload;
    }

    const replay = await callTool("stage_kpi_observations", {
      runId,
      draft: { draftId, chunkIndex: 0 },
      observations: chunks[0],
    });
    expect(replay.isError, JSON.stringify(replay.payload)).toBe(false);
    // Idempotent ack: identical to the first ack except chunksReceived, which
    // truthfully reports the draft's CURRENT chunk count (all chunks were
    // appended before the replay).
    expect(replay.payload).toEqual({
      ...(firstAck as Record<string, unknown>),
      chunksReceived: chunks.length,
    });
    console.log(`chunks appended=${chunks.length}; chunk 0 replayed verbatim → idempotent ack`);

    // (4) Status mid-draft agrees with what landed.
    const midStatus = await callTool("get_kpi_ingest_status", { runId });
    expect(midStatus.isError, JSON.stringify(midStatus.payload)).toBe(false);
    const openDraft = midStatus.payload.openDraft as { draftId: string; chunksReceived: number };
    expect(openDraft.draftId).toBe(draftId);
    expect(openDraft.chunksReceived).toBe(chunks.length);

    // (5) Finalize with the payload's missingReasons.
    const finalized = await callTool("stage_kpi_observations", {
      runId,
      draft: { draftId, final: true },
      missingReasons: payload.missingReasons,
    });
    expect(finalized.isError, JSON.stringify(finalized.payload)).toBe(false);
    expect(finalized.payload.revision).toBe(1);
    expect(finalized.payload.observationCount).toBe(total);

    // (6) Validate — DO NOT force ready; either outcome is §G evidence.
    const validated = await callTool("validate_kpi_ingest", { runId, revision: 1 });
    expect(validated.isError, JSON.stringify(validated.payload)).toBe(false);
    console.log(
      `totalStaged=${total}, chunkCount=${chunks.length}, ` +
        `validateOutcome=${validated.payload.outcome}, ` +
        `severityCounts=${JSON.stringify(validated.payload.severityCounts)}`,
    );

    if (validated.payload.outcome !== "ready") {
      expect(validated.payload).toEqual(
        expect.objectContaining({
          outcome: expect.any(String),
          revision: 1,
          severityCounts: expect.any(Object),
        }),
      );
      const manifestObservations = await pageSection<{
        metricKey: string;
        validationState: string;
        codes: { code: string }[];
      }>(callTool, runId, "manifest", (p) => p.manifest?.observations ?? []);
      const firstTenFlagged = manifestObservations
        .filter((o) => o.validationState === "flagged")
        .slice(0, 10)
        .map((o) => ({ metricKey: o.metricKey, codes: o.codes.map((c) => c.code) }));
      console.log(`validate NOT ready (${validated.payload.outcome}) — first 10 flagged diagnostics:`);
      console.log(JSON.stringify(firstTenFlagged));
      // §G evidence recorded either way — STOP without committing.
      return;
    }

    // (6, ready branch) Commit — bounded summary, excludedCount cross-checked
    // against the payload's own excluded rows via the paged receipt.
    const committed = await callTool("commit_kpi_ingest", {
      runId,
      manifestHash: validated.payload.manifestHash,
      revision: 1,
      execution: { client: "live-spec-s8-q3" },
    });
    expect(committed.isError, JSON.stringify(committed.payload)).toBe(false);
    expect(committed.payload).not.toHaveProperty("outcomes");

    const expectedExcludedCount = payload.observations.filter(
      (o) => o.mappingStatus === "excluded",
    ).length;
    expect(committed.payload.counts.excludedCount).toBe(expectedExcludedCount);

    const receiptOutcomes = await pageSection<{ outcome: string }>(
      callTool,
      runId,
      "receipt",
      (p) => p.receipt?.outcomes ?? [],
    );
    const excludedInReceipt = receiptOutcomes.filter((o) => o.outcome === "excluded").length;
    expect(excludedInReceipt).toBe(expectedExcludedCount);
    console.log(`commit → ${committed.payload.terminalStatus}, excludedCount=${expectedExcludedCount}`);
  } finally {
    await session.close();
  }
});
