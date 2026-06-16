import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyClaimsPanel } from "./CompanyClaimsPanel";
import {
  createManagementClaim,
  listClaimsToVerify,
  listManagementClaims,
  setClaimVerdict,
  type ClaimsToVerify,
  type ManagementClaim,
} from "../../api/managementClaims";

vi.mock("../../api/managementClaims", () => ({
  listManagementClaims: vi.fn(),
  listClaimsToVerify: vi.fn(),
  createManagementClaim: vi.fn(),
  setClaimVerdict: vi.fn(),
}));

const listManagementClaimsMock = vi.mocked(listManagementClaims);
const listClaimsToVerifyMock = vi.mocked(listClaimsToVerify);
const createManagementClaimMock = vi.mocked(createManagementClaim);
const setClaimVerdictMock = vi.mocked(setClaimVerdict);

function claim(overrides: Partial<ManagementClaim> = {}): ManagementClaim {
  return {
    id: "claim_1",
    companyId: "company_gpw_cdr",
    statement: "Net revenue will reach 1,000,000 by Q4 2026.",
    body: "",
    bodyFormat: "markdown",
    madeAt: null,
    sourcePeriodId: null,
    dueFiscalYear: 2026,
    duePeriodType: "Q4",
    status: "pending",
    sourceEvidenceType: "manual",
    sourceEvidenceId: null,
    extractionProposalId: null,
    targetMetricKey: null,
    targetComparator: null,
    targetValueNumeric: null,
    targetUnit: null,
    verifyingFactId: null,
    revisesClaimId: null,
    createdAt: "2026-06-01T10:00:00Z",
    updatedAt: "2026-06-01T10:00:00Z",
    ...overrides,
  };
}

const emptyQueue: ClaimsToVerify = { due: [], overdue: [], upcoming: [] };

describe("CompanyClaimsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listManagementClaimsMock.mockResolvedValue([]);
    listClaimsToVerifyMock.mockResolvedValue(emptyQueue);
    createManagementClaimMock.mockResolvedValue(claim());
    setClaimVerdictMock.mockResolvedValue(claim({ status: "delivered" }));
  });

  it("shows the empty state when no claims are tracked", async () => {
    render(<CompanyClaimsPanel companyId="company_gpw_cdr" />);
    await waitFor(() => {
      expect(screen.getByText("No management claims tracked yet.")).toBeInTheDocument();
    });
  });

  it("resolves a due claim from the review queue", async () => {
    const due = claim();
    listClaimsToVerifyMock.mockResolvedValue({
      due: [
        {
          claim: due,
          arrivedPeriodId: "period_2026_q4",
          verifyingFactCandidate: { factId: "fact_1", valueNumeric: "1250000" },
        },
      ],
      overdue: [],
      upcoming: [],
    });
    listManagementClaimsMock.mockResolvedValue([due]);
    const user = userEvent.setup();

    render(<CompanyClaimsPanel companyId="company_gpw_cdr" />);

    await waitFor(() => {
      expect(screen.getByText("Claims to verify")).toBeInTheDocument();
      expect(screen.getByText(/1250000/)).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Delivered" }));

    await waitFor(() => {
      expect(setClaimVerdictMock).toHaveBeenCalledWith({
        claimId: "claim_1",
        status: "delivered",
      });
    });
  });

  it("creates a new claim from the form", async () => {
    const user = userEvent.setup();
    render(<CompanyClaimsPanel companyId="company_gpw_cdr" />);

    await waitFor(() => {
      expect(screen.getByText("No management claims tracked yet.")).toBeInTheDocument();
    });

    await user.type(
      screen.getByPlaceholderText("What did management promise?"),
      "Dividend will be raised",
    );
    await user.click(screen.getByRole("button", { name: "Add claim" }));

    await waitFor(() => {
      expect(createManagementClaimMock).toHaveBeenCalledWith(
        expect.objectContaining({
          companyId: "company_gpw_cdr",
          statement: "Dividend will be raised",
        }),
      );
    });
  });
});
