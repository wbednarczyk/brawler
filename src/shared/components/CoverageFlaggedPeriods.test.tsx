import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CoverageFlaggedPeriods } from "./CoverageFlaggedPeriods";
import {
  listFlaggedExtractionOutcomes,
  rerunExtractionOutcome,
} from "../../api/fundamentalsExtraction";
import type { ExtractionOutcome, StructuredExtractionSummary } from "../../api/fundamentalsExtraction";

vi.mock("../../api/fundamentalsExtraction", () => ({
  listFlaggedExtractionOutcomes: vi.fn(),
  rerunExtractionOutcome: vi.fn(),
}));

const listFlaggedExtractionOutcomesMock = vi.mocked(listFlaggedExtractionOutcomes);
const rerunExtractionOutcomeMock = vi.mocked(rerunExtractionOutcome);

// A fully-defaulted flagged outcome; each test overrides only the axis it exercises.
function outcome(overrides: Partial<ExtractionOutcome> = {}): ExtractionOutcome {
  return {
    id: "outcome_1",
    companyId: "company_gpw_cdr",
    reportDocumentId: "doc_1",
    fiscalYear: 2025,
    periodType: "Q3",
    periodEnd: "2025-09-30",
    tier: "pdf",
    acceptance: "flagged",
    reasonCode: "validation_failed",
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

function summary(): StructuredExtractionSummary {
  return {
    acceptance: "accepted",
    tier: "pdf",
    emitted: true,
    producedFactIds: ["fact_1"],
    skippedFactIds: [],
    divergentCount: 0,
  };
}

describe("CoverageFlaggedPeriods", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listFlaggedExtractionOutcomesMock.mockResolvedValue([]);
    rerunExtractionOutcomeMock.mockResolvedValue(summary());
  });

  it("lists each flagged period with its period, tier and a human-readable reason", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([
      outcome({ id: "o1", fiscalYear: 2025, periodType: "Q3", reasonCode: "validation_failed" }),
      outcome({
        id: "o2",
        fiscalYear: 2024,
        periodType: "annual",
        reasonCode: "no_deterministic_tier",
        tier: null,
      }),
    ]);
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    // Periods render with the same FY/quarter idiom as the coverage table.
    expect(await screen.findByText("2025 Q3")).toBeInTheDocument();
    expect(screen.getByText("2024 FY")).toBeInTheDocument();
    // The typed reason codes are TRANSLATED, never surfaced raw.
    expect(screen.getByText("The figures failed a consistency check")).toBeInTheDocument();
    expect(screen.getByText("No reader could handle this document")).toBeInTheDocument();
    expect(screen.queryByText("validation_failed")).not.toBeInTheDocument();
    expect(screen.queryByText("no_deterministic_tier")).not.toBeInTheDocument();
    // The attempting tier is named; a period no tier could read says so.
    expect(screen.getByText("Structured read (xHTML)")).toBeInTheDocument();
    expect(screen.getByText("No reader")).toBeInTheDocument();
  });

  it("translates every one of the seven typed reason codes", async () => {
    const codes = [
      "validation_failed",
      "structure_drift",
      "witness_disagreement",
      "no_deterministic_tier",
      "no_period_derived",
      "document_unreadable",
      "emitted",
    ];
    listFlaggedExtractionOutcomesMock.mockResolvedValue(
      codes.map((reasonCode, index) =>
        outcome({ id: `o${index}`, fiscalYear: 2000 + index, reasonCode }),
      ),
    );
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    await screen.findByText("The figures failed a consistency check");
    // No raw code leaks into the UI for ANY of the seven.
    for (const code of codes) {
      expect(screen.queryByText(code)).not.toBeInTheDocument();
    }
  });

  // The "never silently wrong" guarantee, UI half: a period-less failure has no
  // slot key and arrives on the SENTINEL period (fiscalYear 0, empty periodType).
  // Rendering it as "0 " would invent a fiscal year that does not exist.
  it("renders the sentinel period of a no_period_derived failure as 'period unknown'", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([
      outcome({
        fiscalYear: 0,
        periodType: "",
        periodEnd: "",
        reasonCode: "no_period_derived",
      }),
    ]);
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Period unknown")).toBeInTheDocument();
    expect(screen.getByText("The reporting period could not be determined")).toBeInTheDocument();
    // Never a fabricated fiscal year.
    expect(screen.queryByText(/^0\b/)).not.toBeInTheDocument();
  });

  it("shows the failing-check detail and a structure-drift marker when present", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([
      outcome({
        reasonCode: "structure_drift",
        structureChanged: true,
        detailJson: JSON.stringify({ check: "assets = liabilities + equity", residual: "12000" }),
      }),
    ]);
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    // Drift is visible on the row itself, not hidden behind the disclosure.
    expect(await screen.findByText("Layout changed")).toBeInTheDocument();
    // The failing check expands in place.
    await userEvent.click(screen.getByRole("button", { name: /2025 Q3/ }));
    expect(await screen.findByText("assets = liabilities + equity")).toBeInTheDocument();
    expect(screen.getByText("12000")).toBeInTheDocument();
  });

  // The REAL validation-gate payload (owner screenshot, DIG 2025 FY) must render
  // as investor language — formatted amounts and a translated identity label —
  // never as raw JSON keys, braces, or empty arrays.
  it("renders the validation-gate detail as plain language, not raw JSON", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([
      outcome({
        reasonCode: "validation_failed",
        detailJson: JSON.stringify({
          failedCrossChecks: [],
          failedIdentities: [
            {
              detail: {
                actual: "174252681910.00",
                expected: "103634551390.00",
                residual: "70618130520.00",
              },
              id: "balance_sheet_identity",
              label: "Assets = Liabilities + Equity",
            },
          ],
          witnessDisagreements: [],
        }),
      }),
    ]);
    render(<CoverageFlaggedPeriods companyId="company_gpw_dig" />);

    await userEvent.click(await screen.findByRole("button", { name: /2025 Q3/ }));

    // Human identity label + compact formatted amounts.
    expect(await screen.findByText("Assets = Liabilities + Equity")).toBeInTheDocument();
    const detailValue = screen.getByText(/174\.3 B PLN/);
    expect(detailValue).toHaveTextContent("103.6 B PLN");
    expect(detailValue).toHaveTextContent("70.6 B PLN");
    // No machine speak: raw keys, braces, or empty-array noise never reach the UI.
    expect(screen.queryByText(/failedIdentities|failedCrossChecks|witnessDisagreements/)).not.toBeInTheDocument();
    expect(screen.queryByText(/[{[\]}]/)).not.toBeInTheDocument();
  });

  // The reversed-witnessing disagreement the aggregator pull writes
  // (jobs/aggregator_fundamentals_pull.rs::record_disagreement) uses the SAME
  // canonical gate shape as the WDF seam, so the aggregator-vs-issuer diff renders
  // as investor language — never the raw flat {aggregatorValue, issuerValue, …}
  // keys that leaked before (code-review FINDING 3). The extra context nested in
  // `detail` (pageUrl / sourceAdapterId / issuerTier) must not surface either.
  it("renders an aggregator witness_disagreement as plain language, not raw JSON", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([
      outcome({
        reasonCode: "witness_disagreement",
        tier: "esef",
        detailJson: JSON.stringify({
          failedIdentities: [],
          failedCrossChecks: [],
          witnessDisagreements: [
            {
              metricKey: "current_assets",
              detail: {
                // Convention: expected = aggregator, actual = the filing value.
                expected: "16815000.00",
                actual: "999000000.00",
                residual: "982185000.00",
                pageUrl: "https://www.biznesradar.pl/raporty-finansowe-bilans/CDR",
                sourceAdapterId: "biznesradar-fundamenty",
                issuerTier: "esef",
              },
            },
          ],
        }),
      }),
    ]);
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    // The reason is translated prose; the disagreement expands into a sentence.
    expect(await screen.findByText("A second source reported different figures")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /2025 Q3/ }));
    const detailValue = await screen.findByText(/999 M PLN/);
    expect(detailValue).toHaveTextContent("16.8 M PLN");
    // No machine speak: neither the gate keys nor the nested context leak.
    expect(
      screen.queryByText(/aggregatorValue|issuerValue|failedIdentities|witnessDisagreements/),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/pageUrl|sourceAdapterId|issuerTier/)).not.toBeInTheDocument();
    expect(screen.queryByText(/biznesradar\.pl/)).not.toBeInTheDocument();
    expect(screen.queryByText(/[{[\]}]/)).not.toBeInTheDocument();
  });

  // HONEST STATES. "Nothing flagged" is a GOOD state and must never be what a
  // failed read looks like (recorded past defect: a failed category silently
  // degrading to empty).
  it("shows a reassuring empty state when nothing is flagged", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([]);
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    expect(
      await screen.findByText("Nothing flagged — every attempted period produced data."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Try again/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });

  it("shows an explicit, retryable error when the read itself fails — never the empty state", async () => {
    listFlaggedExtractionOutcomesMock.mockRejectedValueOnce(new Error("read exploded"));
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Couldn't load flagged periods.")).toBeInTheDocument();
    expect(screen.getByText("read exploded")).toBeInTheDocument();
    // The failure is NOT dressed up as "nothing flagged".
    expect(
      screen.queryByText("Nothing flagged — every attempted period produced data."),
    ).not.toBeInTheDocument();

    // …and it is retryable.
    listFlaggedExtractionOutcomesMock.mockResolvedValue([outcome()]);
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("2025 Q3")).toBeInTheDocument();
    expect(screen.queryByText("Couldn't load flagged periods.")).not.toBeInTheDocument();
  });

  it("shows a loading state while the read is in flight", async () => {
    listFlaggedExtractionOutcomesMock.mockReturnValue(new Promise(() => {}));
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Checking for flagged periods…")).toBeInTheDocument();
    // Neither honest terminal state is claimed while we do not know yet.
    expect(
      screen.queryByText("Nothing flagged — every attempted period produced data."),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Couldn't load flagged periods.")).not.toBeInTheDocument();
  });

  // --- Re-run action ---

  it("re-runs a flagged period by its outcome id and reloads the list", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([outcome({ id: "outcome_42" })]);
    const onRerun = vi.fn();
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" onRerun={onRerun} />);

    await userEvent.click(await screen.findByRole("button", { name: /Try again/ }));

    // The retry targets the STORED outcome row — never a re-derived slot.
    await waitFor(() =>
      expect(rerunExtractionOutcomeMock).toHaveBeenCalledWith({ outcomeId: "outcome_42" }),
    );
    // A fixed period must leave the list, so the read refetches after the run.
    await waitFor(() => expect(listFlaggedExtractionOutcomesMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(onRerun).toHaveBeenCalledTimes(1));
  });

  // Guard against the missing-busy-guard bug class: a second click while the
  // first run is in flight must not dispatch a second command.
  it("disables the re-run action while a run is in flight (no double dispatch)", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([outcome({ id: "outcome_42" })]);
    let resolveRerun: (value: StructuredExtractionSummary) => void = () => {};
    rerunExtractionOutcomeMock.mockReturnValue(
      new Promise<StructuredExtractionSummary>((resolve) => {
        resolveRerun = resolve;
      }),
    );
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    const button = await screen.findByRole("button", { name: /Try again/ });
    await userEvent.click(button);

    const busy = await screen.findByRole("button", { name: /Trying again…/ });
    expect(busy).toBeDisabled();
    await userEvent.click(busy);
    expect(rerunExtractionOutcomeMock).toHaveBeenCalledTimes(1);

    resolveRerun(summary());
    await waitFor(() => expect(listFlaggedExtractionOutcomesMock).toHaveBeenCalledTimes(2));
  });

  it("surfaces a failed re-run explicitly and re-enables the action", async () => {
    listFlaggedExtractionOutcomesMock.mockResolvedValue([outcome({ id: "outcome_42" })]);
    rerunExtractionOutcomeMock.mockRejectedValue(new Error("no outcome with that id"));
    render(<CoverageFlaggedPeriods companyId="company_gpw_cdr" />);

    await userEvent.click(await screen.findByRole("button", { name: /Try again/ }));

    expect(await screen.findByText("no outcome with that id")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Try again/ })).toBeEnabled();
  });

  it("reloads when the companyId changes", async () => {
    render(<CoverageFlaggedPeriods companyId="company_a" />);
    await waitFor(() =>
      expect(listFlaggedExtractionOutcomesMock).toHaveBeenCalledWith("company_a"),
    );
  });
});
