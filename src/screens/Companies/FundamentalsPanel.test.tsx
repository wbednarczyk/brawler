import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FundamentalsPanel, factsRecordedLabel, tierLabel } from "./FundamentalsPanel";
import { ToolHostContext, type ToolHandle } from "../../shared/toolHost";
import { getPriceContext } from "../../api/marketData";
import { getKpiComparison } from "../../api/comparison";
import type { KpiComparison } from "../../api/comparison";
import { listFactProvenance } from "../../api/fundamentalsExtraction";
import type { FinancialFact, FinancialPeriod, KpiDefinition, KpiRelevance } from "../../api/financialsTypes";
import { buildScenario } from "../../test/scenarios/scenarios";

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

// The fact-detail modal's citation-document-title resolution (card #307) runs
// on every render (fetches the company's report documents); stub it so the
// fetch never hits the raw (unconfigured, synchronous) `invoke` mock.
vi.mock("../../api/reportDocuments", () => ({
  getReportDocumentsView: vi.fn(() =>
    Promise.resolve({ companyId: "company_gpw_cdr", rows: [], totals: {} }),
  ),
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
  financialFactForm: {
    definitionId: "",
    valueNumeric: "",
    currency: "",
    periodId: "",
    annotation: "",
    dataQuality: "final",
  },
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
    // No Sector field renders in this panel.
    expect(container.querySelector('[aria-label="Sector"]')).toBeNull();
  });
});

// A rejected `get_price_context` fetch must surface an inline error, never
// silently disappear.
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
    origin: "seed",
    statementGroup: "other",
    periodNature: "duration",
    createdAt: "",
    updatedAt: "",
  };
}

function fact(id: string, definitionId: string, periodId: string, metricKey = definitionId): FinancialFact {
  return {
    id,
    companyId: "company_gpw_cdr",
    periodId,
    definitionId,
    metricKey,
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

// A real company always has an active/primary "core floor" of kpi_relevance
// rows seeded at creation (ADR 0092 layer 1) — the Kluczowe default tab is
// never empty in practice. Mark this fixture's own KPI set as key so tests
// that click a matrix cell find it under the default tab, matching a real
// company rather than the edge case of relevance data not having run yet.
function keyRelevance(definitionId: string): KpiRelevance {
  return {
    id: `rel_${definitionId}`,
    companyId: "company_gpw_cdr",
    definitionId,
    status: "active",
    source: "core",
    rank: "primary",
    firstSeenPeriod: null,
    lastSeenPeriod: null,
    createdAt: "",
    updatedAt: "",
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
  kpiRelevance: ["def_revenue", "def_net_profit", "def_gm"].map(keyRelevance),
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

// ADR 0093 dec. 2 (epic #285 T5): preliminary/final coexist in the same slot,
// and the matrix must show the final-preferred fact with a compact quality
// marker on the cell (dense grid — mirrors the annotation '*' precedent) plus
// the full StatusChip in the fact detail, next to the Source/Validation chips
// T2 already put there.
describe("FundamentalsPanel preliminary-data marker and detail chip (ADR 0093 dec. 2)", () => {
  it("renders a compact quality marker on a preliminary-backed cell, and none on a final one", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const withPreliminary = {
      ...periodsProps,
      financialFacts: [
        { ...fact("f_rev", "def_revenue", "p_2024"), dataQuality: "preliminary" },
        fact("f_np", "def_net_profit", "p_2024"),
      ],
    };
    const { container } = render(<FundamentalsPanel {...withPreliminary} />);

    await waitFor(() => {
      expect(container.querySelector(".fact-quality-marker")).not.toBeNull();
    });
    const markers = container.querySelectorAll(".fact-quality-marker");
    expect(markers).toHaveLength(1);
    expect(markers[0]).toHaveAttribute("title", "Preliminary");
    expect(markers[0]).toHaveAttribute("aria-label", "Data quality: Preliminary");
  });

  it("shows the full 'Preliminary' StatusChip in the fact detail panel for a selected preliminary fact", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const withPreliminary = {
      ...periodsProps,
      financialFacts: [
        { ...fact("f_rev", "def_revenue", "p_2024"), dataQuality: "preliminary" },
        fact("f_np", "def_net_profit", "p_2024"),
      ],
      selectedFinancialFactId: "f_rev",
    };
    render(<FundamentalsPanel {...withPreliminary} />);

    await waitFor(() => {
      expect(screen.getByLabelText("Financial fact detail")).toBeInTheDocument();
    });
    const detail = screen.getByLabelText("Financial fact detail");
    expect(within(detail).getByText("Preliminary")).toBeInTheDocument();
  });

  // Ties the `preliminary-fundamentals` scenario overlay (ADR 0081 mechanism,
  // src/test/scenarios/overlays.ts) to an actual render, so the T5 dogfooding
  // state — final-preferred selection + the preliminary marker — renders in
  // CI forever, not just as a data-shape assertion (docs/testing.md "UI
  // dogfooding finding ⇒ overlay").
  it("renders the preliminary-fundamentals overlay: final wins the superseded period's cell, and only the preliminary-only period gets a marker", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce({
      granularity: "annual",
      metricKeys: [],
      axis: [],
      series: [],
    });
    const data = buildScenario({ base: "empty", overlays: ["preliminary-fundamentals"] });
    const overlayCompanyId = "company_gpw_preliminary";
    const scenarioProps = {
      ...panelProps,
      companyId: overlayCompanyId,
      financialPeriods: data.financialPeriods.filter((p) => p.companyId === overlayCompanyId),
      financialFacts: data.financialFacts.filter((f) => f.companyId === overlayCompanyId),
      kpiDefinitions: data.kpiDefinitions.filter(
        (d) => d.id === "kpidef_overlay_preliminary_net_profit",
      ),
    };

    const user = userEvent.setup();
    render(<FundamentalsPanel {...scenarioProps} />);

    // No kpi_relevance override here — this fixture's KPI lands under the
    // default "Operating" bucket (statementGroup "other"), not Kluczowe.
    await user.click(await screen.findByRole("button", { name: /Operating/ }));

    const supersededCell = await screen.findByRole("button", { name: "Net profit, 2025 FY" });
    const preliminaryOnlyCell = screen.getByRole("button", { name: "Net profit, 2026 FY" });

    // The final fact (1010.0 million) wins the 2025 cell — the superseded
    // preliminary value (980.0 million) never renders, and no marker: once a
    // final fact lands, the cell is no longer "awaiting the audited figure".
    expect(supersededCell.textContent).toContain("1010.0");
    expect(supersededCell.textContent).not.toContain("980.0");
    expect(supersededCell.querySelector(".fact-quality-marker")).toBeNull();

    // The 2026 period has no final sibling yet — its preliminary fact (1150.0
    // million) shows, with the compact marker.
    expect(preliminaryOnlyCell.textContent).toContain("1150.0");
    expect(preliminaryOnlyCell.querySelector(".fact-quality-marker")).not.toBeNull();

    expect(document.querySelectorAll(".fact-quality-marker")).toHaveLength(1);
  });

  it("renders no quality marker and a plain 'Final' value for a final fact's detail", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const allFinal = {
      ...periodsProps,
      selectedFinancialFactId: "f_rev",
    };
    render(<FundamentalsPanel {...allFinal} />);

    await waitFor(() => {
      expect(screen.getByLabelText("Financial fact detail")).toBeInTheDocument();
    });
    const detail = screen.getByLabelText("Financial fact detail");
    expect(within(detail).getByText("Final")).toBeInTheDocument();
    expect(document.querySelector(".fact-quality-marker")).toBeNull();
  });
});

// Card #307: the fact detail moves into a `Modal` popup — clicking a matrix
// cell opens it (value + chips + citation), and the below-table detail
// section is gone entirely (no `.fact-detail-grid` rendered outside a dialog).
describe("FundamentalsPanel fact detail modal (card #307)", () => {
  // A stateful harness: the dumb `FundamentalsPanel` only calls the
  // `selectFinancialFact`/`cancelEditingFinancialFact` props it is given, so a
  // click-driven "does the modal open" test needs a real setter behind them —
  // mirrors what `useFundamentalsController` provides in production.
  function StatefulHarness(
    props: Omit<typeof periodsProps, "selectedFinancialFactId"> & {
      selectedFinancialFactId?: string | null;
    },
  ) {
    const [selectedId, setSelectedId] = useState<string | null>(
      props.selectedFinancialFactId ?? null,
    );
    return (
      <FundamentalsPanel
        {...props}
        selectedFinancialFactId={selectedId}
        selectFinancialFact={setSelectedId}
        cancelEditingFinancialFact={() => setSelectedId(null)}
      />
    );
  }

  it("opens the modal on a matrix cell click, showing the value, chips, and citation", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    vi.mocked(listFactProvenance).mockResolvedValueOnce([
      {
        factId: "f_rev",
        sourceTier: "esef",
        validationStatus: "passed",
        driftJson: null,
        citation: "p.4, row Revenue = 100",
        witnessValue: null,
        witnessPageUrl: null,
        corroboratedAt: null,
      },
    ]);
    const user = userEvent.setup();
    const { container } = render(<StatefulHarness {...periodsProps} />);

    // No dialog, and no detail grid anywhere, before a cell is clicked.
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(container.querySelector(".fact-detail-grid")).toBeNull();

    const cell = await screen.findByRole("button", { name: /revenue, 2024 ANNUAL/i });
    await user.click(cell);

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Final")).toBeInTheDocument();
    expect(within(dialog).getByText("ESEF (tagged)")).toBeInTheDocument();
    expect(within(dialog).getByText("Validated")).toBeInTheDocument();
    expect(within(dialog).getByText(/p\.4, row Revenue = 100/)).toBeInTheDocument();
    // The below-table section is gone: the detail grid only exists inside the
    // dialog now, never as a sibling of the matrix.
    expect(container.querySelector(".fact-detail-grid")).toBeNull();
    expect(within(dialog).getByRole("heading")).toBeInTheDocument();
  });

  it("closes the modal on the Zamknij/Close action, deselecting the fact", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const user = userEvent.setup();
    render(<StatefulHarness {...periodsProps} selectedFinancialFactId="f_rev" />);

    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Close" }));

    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("shows the edit-form fields inside the SAME modal when isFinancialFactEditMode is true", async () => {
    vi.mocked(getKpiComparison).mockResolvedValueOnce(n1Comparison());
    const editingProps = {
      ...periodsProps,
      selectedFinancialFactId: "f_rev",
      isFinancialFactEditMode: true,
      financialFactForm: {
        definitionId: "def_revenue",
        valueNumeric: "1",
        currency: "PLN",
        periodId: "p_2024",
        annotation: "",
        dataQuality: "final",
      },
    };
    render(<FundamentalsPanel {...editingProps} />);

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByLabelText("Numeric value")).toHaveValue(1);
    expect(within(dialog).getByLabelText("Currency")).toHaveValue("PLN");
    expect(within(dialog).getByRole("button", { name: /Save/ })).toBeInTheDocument();
  });
});

// Statement switcher (epic #398, approved mockup docs/mockups/fundamentals-150-rows.html):
// the panel shows ONE statement at a time — "Kluczowe" ("Key figures") by
// default — instead of the old all-groups-expanded collapsible list.
describe("FundamentalsPanel statement switcher (epic #398)", () => {
  // The periods × deltas section (out of scope here) also fires on mount
  // whenever the matrix has rows; stub it to an empty read model.
  beforeEach(() => {
    vi.mocked(getKpiComparison).mockResolvedValue({
      granularity: "annual",
      metricKeys: [],
      axis: [],
      series: [],
    });
  });

  function statementKpiDef(
    id: string,
    metricKey: string,
    overrides: Partial<Pick<KpiDefinition, "scope" | "statementGroup">> = {},
  ): KpiDefinition {
    return {
      id,
      scope: overrides.scope ?? "global",
      companyId: null,
      sector: null,
      metricKey,
      label: metricKey,
      valueKind: "monetary",
      unit: null,
      computation: "reported",
      formula: null,
      displayFormat: null,
      origin: "seed",
      statementGroup: overrides.statementGroup ?? "other",
      periodNature: "duration",
      createdAt: "",
      updatedAt: "",
    };
  }

  const statementDefs = [
    statementKpiDef("def_revenue", "revenue", { statementGroup: "income" }),
    statementKpiDef("def_total_assets", "total_assets", { statementGroup: "balance" }),
    statementKpiDef("def_broker_clients", "broker_clients", { scope: "company" }),
  ];
  const statementProps = {
    ...panelProps,
    financialPeriods: [period("p_2024", 2024)],
    financialFacts: [
      fact("f_revenue", "def_revenue", "p_2024"),
      fact("f_total_assets", "def_total_assets", "p_2024"),
      fact("f_broker_clients", "def_broker_clients", "p_2024"),
    ],
    kpiDefinitions: statementDefs,
    kpiRelevance: [
      {
        id: "rel_revenue",
        companyId: "company_gpw_cdr",
        definitionId: "def_revenue",
        status: "active",
        source: "manual",
        rank: "primary",
        firstSeenPeriod: null,
        lastSeenPeriod: null,
        createdAt: "",
        updatedAt: "",
      },
    ],
  };

  it("defaults to the Kluczowe (Key figures) tab, showing only the active/primary kpi_relevance rows", async () => {
    render(<FundamentalsPanel {...statementProps} />);

    const keyTab = await screen.findByRole("button", { name: /Key figures/ });
    expect(keyTab).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findByRole("button", { name: /revenue, 2024 ANNUAL/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /total_assets, 2024 ANNUAL/i })).toBeNull();
  });

  it("switches statements on tab click, changing the visible rows, with badges matching each tab's row count", async () => {
    const user = userEvent.setup();
    render(<FundamentalsPanel {...statementProps} />);

    const incomeTab = await screen.findByRole("button", { name: /Income statement/ });
    // One row (revenue) is statementGroup "income".
    expect(within(incomeTab).getByText("1")).toBeInTheDocument();
    const balanceTab = screen.getByRole("button", { name: /Balance sheet/ });
    expect(within(balanceTab).getByText("1")).toBeInTheDocument();

    await user.click(incomeTab);
    expect(incomeTab).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findByRole("button", { name: /revenue, 2024 ANNUAL/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /total_assets, 2024 ANNUAL/i })).toBeNull();

    await user.click(balanceTab);
    expect(balanceTab).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findByRole("button", { name: /total_assets, 2024 ANNUAL/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /revenue, 2024 ANNUAL/i })).toBeNull();
  });

  it("find filters rows within the active statement only", async () => {
    const user = userEvent.setup();
    render(<FundamentalsPanel {...statementProps} />);

    const operatingTab = await screen.findByRole("button", { name: /Operating/ });
    await user.click(operatingTab);
    expect(await screen.findByRole("button", { name: /broker_clients, 2024 ANNUAL/i })).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: "Find a position" }), "nomatch");
    expect(screen.queryByRole("button", { name: /broker_clients, 2024 ANNUAL/i })).toBeNull();
    expect(await screen.findByText("No positions match your search.")).toBeInTheDocument();
  });

  it("reports the completeness bar's uncatalogued-position count for a synthesized row", async () => {
    const withSynthetic = {
      ...statementProps,
      // No matching kpi_definitions row for this fact's definitionId — the
      // matrix synthesizes a placeholder row (isSynthetic: true).
      financialFacts: [...statementProps.financialFacts, fact("f_unknown", "def_unknown", "p_2024")],
    };
    const user = userEvent.setup();
    render(<FundamentalsPanel {...withSynthetic} />);

    const operatingTab = await screen.findByRole("button", { name: /Operating/ });
    await user.click(operatingTab);

    expect(await screen.findByText("1 item awaits a catalog name")).toBeInTheDocument();
  });
});

// sol R1 finding 1: the create-period / add-fact drafts must register with
// the Spółka workshop's dirty gate, a no-op when hosted outside it (every
// static-props `render(<FundamentalsPanel .../>)` above exercises that
// no-op — `updateFundamentalsForm`/`updateFinancialFactForm` are `noop`).
describe("FundamentalsPanel dirty draft registration (sol R1 finding 1)", () => {
  function StatefulFormHarness(props: typeof panelProps) {
    const [fundamentalsForm, setFundamentalsForm] = useState(props.fundamentalsForm);
    const [financialFactForm, setFinancialFactForm] = useState(props.financialFactForm);
    return (
      <FundamentalsPanel
        {...props}
        fundamentalsForm={fundamentalsForm}
        financialFactForm={financialFactForm}
        updateFundamentalsForm={(field, value) =>
          setFundamentalsForm((current) => ({ ...current, [field]: value }))
        }
        updateFinancialFactForm={(field, value) =>
          setFinancialFactForm((current) => ({ ...current, [field]: value }))
        }
      />
    );
  }

  it("registers a dirty draft with the tool host", async () => {
    const user = userEvent.setup();
    let handle: ToolHandle | null = null;
    const register = vi.fn((h: ToolHandle) => {
      handle = h;
      return () => {};
    });
    render(
      <ToolHostContext.Provider value={{ register }}>
        <StatefulFormHarness {...panelProps} />
      </ToolHostContext.Provider>,
    );
    expect(register).toHaveBeenCalled();
    expect(handle!.isDirty()).toBe(false);

    await user.type(screen.getByLabelText("Fiscal year"), "2025");
    expect(handle!.isDirty()).toBe(true);

    act(() => {
      handle!.discard();
    });

    await waitFor(() => {
      expect((screen.getByLabelText("Fiscal year") as HTMLInputElement).value).toBe("");
    });
  });
});

// sol R1 finding 9: "Create period" and "Add fact" both rendered `variant="primary"`
// simultaneously once periods exist — at most one primary action per panel.
describe("FundamentalsPanel primary action (sol R1 finding 9)", () => {
  it("renders at most one primary action once periods exist (Add fact, not Create period)", () => {
    const { container } = render(<FundamentalsPanel {...periodsProps} />);
    const primaries = container.querySelectorAll('[data-ui-button-variant="primary"]');
    expect(primaries.length).toBeLessThanOrEqual(1);
    expect(screen.getByRole("button", { name: /Add fact/ })).toHaveAttribute(
      "data-ui-button-variant",
      "primary",
    );
    expect(screen.getByRole("button", { name: /Create/ })).toHaveAttribute(
      "data-ui-button-variant",
      "secondary",
    );
  });

  it("Create period stays the sole primary action when no periods exist yet", () => {
    render(<FundamentalsPanel {...panelProps} />);
    expect(screen.getByRole("button", { name: /Create/ })).toHaveAttribute(
      "data-ui-button-variant",
      "primary",
    );
    expect(screen.queryByRole("button", { name: /Add fact/ })).not.toBeInTheDocument();
  });
});
