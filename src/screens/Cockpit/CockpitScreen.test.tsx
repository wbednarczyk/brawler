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
import { COCKPIT_LAYOUT_STORAGE_KEY } from "./DockLayout";
import { listCockpitLayouts, saveCockpitLayout } from "../../api/cockpit";

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

  // Guardrail (ADR 0045 harvest) for R1 finding 4: the freeze must hold under
  // EVERY surviving control, not only the ones each individual behavioral
  // test happens to exercise. Panel ids, the live geometry cache
  // (`cockpit.layout.v2`) and the saved `cockpit_layouts` rows must all be
  // byte-identical before and after — on both a named view and a legacy
  // dashboard.
  it("frozen view: panel ids, groups, geometry and storage are unchanged after every available control and shortcut", async () => {
    const user = userEvent.setup();

    async function snapshot(cockpit: HTMLElement) {
      return {
        panelIds: Array.from(cockpit.querySelectorAll("[data-panel-id]"))
          .map((el) => el.getAttribute("data-panel-id"))
          .sort(),
        geometry: window.localStorage.getItem(COCKPIT_LAYOUT_STORAGE_KEY),
        layouts: await listCockpitLayouts(),
      };
    }

    async function exerciseEveryControl(cockpit: HTMLElement) {
      // Structural controls are gone, not just disabled — nothing to click.
      expect(cockpit.querySelectorAll(".cockpit-tab-close")).toHaveLength(0);
      expect(cockpit.querySelectorAll(".cockpit-tab-pin")).toHaveLength(0);
      expect(within(cockpit).queryByRole("button", { name: "Float panel group" })).not.toBeInTheDocument();

      // Activate every surviving tab.
      const tabs = Array.from(cockpit.querySelectorAll<HTMLButtonElement>(".cockpit-tab-activate"));
      for (const tab of tabs) {
        await user.click(tab);
      }

      // Maximize is view-only (nothing added/removed/saved) and stays — round-trip it.
      const maximizeButtons = await within(cockpit).findAllByRole("button", {
        name: "Maximize panel group",
      });
      await user.click(maximizeButtons[0]);
      await user.click(maximizeButtons[0]);

      // Keyboard model: navigation stays live; Alt+W (close) is a no-op while frozen.
      tabs[0]?.focus();
      await user.keyboard("{Alt>}{ArrowRight}{/Alt}");
      await user.keyboard("{Alt>}{ArrowLeft}{/Alt}");
      await user.keyboard("{Alt>}w{/Alt}");
      await user.keyboard("{Alt>}m{/Alt}");
      await user.keyboard("{Alt>}m{/Alt}");

      // Commands palette open/close — navigation-only, no mutation.
      await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
      await user.keyboard("{Escape}");
    }

    // A named view.
    await saveCockpitLayout({
      name: "Frozen invariant view",
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
    renderApp();
    await user.click(await screen.findByRole("button", { name: "Frozen invariant view" }));
    const namedView = await screen.findByLabelText("Research cockpit");
    // The saved layout's panelsJson loads asynchronously, and its one panel's
    // title ("Feed") COLLIDES with the pre-load default linked triad's own
    // "Feed" tab — wait on the unambiguous panel id instead.
    await waitFor(() => {
      expect(namedView.querySelector('[data-panel-id="follow:companyFeed"]')).not.toBeNull();
    });
    const beforeNamed = await snapshot(namedView);
    await exerciseEveryControl(namedView);
    expect(await snapshot(namedView)).toEqual(beforeNamed);

    // A legacy per-company dashboard (seeded by the sample scenario, `dashboard:` layout for CDR).
    await user.click(await screen.findByRole("button", { name: "Legacy dashboard · CDR" }));
    const dashboard = await screen.findByLabelText("Research cockpit");
    await waitFor(() => {
      expect(dashboard.querySelector('[data-panel-id="follow:fundamentals"]')).not.toBeNull();
    });
    const beforeDashboard = await snapshot(dashboard);
    await exerciseEveryControl(dashboard);
    expect(await snapshot(dashboard)).toEqual(beforeDashboard);
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
  // A saved view's `panelsJson` shape, seeded directly through the API the
  // removed add-panel flow used to call (there is no interactive builder any
  // more, R1 finding 4).
  function namedViewPanelsJson(pinned: unknown[], viewCompanyId: string | null) {
    return JSON.stringify({
      pinned,
      openGlobals: [],
      closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
      selectedFeedItemId: null,
      grid: null,
      cells: null,
      viewCompanyId,
    });
  }

  // Navigates to an already-saved named view via the local palette's
  // navigation-only "Open view: …" entry (F3a S3 freeze) — the cockpit must
  // already be mounted (`cockpit` from a prior `screen.findByLabelText`).
  async function openNamedView(
    user: ReturnType<typeof userEvent.setup>,
    cockpit: HTMLElement,
    name: string,
  ) {
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), name);
    await user.click(await screen.findByRole("button", { name: `Open view: ${name}` }));
  }

  // A follow `companyFeed` panel can no longer be added interactively (the
  // "Add panel"/"Open panel" surface is gone with the freeze) — seed a named
  // view carrying one directly, then open it via the navigation-only palette.
  async function openCdrView(user: ReturnType<typeof userEvent.setup>) {
    await saveCockpitLayout({
      name: "CDR follow-up",
      panelsJson: namedViewPanelsJson(
        [{ id: "follow:companyFeed", kind: "companyFeed", mode: "follow" }],
        "company_gpw_cdr",
      ),
      layoutJson: null,
      dockviewVersion: null,
    });
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");
    await openNamedView(user, cockpit, "CDR follow-up");
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
    // The pin toggle is gone with the freeze (R1 finding 4) — seed a panel
    // already pinned, matching what the removed toggle used to produce.
    await saveCockpitLayout({
      name: "CDR pinned feed",
      panelsJson: namedViewPanelsJson(
        [
          {
            id: "companyFeed:company_gpw_cdr",
            kind: "companyFeed",
            mode: "pinned",
            companyId: "company_gpw_cdr",
          },
        ],
        "company_gpw_cdr",
      ),
      layoutJson: null,
      dockviewVersion: null,
    });
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");
    await openNamedView(user, cockpit, "CDR pinned feed");
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

  // R1 finding 4 (ADR 0107 decision 5): the pin↔follow toggle changes which
  // panel id occupies the dock — a structure mutation, same class as
  // add/close/drag — so it is hidden, not just the removed "+ New view"/"Add
  // panel"/preset surface. Float (detaches the group into its own window) is
  // hidden for the same reason; Maximize is a view-only toggle and stays.
  it("hides the pin/follow toggle and Float — structural, gone with the freeze", async () => {
    const cockpit = await openCdrView(userEvent.setup());
    expect(within(cockpit).queryByRole("button", { name: "Pin company" })).not.toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: "Follow view company" })).not.toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: "Float panel group" })).not.toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: "Maximize panel group" })).toBeInTheDocument();
  });

  it("recovers when dockview refuses a panel removal — the dock remounts from the specs (#348)", async () => {
    const user = userEvent.setup();
    // Two named views whose panel sets overlap in KIND but not in id (pinned
    // vs. follow) — switching between them is navigation (not the removed pin
    // toggle) and still drives the same real panel add/remove dockview sees.
    await saveCockpitLayout({
      name: "Pinned CDR feed",
      panelsJson: namedViewPanelsJson(
        [
          {
            id: "companyFeed:company_gpw_cdr",
            kind: "companyFeed",
            mode: "pinned",
            companyId: "company_gpw_cdr",
          },
        ],
        null,
      ),
      layoutJson: null,
      dockviewVersion: null,
    });
    await saveCockpitLayout({
      name: "Follow feed",
      panelsJson: namedViewPanelsJson(
        [{ id: "follow:companyFeed", kind: "companyFeed", mode: "follow" }],
        "company_gpw_cdr",
      ),
      layoutJson: null,
      dockviewVersion: null,
    });
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");
    await openNamedView(user, cockpit, "Pinned CDR feed");
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    const spy = vi
      .spyOn(DockviewComponent.prototype, "removePanel")
      .mockImplementationOnce(() => {
        throw new Error("invalid operation");
      });
    try {
      await openNamedView(user, cockpit, "Follow feed");
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

    // Retargeting the view company is still available while frozen (it
    // renames/re-renders, never adds/removes/saves) — the same re-render that
    // used to follow a pin toggle click, still enough to arm the DOM-witness
    // verification pass and clean up the ghost.
    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_kgh");
    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "GPW:GHOST · Feed" })).toBeNull();
    });
    expect(await within(cockpit).findByRole("button", { name: "Feed" })).toBeInTheDocument();
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
