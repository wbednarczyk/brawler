import { fireEvent } from "@testing-library/react";
import { beforeEach, describe, it, vi } from "vitest";
import packageJson from "../package.json";
import {
  appTestState,
  expect,
  invoke,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "./test/appWorkflowHarness";

describe("Sidebar IA spine (ADR 0054)", () => {
  it("groups navigation into modes, library and utilities with mode destinations", async () => {
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    expect(within(nav).getByText("Modes")).toBeInTheDocument();
    expect(within(nav).getByText("Library")).toBeInTheDocument();
    expect(within(nav).getByText("Utilities")).toBeInTheDocument();
    // /^Today/: the ambient-attention badge (ADR 0097) joins the accessible
    // name when unseen events exist — same idiom as the Inbox unread badge.
    expect(within(nav).getByRole("button", { name: /^Today/ })).toBeInTheDocument();
  });

  it("opens the Today mode home", async () => {
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: /^Today/ }));
    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();
  });

  it("lists pinned companies in the spine and opens the company workspace", async () => {
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    expect(within(nav).getByText("Pinned companies")).toBeInTheDocument();
    const pinned = within(nav).getByRole("button", { name: "CDR" });
    await userEvent.click(pinned);

    // Opening a pinned company lands the Spółka screen (F3a S1, ADR 0107).
    expect(await screen.findByRole("region", { name: "Company view" })).toBeInTheDocument();
  });

  it("unpins a company from the spine and persists via update_settings", async () => {
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    const unpin = within(nav).getByRole("button", { name: /Unpin from sidebar/ });
    await userEvent.click(unpin);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "update_settings",
        expect.objectContaining({ input: expect.objectContaining({ pinnedCompanyIds: [] }) }),
      );
    });
  });

});

describe("App shell", () => {
  it("renders the investor inbox shell", async () => {
    renderApp();

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(
      (await screen.findAllByText("Current report placeholder for watchlist company")).length,
    ).toBeGreaterThan(0);
    expect(within(screen.getByLabelText("Feed items")).getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getAllByText("Current report placeholder for watchlist company").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText("Sample official report used to validate feed filtering and detail rendering.").length,
    ).toBeGreaterThan(0);
    // The brand chip shows the health-reported version; the mock mirrors
    // package.json so this can never rot behind the real app (audit K12).
    expect(await screen.findByText(`v${packageJson.version}`)).toBeInTheDocument();
    expect(screen.queryByText(`ok ${packageJson.version}`)).not.toBeInTheDocument();
    expect(screen.queryByText("AI")).not.toBeInTheDocument();
    expect(screen.queryByText("Data")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open source status" })).toBeInTheDocument();
  });

  it("shows unread feed count in the Inbox navigation item", async () => {
    renderApp();

    const inboxNav = await screen.findByRole("button", { name: /Inbox/ });

    expect(within(inboxNav).getByText("1")).toHaveClass("nav-badge");
    expect(within(inboxNav).getByLabelText("1 unread feed item")).toBeInTheDocument();
  });

  it("shows the unseen-attention badge on the Today navigation item (ADR 0097 dec. 4)", async () => {
    // The sample dataset carries exactly one unseen non-routine attention event.
    renderApp();

    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    expect(await within(todayNav).findByText("1")).toHaveClass("nav-badge");
    expect(within(todayNav).getByLabelText("1 new important item in Today")).toBeInTheDocument();
  });

  it("clears the Today badge on a visit — seen means 'was on screen' (ADR 0097 dec. 5)", async () => {
    renderApp();

    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    await within(todayNav).findByText("1");

    await userEvent.click(todayNav);
    await screen.findByRole("heading", { name: "Today" });

    // Today's stream batch-marks the loaded unseen events; the optimistic flip
    // empties the badge without waiting for a refetch.
    await waitFor(() => expect(within(todayNav).queryByText("1")).toBeNull());
    expect(invoke).toHaveBeenCalledWith(
      "mark_attention_events_seen",
      expect.objectContaining({ input: expect.objectContaining({ ids: expect.any(Array) }) }),
    );
  });

  it("does not count routine attention events toward the Today badge", async () => {
    appTestState.attentionEventsResponse = appTestState.attentionEventsResponse.map((event) => ({
      ...event,
      severity: "routine" as const,
    }));
    renderApp();

    const todayNav = await screen.findByRole("button", { name: "Today" });
    expect(within(todayNav).queryByText(/^\d+$/)).toBeNull();
  });

  it("announces a count INCREASE politely, but never replays the startup backlog (ADR 0097 dec. 4)", async () => {
    const { container } = renderApp();

    // Hydration: the backlog (1 unseen event) lights the badge…
    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    await within(todayNav).findByText("1");
    const liveRegion = container.querySelector('[aria-live="polite"]') as HTMLElement;
    expect(liveRegion).not.toBeNull();
    // …but is NOT announced.
    expect(liveRegion).toHaveTextContent("");

    // A refresh brings two MORE unseen events (3 total) — one coalesced polite
    // announcement states the new count.
    appTestState.attentionEventsResponse = [
      ...appTestState.attentionEventsResponse,
      { ...appTestState.attentionEventsResponse[0], id: "attn_live_2" },
      { ...appTestState.attentionEventsResponse[0], id: "attn_live_3" },
    ];
    await userEvent.click(screen.getByRole("button", { name: "Refresh sources" }));

    await waitFor(() =>
      expect(liveRegion).toHaveTextContent("3 new important items in Today"),
    );
  });

  it("clears the announcement on a decrease so a repeat cycle re-announces (0→1→0→1)", async () => {
    const { container } = renderApp();
    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    await within(todayNav).findByText("1");
    const liveRegion = container.querySelector('[aria-live="polite"]') as HTMLElement;

    // The refresh control's accessible name tracks its state ("Refreshing…",
    // "Sources refreshed") — capture the node once and click it by reference.
    const refreshButton = screen.getByRole("button", { name: "Refresh sources" });

    // Cycle 1: +1 event (1 → 2) — announced.
    const base = appTestState.attentionEventsResponse[0];
    appTestState.attentionEventsResponse = [base, { ...base, id: "attn_cycle_2" }];
    await userEvent.click(refreshButton);
    await waitFor(() => expect(liveRegion).toHaveTextContent("2 new important items in Today"));

    // Visiting Today clears the count — the region must EMPTY (a later identical
    // announcement needs a real DOM mutation, or aria-live stays silent).
    await userEvent.click(todayNav);
    await screen.findByRole("heading", { name: "Today" });
    await waitFor(() => expect(liveRegion).toHaveTextContent(""));

    // Cycle 2: two fresh events arrive (0 → 2) — the SAME sentence re-announces.
    appTestState.attentionEventsResponse = [
      { ...base, id: "attn_cycle_3" },
      { ...base, id: "attn_cycle_4" },
    ];
    await userEvent.click(refreshButton);
    await waitFor(() => expect(liveRegion).toHaveTextContent("2 new important items in Today"));
  });

  it("shows Diagnostics navigation only in Developer mode", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Diagnostics" })).not.toBeInTheDocument();

    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };

    renderApp();

    expect(await screen.findByRole("button", { name: "Diagnostics" })).toBeInTheDocument();
    const navContainers = screen.getAllByLabelText("Primary navigation");
    const currentNav = navContainers[navContainers.length - 1];
    // Diagnostics is the developer-gated utility in the sidebar spine (ADR 0054).
    expect(within(currentNav).getByRole("button", { name: "Diagnostics" })).toBeInTheDocument();
  });

  it("shows Developer-mode local metrics in Diagnostics", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };

    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Diagnostics" }));

    expect(await screen.findByRole("heading", { name: "Metrics" })).toBeInTheDocument();
    expect(await screen.findByText("brawler_source_refresh_total")).toBeInTheDocument();
    expect(screen.getByText("adapter_id=bankier-company-komunikaty · status=succeeded")).toBeInTheDocument();
    expect(screen.getByText("512 KiB")).toBeInTheDocument();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_local_metrics_snapshot");
    });
  });

  it("registers app-level shortcuts and suppresses them while searching", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "2", code: "Digit2", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Companies" })).toBeInTheDocument();

    // Ctrl+F focuses global search without leaving the current section.
    fireEvent.keyDown(document, { key: "F", code: "KeyF", ctrlKey: true });
    const searchInput = screen.getByLabelText("Global search");
    await waitFor(() => expect(searchInput).toHaveFocus());
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();

    fireEvent.keyDown(searchInput, { key: "3", code: "Digit3", ctrlKey: true });
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();

    searchInput.blur();
    fireEvent.keyDown(document, { key: "8", code: "Digit8", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });

  it("opens the global command palette on Ctrl+K", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();

    // Ctrl+K opens the global command palette (v0.50 U6), not focusing search.
    fireEvent.keyDown(document, { key: "K", code: "KeyK", ctrlKey: true });
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    // App-level commands (derived from the shortcut registry) are listed.
    expect(within(palette).getByRole("option", { name: "Open Settings" })).toBeInTheDocument();

    // Running a listed command navigates and closes the palette.
    await userEvent.click(within(palette).getByRole("option", { name: "Open Settings" }));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
  });

  it("also opens the command palette on Meta+K (macOS ⌘K twin)", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "K", code: "KeyK", metaKey: true });
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
  });

  it("uses configured shortcut bindings from settings", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      shortcutBindings: {
        "app.openCompanies": {
          key: "C",
          ctrlKey: true,
        },
        "app.openInbox": {
          key: "1",
          ctrlKey: true,
          disabled: true,
        },
      },
    };

    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_settings");
    });

    fireEvent.keyDown(document, { key: "C", code: "KeyC", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Companies" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "1", code: "Digit1", ctrlKey: true });
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();
  });
});

// sol R1 finding 3: every company-workspace entry point (openCompanyWorkspaceById,
// openCompanyWorkspace, openCompanyClaims, pinned row, GlobalSearch, palette
// "Open company:", Today "Open thesis") commits companyId + section + tool as
// ONE atomic transition behind ONE dirty guard — `selectedCompanyId` never
// changes ahead of it. Minimal scenario: CDR is pinned by default; KGH/PKN
// carry a "to verify" management claim (overdue/due respectively).
describe("Spółka atomic company transitions (sol R1 finding 3)", () => {
  // jsdom does not implement `scrollIntoView` — the documents panel's
  // provenance-highlight effect calls it unguarded (unlike the akcjonariat
  // tool's `?.()`-guarded call), which otherwise throws mid-render and can
  // leave a pending timer bleeding into a later test in this file.
  beforeEach(() => {
    if (!Element.prototype.scrollIntoView) {
      Element.prototype.scrollIntoView = () => {};
    }
  });

  function claimRowFor(ticker: string): HTMLElement {
    const row = Array.from(document.querySelectorAll('[data-dayq-row="true"]')).find((candidate) =>
      candidate.textContent?.includes(ticker),
    );
    if (!row) throw new Error(`No "to verify" claim row for ${ticker}`);
    return row as HTMLElement;
  }

  async function openDirtyNotebookOnSpolka(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole("button", { name: "Notebook" }));
    await screen.findByRole("group", { name: "Workshop tool" });
    await user.click(await screen.findByRole("button", { name: "New note" }));
    const titleField = await screen.findByRole("textbox", { name: "Notebook note title" });
    await user.type(titleField, "Draft in progress");
    return titleField;
  }

  it("clean openCompanyClaims(B) lands on B's claims tool and stays there", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByRole("button", { name: /^Today/ }));
    await screen.findByRole("heading", { name: "Today" });

    const row = claimRowFor("KGH");
    await user.click(within(row).getByRole("button", { name: "Open thesis" }));

    const spolka = await screen.findByRole("region", { name: "Company view" });
    await waitFor(() => {
      expect(spolka).toHaveAttribute("data-company-id", "company_gpw_kgh");
    });
    expect(await within(spolka).findByRole("group", { name: "Workshop tool" })).toHaveAttribute(
      "data-tool",
      "tezy",
    );

    // The clean transition is not reverted by the displayed-company sync
    // effect racing behind it (the exact "openCompanyClaims(B) opens B's
    // tool, then the sync effect clears it back to B's core" blocker) — give
    // it another tick and confirm it's still there.
    await waitFor(() => {
      expect(within(spolka).getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "tezy");
    });
  });

  it("dirty A + Stay keeps A selected everywhere (aria-current) and the tool open", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      pinnedCompanyIds: ["company_gpw_cdr", "company_gpw_kgh"],
    };
    const user = userEvent.setup();
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "CDR" }));
    await screen.findByRole("region", { name: "Company view" });
    await openDirtyNotebookOnSpolka(user);

    await user.click(within(nav).getByRole("button", { name: "KGH" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Stay" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "CDR" })).toHaveAttribute("aria-current", "page");
    expect(within(nav).getByRole("button", { name: "KGH" })).not.toHaveAttribute("aria-current");
    const tool = screen.getByRole("group", { name: "Workshop tool" });
    expect(tool).toHaveAttribute("data-tool", "notatnik");
    expect((screen.getByRole("textbox", { name: "Notebook note title" }) as HTMLInputElement).value).toBe(
      "Draft in progress",
    );
  });

  it("dirty A + Discard lands the full requested transition (company, section, tool)", async () => {
    const user = userEvent.setup();
    appTestState.searchResponse = {
      groups: [
        {
          contentType: "digest",
          matches: [
            {
              contentType: "digest",
              sourceId: "digest_kgh_1",
              companyId: "company_gpw_kgh",
              title: "KGHM digest",
              snippet: "KGHM digest",
              score: 1.0,
            },
          ],
        },
      ],
    };
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "CDR" }));
    await screen.findByRole("region", { name: "Company view" });
    await openDirtyNotebookOnSpolka(user);

    const input = screen.getByLabelText("Global search");
    await user.type(input, "kgh");
    await user.click(await screen.findByRole("option", { name: /KGHM digest/ }));

    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Discard" }));

    const spolka = await screen.findByRole("region", { name: "Company view" });
    await waitFor(() => expect(spolka).toHaveAttribute("data-company-id", "company_gpw_kgh"));
    expect(await within(spolka).findByRole("group", { name: "Workshop tool" })).toHaveAttribute(
      "data-tool",
      "research",
    );
  });

  it("a second transition request while the guard modal is open does not clobber the pending one", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      pinnedCompanyIds: ["company_gpw_cdr", "company_gpw_kgh", "company_gpw_pzu"],
    };
    const user = userEvent.setup();
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "CDR" }));
    await screen.findByRole("region", { name: "Company view" });
    await openDirtyNotebookOnSpolka(user);

    // Request #1: KGH — dirty guard opens the modal.
    await user.click(within(nav).getByRole("button", { name: "KGH" }));
    await screen.findByRole("dialog");

    // Request #2 while the modal is still open — must be IGNORED, not
    // overwrite request #1's pending transition.
    await user.click(within(nav).getByRole("button", { name: "PZU" }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Discard" }));

    const spolka = await screen.findByRole("region", { name: "Company view" });
    await waitFor(() => expect(spolka).toHaveAttribute("data-company-id", "company_gpw_kgh"));
  });

  // sol R1 finding 8: a KPI provenance ticket was rendered as a button but
  // wired to a no-op — clicking it did nothing in the real app.
  it("KPI ticket opens the documents tool with the ticket's document highlighted", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      pinnedCompanyIds: ["company_gpw_kgh"],
    };
    const user = userEvent.setup();
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "KGH" }));
    const spolka = await screen.findByRole("region", { name: "Company view" });

    const ticket = await within(spolka).findByRole("button", { name: "Open source document" });
    const ticketRef = ticket.textContent;
    await user.click(ticket);

    const tool = await within(spolka).findByRole("group", { name: "Workshop tool" });
    expect(tool).toHaveAttribute("data-tool", "dokumenty");
    await waitFor(() => {
      expect(tool.querySelector(`[data-document-id="${ticketRef}"]`)).toHaveAttribute(
        "data-document-highlighted",
        "true",
      );
    });
  });

  it("hosted Research tool loads its data like the standalone route (#450)", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      pinnedCompanyIds: ["company_gpw_kgh"],
    };
    const user = userEvent.setup();
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "KGH" }));
    const spolka = await screen.findByRole("region", { name: "Company view" });
    (invoke as ReturnType<typeof vi.fn>).mockClear();

    await user.click(within(spolka).getByRole("button", { name: "Research" }));
    expect(await within(spolka).findByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "research");

    // The refresh effects fire on "Research is mounted", not on the section.
    await waitFor(() => {
      for (const command of ["list_research_questions", "list_research_reminders", "list_research_evidence"]) {
        expect(invoke).toHaveBeenCalledWith(command, expect.anything());
      }
    });
  });
});

// F3c S1 (plan § Design 5): Ctrl+. jumps to the workshop bar's current tab
// stop; H/L cycle the workshop tools (wrap, Overview included); Shift+J/K
// move to the adjacent company (ADR 0107 dec. 6 — no tool retarget).
describe("Spółka keyboard shortcuts (F3c S1)", () => {
  async function openSpolka(user: ReturnType<typeof userEvent.setup>) {
    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "CDR" }));
    return screen.findByRole("region", { name: "Company view" });
  }

  it("Ctrl+. focuses the workshop bar's tab stop on Spółka", async () => {
    const user = userEvent.setup();
    renderApp();
    const spolka = await openSpolka(user);

    fireEvent.keyDown(document, { key: ".", ctrlKey: true });
    expect(within(spolka).getByRole("button", { name: "Overview" })).toHaveFocus();
  });

  it("Ctrl+. works while focus sits in the company picker (a company switch lands there)", async () => {
    // J8 in the real browser: `Open company: X` puts focus on the company
    // picker `<select>` (intent `company`), and an editable-suppressed
    // shortcut would then ignore Ctrl+. — the chord has no editing meaning.
    const user = userEvent.setup();
    renderApp();
    const spolka = await openSpolka(user);
    const picker = within(spolka).getByRole("combobox", { name: "Company" });
    picker.focus();
    expect(picker).toHaveFocus();

    fireEvent.keyDown(picker, { key: ".", ctrlKey: true });
    expect(within(spolka).getByRole("button", { name: "Overview" })).toHaveFocus();
  });

  it("Ctrl+K opens the palette while focus sits in the company picker", async () => {
    // Same class as Ctrl+. above: the pinned-renderer visual specs open a
    // company (focus → picker) and then press Ctrl+K — a chord with no
    // editing meaning must not be swallowed by an editable target.
    const user = userEvent.setup();
    renderApp();
    const spolka = await openSpolka(user);
    const picker = within(spolka).getByRole("combobox", { name: "Company" });
    picker.focus();

    fireEvent.keyDown(picker, { key: "K", code: "KeyK", ctrlKey: true });
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
  });

  it("Ctrl+. is a no-op off Spółka (no workshop bar mounted)", async () => {
    renderApp();
    await screen.findByRole("heading", { name: "Inbox" });

    fireEvent.keyDown(document, { key: ".", ctrlKey: true });
    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
  });

  it("Ctrl+. is a no-op while a modal is open (never steals focus to the background bar)", async () => {
    const user = userEvent.setup();
    renderApp();
    const spolka = await openSpolka(user);
    const overview = within(within(spolka).getByRole("toolbar", { name: "Workshop" })).getByRole("button", { name: "Overview" });

    fireEvent.keyDown(document, { key: "K", code: "KeyK", ctrlKey: true });
    await screen.findByRole("dialog", { name: "Command palette" });
    // Whatever the palette's own initial focus is (its `autoFocus` input stays
    // a known S2 follow-up — contract § item 5), Ctrl+. must not move it to
    // the background workshop bar while the modal is open.
    const focusedInModal = document.activeElement;

    fireEvent.keyDown(document, { key: ".", ctrlKey: true });
    expect(document.activeElement).toBe(focusedInModal);
    expect(overview).not.toHaveFocus();
  });

  it("H/L cycle workshop tools, wrapping to Overview, focusing the Overview entry", async () => {
    const user = userEvent.setup();
    renderApp();
    const spolka = await openSpolka(user);
    const bar = within(spolka).getByRole("toolbar", { name: "Workshop" });

    fireEvent.keyDown(document, { key: "L", code: "KeyL" });
    await waitFor(() => expect(spolka).toContainElement(screen.getByRole("group", { name: "Workshop tool" })));
    const firstTool = screen.getByRole("group", { name: "Workshop tool" }).getAttribute("data-tool");
    expect(firstTool).toBe("fundamenty");

    fireEvent.keyDown(document, { key: "H", code: "KeyH" });
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(within(bar).getByRole("button", { name: "Overview" })).toHaveFocus();
  });

  it("opening a company from the sidebar leaves the company picker unfocused (no ring for mouse users)", async () => {
    // Only the keyboard adjacent-company shortcuts carry the `company` focus
    // intent; a row/pinned click or a palette hop is `none` — Chromium shows
    // `:focus-visible` on any programmatically focused `<select>`, which put a
    // ring into the at-rest visual baseline (F3c integration).
    const user = userEvent.setup();
    renderApp();
    const spolka = await openSpolka(user);
    expect(within(spolka).getByRole("combobox", { name: "Company" })).not.toHaveFocus();
  });

  it("Shift+J/K move to the adjacent company on Spółka, closing the tool and focusing the company picker", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      pinnedCompanyIds: ["company_gpw_cdr", "company_gpw_kgh"],
    };
    const user = userEvent.setup();
    renderApp();
    const spolka = await openSpolka(user);
    const startingCompanyId = spolka.getAttribute("data-company-id");

    const bar = within(spolka).getByRole("toolbar", { name: "Workshop" });
    await user.click(within(bar).getByRole("button", { name: "Claims" }));
    await screen.findByRole("group", { name: "Workshop tool" });

    fireEvent.keyDown(document, { key: "J", code: "KeyJ", shiftKey: true });

    await waitFor(() => expect(screen.getByRole("region", { name: "Company view" }).getAttribute("data-company-id")).not.toBe(startingCompanyId));
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(screen.getByLabelText("Company")).toHaveFocus();
  });
});

