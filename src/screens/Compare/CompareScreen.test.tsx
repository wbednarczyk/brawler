import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { Company } from "../../api/types";
import type { KpiComparison, KpiComparisonCell, KpiComparisonSeries } from "../../api/comparison";
import { CompareScreen } from "./CompareScreen";
import {
  LocaleContext,
  makeTextTranslator,
  makeTranslator,
  type LocaleCode,
} from "../../shared/locale";

// The screen self-fetches its substrate; mock every command it calls so the
// workflow (select → reactive compute → evidence) is driven deterministically.
vi.mock("../../api/companies", () => ({ listCompanies: vi.fn() }));
vi.mock("../../api/watchlists", () => ({
  listWatchlists: vi.fn(),
  listWatchlistMemberships: vi.fn(),
}));
vi.mock("../../api/financials", () => ({ listKpiDefinitions: vi.fn() }));
vi.mock("../../api/companySector", () => ({ getCompanySector: vi.fn() }));
vi.mock("../../api/comparison", () => ({ getKpiComparison: vi.fn() }));
vi.mock("../../api/sectorPercentiles", () => ({ getSectorPercentiles: vi.fn() }));
vi.mock("../../api/valuation", () => ({ computeComparativeValuation: vi.fn() }));

import { listCompanies } from "../../api/companies";
import { listWatchlists, listWatchlistMemberships } from "../../api/watchlists";
import { listKpiDefinitions } from "../../api/financials";
import { getCompanySector } from "../../api/companySector";
import { getKpiComparison } from "../../api/comparison";
import { getSectorPercentiles } from "../../api/sectorPercentiles";
import { computeComparativeValuation } from "../../api/valuation";
import type { SectorPercentiles } from "../../api/sectorPercentiles";
import type { ComparativeValuation } from "../../api/valuation";

const COMPANIES: Company[] = [
  { id: "company_gpw_pkn", exchange: "GPW", ticker: "PKN", qualifiedTicker: "GPW:PKN", displayName: "PKN Orlen", isin: null, cik: null, lei: null },
  { id: "company_gpw_tpe", exchange: "GPW", ticker: "TPE", qualifiedTicker: "GPW:TPE", displayName: "Tauron", isin: null, cik: null, lei: null },
  { id: "company_gpw_cdr", exchange: "GPW", ticker: "CDR", qualifiedTicker: "GPW:CDR", displayName: "CD Projekt", isin: null, cik: null, lei: null },
];

// Canonical KPI catalog spanning every Różnica case:
// - revenue / net_profit / total_assets → monetary (multiple, or "—" when a
//   company is absent),
// - gross_margin → percentage (p.p. delta, currency-less native value),
// - eps → per-share monetary (always "—", share counts differ),
// - cash → both companies absent (row omitted entirely).
const KPI_DEFS = [
  def("def_revenue", "revenue", "Revenue", "monetary", null),
  def("def_net_profit", "net_profit", "Net profit", "monetary", null),
  def("def_gross_margin", "gross_margin", "Gross margin", "percentage", null),
  def("def_eps", "eps", "EPS", "monetary", "per_share"),
  def("def_total_assets", "total_assets", "Total assets", "monetary", null),
  def("def_cash", "cash", "Cash", "monetary", null),
];

function def(id: string, metricKey: string, label: string, valueKind: string, unit: string | null) {
  return {
    id,
    scope: "canonical",
    companyId: null,
    sector: null,
    metricKey,
    label,
    valueKind,
    unit,
    computation: valueKind === "percentage" ? "derived" : "reported",
    displayFormat: valueKind === "monetary" ? "millions" : null,
    formula: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

const AXIS = [
  { fiscalYear: 2022, periodType: "FY", key: "2022:FY" },
  { fiscalYear: 2023, periodType: "FY", key: "2023:FY" },
  { fiscalYear: 2024, periodType: "FY", key: "2024:FY" },
];

// value trace per (metric, company) over 2022/2023/2024; `null` ⇒ no_fact gap.
const TRACE: Record<string, { A: (number | null)[]; B: (number | null)[]; kind: string }> = {
  revenue: { A: [900, 950, 985], B: [100, 105, 110], kind: "monetary" },
  net_profit: { A: [400, 450, 481], B: [30, 34, 38], kind: "monetary" },
  gross_margin: { A: [39.0, 40.0, 41.0], B: [34.0, 34.5, 35.2], kind: "percentage" },
  eps: { A: [4.0, 4.5, 4.78], B: [15.0, 15.5, 16.1], kind: "monetary" },
  total_assets: { A: [2000, 2200, 2310], B: [null, null, null], kind: "monetary" },
  cash: { A: [null, null, null], B: [null, null, null], kind: "monetary" },
};

function seriesFor(companyId: string, metricKey: string, side: "A" | "B"): KpiComparisonSeries {
  const spec = TRACE[metricKey];
  const points = spec[side];
  const points_kind = spec.kind === "percentage";
  const cells: KpiComparisonCell[] = AXIS.map((period, index) => {
    const raw = points[index];
    const base: KpiComparisonCell = {
      fiscalYear: period.fiscalYear,
      periodType: period.periodType,
      factId: null,
      value: null,
      currency: null,
      valuePln: null,
      fxBasis: null,
      validationStatus: null,
      deltaQoQ: null,
      deltaYoY: null,
      flags: [],
    };
    if (raw === null) return { ...base, flags: ["no_fact"] };
    if (points_kind) {
      // Ratio/percentage: currency-less; the read model does NOT flag
      // currency_unknown here (card b875e69) — the native value IS the
      // comparable number (no conversion needed), so the cell is flag-free.
      return { ...base, factId: `f_${metricKey}_${side}_${index}`, value: String(raw), flags: [], validationStatus: "confirmed" };
    }
    return {
      ...base,
      factId: `f_${metricKey}_${side}_${index}`,
      value: String(raw),
      currency: "PLN",
      valuePln: String(raw),
      fxBasis: "native_pln",
      validationStatus: "confirmed",
    };
  });
  return { companyId, metricKey, valueKind: spec.kind, cells };
}

function buildComparison(companyIds: string[], metricKeys: string[], granularity: string): KpiComparison {
  const series: KpiComparisonSeries[] = [];
  for (const companyId of companyIds) {
    const side = companyId === "company_gpw_pkn" ? "A" : "B";
    for (const metricKey of metricKeys) {
      series.push(seriesFor(companyId, metricKey, side));
    }
  }
  return { granularity, metricKeys, axis: AXIS, series };
}

beforeEach(() => {
  vi.mocked(listCompanies).mockResolvedValue(COMPANIES);
  vi.mocked(listWatchlists).mockResolvedValue([]);
  vi.mocked(listWatchlistMemberships).mockResolvedValue([]);
  vi.mocked(listKpiDefinitions).mockResolvedValue(KPI_DEFS as never);
  vi.mocked(getCompanySector).mockResolvedValue("Energy" as never);
  vi.mocked(getKpiComparison).mockImplementation(async (input) =>
    buildComparison(input.companyIds, input.metricKeys, input.granularity),
  );
  // Valuation seams (§B3): populated for every company by default; individual
  // tests override with a thin / no-sector payload.
  vi.mocked(getSectorPercentiles).mockImplementation(async (companyId) =>
    populatedPercentiles(companyId),
  );
  vi.mocked(computeComparativeValuation).mockImplementation(async (companyId) =>
    populatedValuation(companyId),
  );
});

// --- Valuation payload builders (§B3, matching the B1/B2 DTO shapes) ---------
function populatedPercentiles(companyId: string): SectorPercentiles {
  return {
    companyId,
    sector: "Energy",
    peerCount: 5,
    thin: false,
    emptyReason: null,
    metrics: [
      { metricKey: "pe_ratio", kind: "market_ratio", value: "12.50", percentile: "37.00", median: "14.10", sampleSize: 5, absentReason: null },
      { metricKey: "ev_ebitda", kind: "market_ratio", value: "7.80", percentile: "52.00", median: "7.40", sampleSize: 5, absentReason: null },
      { metricKey: "pbv_ratio", kind: "market_ratio", value: "1.90", percentile: "61.00", median: "1.60", sampleSize: 5, absentReason: null },
      { metricKey: "fcf_yield", kind: "market_ratio", value: null, percentile: null, median: null, sampleSize: 0, absentReason: "no_company_value" },
    ],
  };
}

function populatedValuation(companyId: string): ComparativeValuation {
  return {
    companyId,
    sector: "Energy",
    peerCount: 5,
    thin: false,
    currentPrice: "132",
    dataAsOf: "2026-06-30",
    emptyReason: null,
    methods: [
      { method: "pe_multiple", driverKey: "net_profit_ttm", driverValue: "481", peerMultipleLow: "10.0", peerMultipleBase: "12.0", peerMultipleHigh: "15.0", fairLow: "96", fairBase: "122", fairHigh: "148", peerSampleSize: 4, absentReason: null },
      { method: "ev_ebitda_multiple", driverKey: "ebitda_ttm", driverValue: "600", peerMultipleLow: "6.0", peerMultipleBase: "7.4", peerMultipleHigh: "9.0", fairLow: "108", fairBase: "131", fairHigh: "154", peerSampleSize: 4, absentReason: null },
      { method: "pbv_multiple", driverKey: "total_equity", driverValue: null, peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 0, absentReason: "insufficient_peers" },
    ],
    convergence: { baseLow: "122", baseHigh: "131", spreadPct: "7.11", methodCount: 2 },
    confidence: { grade: "B", composite: "0.72", dataCompleteness: "0.80", peerDepth: "0.60", methodConvergence: "0.70", validation: "0.75" },
  };
}

function thinPercentiles(companyId: string): SectorPercentiles {
  return {
    companyId,
    sector: "Gaming",
    peerCount: 2,
    thin: true,
    emptyReason: null,
    metrics: [
      { metricKey: "pe_ratio", kind: "market_ratio", value: "20.0", percentile: null, median: null, sampleSize: 2, absentReason: "insufficient_peers" },
    ],
  };
}

function thinValuation(companyId: string): ComparativeValuation {
  return {
    companyId,
    sector: "Gaming",
    peerCount: 2,
    thin: true,
    currentPrice: "132",
    dataAsOf: "2026-06-30",
    emptyReason: null,
    methods: [
      { method: "pe_multiple", driverKey: "net_profit_ttm", driverValue: "481", peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 1, absentReason: "insufficient_peers" },
      { method: "ev_ebitda_multiple", driverKey: "ebitda_ttm", driverValue: "600", peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 1, absentReason: "insufficient_peers" },
      { method: "pbv_multiple", driverKey: "total_equity", driverValue: "3000", peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 1, absentReason: "insufficient_peers" },
    ],
    convergence: null,
    confidence: { grade: "D", composite: "0.20", dataCompleteness: "0.50", peerDepth: "0.10", methodConvergence: "0.00", validation: "0.20" },
  };
}

function renderCompare(locale: LocaleCode = "en", openWorkspace?: (id: string) => void) {
  return render(
    <LocaleContext.Provider
      value={{ locale, t: makeTranslator(locale), text: makeTextTranslator(locale) }}
    >
      <CompareScreen openCompanyWorkspace={openWorkspace} />
    </LocaleContext.Provider>,
  );
}

async function selectPair(user: ReturnType<typeof userEvent.setup>) {
  await user.selectOptions(await screen.findByLabelText(/Add company|Dodaj spółkę/), "company_gpw_pkn");
  await user.selectOptions(await screen.findByLabelText(/Add company|Dodaj spółkę/), "company_gpw_tpe");
}

describe("Compare Profil view (ADR 0089 §A7)", () => {
  it("is the default view and computes reactively — no Compare button in the DOM", async () => {
    const user = userEvent.setup();
    renderCompare();

    // No submit button — neither before nor after selection (reactive compute).
    expect(screen.queryByRole("button", { name: /^Compare$/ })).not.toBeInTheDocument();
    await selectPair(user);

    // The Profil table appears without any click, keyed by the KPI column head.
    const table = await screen.findByRole("table");
    expect(within(table).getByRole("columnheader", { name: "KPI" })).toBeInTheDocument();
    // Company columns.
    expect(within(table).getByRole("columnheader", { name: /GPW:PKN/ })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: /GPW:TPE/ })).toBeInTheDocument();
    // Still no submit control anywhere on the screen.
    expect(screen.queryByRole("button", { name: /^Compare$/ })).not.toBeInTheDocument();
  });

  it("renders all-KPI rows with the Różnica column for exactly 2 companies", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);

    const table = await screen.findByRole("table");
    expect(within(table).getByRole("columnheader", { name: "Difference" })).toBeInTheDocument();

    // Row present for each KPI that has data for ≥1 company.
    expect(within(table).getByText("Revenue")).toBeInTheDocument();
    expect(within(table).getByText("Net profit")).toBeInTheDocument();
    expect(within(table).getByText("Gross margin")).toBeInTheDocument();
    expect(within(table).getByText("EPS")).toBeInTheDocument();
    expect(within(table).getByText("Total assets")).toBeInTheDocument();
    // Cash has no data for either company → row omitted.
    expect(within(table).queryByText("Cash")).not.toBeInTheDocument();

    // Monetary → multiple (985 / 110 = 9.0×; 481 / 38 = 12.7×).
    expect(within(table).getByText("9.0×")).toBeInTheDocument();
    expect(within(table).getByText("12.7×")).toBeInTheDocument();
    // Percentage → p.p. delta (41.0 − 35.2 = +5.8 p.p.).
    expect(within(table).getByText("+5.8 p.p.")).toBeInTheDocument();
    // Per-share EPS and the total-assets row (one company absent) → "—".
    expect(within(table).getAllByText("—").length).toBeGreaterThanOrEqual(2);
    // The absent company's total-assets cell shows a typed gap chip.
    expect(within(table).getByText("no data for period")).toBeInTheDocument();
  });

  it("renders currency-less ratio/percentage cells as values with no gap chip (card b875e69)", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);

    const table = await screen.findByRole("table");
    // Gross margin is a currency-less percentage; the read model emits no
    // currency_unknown flag, so its native values render — never a gap chip.
    const marginRow = within(table).getByText("Gross margin").closest("tr")!;
    expect(within(marginRow).getByText("41.0%")).toBeInTheDocument(); // PKN 2024
    expect(within(marginRow).getByText("35.2%")).toBeInTheDocument(); // TPE 2024
    // No "currency unknown" / gap chip anywhere in the ratio row.
    expect(within(marginRow).queryByText(/currency|unknown|no data/i)).not.toBeInTheDocument();
  });

  it("switches the column values when the period selector changes", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);
    await screen.findByRole("table");

    // Default = latest complete year (2024): PKN revenue = 985.
    expect(screen.getByText("985")).toBeInTheDocument();

    // Switch to 2023 → the revenue column now reads 950, not 985.
    await user.selectOptions(screen.getByLabelText(/Period/), "2023:FY");
    expect(await screen.findByText("950")).toBeInTheDocument();
    expect(screen.queryByText("985")).not.toBeInTheDocument();
  });

  it("localizes KPI row names via localizedKpiLabel in Polish", async () => {
    const user = userEvent.setup();
    renderCompare("pl");
    await selectPair(user);

    const table = await screen.findByRole("table");
    // Polish canonical labels, never the raw English def.label.
    expect(within(table).getByText("Przychody")).toBeInTheDocument();
    expect(within(table).getByText("Zysk netto")).toBeInTheDocument();
    expect(within(table).getByText("Marża brutto")).toBeInTheDocument();
    expect(within(table).queryByText("Revenue")).not.toBeInTheDocument();
    expect(within(table).queryByText("Net profit")).not.toBeInTheDocument();
  });

  it("navigates to the fact's workspace via the ⧉ evidence link", async () => {
    const user = userEvent.setup();
    const openWorkspace = vi.fn();
    renderCompare("en", openWorkspace);
    await selectPair(user);

    const table = await screen.findByRole("table");
    const evidence = within(table).getAllByRole("button", { name: /Open evidence for GPW:PKN/ });
    await user.click(evidence[0]);
    expect(openWorkspace).toHaveBeenCalledWith("company_gpw_pkn");
  });
});

describe("Compare view toggle (ADR 0089 §A7)", () => {
  it("localizes KPI picker options in Polish under the Trend view", async () => {
    const user = userEvent.setup();
    renderCompare("pl");

    // Move to Trend, where the KPI picker + granularity live.
    await user.click(await screen.findByRole("button", { name: "Trend" }));
    const picker = await screen.findByLabelText(/Metryka/);
    expect(within(picker).getByRole("option", { name: "Przychody" })).toBeInTheDocument();
    expect(within(picker).queryByRole("option", { name: "Revenue" })).not.toBeInTheDocument();
  });

  it("restores the one-KPI × periods table + trend chart under Trend", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "Trend" }));

    // The Trend table has aligned period columns for the single selected metric.
    const table = await screen.findByRole("table");
    for (const year of ["2022", "2023", "2024"]) {
      expect(within(table).getByRole("columnheader", { name: year })).toBeInTheDocument();
    }
    // The multi-series trend chart is back.
    expect(await screen.findByRole("img", { name: /Trend/ })).toBeInTheDocument();
    // Profil's KPI column head is gone in Trend.
    expect(within(table).queryByRole("columnheader", { name: "KPI" })).not.toBeInTheDocument();
  });
});

// §A4 — multi-series trend overlay under the Trend view (reactive, no button).
const MANY_COMPANIES: Company[] = Array.from({ length: 6 }, (_, i) => {
  const ticker = `C${i}`;
  return {
    id: `company_gpw_c${i}`,
    exchange: "GPW",
    ticker,
    qualifiedTicker: `GPW:${ticker}`,
    displayName: `Company ${i}`,
    isin: null,
    cik: null,
    lei: null,
  };
});

function trendComparison(ids: string[]): KpiComparison {
  return {
    granularity: "annual",
    metricKeys: ["revenue"],
    axis: AXIS,
    series: ids.map((id, index) => ({
      companyId: id,
      metricKey: "revenue",
      valueKind: "monetary",
      cells: AXIS.map((period, cellIndex) => ({
        fiscalYear: period.fiscalYear,
        periodType: period.periodType,
        factId: `fact_${id}_${period.fiscalYear}`,
        value: String(100 + index * 10 + cellIndex),
        currency: "PLN",
        valuePln: String(100 + index * 10 + cellIndex),
        fxBasis: "native_pln",
        validationStatus: "confirmed",
        deltaQoQ: null,
        deltaYoY: null,
        flags: [],
      })),
    })),
  };
}

async function selectIds(user: ReturnType<typeof userEvent.setup>, ids: string[]) {
  for (const id of ids) {
    await user.selectOptions(await screen.findByLabelText(/Add company|Dodaj spółkę/), id);
  }
}

describe("Compare valuation section (ADR 0089 §B3)", () => {
  it("renders percentile chips with an honest N per metric", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);

    const section = await screen.findByRole("region", { name: "Valuation" });
    // Chip form: "<metric>: p<pct> among peers (N=<sampleSize>)" — never a
    // percentile without its N. The reads are debounced, so await the first chip.
    expect(await within(section).findByText(/P\/E: p37 among peers \(N=5\)/)).toBeInTheDocument();
    expect(within(section).getByText(/EV\/EBITDA: p52 among peers \(N=5\)/)).toBeInTheDocument();
    expect(within(section).getByText(/P\/BV: p61 among peers \(N=5\)/)).toBeInTheDocument();
  });

  it("draws a football-field bar per method and names an absent method's reason", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);

    const section = await screen.findByRole("region", { name: "Valuation" });
    const field = await within(section).findByRole("group", { name: /football field|Valuation/i });
    expect(within(field).getByText("P/E × median")).toBeInTheDocument();
    expect(within(field).getByText("EV/EBITDA × median")).toBeInTheDocument();
    expect(within(field).getByText("P/BV × median")).toBeInTheDocument();
    // Two drawable methods → two implied-range fills; P/BV is a typed absence.
    expect(field.querySelectorAll(".ui-range-bar-fill")).toHaveLength(2);
    expect(within(field).getByText("too few peers")).toBeInTheDocument();
    // The range text renders for the drawable methods.
    expect(within(field).getByText(/96.*148/)).toBeInTheDocument();
  });

  it("shows the confidence grade badge with an inspectable four-component breakdown", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);

    const section = await screen.findByRole("region", { name: "Valuation" });
    expect(await within(section).findByText(/Confidence: B/)).toBeInTheDocument();

    // The four components are hidden until the breakdown is opened.
    const toggle = within(section).getByRole("button", { name: /grade breakdown/i });
    await user.click(toggle);
    expect(within(section).getByText("Data completeness")).toBeInTheDocument();
    expect(within(section).getByText("Peer depth")).toBeInTheDocument();
    expect(within(section).getByText("Method convergence")).toBeInTheDocument();
    expect(within(section).getByText("Validation status")).toBeInTheDocument();
  });

  it("renders the thin-state threshold message and no advisory wording", async () => {
    vi.mocked(getSectorPercentiles).mockImplementation(async (companyId) => thinPercentiles(companyId));
    vi.mocked(computeComparativeValuation).mockImplementation(async (companyId) => thinValuation(companyId));
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);

    const section = await screen.findByRole("region", { name: "Valuation" });
    expect(
      await within(section).findByText(/N=2 — too few peers for percentiles and median multiples \(threshold: 4\)/),
    ).toBeInTheDocument();
    // Honest framing footnote, never buy/sell/cheap/expensive language.
    expect(within(section).queryByText(/\b(cheap|expensive|buy|sell|tanio|drogo|kup|sprzedaj)\b/i)).toBeNull();
  });

  it("states the no-sector reason when the company is unclassified", async () => {
    vi.mocked(getSectorPercentiles).mockImplementation(async (companyId) => ({
      companyId, sector: null, peerCount: 0, thin: true, emptyReason: "no_sector", metrics: [],
    }));
    vi.mocked(computeComparativeValuation).mockImplementation(async (companyId) => ({
      companyId, sector: null, peerCount: 0, thin: true, currentPrice: null, dataAsOf: "",
      emptyReason: "no_sector", methods: [], convergence: null,
      confidence: { grade: "D", composite: "0", dataCompleteness: "0", peerDepth: "0", methodConvergence: "0", validation: "0" },
    }));
    const user = userEvent.setup();
    renderCompare();
    await selectPair(user);

    const section = await screen.findByRole("region", { name: "Valuation" });
    expect(await within(section).findByText(/No sector/i)).toBeInTheDocument();
  });
});

describe("Compare trend overlay (ADR 0089 §A4)", () => {
  beforeEach(() => {
    vi.mocked(listCompanies).mockResolvedValue(MANY_COMPANIES);
    vi.mocked(getKpiComparison).mockImplementation(async (input) =>
      input.metricKeys.length === 1
        ? trendComparison(input.companyIds)
        : buildComparison(input.companyIds, input.metricKeys, input.granularity),
    );
  });

  it("caps the chart at the first 4 companies and names the dropped count", async () => {
    const user = userEvent.setup();
    renderCompare();
    await selectIds(user, [
      "company_gpw_c0",
      "company_gpw_c1",
      "company_gpw_c2",
      "company_gpw_c3",
      "company_gpw_c4",
    ]);
    await user.click(await screen.findByRole("button", { name: "Trend" }));

    const chart = await screen.findByRole("img", { name: /Trend/ });
    const legend = chart.parentElement!.querySelector(".ui-multi-line-legend")!;
    const labels = within(legend as HTMLElement).getAllByText(/GPW:C\d/);
    expect(labels.map((el) => el.textContent)).toEqual([
      "GPW:C0",
      "GPW:C1",
      "GPW:C2",
      "GPW:C3",
    ]);
    expect(screen.getByText(/Chart shows the first 4 of\s+5\s+companies/)).toBeInTheDocument();
  });
});
