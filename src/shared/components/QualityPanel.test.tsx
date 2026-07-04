import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QualityPanel } from "./QualityPanel";
import {
  createFrameworkCriterion,
  evaluateFramework,
  getQualitativeAssessment,
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
  createQualityFramework: vi.fn(),
  cloneFramework: vi.fn(),
  resetFrameworkToTemplate: vi.fn(),
  deleteQualityFramework: vi.fn(),
  getQualitativeAssessment: vi.fn(),
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
const runQualitativeAssessmentMock = vi.mocked(runQualitativeAssessment);
const rerunQualitativeCriterionMock = vi.mocked(rerunQualitativeCriterion);

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
    // The verdict chip and measured value appear.
    expect(await screen.findByText("Pass")).toBeInTheDocument();
    expect(screen.getByText("0.18")).toBeInTheDocument();
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
