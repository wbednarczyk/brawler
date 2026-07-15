import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyBasicInfoPanel } from "./CompanyBasicInfoPanel";
import { getCompanyBasicInfo } from "../../api/companyBasicInfo";

vi.mock("../../api/companyBasicInfo", () => ({
  getCompanyBasicInfo: vi.fn(),
}));
// The edit fields load their own state via the mocked `invoke` (out of scope);
// stub them so the toggle test exercises only this panel's behavior.
vi.mock("./CompanySectorField", () => ({
  CompanySectorField: () => <div data-testid="sector-field" />,
}));
vi.mock("./CompanyIrReportsUrlField", () => ({
  CompanyIrReportsUrlField: () => <div data-testid="ir-field" />,
}));

const getMock = vi.mocked(getCompanyBasicInfo);

const INFO = {
  displayName: "CD PROJEKT S.A.",
  exchange: "GPW",
  ticker: "CDR",
  qualifiedTicker: "GPW:CDR",
  isin: "PLOPTTC00011",
  sector: "Gry",
  sectorSource: "registry",
  sharesOutstanding: "99895500",
  sharesOutstandingPeriod: "2025 FY",
};

describe("CompanyBasicInfoPanel (owner request 2026-07-14)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getMock.mockResolvedValue(INFO);
  });

  it("renders identity facts, sector provenance and the shares fact read-only", async () => {
    render(<CompanyBasicInfoPanel companyId="company_gpw_cdr" />);

    await waitFor(() => {
      expect(screen.getByText("CD PROJEKT S.A.")).toBeInTheDocument();
    });
    expect(screen.getByText("PLOPTTC00011")).toBeInTheDocument();
    expect(screen.getByText("Gry")).toBeInTheDocument();
    expect(screen.getByText("from the registry")).toBeInTheDocument();
    expect(screen.getByText("2025 FY")).toBeInTheDocument();
    // Read-only by default: no per-fact edit affordances, no edit fields.
    expect(screen.queryByTestId("sector-field")).not.toBeInTheDocument();
    expect(screen.queryByTestId("ir-field")).not.toBeInTheDocument();
  });

  it("reveals the sector/IR edit fields only behind the panel-level Edit toggle", async () => {
    const user = userEvent.setup();
    render(<CompanyBasicInfoPanel companyId="company_gpw_cdr" />);
    await waitFor(() => expect(screen.getByText("CD PROJEKT S.A.")).toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByTestId("sector-field")).toBeInTheDocument();
    expect(screen.getByTestId("ir-field")).toBeInTheDocument();

    // Leaving edit mode re-fetches (a sector save may change provenance).
    await user.click(screen.getByRole("button", { name: "Done editing" }));
    expect(screen.queryByTestId("sector-field")).not.toBeInTheDocument();
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(2));
  });

  it("renders em dashes for absent optionals, never invented values", async () => {
    getMock.mockResolvedValue({
      ...INFO,
      isin: null,
      sector: null,
      sectorSource: null,
      sharesOutstanding: null,
      sharesOutstandingPeriod: null,
    });
    render(<CompanyBasicInfoPanel companyId="company_gpw_cdr" />);

    await waitFor(() => expect(screen.getByText("CD PROJEKT S.A.")).toBeInTheDocument());
    expect(screen.getAllByText("—").length).toBeGreaterThanOrEqual(3);
    expect(screen.queryByText("from the registry")).not.toBeInTheDocument();
  });
});
