import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyClaimsPanel } from "./CompanyClaimsPanel";
import { formatFinancialValue } from "../format/financialValue";
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

    // The reported figure renders humanized (audit K9, ADR 0076 D4): the raw
    // integer never reaches the user.
    const humanized = formatFinancialValue({ valueNumeric: "1250000" }, "en");
    await waitFor(() => {
      expect(screen.getByText("Claims to verify")).toBeInTheDocument();
      expect(screen.getByText((content) => content.includes(humanized))).toBeInTheDocument();
    });
    expect(screen.queryByText(/1250000/)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Delivered" }));

    await waitFor(() => {
      expect(setClaimVerdictMock).toHaveBeenCalledWith({
        claimId: "claim_1",
        status: "delivered",
      });
    });
  });

  // Issue #87: a mutating verdict action must be guarded while its save is in
  // flight — a double click may not reach the backend twice (latent double-write
  // on any non-idempotent successor command).
  it("ignores a second verdict click while the first save is in flight", async () => {
    const due = claim();
    listClaimsToVerifyMock.mockResolvedValue({
      due: [{ claim: due, arrivedPeriodId: "period_2026_q4", verifyingFactCandidate: null }],
      overdue: [],
      upcoming: [],
    });
    listManagementClaimsMock.mockResolvedValue([due]);
    let resolveSave: (value: ManagementClaim) => void = () => {};
    setClaimVerdictMock.mockImplementation(
      () =>
        new Promise<ManagementClaim>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const user = userEvent.setup();

    render(<CompanyClaimsPanel companyId="company_gpw_cdr" />);
    const delivered = await screen.findByRole("button", { name: "Delivered" });

    await user.click(delivered);
    expect(delivered).toBeDisabled();
    await user.click(delivered);
    expect(setClaimVerdictMock).toHaveBeenCalledTimes(1);

    resolveSave(claim({ status: "delivered" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Delivered" })).not.toBeDisabled();
    });
  });

  // U7-C density contract (ADR 0076 D6): at the S width tier the composer hides
  // behind a "Dodaj tezę" (Add claim) disclosure. jsdom has no container
  // queries, so we assert the disclosure STATE (aria-expanded + the data flag the
  // S-tier CSS keys off); the tier switch itself is browser-tested.
  it("exposes the composer behind an Add claim disclosure toggle", async () => {
    const user = userEvent.setup();
    const { container } = render(<CompanyClaimsPanel companyId="company_gpw_cdr" />);

    await waitFor(() => {
      expect(screen.getByText("No management claims tracked yet.")).toBeInTheDocument();
    });

    const panel = container.querySelector(".company-claims-panel");
    const toggle = container.querySelector(".claims-add-toggle");
    expect(toggle).not.toBeNull();
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(panel).not.toHaveAttribute("data-composer-open");

    await user.click(toggle as HTMLElement);

    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(panel).toHaveAttribute("data-composer-open");
  });

  // U7-C short-tier contract: the queue summary shows per-status counts and only
  // the top 3 due claims (rest behind expansion).
  it("summarizes queue counts and only the top 3 due claims (short tier)", async () => {
    const due = [1, 2, 3, 4].map((n) =>
      claim({ id: `claim_due_${n}`, statement: `Due promise number ${n}` }),
    );
    listClaimsToVerifyMock.mockResolvedValue({
      due: due.map((c) => ({ claim: c, arrivedPeriodId: null, verifyingFactCandidate: null })),
      overdue: [{ claim: claim({ id: "claim_od" }), arrivedPeriodId: null, verifyingFactCandidate: null }],
      upcoming: [],
    });
    listManagementClaimsMock.mockResolvedValue(due);

    const { container } = render(<CompanyClaimsPanel companyId="company_gpw_cdr" />);

    await waitFor(() => {
      expect(container.querySelector(".claims-queue-summary")).not.toBeNull();
    });

    const summary = container.querySelector(".claims-queue-summary") as HTMLElement;
    // The counts line reflects the FULL live counts (4 due, 1 overdue).
    expect(within(summary).getByText(/Due now/)).toBeInTheDocument();
    expect(within(summary).getByText("4")).toBeInTheDocument();
    // Only the top 3 due claims render in the summary.
    const topItems = summary.querySelectorAll(".claims-queue-top-item");
    expect(topItems).toHaveLength(3);
    expect(within(summary).getByText("Due promise number 1")).toBeInTheDocument();
    expect(within(summary).queryByText("Due promise number 4")).not.toBeInTheDocument();
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
    // The composer's own submit — disambiguated from the S-tier disclosure toggle
    // (also "Add claim") by scoping to the create form (U7-C).
    const composer = screen.getByRole("form", { name: "Add a claim" });
    await user.click(within(composer).getByRole("button", { name: "Add claim" }));

    await waitFor(() => {
      expect(createManagementClaimMock).toHaveBeenCalledWith(
        expect.objectContaining({
          companyId: "company_gpw_cdr",
          statement: "Dividend will be raised",
        }),
      );
    });
  });

  // Today's `openCompanyClaims(companyId, claimId)` nav seam (F2 S3, plan
  // decision 6): the claim id reaches the panel and its row is scrolled into
  // view + flashed — asserted on the claim's row, never the screen.
  it("highlights and scrolls the targeted claim once it loads", async () => {
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    const target = claim({ id: "claim_target", statement: "Target claim" });
    listManagementClaimsMock.mockResolvedValue([claim({ id: "claim_other" }), target]);

    render(<CompanyClaimsPanel companyId="company_gpw_cdr" highlightClaimId="claim_target" />);

    await waitFor(() => {
      const row = screen.getByText("Target claim").closest("[data-claim-id]");
      expect(row).toHaveAttribute("data-claim-id", "claim_target");
      expect(row).toHaveClass("claim-row-highlighted");
    });
    expect(scrollIntoView).toHaveBeenCalled();
  });

  it("does not highlight anything when no claim id is targeted", async () => {
    listManagementClaimsMock.mockResolvedValue([claim({ id: "claim_a" })]);
    render(<CompanyClaimsPanel companyId="company_gpw_cdr" />);
    await waitFor(() => {
      expect(screen.getByText("Net revenue will reach 1,000,000 by Q4 2026.")).toBeInTheDocument();
    });
    expect(document.querySelector(".claim-row-highlighted")).toBeNull();
  });
});
