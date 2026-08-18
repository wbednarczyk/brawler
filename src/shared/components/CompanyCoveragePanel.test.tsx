import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyCoveragePanel } from "./CompanyCoveragePanel";
import { getFundamentalsCoverage } from "../../api/fundamentalsCoverage";
import type { CoveragePeriodRow, FundamentalsCoverage } from "../../api/fundamentalsCoverageTypes";
import { backfillCompanyHistory, getBackfillProgress } from "../../api/sources";
import { getHistorySweepProgress, runHistorySweep } from "../../api/historySweep";
import type { HistorySweep, HistorySweepProgress } from "../../api/historySweep";
import {
  getPipelineReextractionProgress,
  runPipelineReextraction,
} from "../../api/pipelineReextraction";
import type {
  PipelineReextractionBatch,
  PipelineReextractionProgress,
} from "../../api/pipelineReextraction";
import type { BackfillProgress } from "../../api/types";
import { getCompanyAutopilot } from "../../api/autopilot";
import {
  listFlaggedExtractionOutcomes,
  listFlaggedFactProvenance,
  rerunExtractionOutcome,
} from "../../api/fundamentalsExtraction";
import type { ExtractionOutcome } from "../../api/fundamentalsExtraction";

vi.mock("../../api/fundamentalsCoverage", () => ({
  getFundamentalsCoverage: vi.fn(),
}));
vi.mock("../../api/sources", () => ({
  backfillCompanyHistory: vi.fn(),
  getBackfillProgress: vi.fn(),
}));
vi.mock("../../api/historySweep", () => ({
  getHistorySweepProgress: vi.fn(),
  runHistorySweep: vi.fn(),
}));
vi.mock("../../api/pipelineReextraction", () => ({
  getPipelineReextractionProgress: vi.fn(),
  runPipelineReextraction: vi.fn(),
}));
vi.mock("../../api/autopilot", () => ({
  getCompanyAutopilot: vi.fn(),
}));
// The flagged-periods (ADR 0061 decision 2) and flagged-figures (epic #229 T5)
// sections are both part of this panel.
vi.mock("../../api/fundamentalsExtraction", () => ({
  listFlaggedExtractionOutcomes: vi.fn(),
  listFlaggedFactProvenance: vi.fn(),
  rerunExtractionOutcome: vi.fn(),
}));

const getFundamentalsCoverageMock = vi.mocked(getFundamentalsCoverage);
const backfillCompanyHistoryMock = vi.mocked(backfillCompanyHistory);
const getBackfillProgressMock = vi.mocked(getBackfillProgress);
const getHistorySweepProgressMock = vi.mocked(getHistorySweepProgress);
const runHistorySweepMock = vi.mocked(runHistorySweep);
const getPipelineReextractionProgressMock = vi.mocked(getPipelineReextractionProgress);
const runPipelineReextractionMock = vi.mocked(runPipelineReextraction);
const getCompanyAutopilotMock = vi.mocked(getCompanyAutopilot);
const listFlaggedExtractionOutcomesMock = vi.mocked(listFlaggedExtractionOutcomes);
const listFlaggedFactProvenanceMock = vi.mocked(listFlaggedFactProvenance);
const rerunExtractionOutcomeMock = vi.mocked(rerunExtractionOutcome);

function flaggedOutcome(overrides: Partial<ExtractionOutcome> = {}): ExtractionOutcome {
  return {
    id: "outcome_1",
    companyId: "company_gpw_cdr",
    reportDocumentId: "doc_1",
    fiscalYear: 2025,
    periodType: "Q3",
    periodEnd: "2025-09-30",
    tier: null,
    acceptance: "flagged",
    reasonCode: "no_deterministic_tier",
    detailJson: null,
    driftJson: null,
    structureChanged: false,
    factCount: 0,
    attemptCount: 1,
    firstAttemptedAt: "2026-07-01T10:00:00Z",
    lastAttemptedAt: "2026-07-01T10:00:00Z",
    ...overrides,
  };
}

function sweep(overrides: Partial<HistorySweep> = {}): HistorySweep {
  return {
    id: "history_sweep:company_gpw_cdr:1",
    companyId: "company_gpw_cdr",
    trigger: "manual",
    status: "completed",
    candidatesTotal: 2,
    runsEnqueued: 2,
    skippedExisting: 0,
    runsFailed: 0,
    skippedReason: null,
    enqueuedRunIds: [],
    error: null,
    createdAt: "2026-06-15T10:00:00Z",
    updatedAt: "2026-06-15T10:01:00Z",
    ...overrides,
  };
}

function progress(s: HistorySweep | null): HistorySweepProgress {
  return { sweep: s, runsTotal: 0, runsDone: 0, runsFailed: 0 };
}

function batch(overrides: Partial<PipelineReextractionBatch> = {}): PipelineReextractionBatch {
  return {
    id: "pipeline_reextraction:company_gpw_cdr:1",
    companyId: "company_gpw_cdr",
    status: "completed",
    candidatesTotal: 2,
    runsEnqueued: 2,
    runsFailed: 0,
    enqueuedRunIds: [],
    error: null,
    createdAt: "2026-06-15T10:00:00Z",
    updatedAt: "2026-06-15T10:01:00Z",
    ...overrides,
  };
}

function reextractProgress(b: PipelineReextractionBatch | null): PipelineReextractionProgress {
  return { batch: b, runsTotal: 0, runsDone: 0, runsFailed: 0 };
}

// `pendingProposals` is dropped from the DTO by the ADR 0084 clean cut but still
// present on the not-yet-regenerated binding. Spread it so these samples satisfy
// both shapes — the panel reads only `flaggedFacts`.
// A fully-defaulted coverage row; each test overrides only the axis it exercises.
function coverageRow(overrides: Partial<CoveragePeriodRow> = {}): CoveragePeriodRow {
  return {
    fiscalYear: 2026,
    periodType: "Q1",
    report: null,
    facts: { total: 0, validated: 0, unvalidated: 0, flagged: 0 },
    review: { flaggedFacts: 0 },
    skippedBudget: false,
    ...overrides,
  };
}

function coverage(periods: CoveragePeriodRow[]): FundamentalsCoverage {
  return { companyId: "company_gpw_cdr", periods };
}

describe("CompanyCoveragePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getFundamentalsCoverageMock.mockResolvedValue(coverage([coverageRow()]));
    // Footer apis default to an opted-in company with no prior history activity.
    getCompanyAutopilotMock.mockResolvedValue({ companyId: "company_gpw_cdr", mode: "assist" });
    getBackfillProgressMock.mockResolvedValue(null);
    getHistorySweepProgressMock.mockResolvedValue(progress(null));
    backfillCompanyHistoryMock.mockResolvedValue({
      companyId: "company_gpw_cdr",
      status: "completed",
      pagesFetched: 3,
      itemsIngested: 12,
      documentsStored: 4,
      detailErrors: 0,
      truncated: false,
      chainedSweepId: null,
      error: null,
      startedAt: "2026-06-15T10:00:00Z",
      updatedAt: "2026-06-15T10:01:00Z",
    });
    runHistorySweepMock.mockResolvedValue(sweep({ status: "queued" }));
    getPipelineReextractionProgressMock.mockResolvedValue(reextractProgress(null));
    runPipelineReextractionMock.mockResolvedValue(batch({ status: "queued" }));
    listFlaggedExtractionOutcomesMock.mockResolvedValue([]);
    listFlaggedFactProvenanceMock.mockResolvedValue([]);
    rerunExtractionOutcomeMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "pdf",
      emitted: true,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      reasonCode: "emitted",
    });
  });

  it("renders one row per coverage period with a formatted period label", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({ fiscalYear: 2026, periodType: "Q1" }),
        coverageRow({ fiscalYear: 2025, periodType: "annual" }),
      ]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    const rows = within(await screen.findByRole("table"));
    // Fiscal-year + period-type, with `annual` rendered as the accented FY label.
    expect(rows.getByText("2026 Q1")).toBeInTheDocument();
    expect(rows.getByText("2025 FY")).toBeInTheDocument();
  });

  it("shows the report kind chip, an ESEF chip for a structured document, and a truncated title", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({
          report: {
            documentId: "doc_1",
            docKind: "periodic_ssf",
            title: "Skonsolidowane sprawozdanie finansowe Grupy 2025",
            structured: true,
            fetched: true,
          },
        }),
      ]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Consolidated report")).toBeInTheDocument();
    expect(screen.getByText("ESEF")).toBeInTheDocument();
    expect(screen.getByText(/Skonsolidowane/)).toBeInTheDocument();
  });

  it("renders the missing-report state when a period has no canonical report", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(coverage([coverageRow({ report: null })]));
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("No report")).toBeInTheDocument();
    expect(screen.getByText("not found in backfill")).toBeInTheDocument();
  });

  it("renders a validated facts cell in the success tone with an all-validated sub-line", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([coverageRow({ facts: { total: 14, validated: 14, unvalidated: 0, flagged: 0 } })]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("14 facts")).toBeInTheDocument();
    expect(screen.getByText("all validated")).toBeInTheDocument();
  });

  it("renders a warning facts cell with a validated/flagged breakdown", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([coverageRow({ facts: { total: 11, validated: 9, unvalidated: 0, flagged: 2 } })]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("11 facts")).toBeInTheDocument();
    expect(screen.getByText("9 validated · 2 flagged/divergent")).toBeInTheDocument();
  });

  it("renders the not-processed hint when a report has no extracted facts yet", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({
          report: {
            documentId: "doc_1",
            docKind: "periodic_ssf",
            title: "Q3 report",
            structured: false,
            fetched: true,
          },
          facts: { total: 0, validated: 0, unvalidated: 0, flagged: 0 },
        }),
      ]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("not processed")).toBeInTheDocument();
    expect(screen.getByText("click → Extract")).toBeInTheDocument();
  });

  // ADR 0084: the tier-4 AI budget is retired with the analysis layer, so the
  // coverage matrix no longer renders an AI-budget skip cell or legend entry.
  it("renders no AI-budget skip state (ADR 0084)", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({
          report: {
            documentId: "doc_1",
            docKind: "periodic_ssf",
            title: "H1 report",
            structured: false,
            fetched: true,
          },
          skippedBudget: true,
        }),
      ]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await screen.findByRole("table");
    expect(screen.queryByText("Skipped — AI budget")).not.toBeInTheDocument();
  });

  it("fires onOpenDocuments when a period row is clicked", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(coverage([coverageRow({ fiscalYear: 2025, periodType: "Q3" })]));
    const onOpenDocuments = vi.fn();
    render(
      <CompanyCoveragePanel companyId="company_gpw_cdr" onOpenDocuments={onOpenDocuments} />,
    );

    const row = await screen.findByRole("button", { name: /2025 Q3/ });
    await userEvent.click(row);
    expect(onOpenDocuments).toHaveBeenCalledTimes(1);
  });

  // ADR 0084: the review cell shows only flagged facts, never proposal counts
  // (`kpi_extraction_proposals` is dropped).
  it("shows flagged facts in the review cell, never proposal counts (ADR 0084)", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({ fiscalYear: 2025, periodType: "Q3", review: { flaggedFacts: 2 } }),
      ]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("2 flagged facts")).toBeInTheDocument();
    expect(screen.queryByText(/proposals?/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Review queue/ })).not.toBeInTheDocument();
  });

  it("shows the empty state when no periods are covered", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(coverage([]));
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await waitFor(() => {
      expect(screen.getByText("No coverage data yet.")).toBeInTheDocument();
    });
  });

  it("reloads coverage when the companyId changes", async () => {
    const { rerender } = render(<CompanyCoveragePanel companyId="company_a" />);
    await waitFor(() => expect(getFundamentalsCoverageMock).toHaveBeenCalledWith("company_a"));

    rerender(<CompanyCoveragePanel companyId="company_b" />);
    await waitFor(() => expect(getFundamentalsCoverageMock).toHaveBeenCalledWith("company_b"));
  });

  // --- Flagged periods (ADR 0061 decision 2, ADR 0084 decision 4/6) ---

  // Reachability: the flagged-extraction read model has no other UI entry point,
  // so a user reaches it here or not at all ("a capability is not done until a
  // user can reach it"). Reddens if the section is dropped from the panel.
  it("hosts the flagged-periods section, scoped to this company", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([
      flaggedOutcome({ fiscalYear: 2025, periodType: "Q3" }),
    ]);
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByRole("heading", { name: "Flagged periods" })).toBeInTheDocument();
    expect(listFlaggedExtractionOutcomesMock).toHaveBeenCalledWith("company_gpw_cdr");
    expect(await screen.findByText("No reader could handle this document")).toBeInTheDocument();
  });

  // A `no_period_derived` failure carries the SENTINEL period, so it can never
  // appear as a coverage-table row — the period-union rule cannot name a period
  // the pipeline failed to derive. Gating the section on a non-empty table would
  // hide exactly the gap it exists to surface.
  it("still shows flagged periods when the coverage table itself is empty", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(coverage([]));
    listFlaggedExtractionOutcomesMock.mockResolvedValue([
      flaggedOutcome({ fiscalYear: 0, periodType: "", periodEnd: "", reasonCode: "no_period_derived" }),
    ]);
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    // The coverage-map empty state and the flagged section coexist.
    expect(await screen.findByText("No coverage data yet.")).toBeInTheDocument();
    expect(screen.getByText("Period unknown")).toBeInTheDocument();
    expect(screen.getByText("The reporting period could not be determined")).toBeInTheDocument();
  });

  // A re-run that now emits writes facts the coverage table must pick up —
  // otherwise the map keeps showing "not processed" beside a fresh success.
  it("reloads the coverage map after a flagged period is successfully re-run", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([flaggedOutcome({ id: "outcome_42" })]);
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await waitFor(() => expect(getFundamentalsCoverageMock).toHaveBeenCalledTimes(1));
    await userEvent.click(await screen.findByRole("button", { name: /Try again/ }));

    await waitFor(() =>
      expect(rerunExtractionOutcomeMock).toHaveBeenCalledWith({ outcomeId: "outcome_42" }),
    );
    await waitFor(() => expect(getFundamentalsCoverageMock).toHaveBeenCalledTimes(2));
  });

  // --- History actions footer (ADR 0077 §3, T3.2) ---

  it("enables both history actions for an opted-in company (idle state)", async () => {
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);
    const backfill = await screen.findByRole("button", { name: /Backfill history/ });
    const extract = screen.getByRole("button", { name: /Extract missing periods/ });
    expect(backfill).toBeEnabled();
    expect(extract).toBeEnabled();
  });

  it("disables both actions with a hint when automation is off", async () => {
    getCompanyAutopilotMock.mockResolvedValue({ companyId: "company_gpw_cdr", mode: "off" });
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Enable automation to extract history.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Backfill history/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Extract missing periods/ })).toBeDisabled();
    // The status line names the off state explicitly.
    expect(screen.getByRole("status")).toHaveTextContent("Automation off");
  });

  // --- Re-extraction footer action (epic #398 Item B) ---

  // Unlike Backfill/Extract-missing, re-extraction reprocesses already-stored
  // documents on explicit request (the "Try again" posture) — it must stay
  // usable even when automation is off, never silently disabled alongside
  // the automation-gated actions.
  it("keeps the re-extract action enabled when automation is off", async () => {
    getCompanyAutopilotMock.mockResolvedValue({ companyId: "company_gpw_cdr", mode: "off" });
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await screen.findByText("Enable automation to extract history.");
    expect(
      screen.getByRole("button", { name: /Re-extract with latest pipeline/ }),
    ).toBeEnabled();
  });

  it("runs a re-extraction batch and refreshes siblings once it settles", async () => {
    getPipelineReextractionProgressMock
      .mockResolvedValueOnce(reextractProgress(null)) // initial mount load
      .mockResolvedValue(reextractProgress(batch({ status: "completed", runsEnqueued: 3 })));
    const onHistoryRefreshed = vi.fn();
    render(
      <CompanyCoveragePanel companyId="company_gpw_cdr" onHistoryRefreshed={onHistoryRefreshed} />,
    );

    const reextract = await screen.findByRole("button", {
      name: /Re-extract with latest pipeline/,
    });
    await userEvent.click(reextract);

    await waitFor(() => expect(runPipelineReextractionMock).toHaveBeenCalledWith("company_gpw_cdr"));
    await waitFor(() => expect(onHistoryRefreshed).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("status")).toHaveTextContent("Re-extracted 3");
  });

  // The other two footer actions must not double-fire while a re-extraction
  // batch is in flight (all three share the `busy` gate).
  it("disables the other footer actions while a re-extraction batch is in flight", async () => {
    let resolveBatch: (value: PipelineReextractionBatch) => void = () => {};
    runPipelineReextractionMock.mockReturnValue(
      new Promise<PipelineReextractionBatch>((resolve) => {
        resolveBatch = resolve;
      }),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    const reextract = await screen.findByRole("button", {
      name: /Re-extract with latest pipeline/,
    });
    await userEvent.click(reextract);

    expect(await screen.findByRole("button", { name: /Re-extracting…/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Backfill history/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Extract missing periods/ })).toBeDisabled();

    resolveBatch(batch({ status: "completed" }));
  });

  it("shows the backfilling phase while a backfill runs, then refreshes siblings", async () => {
    let resolveBackfill: (value: BackfillProgress) => void = () => {};
    backfillCompanyHistoryMock.mockReturnValue(
      new Promise<BackfillProgress>((resolve) => {
        resolveBackfill = resolve;
      }),
    );
    const onHistoryRefreshed = vi.fn();
    render(
      <CompanyCoveragePanel companyId="company_gpw_cdr" onHistoryRefreshed={onHistoryRefreshed} />,
    );

    const backfill = await screen.findByRole("button", { name: /Backfill history/ });
    await userEvent.click(backfill);
    // While in flight the button shows the running label and both actions disable.
    expect(await screen.findByRole("button", { name: /Backfilling…/ })).toBeDisabled();

    resolveBackfill({
      companyId: "company_gpw_cdr",
      status: "completed",
      pagesFetched: 3,
      itemsIngested: 12,
      documentsStored: 4,
      detailErrors: 0,
      truncated: false,
      chainedSweepId: null,
      error: null,
      startedAt: "2026-06-15T10:00:00Z",
      updatedAt: "2026-06-15T10:01:00Z",
    } satisfies BackfillProgress);
    await waitFor(() => expect(onHistoryRefreshed).toHaveBeenCalledTimes(1));
  });

  it("runs a sweep-only extraction and bumps the refresh callback on completion", async () => {
    getHistorySweepProgressMock
      .mockResolvedValueOnce(progress(null)) // initial mount load
      .mockResolvedValue(progress(sweep({ status: "completed", runsEnqueued: 2, skippedExisting: 1 })));
    const onHistoryRefreshed = vi.fn();
    render(
      <CompanyCoveragePanel companyId="company_gpw_cdr" onHistoryRefreshed={onHistoryRefreshed} />,
    );

    const extract = await screen.findByRole("button", { name: /Extract missing periods/ });
    await userEvent.click(extract);

    await waitFor(() => expect(runHistorySweepMock).toHaveBeenCalledWith("company_gpw_cdr"));
    await waitFor(() => expect(onHistoryRefreshed).toHaveBeenCalledTimes(1));
    // The status line reflects the completed sweep counters.
    expect(screen.getByRole("status")).toHaveTextContent("Extracted 2 · skipped 1");
  });

  // Guardrail (T3.2 live-drive harvest): the sweep is a DURABLE job — the start
  // command returns with it merely queued and the runs drain later. A single
  // post-click refresh froze the status on "Queued" forever on the real app.
  // This test reddens if the panel stops polling before the sweep settles.
  it("keeps polling a queued sweep until it settles instead of freezing on the first status", async () => {
    // shouldAdvanceTime keeps RTL's waitFor/findBy working (they poll on real
    // timeouts), while the explicit advances below still control the 1.5s poll.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      getHistorySweepProgressMock
        .mockResolvedValueOnce(progress(null)) // initial mount load
        .mockResolvedValueOnce(progress(sweep({ status: "queued", runsEnqueued: 0 }))) // immediate post-click check
        .mockResolvedValueOnce({
          // job finished enqueuing, extraction runs still draining
          ...progress(sweep({ status: "completed", runsEnqueued: 2 })),
          runsTotal: 2,
          runsDone: 1,
        })
        .mockResolvedValue({
          ...progress(sweep({ status: "completed", runsEnqueued: 2, skippedExisting: 1 })),
          runsTotal: 2,
          runsDone: 2,
        });
      const onHistoryRefreshed = vi.fn();
      render(
        <CompanyCoveragePanel
          companyId="company_gpw_cdr"
          onHistoryRefreshed={onHistoryRefreshed}
        />,
      );

      const extract = await screen.findByRole("button", { name: /Extract missing periods/ });
      await user.click(extract);
      await waitFor(() => expect(runHistorySweepMock).toHaveBeenCalledWith("company_gpw_cdr"));

      // Queued, then draining: the action stays in flight and nothing reloads
      // yet — and the status line shows the live drain counter (T3.3). React 19
      // defers the tick's render, so the counter is awaited, not read inline.
      await vi.advanceTimersByTimeAsync(1500);
      expect(onHistoryRefreshed).not.toHaveBeenCalled();
      await waitFor(() =>
        expect(screen.getByRole("status")).toHaveTextContent("Extracting… 1/2"),
      );

      // Drained: polling stops, siblings reload, counters land in the status line.
      await vi.advanceTimersByTimeAsync(1500);
      await waitFor(() => expect(onHistoryRefreshed).toHaveBeenCalledTimes(1));
      expect(screen.getByRole("status")).toHaveTextContent("Extracted 2 · skipped 1");
    } finally {
      vi.useRealTimers();
    }
  });

  // Live-validation harvest (MDV): the sweep poll targets the sweep we STARTED
  // by id, never "the latest sweep". When it sits behind a busy job queue the row
  // may be absent for many ticks; the panel must keep "Extracting…" (no 4-tick
  // give-up) and settle ONLY when our sweep appears terminal+drained — and the
  // AI-budget footer then reflects that settled sweep.
  it("keeps polling while the expected sweep is absent, then settles when it appears", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      runHistorySweepMock.mockResolvedValue(
        sweep({ id: "history_sweep:company_gpw_cdr:9", status: "queued", runsEnqueued: 0 }),
      );
      // Our sweep is queued behind earlier work: the progress endpoint reports no
      // sweep row (for this company) until it finally runs, terminal + drained.
      let sweepVisible = false;
      getHistorySweepProgressMock.mockImplementation(async () => {
        if (!sweepVisible) return progress(null);
        return {
          ...progress(
            sweep({
              id: "history_sweep:company_gpw_cdr:9",
              status: "completed",
              runsEnqueued: 2,
              skippedExisting: 1,
            }),
          ),
          runsTotal: 2,
          runsDone: 2,
        };
      });
      const onHistoryRefreshed = vi.fn();
      render(
        <CompanyCoveragePanel companyId="company_gpw_cdr" onHistoryRefreshed={onHistoryRefreshed} />,
      );

      const extract = await screen.findByRole("button", { name: /Extract missing periods/ });
      await user.click(extract);
      await waitFor(() => expect(runHistorySweepMock).toHaveBeenCalledWith("company_gpw_cdr"));

      // Six poll ticks with our sweep still absent — well past the OLD 4-tick
      // give-up. The panel must keep "Extracting…", never reload, no stale budget.
      for (let i = 0; i < 6; i += 1) {
        await vi.advanceTimersByTimeAsync(1500);
      }
      expect(onHistoryRefreshed).not.toHaveBeenCalled();
      expect(screen.getByRole("status")).toHaveTextContent("Extracting…");

      // Our sweep surfaces terminal+drained → settle and reload.
      sweepVisible = true;
      await vi.advanceTimersByTimeAsync(1500);
      await waitFor(() => expect(onHistoryRefreshed).toHaveBeenCalledTimes(1));
      expect(screen.getByRole("status")).toHaveTextContent("Extracted 2 · skipped 1");
    } finally {
      vi.useRealTimers();
    }
  });

  // The false-settle trap the explicit id closes: an OLDER, already-completed
  // sweep is reported as "the latest" while our new sweep is still queued. Polling
  // "latest" would settle on it (terminal+drained) and freeze a stale "AI: 0/2".
  // With an explicit target the panel ignores it and keeps waiting for ours.
  it("ignores a different (stale) sweep reported as latest and never false-settles on it", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      runHistorySweepMock.mockResolvedValue(
        sweep({ id: "history_sweep:company_gpw_cdr:NEW", status: "queued", runsEnqueued: 0 }),
      );
      getHistorySweepProgressMock
        .mockResolvedValueOnce(progress(null)) // mount load — no prior sweep shown
        .mockResolvedValue({
          // A stale OLD sweep, terminal + drained, but NOT the one we started.
          ...progress(
            sweep({
              id: "history_sweep:company_gpw_cdr:OLD",
              status: "completed",
              runsEnqueued: 5,
            }),
          ),
          runsTotal: 0,
          runsDone: 0,
        });
      const onHistoryRefreshed = vi.fn();
      render(
        <CompanyCoveragePanel companyId="company_gpw_cdr" onHistoryRefreshed={onHistoryRefreshed} />,
      );

      const extract = await screen.findByRole("button", { name: /Extract missing periods/ });
      await user.click(extract);
      await waitFor(() => expect(runHistorySweepMock).toHaveBeenCalledWith("company_gpw_cdr"));

      for (let i = 0; i < 5; i += 1) {
        await vi.advanceTimersByTimeAsync(1500);
      }
      // The stale sweep is terminal+drained, but it is not ours → keep waiting.
      expect(onHistoryRefreshed).not.toHaveBeenCalled();
      expect(screen.getByRole("status")).toHaveTextContent("Extracting…");
      expect(screen.queryByText("AI: 0/2")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  // Card bfc4c98 (UI half): a backfill on a market with no history-capable source
  // adapter (NewConnect today) fails with the machine code `unsupported_market`.
  // The status line must say so specifically, not the generic "Backfill failed".
  it("maps an unsupported_market backfill failure to a market-specific message", async () => {
    getBackfillProgressMock.mockResolvedValue({
      companyId: "company_gpw_cdr",
      status: "failed",
      pagesFetched: 0,
      itemsIngested: 0,
      documentsStored: 0,
      detailErrors: 0,
      truncated: false,
      chainedSweepId: null,
      error: "unsupported_market",
      startedAt: "2026-06-15T10:00:00Z",
      updatedAt: "2026-06-15T10:01:00Z",
    });
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent(
        "Backfill isn't available for this company's market (NewConnect) yet",
      ),
    );
    expect(screen.queryByText("Backfill failed")).not.toBeInTheDocument();
  });

  // The five other typed causes (card bfc4c98, B5) must each surface their own
  // message — never collapse into the generic "Backfill failed". One branch per
  // code; parameterized so a new code without a mapping reddens the class here.
  it.each([
    ["no_bankier_page: slug unresolvable", "No Bankier page was found for this company"],
    ["http_error: 503 from bankier.pl", "Couldn't reach Bankier — try again later"],
    ["parse_error: missing listing table", "The page was fetched but couldn't be read"],
    ["not_tracked: unknown company id", "This company isn't tracked by this source"],
  ])("maps a %s backfill failure to its own message", async (error, message) => {
    getBackfillProgressMock.mockResolvedValue({
      companyId: "company_gpw_cdr",
      status: "failed",
      pagesFetched: 0,
      itemsIngested: 0,
      documentsStored: 0,
      detailErrors: 0,
      truncated: false,
      chainedSweepId: null,
      error,
      startedAt: "2026-06-15T10:00:00Z",
      updatedAt: "2026-06-15T10:01:00Z",
    });
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(message));
    expect(screen.queryByText("Backfill failed")).not.toBeInTheDocument();
  });

  it("keeps the generic failed message for a non-market backfill error", async () => {
    getBackfillProgressMock.mockResolvedValue({
      companyId: "company_gpw_cdr",
      status: "failed",
      pagesFetched: 0,
      itemsIngested: 0,
      documentsStored: 0,
      detailErrors: 0,
      truncated: false,
      chainedSweepId: null,
      error: "network timeout",
      startedAt: "2026-06-15T10:00:00Z",
      updatedAt: "2026-06-15T10:01:00Z",
    });
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Backfill failed"));
  });

  // T3.3: a backfill that hit the page cap before the cutoff reports truncation
  // honestly — the panel surfaces an explicit warning, never a silent gap.
  it("renders a truncation warning when the latest backfill hit the page cap", async () => {
    getBackfillProgressMock.mockResolvedValue({
      companyId: "company_gpw_cdr",
      status: "completed",
      pagesFetched: 80,
      itemsIngested: 200,
      documentsStored: 40,
      detailErrors: 0,
      truncated: true,
      chainedSweepId: null,
      error: null,
      startedAt: "2026-06-15T10:00:00Z",
      updatedAt: "2026-06-15T10:01:00Z",
    });
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(
        await screen.findByText("History truncated at the page cap — older filings may be missing."),
      ).toBeInTheDocument();
  });

  // T3.3 (owner real-DB finding): a metadata-only (link-only) canonical report
  // has no stored file, so "click → Extract" would mislead. The Data cell must
  // say so instead — aligned with the sweep, which skips these.
  it("shows a link-only hint for a metadata-only report instead of the extract hint", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({
          report: {
            documentId: "doc_1",
            docKind: "periodic_ssf",
            title: "Raport roczny 2024",
            structured: false,
            fetched: false,
          },
          facts: { total: 0, validated: 0, unvalidated: 0, flagged: 0 },
        }),
      ]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("link-only — no stored file")).toBeInTheDocument();
    expect(screen.queryByText("not processed")).not.toBeInTheDocument();
    expect(screen.queryByText("click → Extract")).not.toBeInTheDocument();
  });

  // ADR 0084: the sweep footer no longer reports tier-4 AI spend — there is no
  // AI lane left to budget.
  it("shows no AI call-budget footer for a sweep (ADR 0084)", async () => {
    getHistorySweepProgressMock.mockResolvedValue(
      progress(sweep({ status: "completed" })),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await screen.findByRole("button", { name: /Backfill history/ });
    expect(screen.queryByText(/^AI:/)).not.toBeInTheDocument();
  });
});
