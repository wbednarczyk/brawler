import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyCoveragePanel } from "./CompanyCoveragePanel";
import { getFundamentalsCoverage } from "../../api/fundamentalsCoverage";
import type { CoveragePeriodRow, FundamentalsCoverage } from "../../api/fundamentalsCoverageTypes";
import { backfillCompanyHistory, getBackfillProgress } from "../../api/sources";
import { getHistorySweepProgress, runHistorySweep } from "../../api/historySweep";
import type { HistorySweep, HistorySweepProgress } from "../../api/historySweep";
import type { BackfillProgress } from "../../api/types";
import { getCompanyAutopilot } from "../../api/autopilot";

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
vi.mock("../../api/autopilot", () => ({
  getCompanyAutopilot: vi.fn(),
}));

const getFundamentalsCoverageMock = vi.mocked(getFundamentalsCoverage);
const backfillCompanyHistoryMock = vi.mocked(backfillCompanyHistory);
const getBackfillProgressMock = vi.mocked(getBackfillProgress);
const getHistorySweepProgressMock = vi.mocked(getHistorySweepProgress);
const runHistorySweepMock = vi.mocked(runHistorySweep);
const getCompanyAutopilotMock = vi.mocked(getCompanyAutopilot);

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
    aiCallsUsed: 0,
    aiCallLimit: 30,
    error: null,
    createdAt: "2026-06-15T10:00:00Z",
    updatedAt: "2026-06-15T10:01:00Z",
    ...overrides,
  };
}

function progress(s: HistorySweep | null): HistorySweepProgress {
  return { sweep: s, runsTotal: 0, runsDone: 0, runsFailed: 0 };
}

// A fully-defaulted coverage row; each test overrides only the axis it exercises.
function coverageRow(overrides: Partial<CoveragePeriodRow> = {}): CoveragePeriodRow {
  return {
    fiscalYear: 2026,
    periodType: "Q1",
    report: null,
    facts: { total: 0, validated: 0, unvalidated: 0, flagged: 0 },
    review: { pendingProposals: 0, flaggedFacts: 0 },
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

  it("renders the AI-budget-skipped state behind the skippedBudget flag", async () => {
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

    // "Skipped — AI budget" is also a legend entry, so scope to the table cell.
    const table = within(await screen.findByRole("table"));
    expect(table.getByText("Skipped — AI budget")).toBeInTheDocument();
  });

  it("renders pending proposals with a review-queue sub-line", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([coverageRow({ review: { pendingProposals: 2, flaggedFacts: 0 } })]),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("2 proposals")).toBeInTheDocument();
    expect(screen.getByText("→ review queue")).toBeInTheDocument();
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

  it("fires onOpenReview (not onOpenDocuments) when the pending-proposals review cell is clicked", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({ fiscalYear: 2025, periodType: "Q3", review: { pendingProposals: 2, flaggedFacts: 0 } }),
      ]),
    );
    const onOpenDocuments = vi.fn();
    const onOpenReview = vi.fn();
    render(
      <CompanyCoveragePanel
        companyId="company_gpw_cdr"
        onOpenDocuments={onOpenDocuments}
        onOpenReview={onOpenReview}
      />,
    );

    const reviewButton = await screen.findByRole("button", { name: /Review queue — 2 proposals/ });
    await userEvent.click(reviewButton);
    expect(onOpenReview).toHaveBeenCalledTimes(1);
    // stopPropagation keeps the row's documents handler off the review click.
    expect(onOpenDocuments).not.toHaveBeenCalled();
  });

  it("opens the review queue (not documents) via keyboard on the review cell", async () => {
    getFundamentalsCoverageMock.mockResolvedValue(
      coverage([
        coverageRow({ fiscalYear: 2025, periodType: "Q3", review: { pendingProposals: 1, flaggedFacts: 0 } }),
      ]),
    );
    const onOpenDocuments = vi.fn();
    const onOpenReview = vi.fn();
    render(
      <CompanyCoveragePanel
        companyId="company_gpw_cdr"
        onOpenDocuments={onOpenDocuments}
        onOpenReview={onOpenReview}
      />,
    );

    const reviewButton = await screen.findByRole("button", { name: /Review queue — 1 proposal/ });
    reviewButton.focus();
    await userEvent.keyboard("{Enter}");
    expect(onOpenReview).toHaveBeenCalledTimes(1);
    expect(onOpenDocuments).not.toHaveBeenCalled();
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

    await waitFor(() =>
      expect(screen.getByText("Enable automation to extract history.")).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: /Backfill history/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Extract missing periods/ })).toBeDisabled();
    // The status line names the off state explicitly.
    expect(screen.getByRole("status")).toHaveTextContent("Automation off");
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
      // yet — and the status line shows the live drain counter (T3.3).
      await vi.advanceTimersByTimeAsync(1500);
      expect(onHistoryRefreshed).not.toHaveBeenCalled();
      expect(screen.getByRole("status")).toHaveTextContent("Extracting… 1/2");

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
  // AI-budget footer then reflects that settled sweep. Reddens on the old code,
  // which gave up after 4 ticks without a sweep row and false-settled.
  it("keeps polling while the expected sweep is absent, then settles and shows its budget when it appears", async () => {
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
              aiCallsUsed: 2,
              aiCallLimit: 2,
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
      expect(screen.queryByText(/^AI:/)).not.toBeInTheDocument();

      // Our sweep surfaces terminal+drained → settle, reload, budget reflects it.
      sweepVisible = true;
      await vi.advanceTimersByTimeAsync(1500);
      await waitFor(() => expect(onHistoryRefreshed).toHaveBeenCalledTimes(1));
      expect(screen.getByRole("status")).toHaveTextContent("Extracted 2 · skipped 1");
      expect(screen.getByText("AI: 2/2")).toBeInTheDocument();
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
              aiCallsUsed: 0,
              aiCallLimit: 2,
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

    await waitFor(() =>
      expect(
        screen.getByText(
          "History truncated at the page cap — older filings may be missing.",
        ),
      ).toBeInTheDocument(),
    );
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

  // T5.3 (ADR 0077 §6): the footer status line additionally shows the latest
  // sweep's tier-4 AI-call spend, whenever a sweep row exists.
  it("shows the AI call budget for the latest sweep", async () => {
    getHistorySweepProgressMock.mockResolvedValue(
      progress(sweep({ status: "completed", aiCallsUsed: 3, aiCallLimit: 30 })),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await waitFor(() => expect(screen.getByText("AI: 3/30")).toBeInTheDocument());
  });

  it("renders the no-limit variant when the sweep's AI budget is unlimited", async () => {
    getHistorySweepProgressMock.mockResolvedValue(
      progress(sweep({ status: "completed", aiCallsUsed: 5, aiCallLimit: 0 })),
    );
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await waitFor(() => expect(screen.getByText("AI: 5 (no limit)")).toBeInTheDocument());
  });

  it("shows no AI budget text when there is no sweep row yet", async () => {
    render(<CompanyCoveragePanel companyId="company_gpw_cdr" />);

    await screen.findByRole("button", { name: /Backfill history/ });
    expect(screen.queryByText(/^AI:/)).not.toBeInTheDocument();
  });
});
