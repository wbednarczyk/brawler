import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import {
  isCheckpointRun,
  recordCheckpoint,
  requireCheckpointMeta,
} from "./helpers/checkpointEvidence";
// Single-source pattern + generic copies, reused from the app (never duplicated):
// the anti-filename regex (documentTitle.ts) and the localized generic fallbacks.
import { FILENAME_EXTENSION } from "../../src/screens/Today/documentTitle";
import { makeTextTranslator, supportedLocales } from "../../src/shared/locale";

// The generic attention-fallback statements (both locales) — a row whose statement
// is one of these stated only its category, not WHAT concretely happened. Resolved
// through the locale layer so the copy stays single-source (never hardcoded PL).
const GENERIC_FALLBACK_KEYS = [
  "Autopilot finished",
  "52-week low",
  "Price range",
  "Official report missed by the primary source",
];
// The raw trigger enums a title composer falls back to when even the rule is gone.
const RAW_TRIGGER_FALLBACKS = [
  "signal_category",
  "autopilot_run_completed",
  "price_enters_range",
  "price_week52_low",
  "source_reconciliation",
];
const GENERIC_STATEMENTS = new Set<string>([
  ...supportedLocales.flatMap((loc) =>
    GENERIC_FALLBACK_KEYS.map((key) => makeTextTranslator(loc)(key)),
  ),
  ...RAW_TRIGGER_FALLBACKS,
]);

// Q6 live UX checkpoint (ADR 0081). A generic, exploration-support spec: it drives
// the *mechanical* J1 path against the real Windows app and records evidence, so a
// human can then answer whether the journey is clear, useful, and trustworthy.
// Automation NEVER prints "UX good" — clarity/usefulness is a human judgment.
//
// Run scoped, with a charter's metadata, NOT as part of `make check`:
//   BRAWLER_UX_JOURNEY="J1 morning review" BRAWLER_UX_CARD=a26cc6e \
//   BRAWLER_UX_STAGE=vertical \
//   make live-cycle LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts
//
// Deliberately non-mutating: navigation + reads only. Feature-specific mutating
// steps belong to an explicit pilot spec and require owner intent.

let connection: LiveConnection;

// The generic checkpoint only runs inside an intentional checkpoint session; the
// full historical live suite (empty LIVE_SPEC, no metadata) skips it rather than
// failing, so it never blocks an ordinary live-drive sweep.
test.skip(!isCheckpointRun(), "not a UX checkpoint run (set BRAWLER_UX_STAGE/JOURNEY/CARD)");

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection?.browser.close();
});

test("J1 morning-review mechanical path + checkpoint evidence", async () => {
  // Refuses to run without full metadata (red condition) — no unattributable evidence.
  const meta = requireCheckpointMeta();
  const { page } = connection;
  const observations: string[] = [];

  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await expect(nav).toBeVisible();
  await nav.getByRole("button", { name: /Today|Dziś/ }).click();

  // Today must render an attention stream OR an explicit quiet state — never a
  // blank pane and never the generic error fallback. This is the mechanical
  // "renders vs broken" distinction; whether the content is *useful* is human.
  const errorFallback = page.locator(".app-error-recovery");
  await expect(errorFallback).toHaveCount(0);

  const attentionRows = page.locator(".today-stream-row");
  const quietState = page.locator(".today-stream-quiet");
  const attentionCount = await attentionRows.count();
  const isQuiet = (await quietState.count()) > 0;
  expect(attentionCount > 0 || isQuiet).toBeTruthy();
  observations.push(
    isQuiet
      ? "Today: explicit quiet state ('nothing needs attention'), not a blank pane."
      : `Today: attention stream rendered ${attentionCount} row(s).`,
  );

  // A visible Review action (present only when there is something to review) opens
  // a company-scoped cockpit; then navigation back to Today works.
  const review = page.locator(".today-row-review").first();
  if (await review.count()) {
    await review.click();
    const cockpit = page.getByLabel(/Research cockpit|Kokpit badawczy/);
    await expect(cockpit).toBeVisible();
    await expect(cockpit).not.toHaveAttribute("data-company-id", "");
    observations.push("Review action opened a company-scoped cockpit.");

    await nav.getByRole("button", { name: /Today|Dziś/ }).click();
    await expect(errorFallback).toHaveCount(0);
    observations.push("Return to Today from the cockpit works.");
  } else {
    observations.push("No Review action present (quiet morning) — nothing to open.");
  }

  // Specificity probe over the REAL rendered stream (owner dogfooding 2026-07-23):
  // how many rows state something CONCRETE vs a bare generic fallback, and — the
  // hard gate — how many state a raw filename (must be zero on real data). The
  // "specific" count is advisory (clarity is a human verdict); the filename count
  // is a HARD assertion. Metrics land in the manifest for the human charter.
  const rowStatements = await page.locator(".today-row-title").allInnerTexts();
  const trimmed = rowStatements.map((s) => s.trim()).filter((s) => s.length > 0);
  const filenameAsStatement = trimmed.filter((s) => FILENAME_EXTENSION.test(s)).length;
  const rowsWithSpecificStatement = trimmed.filter((s) => !GENERIC_STATEMENTS.has(s)).length;
  const streamMetrics = {
    totalRows: trimmed.length,
    rowsWithSpecificStatement,
    filenameAsStatement,
  };
  observations.push(
    `Stream specificity: ${rowsWithSpecificStatement}/${trimmed.length} rows state something concrete; ` +
      `${filenameAsStatement} filename-as-statement (must be 0).`,
  );

  const manifest = await recordCheckpoint(page, meta, {
    datasetLabel: process.env.BRAWLER_UX_DATASET ?? "owner real database (label only)",
    nowIso: new Date().toISOString(),
    observations,
    streamMetrics,
  });

  // HARD gate: a filename must NEVER be a row's statement on the owner's real data
  // (owner dogfooding 2026-07-23). Advisory metrics aside, this one reddens.
  expect(
    streamMetrics.filenameAsStatement,
    "no Today row statement may be a raw filename on real data",
  ).toBe(0);
  await page.screenshot({
    path: `${manifest.screenshotDir}/today.png`,
    fullPage: true,
  });

  // Evidence is recorded; the human charter (exploratory question, P1/P2/P3
  // findings, verdict proceed|revise|block) is answered OUTSIDE this automation.
  expect(manifest.windowsNative, "checkpoint must run against the native Windows app").toBeTruthy();
});
