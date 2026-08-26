import { describe, it, vi } from "vitest";
import {
  expect,
  renderApp,
  renderAppDefaultShell,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";
import { DockviewComponent } from "dockview";
import { parsePanels } from "./CockpitScreen";
import { saveCockpitLayout } from "../../api/cockpit";

// F3a S3 (ADR 0107 decision 5): freeform layout STRUCTURE is frozen — every
// command that adds/removes/reorders a panel, applies a preset, saves a
// layout, or creates a view is gone. Domain editing INSIDE an already-open
// panel (facts, claims, notes, journal, quality) stays fully writable; the
// linked selection workflow (feed → inspector → claims/diff) is not a
// structure mutation and keeps working.
describe("Research cockpit shell — frozen (F3a S3, ADR 0107 decision 5)", () => {
  it("is not the default shell and has no standalone blank nav entry (ADR 0057 decision 5)", async () => {
    // Today/Pulse is the landing home; the cockpit is reached only via a saved
    // named view or a legacy dashboard row — there is no standalone
    // blank-canvas "Cockpit" sidebar button.
    renderAppDefaultShell();
    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Research cockpit")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cockpit" })).not.toBeInTheDocument();
  });

  it("renders the frozen-layout strip and carries no structure-mutating toolbar control", async () => {
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    expect(within(cockpit).getByText("Layout frozen until the engine decision")).toBeInTheDocument();
    expect(within(cockpit).getByText("Edit inside panels still saves.")).toBeInTheDocument();

    // Every structure-mutating command is gone: Add panel, Reset layout, Save
    // dashboard, the Preset select. Only "Commands" (⌘K, navigation-only) stays.
    expect(within(cockpit).queryByRole("button", { name: "Add panel" })).not.toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: "Reset layout" })).not.toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: "Save dashboard" })).not.toBeInTheDocument();
    expect(within(cockpit).queryByLabelText("Preset")).not.toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: /Commands/ })).toBeInTheDocument();
  });

  it("hides the tab close (×) affordance and disables drag/drop — the dock is read-only arrangement", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    const inspectorTab = await within(cockpit).findByRole("button", { name: "Inspector" });
    expect(within(cockpit).queryByRole("button", { name: "Close Inspector" })).not.toBeInTheDocument();

    // Alt+W (the DockLayout keyboard close model) must be a no-op while frozen.
    await user.click(inspectorTab);
    await user.keyboard("{Alt>}w{/Alt}");
    await waitFor(() => {
      expect(within(cockpit).getByRole("button", { name: "Inspector" })).toBeInTheDocument();
    });
  });

  it("opens the cockpit with accessible tabs", async () => {
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");
    const tabs = await within(cockpit).findAllByRole("button");
    expect(tabs.length).toBeGreaterThan(0);
  });

  it("opens a company directly into its curated dashboard as FOLLOW panels with unprefixed titles (U-Ra)", async () => {
    const user = userEvent.setup();
    renderApp();

    // The sample scenario seeds a `dashboard:` layout for CDR, so its "Legacy
    // dashboard · CDR" Widoki row reaches the cockpit scoped to that company.
    await user.click(await screen.findByRole("button", { name: "Legacy dashboard · CDR" }));

    const cockpit = await screen.findByLabelText("Research cockpit");
    // The dashboard renders company-scoped FOLLOW panels: their tab titles are
    // kind-only (D4), NOT prefixed with the ticker, and the linked Inspector
    // stays closed. Legacy-dashboard views are fixed to their company (F3a
    // S3): no "View company" selector — the row already chose it.
    expect(await within(cockpit).findByRole("button", { name: "Fundamentals" })).toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: "Notebook" })).toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: /GPW:CDR · Fundamentals/ })).toBeNull();
    expect(within(cockpit).queryByLabelText("View company")).not.toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: "Inspector" })).not.toBeInTheDocument();
    expect(cockpit).toHaveAttribute("data-company-id", "company_gpw_cdr");

    // The curated dashboard's Fundamentals FOLLOW panel renders the real
    // editable panel (period/fact forms), not a read-only matrix — proving
    // domain editing inside a frozen view's panels stays fully writable.
    await user.click(within(cockpit).getByRole("button", { name: "Fundamentals" }));
    expect(
      await within(cockpit).findByRole("heading", { name: "New reporting period" }),
    ).toBeInTheDocument();
  });

  // F3a S3 addenda (26.08): red left by S3a — the claims seam no longer routes
  // through the cockpit at all (ADR 0107 decision 2 mapping
  // "Claims/highlightClaimId→{t:'tezy', claimId}"). "Open thesis" lands
  // directly on the Spółka screen's claims tool with the claim highlighted.
  it("Today's 'Open thesis' lands on Spółka's claims tool with the claim highlighted (F3a S1, ADR 0107)", async () => {
    // jsdom has no `scrollIntoView` (CompanyClaimsPanel's own highlight-scroll
    // effect calls it once a real highlighted claim row exists).
    Element.prototype.scrollIntoView = vi.fn();
    const user = userEvent.setup();
    renderAppDefaultShell();

    await screen.findByRole("heading", { name: "Today" });
    const [openThesis] = await screen.findAllByRole("button", { name: "Open thesis" });
    await user.click(openThesis);

    const company = await screen.findByLabelText("Company view");
    const tool = await within(company).findByRole("group", { name: "Workshop tool" });
    expect(tool).toHaveAttribute("data-tool", "tezy");
    await waitFor(() => expect(tool.querySelector(".claim-row-highlighted")).not.toBeNull());
    expect(screen.queryByLabelText("Research cockpit")).not.toBeInTheDocument();
  });

  it("filters the feed panel by ticker or title (cockpit-native, phase 4a)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // The Feed panel is open in the default linked layout; both sample items are
    // listed until the filter narrows them.
    expect(
      await within(cockpit).findByRole("button", {
        name: "Inspect feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();

    await user.type(await within(cockpit).findByLabelText("Filter feed items"), "KGH");

    await waitFor(() => {
      expect(
        within(cockpit).queryByRole("button", {
          name: "Inspect feed item: Current report placeholder for watchlist company",
        }),
      ).toBeNull();
    });
    expect(
      within(cockpit).getByRole("button", {
        name: "Inspect feed item: Transcript-derived note candidate waits for future provider work",
      }),
    ).toBeInTheDocument();
  });

  // F3a S3 addenda (26.08): red left by S3a — "Load layout: …" is gone; a
  // named view is now a navigation-only palette entry.
  it("offers a saved layout as a navigation-only 'Open view: …' palette entry", async () => {
    const user = userEvent.setup();
    // View creation ("+ New view") no longer exists in the UI (frozen) — seed a
    // named view through the same command the removed creator used to call,
    // matching a pre-freeze user's existing saved view.
    await saveCockpitLayout({
      name: "Deep dive",
      panelsJson: JSON.stringify({
        pinned: [],
        openGlobals: [],
        closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
        selectedFeedItemId: null,
        grid: null,
        cells: null,
        viewCompanyId: null,
      }),
      layoutJson: null,
      dockviewVersion: null,
    });

    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Deep dive");
    expect(await screen.findByRole("button", { name: "Open view: Deep dive" })).toBeInTheDocument();
    // Navigation-only: the removed "Load layout"/"Apply preset" wording never
    // appears again.
    expect(screen.queryByRole("button", { name: /Load layout/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Apply preset/ })).toBeNull();
  });

  // Plan § "Defekty mechaniczne… inline": a rejected saved geometry (version
  // mismatch or corrupt JSON) is now surfaced, never a silent catch.
  it("shows the restored-default note when a saved layout's geometry is from another dockview version", async () => {
    const user = userEvent.setup();
    await saveCockpitLayout({
      name: "Deep dive",
      panelsJson: JSON.stringify({
        pinned: [],
        openGlobals: [],
        closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
        selectedFeedItemId: null,
        grid: null,
        cells: null,
        viewCompanyId: null,
      }),
      layoutJson: JSON.stringify({ grid: { root: { type: "leaf", data: { id: "g", views: [] } } } }),
      dockviewVersion: "0.0.0-old",
    });

    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");
    // Navigate to the named view via the palette (its own dockviewVersion
    // mismatches the running build's — the geometry replay is rejected).
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Deep dive");
    await user.click(await screen.findByRole("button", { name: "Open view: Deep dive" }));

    expect(
      await within(await screen.findByLabelText("Research cockpit")).findByText(
        "Saved layout comes from another version — default layout restored",
      ),
    ).toBeInTheDocument();
  });
});

// U-Ra (ADR 0076): a NAMED cockpit view carries one "view company"; a legacy
// dashboard is fixed to the row's company instead (no selector — see the
// "curated dashboard" test above). Company-scoped panels follow the view
// company by default and retarget in place on switch; a single panel may pin
// a different company. Retargeting a named view's company is not a structure
// mutation (nothing is added/removed/saved), so the selector stays for named
// views — this is also the regression harness for #348 (declarative dock
// reconciliation).
describe("Research cockpit — view company context (U-Ra)", () => {
  // A follow `companyFeed` panel can no longer be added interactively (the
  // "Add panel"/"Open panel" surface is gone with the freeze) — seed a named
  // view carrying one directly through the API the removed add-panel flow
  // used to call, then open it via the navigation-only palette.
  async function openCdrView(user: ReturnType<typeof userEvent.setup>) {
    await saveCockpitLayout({
      name: "CDR follow-up",
      panelsJson: JSON.stringify({
        pinned: [{ id: "follow:companyFeed", kind: "companyFeed", mode: "follow" }],
        openGlobals: [],
        closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
        selectedFeedItemId: null,
        grid: null,
        cells: null,
        viewCompanyId: "company_gpw_cdr",
      }),
      layoutJson: null,
      dockviewVersion: null,
    });
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "CDR follow-up");
    await user.click(await screen.findByRole("button", { name: "Open view: CDR follow-up" }));
    return screen.findByLabelText("Research cockpit");
  }

  it("retargets a FOLLOW panel's content in place when the view company changes", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrView(user);

    expect(
      await within(cockpit).findByRole("button", {
        name: "Open company feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: "Feed" })).toBeInTheDocument();

    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_kgh");

    expect(
      await within(cockpit).findByRole("button", {
        name: "Open company feed item: Transcript-derived note candidate waits for future provider work",
      }),
    ).toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: "Feed" })).toBeInTheDocument();
    await waitFor(() => {
      expect(
        within(cockpit).queryByRole("button", {
          name: "Open company feed item: Current report placeholder for watchlist company",
        }),
      ).toBeNull();
    });
  });

  it("keeps a PINNED panel frozen on its company across a view-company switch", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrView(user);

    const feedTab = (await within(cockpit).findByRole("button", { name: "Feed" })).closest(
      ".cockpit-tab",
    ) as HTMLElement;
    await user.click(within(feedTab).getByRole("button", { name: "Pin company" }));
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_kgh");
    await waitFor(() => {
      expect(within(cockpit).getByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();
    });
    expect(
      within(cockpit).getByRole("button", {
        name: "Open company feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
  });

  it("pins a follow panel to the current company and rejoins the view company", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrView(user);

    const feedTab = (await within(cockpit).findByRole("button", { name: "Feed" })).closest(
      ".cockpit-tab",
    ) as HTMLElement;
    await user.click(within(feedTab).getByRole("button", { name: "Pin company" }));
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_kgh");
    await waitFor(() => {
      expect(within(cockpit).getByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();
    });

    await user.click(within(cockpit).getByRole("button", { name: "Follow view company" }));
    expect(await within(cockpit).findByRole("button", { name: "Feed" })).toBeInTheDocument();
    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "GPW:CDR · Feed" })).toBeNull();
    });
    expect(
      await within(cockpit).findByRole("button", {
        name: "Open company feed item: Transcript-derived note candidate waits for future provider work",
      }),
    ).toBeInTheDocument();
  });

  it("recovers when dockview refuses a panel removal — the dock remounts from the specs (#348)", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrView(user);
    const feedTab = (await within(cockpit).findByRole("button", { name: "Feed" })).closest(
      ".cockpit-tab",
    ) as HTMLElement;
    await user.click(within(feedTab).getByRole("button", { name: "Pin company" }));
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    const spy = vi
      .spyOn(DockviewComponent.prototype, "removePanel")
      .mockImplementationOnce(() => {
        throw new Error("invalid operation");
      });
    try {
      await user.click(within(cockpit).getByRole("button", { name: "Follow view company" }));
      expect(await within(cockpit).findByRole("button", { name: "Feed" })).toBeInTheDocument();
      await waitFor(() => {
        expect(within(cockpit).queryByRole("button", { name: "GPW:CDR · Feed" })).toBeNull();
      });
      expect(spy).toHaveBeenCalled();
    } finally {
      spy.mockRestore();
    }
  });

  it("removes a DOM ghost tab the dock's own model no longer knows about (#348)", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrView(user);
    const feedTab = (await within(cockpit).findByRole("button", { name: "Feed" })).closest(
      ".cockpit-tab",
    ) as HTMLElement;
    await user.click(within(feedTab).getByRole("button", { name: "Pin company" }));
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    function injectGhost(name: string) {
      const tabsList = cockpit.querySelector(".dv-tabs-container");
      expect(tabsList).not.toBeNull();
      const ghost = document.createElement("span");
      ghost.className = "cockpit-tab";
      ghost.setAttribute("data-panel-id", `ghost:${name}`);
      const ghostButton = document.createElement("button");
      ghostButton.type = "button";
      ghostButton.textContent = name;
      ghost.appendChild(ghostButton);
      (tabsList as Element).appendChild(ghost);
      expect(within(cockpit).getByRole("button", { name })).toBeInTheDocument();
    }
    injectGhost("GPW:GHOST · Feed");

    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_kgh");
    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "GPW:GHOST · Feed" })).toBeNull();
    });
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    injectGhost("GPW:GHOST2 · Feed");
    await user.click(within(cockpit).getByRole("button", { name: "Follow view company" }));
    expect(await within(cockpit).findByRole("button", { name: "Feed" })).toBeInTheDocument();
    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "GPW:GHOST2 · Feed" })).toBeNull();
    });
    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "GPW:CDR · Feed" })).toBeNull();
    });
  });

  it("offers no per-company palette entries — the header selector is the one way to retarget", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Switch view company");
    expect(screen.queryAllByRole("button", { name: /Switch view company/ })).toHaveLength(0);
    expect(within(cockpit).getByLabelText("View company")).toBeInTheDocument();
  });

  it("styles the palette search through the design system (search-box), not a bare input", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    expect(
      (await within(cockpit).findByLabelText("Filter feed items")).closest(".search-box"),
    ).not.toBeNull();
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    const input = await screen.findByLabelText("Search commands");
    expect(input.closest(".search-box")).not.toBeNull();
  });

  // Plan § "Kokpit freeform zostaje... ZAMROŻONY": the linked selection
  // workflow (feed → inspector → claims/diff) is NOT a structure mutation —
  // it must keep working in a frozen view.
  it("linked selection still drives inspector, claims and diff selection in a frozen view", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    const feedItem = await within(cockpit).findByRole("button", {
      name: "Inspect feed item: Current report placeholder for watchlist company",
    });
    await user.click(feedItem);

    expect(
      await within(cockpit).findByLabelText("Feed item inspector"),
    ).toHaveTextContent("Current report placeholder for watchlist company");
    expect(
      within(cockpit).getByRole("button", { name: /Claims · GPW:CDR/ }),
    ).toBeInTheDocument();
    expect(
      within(cockpit).getByRole("button", { name: /Report comparison · GPW:CDR/ }),
    ).toBeInTheDocument();
  });
});

describe("parsePanels (U-Ra persistence)", () => {
  it("round-trips viewCompanyId and per-panel modes", () => {
    const descriptor = {
      pinned: [
        { id: "follow:fundamentals", kind: "fundamentals", mode: "follow" },
        {
          id: "claims:company_gpw_cdr",
          kind: "claims",
          mode: "pinned",
          companyId: "company_gpw_cdr",
        },
      ],
      openGlobals: [],
      closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
      selectedFeedItemId: null,
      grid: null,
      cells: null,
      viewCompanyId: "company_gpw_kgh",
    };
    const parsed = parsePanels(JSON.stringify(descriptor));
    expect(parsed).not.toBeNull();
    expect(parsed?.viewCompanyId).toBe("company_gpw_kgh");
    expect(parsed?.pinned).toEqual([
      { id: "follow:fundamentals", kind: "fundamentals", mode: "follow" },
      {
        id: "claims:company_gpw_cdr",
        kind: "claims",
        mode: "pinned",
        companyId: "company_gpw_cdr",
      },
    ]);
  });

  it("parses a legacy descriptor (no mode / no viewCompanyId) as all-pinned with null view company", () => {
    const legacy = JSON.stringify({
      pinned: [{ id: "fundamentals:company_gpw_cdr", kind: "fundamentals", companyId: "company_gpw_cdr" }],
      openGlobals: [],
      closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
      selectedFeedItemId: null,
    });
    const parsed = parsePanels(legacy);
    expect(parsed?.viewCompanyId).toBeNull();
    expect(parsed?.pinned).toEqual([
      {
        id: "fundamentals:company_gpw_cdr",
        kind: "fundamentals",
        mode: "pinned",
        companyId: "company_gpw_cdr",
      },
    ]);
  });
});
