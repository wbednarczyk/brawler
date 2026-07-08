import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QualityPanel } from "./QualityPanel";
import {
  createFrameworkCriterion,
  deleteFrameworkEvaluation,
  evaluateFramework,
  getQualitativeAssessment,
  getQualitativeAssessmentStatus,
  listAvailableMetricKeys,
  listFrameworkEvaluations,
  listQualityFrameworks,
  rerunQualitativeCriterion,
  runQualitativeAssessment,
  validateCriterionExpression,
} from "../../api/qualityFrameworks";
import type {
  CriterionResult,
  FrameworkEvaluation,
  QualityFramework,
} from "../../api/qualityFrameworksTypes";

vi.mock("../../api/qualityFrameworks", () => ({
  listQualityFrameworks: vi.fn(),
  listFrameworkEvaluations: vi.fn(),
  listAvailableMetricKeys: vi.fn(),
  evaluateFramework: vi.fn(),
  validateCriterionExpression: vi.fn(),
  createFrameworkCriterion: vi.fn(),
  deleteFrameworkCriterion: vi.fn(),
  deleteFrameworkEvaluation: vi.fn(),
  createQualityFramework: vi.fn(),
  cloneFramework: vi.fn(),
  resetFrameworkToTemplate: vi.fn(),
  deleteQualityFramework: vi.fn(),
  getQualitativeAssessment: vi.fn(),
  getQualitativeAssessmentStatus: vi.fn(),
  runQualitativeAssessment: vi.fn(),
  rerunQualitativeCriterion: vi.fn(),
}));

const listQualityFrameworksMock = vi.mocked(listQualityFrameworks);
const listFrameworkEvaluationsMock = vi.mocked(listFrameworkEvaluations);
const listAvailableMetricKeysMock = vi.mocked(listAvailableMetricKeys);
const evaluateFrameworkMock = vi.mocked(evaluateFramework);
const validateCriterionExpressionMock = vi.mocked(validateCriterionExpression);
const createFrameworkCriterionMock = vi.mocked(createFrameworkCriterion);
const getQualitativeAssessmentMock = vi.mocked(getQualitativeAssessment);
const getQualitativeAssessmentStatusMock = vi.mocked(getQualitativeAssessmentStatus);
const runQualitativeAssessmentMock = vi.mocked(runQualitativeAssessment);
const rerunQualitativeCriterionMock = vi.mocked(rerunQualitativeCriterion);
const deleteFrameworkEvaluationMock = vi.mocked(deleteFrameworkEvaluation);

function framework(overrides: Partial<QualityFramework> = {}): QualityFramework {
  return {
    id: "qframework_1",
    name: "Quality (Kroeze-style)",
    description: null,
    origin: "app_template",
    templateKey: "kroeze_quality",
    clonedFrom: null,
    version: 1,
    createdAt: "2026-06-01T10:00:00Z",
    updatedAt: "2026-06-01T10:00:00Z",
    criteria: [
      {
        id: "qcriterion_1",
        frameworkId: "qframework_1",
        ordinal: 0,
        label: "Strong return on equity",
        expression: "roe >= 15%",
        weight: null,
        partialBand: "10%",
        kind: "quantitative",
        assessmentGuidance: null,
        createdAt: "2026-06-01T10:00:00Z",
        updatedAt: "2026-06-01T10:00:00Z",
      },
    ],
    ...overrides,
  };
}

function evaluation(overrides: Partial<FrameworkEvaluation> = {}): FrameworkEvaluation {
  return {
    id: "qeval_1",
    frameworkId: "qframework_1",
    frameworkVersion: 1,
    companyId: "company_gpw_cdr",
    periodId: "period_1",
    passCount: 1,
    partialCount: 0,
    failCount: 0,
    unavailableCount: 0,
    engineVersion: "qf-1",
    createdAt: "2026-06-10T10:00:00Z",
    results: [
      {
        id: "qresult_1",
        evaluationId: "qeval_1",
        criterionId: "qcriterion_1",
        ordinal: 0,
        label: "Strong return on equity",
        expression: "roe >= 15%",
        verdict: "pass",
        measuredValue: "0.18",
        measuredUnit: null,
        threshold: "0.15",
        inputsJson: '["roe"]',
        note: null,
        reasoning: null,
        citations: null,
        confidence: null,
        promptVersion: null,
        source: "engine",
      },
    ],
    ...overrides,
  };
}

describe("QualityPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listQualityFrameworksMock.mockResolvedValue([framework()]);
    listFrameworkEvaluationsMock.mockResolvedValue([]);
    listAvailableMetricKeysMock.mockResolvedValue([
      {
        key: "roe",
        label: "Return on equity",
        unit: null,
        valueKind: "percentage",
        computation: "derived",
        scope: "canonical",
      },
    ]);
    evaluateFrameworkMock.mockResolvedValue(evaluation());
    validateCriterionExpressionMock.mockResolvedValue({
      ok: true,
      error: null,
      referencedMetricKeys: ["roe"],
    });
    getQualitativeAssessmentMock.mockResolvedValue([]);
    getQualitativeAssessmentStatusMock.mockResolvedValue({
      status: "queued",
      attempts: 0,
      lastError: null,
    });
    runQualitativeAssessmentMock.mockResolvedValue(undefined);
    rerunQualitativeCriterionMock.mockResolvedValue(undefined);
    createFrameworkCriterionMock.mockResolvedValue({
      id: "qcriterion_new",
      frameworkId: "qframework_1",
      ordinal: 1,
      label: "Wide moat",
      expression: "",
      weight: null,
      partialBand: null,
      kind: "qualitative",
      assessmentGuidance: "Assess durable advantage.",
      createdAt: "2026-06-01T10:00:00Z",
      updatedAt: "2026-06-01T10:00:00Z",
    });
  });

  // A framework carrying one qualitative criterion (ADR 0075).
  function qualitativeFramework(): QualityFramework {
    return framework({
      criteria: [
        {
          id: "qcriterion_moat",
          frameworkId: "qframework_1",
          ordinal: 0,
          label: "Wide moat",
          expression: "",
          weight: null,
          partialBand: null,
          kind: "qualitative",
          assessmentGuidance: "Assess durable competitive advantage.",
          createdAt: "2026-06-01T10:00:00Z",
          updatedAt: "2026-06-01T10:00:00Z",
        },
      ],
    });
  }

  function agentResult(overrides: Partial<CriterionResult> = {}): CriterionResult {
    return {
      id: "qresult_moat",
      evaluationId: "qeval_agent",
      criterionId: "qcriterion_moat",
      ordinal: 0,
      label: "Wide moat",
      expression: "",
      verdict: "pass",
      measuredValue: null,
      measuredUnit: null,
      threshold: null,
      inputsJson: null,
      note: null,
      reasoning: "The company shows durable pricing power across cycles.",
      citations: JSON.stringify([
        {
          citationKey: "E1",
          evidenceType: "feed_item",
          evidenceId: "feed_1",
          label: "Q4 report",
          snippet: "Margins expanded again.",
        },
      ]),
      confidence: "high",
      promptVersion: "qualitative_assessment_v1",
      source: "agent",
      ...overrides,
    };
  }

  it("renders the framework and its criteria", async () => {
    render(<QualityPanel companyId="company_gpw_cdr" />);
    expect(await screen.findByText("Strong return on equity")).toBeInTheDocument();
    expect(screen.getByText("roe >= 15%")).toBeInTheDocument();
  });

  it("moves focus to the next history row after deleting an evaluation (ADR 0076 D9)", async () => {
    const evalA = evaluation({ id: "qeval_a", createdAt: "2026-06-10T10:00:00Z" });
    const evalB = evaluation({ id: "qeval_b", createdAt: "2026-06-11T10:00:00Z" });
    const evalC = evaluation({ id: "qeval_c", createdAt: "2026-06-12T10:00:00Z" });
    listFrameworkEvaluationsMock
      .mockReset()
      .mockResolvedValueOnce([evalC, evalB, evalA]) // initial load (newest first)
      .mockResolvedValueOnce([evalC, evalA]); // after deleting the middle row
    deleteFrameworkEvaluationMock.mockResolvedValue(undefined);

    const user = userEvent.setup();
    const { container } = render(<QualityPanel companyId="company_gpw_cdr" />);

    // Three history rows render, newest first.
    await waitFor(() => {
      expect(container.querySelectorAll(".quality-history-row")).toHaveLength(3);
    });

    // Delete the middle row (evalB, slot 1). Its delete button lives inside the row.
    const deleteButtons = screen.getAllByRole("button", { name: "Delete evaluation" });
    await user.click(deleteButtons[1]);

    // The row that slid into slot 1 (evalA) now owns focus — not <body>.
    await waitFor(() => {
      const rows = container.querySelectorAll<HTMLElement>(".quality-history-row");
      expect(rows).toHaveLength(2);
      expect(document.activeElement).toBe(rows[1]);
      expect(rows[1].getAttribute("aria-label")).toContain("2026-06-10");
    });
  });

  it("runs an evaluation and shows the scorecard verdict", async () => {
    const user = userEvent.setup();
    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Strong return on equity");

    await user.click(screen.getByRole("button", { name: "Evaluate" }));

    await waitFor(() => {
      expect(evaluateFrameworkMock).toHaveBeenCalledWith({
        frameworkId: "qframework_1",
        companyId: "company_gpw_cdr",
      });
    });
    // The verdict chip and measured value appear — the measure renders through
    // the format layer (T7 round-2: percent criterion → percentage, never the
    // raw decimal-exact engine text).
    expect(await screen.findByText("Pass")).toBeInTheDocument();
    expect(screen.getByText("18%")).toBeInTheDocument();
  });

  it("validates a criterion expression as it is typed", async () => {
    const user = userEvent.setup();
    validateCriterionExpressionMock.mockResolvedValue({
      ok: false,
      error: "parse error: expected a value",
      referencedMetricKeys: [],
    });
    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Strong return on equity");

    const expressionField = screen.getByPlaceholderText("roe >= 15%");
    await user.type(expressionField, "roe >=");

    expect(await screen.findByText(/parse error/)).toBeInTheDocument();
  });

  it("inserts a picked metric into the expression", async () => {
    const user = userEvent.setup();
    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Strong return on equity");

    await user.selectOptions(screen.getByLabelText("Insert metric"), "roe");

    expect(screen.getByPlaceholderText("roe >= 15%")).toHaveValue("roe");
  });

  it("expands a history run to reveal its per-criterion snapshot", async () => {
    const user = userEvent.setup();
    const run = evaluation();
    // A distinct snapshot label so it can only come from the expanded history
    // detail, not the live scorecard (which renders the framework's criteria).
    run.results = [{ ...run.results[0], label: "Snapshot criterion", verdict: "fail" }];
    listFrameworkEvaluationsMock.mockResolvedValue([run]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Strong return on equity");

    // Collapsed by default: the snapshot detail is not shown.
    expect(screen.queryByText("Snapshot criterion")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /2026-06-10/ }));

    expect(await screen.findByText("Snapshot criterion")).toBeInTheDocument();
  });

  it("shows the empty state when there are no frameworks", async () => {
    listQualityFrameworksMock.mockResolvedValue([]);
    render(<QualityPanel companyId="company_gpw_cdr" />);
    expect(
      await screen.findByText("No quality frameworks yet. Create one or clone a template."),
    ).toBeInTheDocument();
  });

  it("creates a qualitative criterion with kind and guidance", async () => {
    const user = userEvent.setup();
    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Strong return on equity");

    await user.click(screen.getByRole("button", { name: "Qualitative" }));
    await user.type(screen.getByPlaceholderText("Wide, durable moat"), "Wide moat");
    await user.type(
      screen.getByLabelText("Assessment guidance"),
      "Assess durable advantage.",
    );
    await user.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(createFrameworkCriterionMock).toHaveBeenCalledWith({
        frameworkId: "qframework_1",
        label: "Wide moat",
        expression: "",
        kind: "qualitative",
        assessmentGuidance: "Assess durable advantage.",
      });
    });
  });

  it("renders an agent assessment result, labelled and cited", async () => {
    const user = userEvent.setup();
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);
    getQualitativeAssessmentMock.mockResolvedValue([agentResult()]);

    render(<QualityPanel companyId="company_gpw_cdr" />);

    // Wait for the assessment to load (verdict chip proves the result branch,
    // distinct from the not-yet-assessed row).
    expect(await screen.findByText("Pass")).toBeInTheDocument();
    expect(screen.getByText("Agent-assessed")).toBeInTheDocument();

    // Expanding reveals the reasoning and its citation.
    await user.click(screen.getByRole("button", { name: /Wide moat/ }));
    expect(
      await screen.findByText("The company shows durable pricing power across cycles."),
    ).toBeInTheDocument();
    expect(screen.getByText("Q4 report")).toBeInTheDocument();
  });

  it("does not label a never-assessed criterion as agent-assessed", async () => {
    // "Agent-assessed" reads as a STATE (past tense) — it must appear only when
    // an agent result actually exists; a never-assessed criterion shows the
    // not-assessed state instead.
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);
    getQualitativeAssessmentMock.mockResolvedValue([]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Wide moat");

    expect(screen.queryByText("Agent-assessed")).not.toBeInTheDocument();
    expect(screen.getByText("Not assessed yet")).toBeInTheDocument();
  });

  it("shows the insufficient-evidence verdict distinctly", async () => {
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);
    getQualitativeAssessmentMock.mockResolvedValue([
      agentResult({ verdict: "insufficient_evidence", citations: null }),
    ]);

    render(<QualityPanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Insufficient evidence")).toBeInTheDocument();
  });

  it("enqueues an assessment run for the framework", async () => {
    const user = userEvent.setup();
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Wide moat");

    await user.click(screen.getByRole("button", { name: "Assess" }));

    await waitFor(() => {
      expect(runQualitativeAssessmentMock).toHaveBeenCalledWith({
        frameworkId: "qframework_1",
        companyId: "company_gpw_cdr",
      });
    });
    expect(
      await screen.findByText(/Assessment queued/),
    ).toBeInTheDocument();
  });

  it("surfaces a failed assessment run with the backend error (P1)", async () => {
    // P1 (owner T7): a queued assessment that fails asynchronously (no provider,
    // uncited response) used to clear its hint silently. The poll now reads the
    // job status and, on a terminal failure, surfaces the backend error string.
    // Fake timers drive the bounded poll deterministically; `fireEvent` (not
    // userEvent) avoids userEvent's internal real-timer delays under fake clocks.
    vi.useFakeTimers();
    try {
      listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);
      getQualitativeAssessmentStatusMock.mockResolvedValue({
        status: "failed",
        attempts: 3,
        lastError:
          "no usable AI provider for capability qualitative_assessment. Fix this in Settings → AI.",
      });

      render(<QualityPanel companyId="company_gpw_cdr" />);
      const flush = async () => {
        await act(async () => {
          for (let i = 0; i < 8; i += 1) await Promise.resolve();
        });
      };
      // Settle the initial async framework/assessment load.
      await flush();

      fireEvent.click(screen.getByRole("button", { name: "Assess" }));
      await flush();
      expect(screen.getByText(/Assessment queued/)).toBeInTheDocument();

      // One poll interval elapses; the status read returns a terminal failure.
      await act(async () => {
        vi.advanceTimersByTime(3000);
        for (let i = 0; i < 8; i += 1) await Promise.resolve();
      });

      expect(screen.getByText(/no usable AI provider/)).toBeInTheDocument();
      expect(screen.queryByText(/Assessment queued/)).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the quantitative scorecard when a newer qual-only snapshot exists", async () => {
    // ADR 0075 Decision 5: a qualitative-only snapshot (written by the agent
    // job into the same framework_evaluations table) must never become the
    // panel's quantitative "latest" — the scorecard summary and per-criterion
    // verdicts read the newest snapshot that carries engine results.
    const mixed = framework({
      criteria: [
        ...framework().criteria,
        ...qualitativeFramework().criteria.map((criterion) => ({
          ...criterion,
          ordinal: 1,
        })),
      ],
    });
    listQualityFrameworksMock.mockResolvedValue([mixed]);
    const qualOnly = evaluation({
      id: "qeval_agent",
      periodId: null,
      passCount: 0,
      partialCount: 0,
      failCount: 1,
      unavailableCount: 0,
      createdAt: "2026-06-20T10:00:00Z",
      results: [agentResult({ verdict: "fail" })],
    });
    listFrameworkEvaluationsMock.mockResolvedValue([qualOnly, evaluation()]);
    getQualitativeAssessmentMock.mockResolvedValue([agentResult({ verdict: "fail" })]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Strong return on equity");

    // The quant criterion keeps its measured value + verdict from the older
    // quantitative snapshot; the summary chips show the quant counts.
    expect(await screen.findByText("18%")).toBeInTheDocument();
    expect(screen.getByText("1 pass")).toBeInTheDocument();
  });

  it("folds a quantitative criterion's expression into an expansion (U7-B density)", async () => {
    // Density contract (ADR 0076 D6, Quality): the expression is reachable behind
    // a per-criterion disclosure so it can fold at the S tier. The row exposes it
    // as an expandable disclosure whose detail carries the expression.
    const user = userEvent.setup();
    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Strong return on equity");

    // The criterion row is an expandable disclosure (aria-expanded).
    const row = screen.getByRole("button", { name: /Strong return on equity/ });
    expect(row).toHaveAttribute("aria-expanded", "false");

    await user.click(row);
    expect(row).toHaveAttribute("aria-expanded", "true");
    // The expression is reachable in the expanded detail.
    expect(screen.getAllByText("roe >= 15%").length).toBeGreaterThan(0);
  });

  it("folds the evaluation history behind a disclosure toggle (U7-B density)", async () => {
    // Density contract: history folds when short. It is always reachable behind a
    // disclosure; the toggle collapses/reveals the run list.
    const run = evaluation({ id: "qeval_hist", createdAt: "2026-06-10T10:00:00Z" });
    listFrameworkEvaluationsMock.mockResolvedValue([run]);

    const user = userEvent.setup();
    render(<QualityPanel companyId="company_gpw_cdr" />);

    // Default (tall) shows the history list expanded.
    const toggle = await screen.findByRole("button", { name: /Evaluation history/ });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: /2026-06-10/ })).toBeInTheDocument();

    // Collapsing the disclosure hides the run rows.
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: /2026-06-10/ })).not.toBeInTheDocument();
    });
  });

  it("re-runs a single qualitative criterion", async () => {
    const user = userEvent.setup();
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);
    getQualitativeAssessmentMock.mockResolvedValue([agentResult()]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Agent-assessed");

    await user.click(screen.getByRole("button", { name: "Re-run assessment" }));

    await waitFor(() => {
      expect(rerunQualitativeCriterionMock).toHaveBeenCalledWith({
        frameworkId: "qframework_1",
        companyId: "company_gpw_cdr",
        criterionId: "qcriterion_moat",
      });
    });
  });
});
