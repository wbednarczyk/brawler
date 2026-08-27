import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { SpolkaScreen, type SpolkaScreenProps } from "./SpolkaScreen";
import { useSpolkaToolHost } from "./ToolHost";
import { SpolkaScreenHost } from "../../app/useSpolkaScreenWiring";
import { ToastProvider } from "../../ui";
import { CommandPaletteProvider } from "../../app/commandPalette";
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
    companies: [company],
    spolkaTool: overrides.spolkaTool as SpolkaScreenProps["spolkaTool"],
    feedItems: [],
    rootHighlightClaimId: null,
    onOpenDocument: vi.fn(),
    onOpenFeedItem: vi.fn(),
    onSwitchCompany: vi.fn(),
    refreshCompletionCount: 0,
    ...overrides,
  };
}

function Harness(overrides: Partial<SpolkaScreenProps>) {
  const spolkaTool = useSpolkaToolHost();
  return (
    <CommandPaletteProvider appCommands={[]} text={(s) => s}>
      <ToastProvider>
        <ResearchProvider value={researchViewModelStub}>
          <SpolkaScreen {...baseProps({ ...overrides, spolkaTool })} />
        </ResearchProvider>
      </ToastProvider>
    </CommandPaletteProvider>
  );
}

async function openDirtyNotebookTool(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Notebook" }));
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
    if (command === "get_company_context") {
      return Promise.resolve({
        companyId: company.id,
        latestPeriodFacts: null,
        upcomingEvents: [],
        notebook: { count: 0, latestAt: null },
        claimsDue: { due: 0, overdue: 0 },
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

    await user.click(screen.getByRole("button", { name: "Notebook" }));
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

    await user.click(screen.getByRole("button", { name: "Claims" }));

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
        <CommandPaletteProvider appCommands={[]} text={(s) => s}>
          <ToastProvider>
            <ResearchProvider value={researchViewModelStub}>
              <SpolkaScreen {...baseProps({ spolkaTool })} />
            </ResearchProvider>
          </ToastProvider>
        </CommandPaletteProvider>
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
        <CommandPaletteProvider appCommands={[]} text={(s) => s}>
          <ToastProvider>
            <ResearchProvider value={researchViewModelStub}>
              <SpolkaScreenHost
                companies={[company, companyB]}
                selectedCompanyId={selectedCompanyId}
                spolkaTool={spolkaTool}
                feedItems={[]}
                rootHighlightClaimId={null}
                openInboxItem={() => {}}
                onSwitchCompany={() => {}}
                refreshCompletionCount={0}
              />
            </ResearchProvider>
          </ToastProvider>
        </CommandPaletteProvider>
      );
    }
    const { rerender } = render(<Host selectedCompanyId={company.id} />);
    await screen.findByText("CD Projekt");
    await user.click(screen.getByRole("button", { name: "Notebook" }));
    await screen.findByRole("group", { name: "Workshop tool" });

    rerender(<Host selectedCompanyId={companyB.id} />);

    await screen.findByText("PKN Orlen");
    expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument();
  });

  // Owner dogfooding v0.74, item 6: the header's company picker routes
  // through the SAME guarded transition as every other entry point
  // (`onSwitchCompany` → `spolkaTool.guardNavigation`, mirroring
  // `useSpolkaNavigate`'s `navigate` in the real app) — a dirty tool asks
  // stay/discard instead of silently switching company under it.
  it("company picker switches through the guarded transition", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockImplementation((id: string) =>
      Promise.resolve(
        id === companyB.id
          ? emptyView({ companyId: companyB.id, qualifiedTicker: companyB.qualifiedTicker, displayName: companyB.displayName })
          : emptyView(),
      ),
    );
    function Host() {
      const spolkaTool = useSpolkaToolHost();
      const [selectedCompanyId, setSelectedCompanyId] = useState(company.id);
      return (
        <CommandPaletteProvider appCommands={[]} text={(s) => s}>
          <ToastProvider>
            <ResearchProvider value={researchViewModelStub}>
              <SpolkaScreenHost
                companies={[company, companyB]}
                selectedCompanyId={selectedCompanyId}
                spolkaTool={spolkaTool}
                feedItems={[]}
                rootHighlightClaimId={null}
                openInboxItem={() => {}}
                onSwitchCompany={(id) => spolkaTool.guardNavigation(() => setSelectedCompanyId(id))}
                refreshCompletionCount={0}
              />
            </ResearchProvider>
          </ToastProvider>
        </CommandPaletteProvider>
      );
    }
    render(<Host />);
    await screen.findByText("CD Projekt");
    await openDirtyNotebookTool(user);

    await user.selectOptions(screen.getByLabelText("Company"), companyB.id);

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("CD Projekt")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Workshop tool" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Discard" }));
    await screen.findByText("PKN Orlen");
    expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument();
  });

  it("a late view response for A cannot reopen A's tool over B", async () => {
    getCompanyViewMock.mockResolvedValue(emptyView());
    const user = userEvent.setup();
    function Host(overrides: Partial<SpolkaScreenProps>) {
      const spolkaTool = useSpolkaToolHost();
      return (
        <CommandPaletteProvider appCommands={[]} text={(s) => s}>
          <ToastProvider>
            <ResearchProvider value={researchViewModelStub}>
              <SpolkaScreen {...baseProps({ ...overrides, spolkaTool })} />
            </ResearchProvider>
          </ToastProvider>
        </CommandPaletteProvider>
      );
    }
    const { rerender } = render(<Host />);
    await screen.findByText("CD Projekt");
    await user.click(screen.getByRole("button", { name: "Claims" }));
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

    // The element that actually scrolls (owner dogfooding v0.74, item 1):
    // `.spolka-layout` itself never scrolls any more.
    const layout = container.querySelector(".spolka-body-scroll") as HTMLElement;
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
        <CommandPaletteProvider appCommands={[]} text={(s) => s}>
          <ToastProvider>
            <ResearchProvider value={researchViewModelStub}>
              <SpolkaScreen {...baseProps({ ...overrides, spolkaTool })} />
            </ResearchProvider>
          </ToastProvider>
        </CommandPaletteProvider>
      );
    }
    const { rerender } = render(<Host refreshCompletionCount={0} />);
    await screen.findByText("CD Projekt");

    await user.click(screen.getByRole("button", { name: "Notebook" }));
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

// sol R1 finding 1: production hosts multiple draft-owning subforms under
// ONE tool (notebook/journal/claims composers, sector/IR-URL fields,
// ownership retyping) — the registry must be a keyed Set, not one
// overwriteable handle, or registering a second draft silently drops the
// first from the dirty check.
describe("Spółka tool host — multi-handle dirty registry (sol R1 finding 1)", () => {
  it.each([
    { aDirty: false, bDirty: false, expectDirty: false },
    { aDirty: true, bDirty: false, expectDirty: true },
    { aDirty: false, bDirty: true, expectDirty: true },
    { aDirty: true, bDirty: true, expectDirty: true },
  ])(
    "two registered handles (aDirty=$aDirty, bDirty=$bDirty) → isDirty() is $expectDirty",
    ({ aDirty, bDirty, expectDirty }) => {
      const { result } = renderHook(() => useSpolkaToolHost());

      act(() => {
        result.current.register({ isDirty: () => aDirty, discard: () => {} });
        result.current.register({ isDirty: () => bDirty, discard: () => {} });
      });

      expect(result.current.isDirty()).toBe(expectDirty);
    },
  );

  it("the guard covers a handle registered AFTER an earlier clean one, and discard clears every handle", () => {
    const { result } = renderHook(() => useSpolkaToolHost());
    const discardA = vi.fn();
    const discardB = vi.fn();

    act(() => {
      result.current.register({ isDirty: () => false, discard: discardA });
      result.current.register({ isDirty: () => true, discard: discardB });
    });

    expect(result.current.isDirty()).toBe(true);

    const next = vi.fn();
    act(() => {
      result.current.guardNavigation(next);
    });
    expect(result.current.confirming).toBe(true);
    expect(next).not.toHaveBeenCalled();

    act(() => {
      result.current.discardAndProceed();
    });

    expect(discardA).toHaveBeenCalledTimes(1);
    expect(discardB).toHaveBeenCalledTimes(1);
    expect(next).toHaveBeenCalledTimes(1);
    expect(result.current.isDirty()).toBe(false);
  });

  it("an unregistered handle no longer counts toward the dirty check", () => {
    const { result } = renderHook(() => useSpolkaToolHost());
    let unregisterDirty: () => void = () => {};

    act(() => {
      unregisterDirty = result.current.register({ isDirty: () => true, discard: () => {} });
    });
    expect(result.current.isDirty()).toBe(true);

    act(() => {
      unregisterDirty();
    });
    expect(result.current.isDirty()).toBe(false);
  });

  it("a second guard request while the modal is already open is ignored, not clobbered (sol R1 finding 3)", () => {
    const { result } = renderHook(() => useSpolkaToolHost());
    act(() => {
      result.current.register({ isDirty: () => true, discard: () => {} });
    });

    const firstNext = vi.fn();
    const secondNext = vi.fn();
    act(() => {
      result.current.guardNavigation(firstNext);
      result.current.guardNavigation(secondNext);
    });
    expect(result.current.confirming).toBe(true);

    act(() => {
      result.current.discardAndProceed();
    });

    expect(firstNext).toHaveBeenCalledTimes(1);
    expect(secondNext).not.toHaveBeenCalled();
  });
});
