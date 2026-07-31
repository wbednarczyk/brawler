import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FundamentalsPanel, factsRecordedLabel, tierLabel } from "./FundamentalsPanel";
import { getPriceContext } from "../../api/marketData";
import { getKpiComparison } from "../../api/comparison";
import type { KpiComparison } from "../../api/comparison";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";

// The Autopilot / custom-KPI child fields load state via the mocked `invoke`
// on mount (out of scope here); stub them so the render exercises only the
// panel's own U7-A density disclosures. (Sector + IR URL fields moved to the
// Basic info panel, owner request 2026-07-14.)
vi.mock("../../shared/components/CompanyAutopilotField", () => ({
  CompanyAutopilotField: () => <div data-testid="autopilot-field" />,
}));
vi.mock("../../shared/components/CustomKpiManager", () => ({
  CustomKpiManager: () => <div data-testid="kpi-manager" />,
}));
// The panel's own price-context fetch (v0.53 T5) also hits the mocked
// `invoke`, which is an unconfigured `vi.fn()` here (resolves to `undefined`,
// not a promise) — stub the api call directly so the fetch never runs.
// `getPriceContext` is re-assigned per test (module-mock hoisting means the
// mock factory below returns a `vi.fn()` whose resolution each test controls).
vi.mock("../../api/marketData", () => ({
  getPriceContext: vi.fn(() => Promise.resolve(null)),
}));

// The panel also reads the newest analyst target for the "vs target" readout
// (v0.58 A3); stub it so the fetch resolves to no target in these tests.
vi.mock("../../api/analystRecommendations", () => ({
  getAnalystRecommendations: vi.fn(() =>
    Promise.resolve({ companyId: "c", entries: [] }),
  ),
}));

// The periods×deltas section (v0.61 §A5) calls the N=1 comparison read model;
// stub it so each test controls the aligned axis + deltas it renders.
vi.mock("../../api/comparison", () => ({
  getKpiComparison: vi.fn(),
}));

// The fact-provenance fetch runs whenever facts are present; stub it to no rows
// so the periods-section tests (which supply facts) don't hit the raw invoke.
vi.mock("../../api/fundamentalsExtraction", () => ({
  listFactProvenance: vi.fn(() => Promise.resolve([])),
}));

const identity = (value: string) => value;

const noop = () => {};
const asyncNoop = async () => {};

// Minimal props to render the panel for the U7-A density toggle assertions. The
// child config fields (Autopilot / IR / custom KPI) hit the globally-mocked
// `invoke`, which resolves to undefined — harmless for the disclosure semantics.
const panelProps = {
  companyId: "company_gpw_cdr",
  financialPeriods: [],
  financialFacts: [],
  kpiDefinitions: [],
  fundamentalsForm: { periodFiscalYear: "", periodType: "annual" },
  financialFactForm: { definitionId: "", valueNumeric: "", currency: "", periodId: "", annotation: "" },
  selectedFinancialFactId: null,
  isFinancialFactEditMode: false,
  fundamentalsError: null,
  fundamentalsLoadError: null,
  createFinancialPeriod: asyncNoop,
  saveFinancialFact: asyncNoop,
  deleteFinancialFact: asyncNoop,
  selectFinancialFact: noop,
  startEditingFinancialFact: noop,
  cancelEditingFinancialFact: noop,
  updateFundamentalsForm: noop,
  updateFinancialFactForm: noop,
};

// Bug e77a1a2 part 3: the header used a `n === 1 ? "fact" : "facts"` two-way
// ternary for both the noun and "recorded", which cannot express Polish's
// three plural categories — a real run's "40 fakty zapisanych" was wrong
// declension (should be "40 faktów zapisanych").
describe("factsRecordedLabel", () => {
  it("uses English one/other for both the noun and the adjective", () => {
    expect(factsRecordedLabel(1, "en")).toBe("1 fact recorded");
    expect(factsRecordedLabel(0, "en")).toBe("0 facts recorded");
    expect(factsRecordedLabel(40, "en")).toBe("40 facts recorded");
  });

  it("declines the Polish noun and adjective together (three plural categories)", () => {
    expect(factsRecordedLabel(1, "pl")).toBe("1 fakt zapisany");
    expect(factsRecordedLabel(3, "pl")).toBe("3 fakty zapisane");
    expect(factsRecordedLabel(40, "pl")).toBe("40 faktów zapisanych");
  });
});

describe("tierLabel", () => {
  it("maps the AI-confirmed-fact tier (Radicle 4fde931) to the AI label", () => {
    expect(tierLabel("ai", identity)).toBe("AI");
  });

  it("maps the legacy ai_text tier to the same AI label", () => {
    expect(tierLabel("ai_text", identity)).toBe("AI");
  });

  it("maps the tagged ESEF/iXBRL tier to its label", () => {
    expect(tierLabel("esef", identity)).toBe("ESEF (tagged)");
  });

  it("maps the ADR 0093 agent (MCP) tier to an honest label", () => {
    expect(tierLabel("agent", identity)).toBe("Agent (MCP)");
  });

  it("falls back to the raw tier string for an unknown tier", () => {
    expect(tierLabel("mystery_tier", identity)).toBe("mystery_tier");
  });
});

// U7-A density-contract disclosures (ADR 0076 D6). The container-query tier
// SWITCH is browser-only (density-companies.spec.ts); here we assert the toggle
// STATE the folds introduce — jsdom has no container queries, so only the
// aria-expanded / open-modifier semantics are exercised.
describe("FundamentalsPanel density disclosures", () => {
  it("Autopilot section folds to one row + expand (S tier)", async () => {
    const user = userEvent.setup();
    const { container } = render(<FundamentalsPanel {...panelProps} />);

    const toggle = container.querySelector(".fundamentals-autopilot-toggle");
    const section = container.querySelector(".fundamentals-autopilot");
    expect(toggle, "autopilot summary toggle rendered").not.toBeNull();
    expect(section, "autopilot section wrapper rendered").not.toBeNull();
    // Collapsed by default: the summary row is the only affordance at S.
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(section?.className).not.toContain("is-expanded");

    await user.click(toggle as HTMLElement);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(section?.className).toContain("is-expanded");
  });

  it("reporting forms fold behind a disclosure (short tier)", async () => {
    const user = userEvent.setup();
    const { container } = render(<FundamentalsPanel {...panelProps} />);

    const toggle = container.querySelector(".fundamentals-forms-toggle");
    const forms = container.querySelector(".fundamentals-forms");
    expect(toggle, "forms disclosure toggle rendered").not.toBeNull();
    expect(forms, "forms wrapper rendered").not.toBeNull();
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(forms?.className).not.toContain("is-expanded");

    await user.click(toggle as HTMLElement);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(forms?.className).toContain("is-expanded");
  });
});

// Section order (owner request 2026-07-14): price context leads the panel,
// the financial-facts matrix follows, everything else (periods, autopilot,
// custom KPIs, forms) comes after. Sector/IR fields are gone (Basic info panel).
describe("FundamentalsPanel section order", () => {
  it("renders price context before financial facts, and facts before the rest", async () => {
    vi.mocked(getPriceContext).mockResolvedValueOnce({
      lastClose: 100,
      lastDate: "2026-07-14",
      changeAbs: 1,
      changePct: 1,
      currency: "PLN",
      week52High: 120,
      week52Low: 80,
      week52HighDistPct: -16.7,
      week52LowDistPct: 25,
      marketCap: null,
      ratios: { pe: null, pbv: null, evEbitda: null, divYield: null, fcfYield: null, ownHistPercentile: null },
      history: [],
      fetchedAt: "2026-07-14T18:00:00Z",
    });
    const { container } = render(<FundamentalsPanel {...panelProps} />);

    await waitFor(() => {
      expect(container.querySelector(".price-context-section")).not.toBeNull();
    });
    const price = container.querySelector(".price-context-section")!;
    const facts = container.querySelector('[aria-label="Financial facts"]')!;
    const periods = container.querySelector('[aria-label="Reporting periods"]')!;
    expect(facts).not.toBeNull();
    expect(periods).not.toBeNull();
    // compareDocumentPosition: FOLLOWING = the argument comes after the receiver.
    expect(price.compareDocumentPosition(facts) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(facts.compareDocumentPosition(periods) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // The sector/IR fields no longer live in this panel.
    expect(container.querySelector('[aria-label="Sector"]')).toBeNull();
  });
});

// Price context load failure (minor fix, code review): a rejected
// `get_price_context` fetch used to swallow the error into `null`, so the
// section simply vanished with no signal. It must now surface an inline
// error instead of silently disappearing.
describe("FundamentalsPanel price context error state", () => {
  it("renders an inline error when the price context fetch rejects", async () => {
    vi.mocked(getPriceContext).mockRejectedValueOnce(new Error("network unreachable"));

    render(<FundamentalsPanel {...panelProps} />);

    await waitFor(() => {
      expect(screen.getByText(/Failed to load price context/)).toBeInTheDocument();
    });
    expect(screen.getByText(/network unreachable/)).toBeInTheDocument();
  });
});

// Periods × deltas section (v0.61 §A5, ADR 0089 dec. 1 N=1). The panel derives
// its metric keys from its own KPI set (the fact matrix) and drives the N=1
// comparison read model, then renders one row per KPI with QoQ/YoY inline. The
// deltas + typed flags come straight from the read model — the section renders
// them, it never recomputes.
function kpiDef(id: string, metricKey: string, valueKind: string): KpiDefinition {
  return {
    id,
    scope: "global",
    companyId: null,
    sector: null,
    metricKey,
    label: metricKey,
    valueKind,
    unit: null,
    computation: "reported",
    formula: null,
    displayFormat: null,
    createdAt: "",
    updatedAt: "",
  };
}

function fact(id: string, definitionId: string, periodId: string): FinancialFact {
  return {
    id,
    companyId: "company_gpw_cdr",
    periodId,
    definitionId,
    valueNumeric: "1",
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
    createdAt: "",
    updatedAt: "",
  };
}

const period = (id: string, fiscalYear: number): FinancialPeriod => ({
  id,
  companyId: "company_gpw_cdr",
  fiscalYear,
  periodType: "annual",
  periodEndDate: `${fiscalYear}-12-31`,
  reportEvidenceRef: null,
  createdAt: "",
  updatedAt: "",
});

// A populated N=1 comparison: revenue (monetary, YoY %), net profit (monetary,
// YoY undefined = non-positive/sign-flip base), gross margin (percentage → p.p.,
// with a no_fact gap in the prior period).
function n1Comparison(): KpiComparison {
  return {
    granularity: "annual",
    metricKeys: ["revenue", "net_profit", "gross_margin"],
    axis: [
      { fiscalYear: 2023, periodType: "FY", key: "2023:FY" },
      { fiscalYear: 2024, periodType: "FY", key: "2024:FY" },
    ],
    series: [
      {
        companyId: "company_gpw_cdr",
        metricKey: "revenue",
        valueKind: "monetary",
        cells: [
          { fiscalYear: 2023, periodType: "FY", factId: "fact_rev_2023", value: "100", currency: "PLN", valuePln: "100", fxBasis: "native_pln", validationStatus: "passed", deltaQoQ: null, deltaYoY: null, flags: [] },
          { fiscalYear: 2024, periodType: "FY", factId: "fact_rev_2024", value: "120", currency: "PLN", valuePln: "120", fxBasis: "native_pln", validationStatus: "passed", deltaQoQ: null, deltaYoY: "20", flags: [] },
        ],
      },
      {
        companyId: "company_gpw_cdr",
        metricKey: "net_profit",
        valueKind: "monetary",
        cells: [
          { fiscalYear: 2023, periodType: "FY", factId: "fact_np_2023", value: "-5", currency: "PLN", valuePln: "-5", fxBasis: "native_pln", validationStatus: "passed", deltaQoQ: null, deltaYoY: null, flags: [] },
          { fiscalYear: 2024, periodType: "FY", factId: "fact_np_2024", value: "8", currency: "PLN", valuePln: "8", fxBasis: "native_pln", validationStatus: "passed", deltaQoQ: null, deltaYoY: null, flags: ["delta_yoy_undefined"] },
        ],
      },
      {
        companyId: "company_gpw_cdr",
        metricKey: "gross_margin",
        valueKind: "percentage",
        cells: [
          { fiscalYear: 2023, periodType: "FY", factId: null, value: null, currency: null, valuePln: null, fxBasis: null, validationStatus: null, deltaQoQ: null, deltaYoY: null, flags: ["no_fact"] },
          { fiscalYear: 2024, periodType: "FY", factId: "fact_gm_2024", value: "38.2", currency: null, valuePln: null, fxBasis: null, validationStatus: "passed", deltaQoQ: null, deltaYoY: "2.8", flags: [] },
        ],
      },
    ],
  };
}

const periodsProps = {
  ...panelProps,
  financialPeriods: [period("p_2023", 2023), period("p_2024", 2024)],
  financialFacts: [
    fact("f_rev", "def_revenue", "p_2024"),
    fact("f_np", "def_net_profit", "p_2024"),
    fact("f_gm", "def_gm", "p_2024"),
  ],
  kpiDefinitions: [
    kpiDef("def_revenue", "revenue", "monetary"),
    kpiDef("def_net_profit", "net_profit", "monetary"),
    kpiDef("def_gm", "gross_margin", "percentage"),
  ],
};

describe("FundamentalsPanel periods × deltas section", () => {
  it("renders the aligned period columns and inline YoY deltas from the read model", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const { container } = render(<FundamentalsPanel {...periodsProps} />);

    const section = await waitFor(() => {
      const el = container.querySelector('[aria-label="Positions × periods"]');
      expect(el).not.toBeNull();
      return el as HTMLElement;
    });
    // N=1 call: exactly this company, its KPI set, default annual granularity.
    expect(vi.mocked(getKpiComparison)).toHaveBeenCalledWith({
      companyIds: ["company_gpw_cdr"],
      metricKeys: ["revenue", "net_profit", "gross_margin"],
      granularity: "annual",
    });
    await waitFor(() => {
      expect(section.querySelector("table")).not.toBeNull();
    });
    // Period columns present; a monetary YoY renders as a percent.
    expect(within(section).getByText("2023")).toBeInTheDocument();
    expect(within(section).getByText("2024")).toBeInTheDocument();
    expect(within(section).getByText("+20%")).toBeInTheDocument();
  });

  it("renders a delta em-dash with the undefined-change reason as a tooltip", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const { container } = render(<FundamentalsPanel {...periodsProps} />);

    await waitFor(() => {
      expect(container.querySelector('[aria-label="Positions × periods"] table')).not.toBeNull();
    });
    const reason =
      "Undefined change (non-positive or sign-flipped base) — an honest gap, not a fabricated number.";
    expect(container.querySelector(`[title="${reason}"]`)).not.toBeNull();
  });

  it("renders the typed no_fact flag for a missing period cell", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const { container } = render(<FundamentalsPanel {...periodsProps} />);

    const section = await waitFor(() => {
      const el = container.querySelector('[aria-label="Positions × periods"] table');
      expect(el).not.toBeNull();
      return el as HTMLElement;
    });
    expect(within(section).getByText("no data for period")).toBeInTheDocument();
  });

  it("uses p.p. for a percentage KPI delta and % for a monetary KPI delta", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const { container } = render(<FundamentalsPanel {...periodsProps} />);

    const section = await waitFor(() => {
      const el = container.querySelector('[aria-label="Positions × periods"] table');
      expect(el).not.toBeNull();
      return el as HTMLElement;
    });
    // percentage KPI → plain-difference p.p.; monetary KPI → percent change.
    expect(within(section).getByText("+2.8 p.p.")).toBeInTheDocument();
    expect(within(section).getByText("+20%")).toBeInTheDocument();
  });
});

describe("FundamentalsPanel fact annotation marker (#156)", () => {
  it("renders a '*' with the note as tooltip on an annotated fact, and no marker otherwise", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const annotated = {
      ...periodsProps,
      financialFacts: [
        { ...fact("f_rev", "def_revenue", "p_2024"), annotation: "includes a one-off gain" },
        fact("f_np", "def_net_profit", "p_2024"),
      ],
    };
    const { container } = render(<FundamentalsPanel {...annotated} />);

    await waitFor(() => {
      expect(container.querySelector(".fact-annotation-marker")).not.toBeNull();
    });
    const markers = container.querySelectorAll(".fact-annotation-marker");
    expect(markers).toHaveLength(1);
    expect(markers[0]).toHaveAttribute("title", "includes a one-off gain");
    expect(markers[0]).toHaveAttribute(
      "aria-label",
      "Annotation: includes a one-off gain",
    );
  });
});
