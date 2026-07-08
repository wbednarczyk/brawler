import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FundamentalsPanel, factsRecordedLabel, tierLabel } from "./FundamentalsPanel";

// The Autopilot / IR-URL / custom-KPI child fields load state via the mocked
// `invoke` on mount (out of scope here); stub them so the render exercises only
// the panel's own U7-A density disclosures.
vi.mock("../../shared/components/CompanyAutopilotField", () => ({
  CompanyAutopilotField: () => <div data-testid="autopilot-field" />,
}));
vi.mock("../../shared/components/CompanyIrReportsUrlField", () => ({
  CompanyIrReportsUrlField: () => <div data-testid="ir-field" />,
}));
vi.mock("../../shared/components/CustomKpiManager", () => ({
  CustomKpiManager: () => <div data-testid="kpi-manager" />,
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
