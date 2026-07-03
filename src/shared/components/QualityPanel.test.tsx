import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QualityPanel } from "./QualityPanel";
import {
  evaluateFramework,
  listAvailableMetricKeys,
  listFrameworkEvaluations,
  listQualityFrameworks,
  validateCriterionExpression,
} from "../../api/qualityFrameworks";
import type {
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
}));

const listQualityFrameworksMock = vi.mocked(listQualityFrameworks);
const listFrameworkEvaluationsMock = vi.mocked(listFrameworkEvaluations);
const listAvailableMetricKeysMock = vi.mocked(listAvailableMetricKeys);
const evaluateFrameworkMock = vi.mocked(evaluateFramework);
const validateCriterionExpressionMock = vi.mocked(validateCriterionExpression);

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
  });

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
});
