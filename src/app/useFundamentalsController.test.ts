import { act, renderHook } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as financialsApi from "../api/financials";
import type { FinancialFact, FinancialPeriod } from "../api/financialsTypes";
import {
  useFundamentalsController,
  type FinancialFactForm,
  type FundamentalsForm,
} from "./useFundamentalsController";

// useFundamentalsController is the shared editing/action layer behind both the
// Companies-screen Fundamentals tab and the cockpit's fundamentals panel
// (useCockpitFundamentals). It owns period/fact create-edit-delete and the two
// forms; the surrounding screens only own load state. Exercised here directly
// via renderHook (a mocked api/financials module) rather than through a full
// screen workflow: reaching it from the app requires drilling into the cockpit
// via company selection + panel pinning, which is disproportionate to pin this
// controller's own branches (valid/invalid input, create vs. edit, error
// surfacing) — those are the controller's job, not the screen's.

vi.mock("../api/financials");

const EMPTY_FORM: FundamentalsForm = { periodFiscalYear: "", periodType: "annual" };
const EMPTY_FACT_FORM: FinancialFactForm = {
  definitionId: "",
  valueNumeric: "",
  currency: "",
  periodId: "",
  annotation: "",
  dataQuality: "final",
};

const period: FinancialPeriod = {
  id: "period_1",
  companyId: "company_gpw_cdr",
  fiscalYear: 2025,
  periodType: "annual",
  periodEndDate: null,
  reportEvidenceRef: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const fact: FinancialFact = {
  id: "fact_1",
  companyId: "company_gpw_cdr",
  periodId: "period_1",
  definitionId: "kpi_revenue",
  metricKey: "revenue",
  valueNumeric: "1000",
  currency: "PLN",
  statementBasis: "consolidated",
  attribution: "total",
  variant: "reported",
  measureWindow: "flow",
  dataQuality: "final",
  asReportedValue: null,
  asReportedScale: null,
  reportingStandard: null,
  extractionMethod: "manual",
  confidence: null,
  confirmationState: "confirmed",
  supersedesId: null,
  sourceDocumentRef: null,
  annotation: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

// ADR 0093 dec. 2: a preliminary fact, otherwise identical to `fact` — used to
// prove `saveFinancialFact`'s update path sends the ROW'S EXISTING
// `dataQuality`, not a hardcoded "final" (which would trip the backend's
// typed `FinancialFactDataQualityLocked` conflict on every edit of a
// preliminary/estimated fact).
const preliminaryFact: FinancialFact = {
  ...fact,
  id: "fact_preliminary_1",
  dataQuality: "preliminary",
  extractionMethod: "mcp_agent",
};

const fakeSubmitEvent = { preventDefault: () => {} } as unknown as React.FormEvent<HTMLFormElement>;

const refreshFinancialPeriods = vi.fn(async () => {});
const refreshFinancialFacts = vi.fn(async () => {});
const refreshKpiDefinitions = vi.fn(async () => {});

function useHarness(props: { periods?: FinancialPeriod[]; facts?: FinancialFact[] } = {}) {
  const [financialPeriods] = useState<FinancialPeriod[]>(props.periods ?? []);
  const [financialFacts] = useState<FinancialFact[]>(props.facts ?? []);
  const [fundamentalsForm, setFundamentalsForm] = useState<FundamentalsForm>(EMPTY_FORM);
  const [financialFactForm, setFinancialFactForm] = useState<FinancialFactForm>(EMPTY_FACT_FORM);
  const [selectedFinancialFactId, setSelectedFinancialFactId] = useState<string | null>(null);
  const [isFinancialFactEditMode, setIsFinancialFactEditMode] = useState(false);
  const [fundamentalsError, setFundamentalsError] = useState<string | null>(null);

  const controller = useFundamentalsController({
    companyId: "company_gpw_cdr",
    financialPeriods,
    financialFacts,
    kpiDefinitions: [],
    fundamentalsForm,
    setFundamentalsForm,
    financialFactForm,
    setFinancialFactForm,
    selectedFinancialFactId,
    setSelectedFinancialFactId,
    isFinancialFactEditMode,
    setIsFinancialFactEditMode,
    fundamentalsError,
    setFundamentalsError,
    refreshFinancialPeriods,
    refreshFinancialFacts,
    refreshKpiDefinitions,
    text: (value: string) => value,
  });

  return {
    ...controller,
    fundamentalsForm,
    financialFactForm,
    selectedFinancialFactId,
    isFinancialFactEditMode,
    fundamentalsError,
  };
}

beforeEach(() => {
  vi.mocked(financialsApi.createFinancialPeriod).mockReset().mockResolvedValue(period);
  vi.mocked(financialsApi.createFinancialFact).mockReset().mockResolvedValue(fact);
  vi.mocked(financialsApi.updateFinancialFact).mockReset().mockResolvedValue(fact);
  vi.mocked(financialsApi.deleteFinancialFact).mockReset().mockResolvedValue(undefined);
  refreshFinancialPeriods.mockClear();
  refreshFinancialFacts.mockClear();
  refreshKpiDefinitions.mockClear();
});

describe("useFundamentalsController", () => {
  it("creates a financial period, resets the form, and refreshes periods", async () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.updateFundamentalsForm("periodFiscalYear", "2025");
      result.current.updateFundamentalsForm("periodType", "q1");
    });

    await act(async () => {
      await result.current.createFinancialPeriod(fakeSubmitEvent);
    });

    expect(financialsApi.createFinancialPeriod).toHaveBeenCalledWith({
      companyId: "company_gpw_cdr",
      fiscalYear: 2025,
      periodType: "q1",
    });
    expect(refreshFinancialPeriods).toHaveBeenCalledTimes(1);
    expect(result.current.fundamentalsForm).toEqual({ periodFiscalYear: "", periodType: "annual" });
  });

  it("rejects a non-numeric fiscal year without calling the API", async () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.updateFundamentalsForm("periodFiscalYear", "not-a-year");
    });

    await act(async () => {
      await result.current.createFinancialPeriod(fakeSubmitEvent);
    });

    expect(financialsApi.createFinancialPeriod).not.toHaveBeenCalled();
    expect(refreshFinancialPeriods).not.toHaveBeenCalled();
    expect(result.current.fundamentalsError).toBe("Invalid fiscal year");
  });

  it("surfaces the API failure message when creating a period fails", async () => {
    vi.mocked(financialsApi.createFinancialPeriod).mockRejectedValue(new Error("db locked"));
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.updateFundamentalsForm("periodFiscalYear", "2025");
    });

    await act(async () => {
      await result.current.createFinancialPeriod(fakeSubmitEvent);
    });

    expect(result.current.fundamentalsError).toBe("db locked");
    expect(refreshFinancialPeriods).not.toHaveBeenCalled();
  });

  it("creates a financial fact against the selected period, resets the form, and refreshes facts", async () => {
    const { result } = renderHook(() => useHarness({ periods: [period] }));

    act(() => {
      result.current.updateFinancialFactForm("definitionId", "kpi_revenue");
      result.current.updateFinancialFactForm("periodId", "period_1");
      result.current.updateFinancialFactForm("valueNumeric", "1000");
      result.current.updateFinancialFactForm("currency", "PLN");
    });

    await act(async () => {
      await result.current.saveFinancialFact(fakeSubmitEvent);
    });

    expect(financialsApi.createFinancialFact).toHaveBeenCalledWith({
      companyId: "company_gpw_cdr",
      periodId: "period_1",
      definitionId: "kpi_revenue",
      valueNumeric: "1000",
      currency: "PLN",
      statementBasis: "consolidated",
      attribution: "total",
      variant: "reported",
      measureWindow: "flow",
      dataQuality: "final",
      extractionMethod: "manual",
      confirmationState: "confirmed",
    });
    expect(refreshFinancialFacts).toHaveBeenCalledTimes(1);
    expect(result.current.financialFactForm).toEqual(EMPTY_FACT_FORM);
    expect(result.current.selectedFinancialFactId).toBeNull();
  });

  it("rejects an unresolved reporting period without calling the API", async () => {
    const { result } = renderHook(() => useHarness({ periods: [period] }));

    act(() => {
      result.current.updateFinancialFactForm("definitionId", "kpi_revenue");
      result.current.updateFinancialFactForm("periodId", "does-not-exist");
      result.current.updateFinancialFactForm("valueNumeric", "1000");
    });

    await act(async () => {
      await result.current.saveFinancialFact(fakeSubmitEvent);
    });

    expect(financialsApi.createFinancialFact).not.toHaveBeenCalled();
    expect(result.current.fundamentalsError).toBe("Please select a reporting period");
  });

  it("rejects a non-numeric fact value without calling the API", async () => {
    const { result } = renderHook(() => useHarness({ periods: [period] }));

    act(() => {
      result.current.updateFinancialFactForm("periodId", "period_1");
      result.current.updateFinancialFactForm("valueNumeric", "not-a-number");
    });

    await act(async () => {
      await result.current.saveFinancialFact(fakeSubmitEvent);
    });

    expect(financialsApi.createFinancialFact).not.toHaveBeenCalled();
    expect(result.current.fundamentalsError).toBe("Invalid numeric value");
  });

  it("updates a fact in place when in edit mode, then leaves edit mode", async () => {
    const { result } = renderHook(() => useHarness({ periods: [period], facts: [fact] }));

    act(() => {
      result.current.selectFinancialFact("fact_1");
    });
    expect(result.current.financialFactForm).toEqual({
      annotation: "",
      definitionId: "kpi_revenue",
      valueNumeric: "1000",
      currency: "PLN",
      periodId: "period_1",
      dataQuality: "final",
    });

    act(() => {
      result.current.startEditingFinancialFact();
      result.current.updateFinancialFactForm("valueNumeric", "1200");
    });
    expect(result.current.isFinancialFactEditMode).toBe(true);

    await act(async () => {
      await result.current.saveFinancialFact(fakeSubmitEvent);
    });

    expect(financialsApi.updateFinancialFact).toHaveBeenCalledWith({
      id: "fact_1",
      valueNumeric: "1200",
      currency: "PLN",
      dataQuality: "final",
      confirmationState: "confirmed",
      // Always sent on update: "" is the explicit clear-the-annotation signal.
      annotation: "",
    });
    expect(financialsApi.createFinancialFact).not.toHaveBeenCalled();
    expect(refreshFinancialFacts).toHaveBeenCalledTimes(1);
    expect(result.current.isFinancialFactEditMode).toBe(false);
    expect(result.current.selectedFinancialFactId).toBeNull();
  });

  // ADR 0093 dec. 2 (epic #285 T5): the update path used to hardcode
  // `dataQuality: "final"` regardless of the fact being edited — which would
  // attempt an illegal quality flip on every edit of a preliminary/estimated
  // fact and trip the backend's typed `FinancialFactDataQualityLocked`
  // conflict. It must send the ROW'S OWN existing quality instead (a no-op
  // resend the backend passes straight through).
  it("preserves a preliminary fact's dataQuality on update instead of hardcoding final", async () => {
    vi.mocked(financialsApi.updateFinancialFact).mockResolvedValue(preliminaryFact);
    const { result } = renderHook(() =>
      useHarness({ periods: [period], facts: [preliminaryFact] }),
    );

    act(() => {
      result.current.selectFinancialFact("fact_preliminary_1");
    });
    expect(result.current.financialFactForm.dataQuality).toBe("preliminary");

    act(() => {
      result.current.startEditingFinancialFact();
      result.current.updateFinancialFactForm("valueNumeric", "1200");
    });

    await act(async () => {
      await result.current.saveFinancialFact(fakeSubmitEvent);
    });

    expect(financialsApi.updateFinancialFact).toHaveBeenCalledWith(
      expect.objectContaining({ id: "fact_preliminary_1", dataQuality: "preliminary" }),
    );
  });

  // Defense-in-depth: IF the backend ever does reject a quality-locked update
  // (e.g. a future direct edit path bypassing the preserved-quality form
  // field), the typed `FinancialFactDataQualityLocked` conflict must surface
  // readably — the existing `error instanceof Error` catch already does this
  // for any `CommandInvocationError` (ADR 0070), pinned here so the path
  // never regresses silently.
  it("surfaces a typed data-quality-locked conflict readably when an update is rejected", async () => {
    vi.mocked(financialsApi.updateFinancialFact).mockRejectedValue(
      new Error(
        "fact fact_1 data_quality is immutable via update (final -> preliminary): data_quality is a uniqueness-slot dimension (ADR 0093) — create a new final fact instead so supersession records the change",
      ),
    );
    const { result } = renderHook(() => useHarness({ periods: [period], facts: [fact] }));

    act(() => {
      result.current.selectFinancialFact("fact_1");
      result.current.startEditingFinancialFact();
    });

    await act(async () => {
      await result.current.saveFinancialFact(fakeSubmitEvent);
    });

    expect(result.current.fundamentalsError).toBe(
      "fact fact_1 data_quality is immutable via update (final -> preliminary): data_quality is a uniqueness-slot dimension (ADR 0093) — create a new final fact instead so supersession records the change",
    );
  });

  it("cancels an in-progress edit and clears the form and selection", () => {
    const { result } = renderHook(() => useHarness({ periods: [period], facts: [fact] }));

    act(() => {
      result.current.selectFinancialFact("fact_1");
      result.current.startEditingFinancialFact();
    });
    expect(result.current.isFinancialFactEditMode).toBe(true);

    act(() => {
      result.current.cancelEditingFinancialFact();
    });

    expect(result.current.isFinancialFactEditMode).toBe(false);
    expect(result.current.selectedFinancialFactId).toBeNull();
    expect(result.current.financialFactForm).toEqual(EMPTY_FACT_FORM);
  });

  it("deletes a financial fact, clears the selection, and refreshes facts", async () => {
    const { result } = renderHook(() => useHarness({ periods: [period], facts: [fact] }));

    act(() => {
      result.current.selectFinancialFact("fact_1");
    });

    await act(async () => {
      await result.current.deleteFinancialFact("fact_1");
    });

    expect(financialsApi.deleteFinancialFact).toHaveBeenCalledWith("fact_1");
    expect(refreshFinancialFacts).toHaveBeenCalledTimes(1);
    expect(result.current.selectedFinancialFactId).toBeNull();
    expect(result.current.isFinancialFactEditMode).toBe(false);
  });

  it("surfaces the API failure message when deleting a fact fails", async () => {
    vi.mocked(financialsApi.deleteFinancialFact).mockRejectedValue(new Error("fact in use"));
    const { result } = renderHook(() => useHarness({ periods: [period], facts: [fact] }));

    await act(async () => {
      await result.current.deleteFinancialFact("fact_1");
    });

    expect(result.current.fundamentalsError).toBe("fact in use");
  });
});
