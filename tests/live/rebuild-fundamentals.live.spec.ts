import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// Track C C4 (ADR 0086 dec. 6): drive the one-off `rebuild_fundamentals` command
// on the real Windows app after the manual facts wipe. Env-gated so ordinary live
// sweeps never trigger a full BR pull + ESEF re-extraction + WDF re-scan.
//
// Punchline-probe rule (testing.md § Live drive): assert the OUTCOME — facts per
// tier after the rebuild — never just that the command settled.

let connection: LiveConnection;

test.beforeAll(async () => {
  test.skip(process.env.BRAWLER_REBUILD !== "1", "set BRAWLER_REBUILD=1 to run the rebuild driver");
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection?.browser.close();
});

test("rebuild_fundamentals repopulates facts from BR + ESEF + WDF", async ({}) => {
  const { page } = connection;
  // BR pull: ~3 polite fetches per tracked company; ESEF re-extraction: file IO
  // + parse per stored document. Generously long.
  test.setTimeout(3_000_000);

  const summary = await page.evaluate(async () => {
    const internals = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
    if (!internals?.invoke) throw new Error("__TAURI_INTERNALS__.invoke unavailable");
    return await internals.invoke("rebuild_fundamentals", {});
  });

  console.log(`rebuild summary: ${JSON.stringify(summary, null, 2)}`);

  const byTier: Array<{ sourceTier: string; facts: number }> =
    (summary as any).factsByTier?.byTier ?? [];
  const tierCount = (tier: string) =>
    byTier.find((row) => row.sourceTier === tier)?.facts ?? 0;

  // The punchline: the automaton refilled the wiped database.
  expect(tierCount("html_aggregator")).toBeGreaterThan(0);
  expect(tierCount("esef")).toBeGreaterThan(0);
  // WDF depends on how many cover-note carriers survived feed pruning — logged,
  // not hard-asserted (448/451 historical bodies were pruned before the ingest
  // hook existed).
  console.log(`espi_cover_note facts: ${tierCount("espi_cover_note")}`);
});
