import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QualityPanel } from "./QualityPanel";
import {
  createFrameworkCriterion,
  deleteFrameworkEvaluation,
  evaluateFramework,
  listAvailableMetricKeys,
  listFrameworkEvaluations,
  listQualityFrameworks,
  validateCriterionExpression,
} from "../../api/qualityFrameworks";
import type {
  CriterionResult,
  FrameworkEvaluation,
  QualityFramework,
} from "../../api/qualityFrameworksTypes";

vi.mock("../../api/companyHealth", () => ({
  // CompanyHealthSection fetches on mount inside QualityPanel; an unmocked
  // module would fall through to the real invoke (undefined in vitest).
  getCompanyHealth: vi.fn(() =>
    Promise.resolve({ companyId: "company_1", periods: [] }),
  ),
}));

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
}));

const listQualityFrameworksMock = vi.mocked(listQualityFrameworks);
const listFrameworkEvaluationsMock = vi.mocked(listFrameworkEvaluations);
const listAvailableMetricKeysMock = vi.mocked(listAvailableMetricKeys);
const evaluateFrameworkMock = vi.mocked(evaluateFramework);
const validateCriterionExpressionMock = vi.mocked(validateCriterionExpression);
const createFrameworkCriterionMock = vi.mocked(createFrameworkCriterion);
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

  it("formats the measured value AND threshold through the format layer (ADR 0076, card 619c8d9)", async () => {
    // Live T7 finding (card 619c8d9): the panel rendered the raw decimal-exact
    // engine value ("0.2862473442785260575536259831") and the normalized decimal
    // threshold ("0.15") inline. Both are value_numeric-style measures and must
    // render through the ONE shared format layer — a percent-threshold criterion
    // as a locale percentage — never the raw high-precision / normalized string.
    const user = userEvent.setup();
    listFrameworkEvaluationsMock.mockResolvedValue([
      evaluation({
        results: [
          {
            id: "qresult_roe",
            evaluationId: "qeval_1",
            criterionId: "qcriterion_1",
            ordinal: 0,
            label: "Strong return on equity",
            expression: "roe >= 15%",
            verdict: "pass",
            measuredValue: "0.2862473442785260575536259831",
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
      }),
    ]);
    render(<QualityPanel companyId="company_gpw_cdr" />);

    // The measured value renders as a percentage, never the raw 26-digit decimal.
    expect(await screen.findByText("28.6%")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("0.2862473442785260575536259831");

    // Expand the row to reveal the measured / threshold detail line.
    await user.click(screen.getByRole("button", { name: "Strong return on equity — Pass" }));

    // The threshold is a value_numeric-style measure too: it must format through
    // the same layer (percent threshold → "15%"), never the raw normalized decimal.
    expect(screen.getByText("Threshold: 15%")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("Threshold: 0.15");
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

  // ADR 0084 decision 5 (clean cut): `criterion_results` written by the AI
  // assessor were DROPPED with their read command, so a qualitative criterion
  // has no verdict to show — it renders as the user-authored check it is.
  it("renders a qualitative criterion with its guidance and no verdict", async () => {
    const user = userEvent.setup();
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Wide moat");

    expect(screen.getByText("Not assessed yet")).toBeInTheDocument();
    expect(screen.queryByText("Agent-assessed")).not.toBeInTheDocument();
    expect(screen.queryByText("Citations")).not.toBeInTheDocument();

    // Expanding reveals the owner-authored guidance, not an AI reasoning blob.
    await user.click(screen.getByRole("button", { name: /Wide moat/ }));
    expect(await screen.findByText("Assess durable competitive advantage.")).toBeInTheDocument();
  });

  // Card b875e69 (ADR 0088): agents write qualitative verdicts over MCP with
  // `set_qualitative_verdicts`; those land in the evaluation history as
  // `source: "agent"` rows. The panel must SURFACE the newest agent verdict per
  // qualitative criterion — otherwise the write capability is unreachable.
  it("renders an MCP-written qualitative verdict and its reasoning", async () => {
    const user = userEvent.setup();
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);
    listFrameworkEvaluationsMock.mockResolvedValue([
      evaluation({
        id: "qeval_agent",
        periodId: null,
        passCount: 0,
        partialCount: 0,
        failCount: 0,
        unavailableCount: 0,
        createdAt: "2026-07-01T10:00:00Z",
        results: [agentResult({ verdict: "partial", reasoning: "Moat is real but narrowing." })],
      }),
    ]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Wide moat");

    // The agent's verdict is visible — not the hardcoded "Not assessed yet".
    expect(screen.getByText("Partial")).toBeInTheDocument();
    expect(screen.queryByText("Not assessed yet")).not.toBeInTheDocument();

    // Expanding reveals the agent's written reasoning alongside the guidance.
    await user.click(screen.getByRole("button", { name: /Wide moat/ }));
    expect(await screen.findByText("Moat is real but narrowing.")).toBeInTheDocument();
    expect(screen.getByText("Assess durable competitive advantage.")).toBeInTheDocument();
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

  // ADR 0084 clean cut: no stored AI verdict surface survives, and no in-app
  // (re-)assessment affordance exists. Verdicts return as agent writes over MCP.
  it("renders no stored AI verdict surface at all (ADR 0084 clean cut)", async () => {
    listQualityFrameworksMock.mockResolvedValue([qualitativeFramework()]);

    render(<QualityPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Wide moat");

    for (const name of ["Assess", "Re-run assessment", "Assess this criterion"]) {
      expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    }
    expect(screen.queryByText(/Assessment queued/)).not.toBeInTheDocument();
    expect(screen.queryByText("Agent-assessed")).not.toBeInTheDocument();
    expect(screen.queryByText("Confidence")).not.toBeInTheDocument();
  });

  // THE GUARD: `criterion_results` in the owner's real database are ALL
  // `source: "engine"` — deterministic DSL evaluations (ADR 0046), never AI.
  // ADR 0084 keeps them explicitly. This test passes before AND after the AI
  // cut and reddens the moment the deterministic verdict path is over-deleted.
  it("still renders engine-sourced criterion verdicts and the scorecard (ADR 0046)", async () => {
    // One stored engine evaluation — the deterministic DSL snapshot (source:
    // "engine"), the ONLY kind that exists in the owner's real database.
    listFrameworkEvaluationsMock.mockResolvedValue([evaluation()]);

    render(<QualityPanel companyId="company_gpw_cdr" />);

    // The engine criterion, its measured value, its threshold and its verdict.
    expect(await screen.findByText("Strong return on equity")).toBeInTheDocument();
    expect(screen.getByText("18%")).toBeInTheDocument();
    expect(screen.getByText("Pass")).toBeInTheDocument();
    // The deterministic scorecard summary.
    expect(screen.getByText("1 pass")).toBeInTheDocument();

    // The engine verdict came from the deterministic evaluation read, not from
    // any retired AI assessment command.
    expect(listFrameworkEvaluationsMock).toHaveBeenCalled();
  });
});
