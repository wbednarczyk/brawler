import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { SpolkaScreen, type SpolkaScreenProps } from "./SpolkaScreen";
import { useSpolkaToolHost } from "./ToolHost";
import { SpolkaScreenHost } from "../../app/useSpolkaScreenWiring";
import { ToastProvider } from "../../ui";
import { ResearchProvider } from "../../app/state/screenViewModels";
import type { ResearchScreenProps } from "../Research/ResearchScreen";
import { getCompanyView } from "../../api/companyView";
import type { CompanyView } from "../../api/generated/CompanyView";
import { COMPANY_SPECS, makeCompany } from "../../test/scenarios/entities";

vi.mock("../../api/companyView", () => ({
  getCompanyView: vi.fn(),
}));

const getCompanyViewMock = vi.mocked(getCompanyView);

const researchViewModelStub: ResearchScreenProps = {
  companies: [],
  watchlists: [],
  watchlistMemberships: [],
  mode: "company",
  selectedCompanyId: null,
  selectedWatchlistId: null,
  selectedWatchlistCompanyId: null,
  cascadeToCompanies: false,
  selectedEvidenceTypes: [],
  changedOnly: false,
  timeline: null,
  questions: [],
  selectedQuestionId: null,
  questionTitle: "",
  questionBody: "",
  questionLinks: [],
  reminders: [],
  error: null,
  loading: false,
  reviewInFlight: false,
  questionInFlight: false,
  reminderInFlight: false,
  setMode: () => {},
  setSelectedCompanyId: () => {},
  setSelectedWatchlistId: () => {},
  setSelectedWatchlistCompanyId: () => {},
  setSelectedQuestionId: () => {},
  setQuestionTitle: () => {},
  setQuestionBody: () => {},
  setCascadeToCompanies: () => {},
  setChangedOnly: () => {},
  toggleEvidenceType: () => {},
  clearEvidenceTypes: () => {},
  refreshTimeline: () => {},
  markReviewed: () => {},
  createQuestion: () => {},
  updateQuestionStatus: () => {},
  deleteQuestion: () => {},
  linkEvidence: () => {},
  unlinkEvidence: () => {},
  createReminder: () => {},
  completeReminder: () => {},
  snoozeReminder: () => {},
  reopenReminder: () => {},
  deleteReminder: () => {},
  openEvidence: () => {},
  openEvidenceUrl: () => {},
  formatTimestamp: (v) => v ?? "",
};

const invokeMock = vi.mocked(invoke);

const company = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "cdr")!);
const companyB = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "pkn")!);

function emptyView(overrides: Partial<CompanyView> = {}): CompanyView {
  return {
    companyId: company.id,
    qualifiedTicker: company.qualifiedTicker,
    displayName: company.displayName,
    isin: company.isin ?? undefined,
    counters: {
      signals: { unacked: 3, byCategory: [] },
      claims: { open: 2, nearestDue: "FY 2026" },
      shorts: { activeSumPct: 0.6, largestHolder: "Qube" },
      events: { upcoming: 2 },
    },
    kpi: undefined,
    feed: [
      { feedItemId: "feed_0", title: "Report 0", publishedAt: "2026-08-20T09:00:00Z", read: false, itemType: "Official report", sourceName: "ESPI", presentationKind: "report" },
    ],
    price: undefined,
    coverage: [],
    recommendations: [],
    sectionErrors: {},
    ...overrides,
  };
}

function baseProps(overrides: Partial<SpolkaScreenProps> = {}): SpolkaScreenProps {
  return {
    companyId: company.id,
    company,
    spolkaTool: overrides.spolkaTool as SpolkaScreenProps["spolkaTool"],
    feedItems: [],
    rootHighlightClaimId: null,
    onOpenDocument: vi.fn(),
    onOpenFeedItem: vi.fn(),
    refreshCompletionCount: 0,
    ...overrides,
  };
}

function Harness(overrides: Partial<SpolkaScreenProps>) {
  const spolkaTool = useSpolkaToolHost();
  return (
    <ToastProvider>
      <ResearchProvider value={researchViewModelStub}>
      <SpolkaScreen {...baseProps({ ...overrides, spolkaTool })} />
    </ResearchProvider>
    </ToastProvider>
  );
}

async function openDirtyNotebookTool(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Open notebook" }));
  await screen.findByRole("group", { name: "Workshop tool" });
  await user.click(await screen.findByRole("button", { name: "New note" }));
  const titleField = await screen.findByRole("textbox", { name: "Notebook note title" });
  await user.type(titleField, "Draft in progress");
  return titleField;
}

beforeEach(() => {
  getCompanyViewMock.mockReset();
  getCompanyViewMock.mockResolvedValue(emptyView());
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: unknown) => {
    if (command === "list_claims_to_verify") {
      return Promise.resolve({ due: [], overdue: [], upcoming: [] });
    }
    if (command === "get_ownership_overview") {
      return Promise.resolve({ holders: [], residuals: [], insiderHoldersCount: 0, lastUpdatedAt: null });
    }
    if (command === "get_company_basic_info") {
      return Promise.resolve({
        displayName: company.displayName,
        exchange: "GPW",
        ticker: company.qualifiedTicker.split(":")[1] ?? company.qualifiedTicker,
        qualifiedTicker: company.qualifiedTicker,
        isin: null,
        sector: null,
        sectorSource: null,
        sharesOutstanding: null,
        sharesOutstandingPeriod: null,
      });
    }
    if (command === "get_report_documents_view") {
      return Promise.resolve({ rows: [], totals: null });
    }
    if (command === "get_fundamentals_coverage") {
      return Promise.resolve({ periods: [] });
    }
    if (command === "get_price_context") {
      return Promise.resolve({
        lastClose: 0,
        lastDate: "",
        changeAbs: 0,
        changePct: 0,
        currency: "PLN",
        week52High: 0,
        week52Low: 0,
        week52HighDistPct: 0,
        week52LowDistPct: 0,
        marketCap: null,
        ratios: {},
        history: [],
        fetchedAt: "",
        emptyReason: "no_quotes",
      });
    }
    if (command === "get_insider_overview") {
      return Promise.resolve({
        companyId: company.id,
        transactions: [],
        holdings: [],
        window90d: { buyCount: 0, sellCount: 0, netValue: null },
        window12m: { buyCount: 0, sellCount: 0, netValue: null },
      });
    }
    return Promise.resolve([]);
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("Spółka tool host — dirty guard", () => {
  it("clean unmount proceeds immediately", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");

    await user.click(screen.getByRole("button", { name: "Open notebook" }));
    await screen.findByRole("group", { name: "Workshop tool" });
    await user.click(screen.getByRole("button", { name: "Close tool" }));

    expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("dirty guard on tool close", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");
    await openDirtyNotebookTool(user);

    await user.click(screen.getByRole("button", { name: "Close tool" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Workshop tool" })).toBeInTheDocument();
  });

  it("dirty guard on switching tools", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");
    await openDirtyNotebookTool(user);

    await user.click(screen.getByRole("button", { name: "Open claims" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Workshop tool" }).getAttribute("data-tool")).toBe("notatnik");
  });

  it.each([
    { target: "Dziś" },
    { target: "Inbox" },
    { target: "global screen" },
    { target: "namedView" },
  ])("dirty guard on navigating away: $target", async () => {
    const user = userEvent.setup();
    let capturedGuard: ((next: () => void) => void) | null = null;
    function Capture() {
      const spolkaTool = useSpolkaToolHost();
      capturedGuard = spolkaTool.guardNavigation;
      return (
        <ToastProvider>
      <ResearchProvider value={researchViewModelStub}>
          <SpolkaScreen {...baseProps({ spolkaTool })} />
        </ResearchProvider>
    </ToastProvider>
      );
    }
    render(<Capture />);
    await screen.findByText("CD Projekt");
    await openDirtyNotebookTool(user);

    const next = vi.fn();
    capturedGuard!(next);

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(next).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Discard" }));
    expect(next).toHaveBeenCalledTimes(1);
  });

  it("stay keeps tool and draft untouched", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");
    await openDirtyNotebookTool(user);

    await user.click(screen.getByRole("button", { name: "Close tool" }));
    await screen.findByRole("dialog");
    await user.click(screen.getByRole("button", { name: "Stay" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Workshop tool" }).getAttribute("data-tool")).toBe("notatnik");
    expect((screen.getByRole("textbox", { name: "Notebook note title" }) as HTMLInputElement).value).toBe(
      "Draft in progress",
    );
  });

  it("discard calls discard() and proceeds", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");
    await openDirtyNotebookTool(user);

    await user.click(screen.getByRole("button", { name: "Close tool" }));
    await screen.findByRole("dialog");
    await user.click(screen.getByRole("button", { name: "Discard" }));

    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("switching company closes a clean tool", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockImplementation((id: string) =>
      Promise.resolve(id === companyB.id ? emptyView({ companyId: companyB.id, qualifiedTicker: companyB.qualifiedTicker, displayName: companyB.displayName }) : emptyView()),
    );
    // Render through a harness so the SAME host instance persists across rerenders.
    function Host({ selectedCompanyId }: { selectedCompanyId: string }) {
      const spolkaTool = useSpolkaToolHost();
      return (
        <ToastProvider>
      <ResearchProvider value={researchViewModelStub}>
          <SpolkaScreenHost
            companies={[company, companyB]}
            selectedCompanyId={selectedCompanyId}
            spolkaTool={spolkaTool}
            feedItems={[]}
            rootHighlightClaimId={null}
            openInboxItem={() => {}}
            refreshCompletionCount={0}
          />
        </ResearchProvider>
    </ToastProvider>
      );
    }
    const { rerender } = render(<Host selectedCompanyId={company.id} />);
    await screen.findByText("CD Projekt");
    await user.click(screen.getByRole("button", { name: "Open notebook" }));
    await screen.findByRole("group", { name: "Workshop tool" });

    rerender(<Host selectedCompanyId={companyB.id} />);

    await screen.findByText("PKN Orlen");
    expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument();
  });

  it("a late view response for A cannot reopen A's tool over B", async () => {
    getCompanyViewMock.mockResolvedValue(emptyView());
    const user = userEvent.setup();
    function Host(overrides: Partial<SpolkaScreenProps>) {
      const spolkaTool = useSpolkaToolHost();
      return (
        <ToastProvider>
      <ResearchProvider value={researchViewModelStub}>
          <SpolkaScreen {...baseProps({ ...overrides, spolkaTool })} />
        </ResearchProvider>
    </ToastProvider>
      );
    }
    const { rerender } = render(<Host />);
    await screen.findByText("CD Projekt");
    await user.click(screen.getByRole("button", { name: "Open claims" }));
    await screen.findByRole("group", { name: "Workshop tool" });

    getCompanyViewMock.mockResolvedValue(
      emptyView({ companyId: companyB.id, qualifiedTicker: companyB.qualifiedTicker, displayName: companyB.displayName }),
    );
    rerender(<Host companyId={companyB.id} company={companyB} />);

    await screen.findByText("PKN Orlen");
    expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument();
  });

  it("closing a tool restores core scroll and selection", async () => {
    const user = userEvent.setup();
    const { container } = render(<Harness />);
    await screen.findByText("CD Projekt");

    const layout = container.querySelector(".spolka-layout") as HTMLElement;
    layout.scrollTop = 240;
    await user.click(screen.getByRole("button", { name: /Report 0/ }));
    await screen.findByRole("group", { name: "Workshop tool" });

    await user.click(screen.getByRole("button", { name: "Close tool" }));
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());

    expect(layout.scrollTop).toBe(240);
    const selectedRow = container.querySelector('[data-selected="true"]');
    expect(selectedRow?.textContent).toContain("Report 0");
  });

  it("refresh completion refreshes the strip and leaves the tool mounted", async () => {
    const user = userEvent.setup();
    function Host(overrides: Partial<SpolkaScreenProps>) {
      const spolkaTool = useSpolkaToolHost();
      return (
        <ToastProvider>
      <ResearchProvider value={researchViewModelStub}>
          <SpolkaScreen {...baseProps({ ...overrides, spolkaTool })} />
        </ResearchProvider>
    </ToastProvider>
      );
    }
    const { rerender } = render(<Host refreshCompletionCount={0} />);
    await screen.findByText("CD Projekt");

    await user.click(screen.getByRole("button", { name: "Open notebook" }));
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    frame.dataset.toolInstanceMarker = "still-here";

    rerender(<Host refreshCompletionCount={1} />);

    await waitFor(() => expect(getCompanyViewMock).toHaveBeenCalledTimes(2));
    const frameAfter = screen.getByRole("group", { name: "Workshop tool" });
    expect(frameAfter).toBe(frame);
    expect(frameAfter.dataset.toolInstanceMarker).toBe("still-here");
  });

  it("claims counter drill lands on the claims tool", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");
    await user.click(screen.getByRole("button", { name: "Claims counter" }));
    expect((await screen.findByRole("group", { name: "Workshop tool" })).getAttribute("data-tool")).toBe("tezy");
  });

  it("short counter drill lands on akcjonariat's short section", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");
    await user.click(screen.getByRole("button", { name: "Shorts counter" }));
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    expect(frame.getAttribute("data-tool")).toBe("akcjonariat");
    expect(frame.querySelector('[data-section="shorts"]')).not.toBeNull();
  });

  it("events tool lists upcoming events chronologically", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_company_events") {
        return Promise.resolve([
          { id: "e2", companyId: company.id, company: company.qualifiedTicker, companyName: company.displayName, eventType: "general_meeting", title: "Later event", eventDate: "2026-09-15", eventTime: null, status: "scheduled", sourceType: "manual", sourceAdapterId: null, sourceEventKey: null, sourceUrl: null, attribution: null, fetchedAt: null, manual: true, createdAt: "", updatedAt: "" },
          { id: "e1", companyId: company.id, company: company.qualifiedTicker, companyName: company.displayName, eventType: "general_meeting", title: "Earlier event", eventDate: "2026-09-01", eventTime: null, status: "scheduled", sourceType: "manual", sourceAdapterId: null, sourceEventKey: null, sourceUrl: null, attribution: null, fetchedAt: null, manual: true, createdAt: "", updatedAt: "" },
        ]);
      }
      return Promise.resolve([]);
    });
    const user = userEvent.setup();
    render(<Harness />);
    await screen.findByText("CD Projekt");
    await user.click(screen.getByRole("button", { name: "Events counter" }));
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    await within(frame).findByText("Earlier event");
    const text = frame.textContent ?? "";
    expect(text.indexOf("Earlier event")).toBeLessThan(text.indexOf("Later event"));
  });
});
