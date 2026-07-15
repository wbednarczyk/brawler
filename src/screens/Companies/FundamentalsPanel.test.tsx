import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FundamentalsPanel, factsRecordedLabel, tierLabel } from "./FundamentalsPanel";
import { getPriceContext } from "../../api/marketData";

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
  financialFactForm: { definitionId: "", valueNumeric: "", currency: "", periodId: "" },
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
