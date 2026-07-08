import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { useCockpitFundamentals } from "./useCockpitFundamentals";
import {
  listFinancialFacts,
  listFinancialPeriods,
  listKpiDefinitions,
} from "../../api/financials";

vi.mock("../../api/financials", () => ({
  listFinancialFacts: vi.fn(),
  listFinancialPeriods: vi.fn(),
  listKpiDefinitions: vi.fn(),
}));

const listFinancialFactsMock = vi.mocked(listFinancialFacts);
const listFinancialPeriodsMock = vi.mocked(listFinancialPeriods);
const listKpiDefinitionsMock = vi.mocked(listKpiDefinitions);

describe("useCockpitFundamentals", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listFinancialFactsMock.mockResolvedValue([]);
    listFinancialPeriodsMock.mockResolvedValue([]);
    listKpiDefinitionsMock.mockResolvedValue([]);
  });

  // Cross-panel staleness fix: a sibling report-documents extraction bumps the
  // `revision`, which must force the facts read to refetch — otherwise the panel
  // shows stale data after a successful extraction (the owner's P1).
  it("refetches facts when the revision bumps", async () => {
    const { rerender } = renderHook(
      ({ revision }: { revision: number }) => useCockpitFundamentals("company_gpw_cdr", revision),
      { initialProps: { revision: 0 } },
    );

    await waitFor(() => expect(listFinancialFactsMock).toHaveBeenCalledTimes(1));

    rerender({ revision: 1 });

    await waitFor(() => expect(listFinancialFactsMock).toHaveBeenCalledTimes(2));
  });

  it("does not refetch when the revision is unchanged", async () => {
    const { rerender } = renderHook(
      ({ revision }: { revision: number }) => useCockpitFundamentals("company_gpw_cdr", revision),
      { initialProps: { revision: 3 } },
    );

    await waitFor(() => expect(listFinancialFactsMock).toHaveBeenCalledTimes(1));

    rerender({ revision: 3 });
    // A re-render with the same revision must not trigger a new fetch.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(listFinancialFactsMock).toHaveBeenCalledTimes(1);
  });
});
