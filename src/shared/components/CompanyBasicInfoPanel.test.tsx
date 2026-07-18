import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyBasicInfoPanel } from "./CompanyBasicInfoPanel";
import { getCompanyBasicInfo } from "../../api/companyBasicInfo";
import { getOwnershipOverview } from "../../api/ownership";
import { ToastProvider } from "../../ui";

vi.mock("../../api/companyBasicInfo", () => ({
  getCompanyBasicInfo: vi.fn(),
}));
// The Ownership section loads its own overview via this module; stub it so the
// panel-identity tests exercise only the identity/edit behavior.
vi.mock("../../api/ownership", () => ({
  getOwnershipOverview: vi.fn(),
  backfillOwnershipExtraction: vi.fn(),
  setOwnershipHolderType: vi.fn(),
  confirmOwnershipHolderTypeProposal: vi.fn(),
  rejectOwnershipHolderTypeProposal: vi.fn(),
  runOwnershipClassification: vi.fn(),
  runCompanyOwnershipOcr: vi.fn(),
  confirmOwnershipOcrProposal: vi.fn(),
  rejectOwnershipOcrProposal: vi.fn(),
}));
// The Insiderzy block (v0.57 T6) fetches on mount; stub it so this panel's
// identity-facts tests exercise only the Basic-info behavior.
vi.mock("../../api/insider", () => ({
  getInsiderOverview: vi.fn(() => Promise.resolve(null)),
}));

const EMPTY_OWNERSHIP = {
  companyId: "company_gpw_cdr",
  freeFloatPct: "100",
  disclosedSum: "0",
  holders: [],
  history: [],
  freeFloatHistory: [],
  residuals: [],
  pendingProposals: [],
  ocrProposals: [],
};

function renderPanel(companyId: string) {
  return render(
    <ToastProvider>
      <CompanyBasicInfoPanel companyId={companyId} />
    </ToastProvider>,
  );
}
// The edit fields load their own state via the mocked `invoke` (out of scope);
// stub them so the toggle test exercises only this panel's behavior.
vi.mock("./CompanySectorField", () => ({
  CompanySectorField: () => <div data-testid="sector-field" />,
}));
vi.mock("./CompanyIrReportsUrlField", () => ({
  CompanyIrReportsUrlField: () => <div data-testid="ir-field" />,
}));

const getMock = vi.mocked(getCompanyBasicInfo);
const ownershipMock = vi.mocked(getOwnershipOverview);

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
    ownershipMock.mockResolvedValue(EMPTY_OWNERSHIP);
  });

  it("renders identity facts, sector provenance and the shares fact read-only", async () => {
    renderPanel("company_gpw_cdr");

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
    renderPanel("company_gpw_cdr");
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
    renderPanel("company_gpw_cdr");

    await waitFor(() => expect(screen.getByText("CD PROJEKT S.A.")).toBeInTheDocument());
    expect(screen.getAllByText("—").length).toBeGreaterThanOrEqual(3);
    expect(screen.queryByText("from the registry")).not.toBeInTheDocument();
  });
});
