import { describe, it, expect, vi, beforeEach } from "vitest";
import type { JSX } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyHealthSection } from "./CompanyHealthSection";
import { getCompanyHealth } from "../../api/companyHealth";
import type { CompanyHealth, HealthPeriodScores } from "../../api/companyHealth";
import { LocaleContext, makeTextTranslator, makeTranslator } from "../locale";

function renderPl(ui: JSX.Element) {
  return render(
    <LocaleContext.Provider value={{ locale: "pl", t: makeTranslator("pl"), text: makeTextTranslator("pl") }}>
      {ui}
    </LocaleContext.Provider>,
  );
}

vi.mock("../../api/companyHealth", () => ({
  getCompanyHealth: vi.fn(),
}));

const getCompanyHealthMock = vi.mocked(getCompanyHealth);

function headlinePeriod(): HealthPeriodScores {
  return {
    periodId: "p2025",
    fiscalYear: 2025,
    piotroski: {
      state: "headline",
      score: 8,
      signals: [
        {
          code: "F1",
          name: "roa_positive",
          passed: true,
          points: 1,
          inputs: [
            { key: "net_profit@FY2025", value: "100" },
            { key: "total_assets@FY2025", value: "1000" },
          ],
        },
      ],
    },
    altman: {
      state: "headline",
      zScore: "4.9994",
      band: "safe",
      components: [
        {
          code: "X1",
          name: "working_capital_to_assets",
          weight: "6.56",
          ratio: "0.25",
          contribution: "1.64",
          inputs: [{ key: "working_capital@FY2025", value: "250" }],
        },
      ],
    },
  };
}

function health(overrides: Partial<CompanyHealth> = {}): CompanyHealth {
  const latest = overrides.latest ?? headlinePeriod();
  return {
    companyId: "company_gpw_cdr",
    statementType: "industrial",
    piotroskiVariant: "Piotroski F (2000)",
    altmanVariant: "Altman Z″ EM (1995)",
    latest,
    history: latest ? [latest] : [],
    ...overrides,
  };
}

describe("CompanyHealthSection", () => {
  beforeEach(() => {
    getCompanyHealthMock.mockReset();
  });

  it("renders both score tiles and expands the Piotroski breakdown", async () => {
    getCompanyHealthMock.mockResolvedValue(health());
    render(<CompanyHealthSection companyId="company_gpw_cdr" />);

    expect(await screen.findByText("8/9")).toBeInTheDocument();
    expect(screen.getByText(/4\.9994 · Safe/)).toBeInTheDocument();
    // The published-formula citations are visible as the tile labels.
    expect(screen.getAllByText("Piotroski F (2000)").length).toBeGreaterThan(0);

    // Expanding the F tile reveals the per-signal breakdown with measured inputs.
    await userEvent.click(screen.getByRole("button", { name: /Piotroski F \(2000\)/ }));
    expect(await screen.findByText(/Return on assets is positive/)).toBeInTheDocument();
    expect(screen.getByText(/net_profit@FY2025 = 100/)).toBeInTheDocument();
  });

  it("shows the insufficient-data state with the missing-input list, localized (en)", async () => {
    const period = headlinePeriod();
    period.piotroski = {
      state: "insufficient_data",
      signals: [],
      missing: [
        { metric: "long_term_debt", period: "FY2025" },
        { metric: "long_term_debt", period: "FY2024" },
      ],
    };
    getCompanyHealthMock.mockResolvedValue(health({ latest: period }));
    render(<CompanyHealthSection companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Insufficient data")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Piotroski F \(2000\)/ }));
    // The localized KPI name, never the raw `long_term_debt` metric key.
    expect(
      await screen.findByText(/Missing inputs: Long-term debt \(FY2025\), Long-term debt \(FY2024\)/),
    ).toBeInTheDocument();
  });

  it("localizes a missing current_liabilities input to a Polish KPI name (pl)", async () => {
    const period = headlinePeriod();
    period.piotroski = {
      state: "insufficient_data",
      signals: [],
      missing: [{ metric: "current_liabilities", period: "FY2025" }],
    };
    getCompanyHealthMock.mockResolvedValue(health({ latest: period }));
    renderPl(<CompanyHealthSection companyId="company_gpw_cdr" />);

    await userEvent.click(await screen.findByRole("button", { name: /Piotroski F \(2000\)/ }));
    // Localized Polish KPI name, never the raw `current_liabilities` metric key.
    expect(
      await screen.findByText(/Brakujące dane: Zobowiązania krótkoterminowe \(FY2025\)/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/current_liabilities/)).not.toBeInTheDocument();
  });

  it("renders the prior_fy_period reason as a human sentence, not a raw token (en)", async () => {
    const period = headlinePeriod();
    period.piotroski = {
      state: "insufficient_data",
      signals: [],
      missing: [{ metric: "prior_fy_period", period: "before FY2025" }],
    };
    getCompanyHealthMock.mockResolvedValue(health({ latest: period }));
    render(<CompanyHealthSection companyId="company_gpw_cdr" />);

    await userEvent.click(await screen.findByRole("button", { name: /Piotroski F \(2000\)/ }));
    expect(
      await screen.findByText(/Missing inputs: prior fiscal year \(needed before FY2025\)/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/prior_fy_period/)).not.toBeInTheDocument();
  });

  it("renders the prior_fy_period reason as a human Polish sentence, not a raw token (pl)", async () => {
    const period = headlinePeriod();
    period.piotroski = {
      state: "insufficient_data",
      signals: [],
      missing: [{ metric: "prior_fy_period", period: "before FY2025" }],
    };
    getCompanyHealthMock.mockResolvedValue(health({ latest: period }));
    renderPl(<CompanyHealthSection companyId="company_gpw_cdr" />);

    await userEvent.click(await screen.findByRole("button", { name: /Piotroski F \(2000\)/ }));
    expect(
      await screen.findByText(/Brakujące dane: poprzedni rok obrotowy \(potrzebny przed FY2025\)/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/prior_fy_period/)).not.toBeInTheDocument();
    // No stray English inside the Polish sentence.
    expect(screen.queryByText(/before FY2025/)).not.toBeInTheDocument();
  });

  it("shows the not-applicable state for a financial company", async () => {
    const period = headlinePeriod();
    period.piotroski = { state: "not_applicable", reason: "bank" };
    period.altman = { state: "not_applicable", reason: "bank" };
    getCompanyHealthMock.mockResolvedValue(
      health({ latest: period, statementType: "bank" }),
    );
    render(<CompanyHealthSection companyId="company_gpw_cdr" />);

    expect(await screen.findAllByText("Not applicable")).toHaveLength(2);
  });

  it("localizes the not-applicable reason for a recognized statement_type (pl)", async () => {
    const period = headlinePeriod();
    period.piotroski = { state: "not_applicable", reason: "banking" };
    period.altman = { state: "not_applicable", reason: "banking" };
    getCompanyHealthMock.mockResolvedValue(
      health({ latest: period, statementType: "banking" }),
    );
    renderPl(<CompanyHealthSection companyId="company_gpw_cdr" />);

    await userEvent.click(await screen.findByRole("button", { name: /Piotroski F \(2000\)/ }));
    expect(await screen.findByText(/Nie dotyczy spółek finansowych \(Bankowość\)/)).toBeInTheDocument();
    expect(screen.queryByText(/banking/)).not.toBeInTheDocument();
  });

  it("localizes the averaging-basis and leverage-basis marker inputs, not raw tokens (pl)", async () => {
    const period = headlinePeriod();
    period.piotroski = {
      state: "headline",
      score: 1,
      signals: [
        {
          code: "F5",
          name: "leverage_improved",
          passed: true,
          points: 1,
          inputs: [
            { key: "leverage_input", value: "non_current_liabilities" },
            { key: "basis", value: "two_year_average" },
          ],
        },
      ],
    };
    getCompanyHealthMock.mockResolvedValue(health({ latest: period }));
    renderPl(<CompanyHealthSection companyId="company_gpw_cdr" />);

    await userEvent.click(await screen.findByRole("button", { name: /Piotroski F \(2000\)/ }));
    expect(await screen.findByText(/Podstawa dźwigni = zobowiązania długoterminowe/)).toBeInTheDocument();
    expect(screen.getByText(/Podstawa uśredniania = średnia dwuletnia/)).toBeInTheDocument();
    expect(screen.queryByText(/non_current_liabilities/)).not.toBeInTheDocument();
    expect(screen.queryByText(/two_year_average/)).not.toBeInTheDocument();
  });

  it("renders the empty state when there are no annual periods", async () => {
    getCompanyHealthMock.mockResolvedValue(health({ latest: undefined, history: [] }));
    render(<CompanyHealthSection companyId="company_gpw_cdr" />);

    await waitFor(() =>
      expect(screen.getByText(/No annual periods yet/)).toBeInTheDocument(),
    );
  });
});
