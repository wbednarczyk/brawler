import { describe, it } from "vitest";
import { fireEvent } from "@testing-library/react";
import {
  expect,
  renderApp,
  renderAppDefaultShell,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";
import { parsePanels } from "./CockpitScreen";

describe("Research cockpit shell", () => {
  it("is not the default shell and has no standalone blank nav entry (ADR 0057 decision 5)", async () => {
    // Today/Pulse is the landing home; the cockpit is reached only via a saved
    // named view, the "+ New view" creator, or a company's curated dashboard —
    // there is no standalone blank-canvas "Cockpit" sidebar button anymore.
    renderAppDefaultShell();
    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Research cockpit")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cockpit" })).not.toBeInTheDocument();
  });

  it("keeps only the Preset select from the legacy layout toolbar (issue 432de51)", async () => {
    // The pre-ADR-0057 toolbar's saved-layout load select, free-text layout name
    // field, save button, and delete select duplicated the sidebar saved-views
    // flow (same cockpit_layouts storage, conflicting "układ"/"widok" vocabulary).
    // Option B keeps only the Preset select; the rest must never come back.
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    expect(within(cockpit).getByLabelText("Preset")).toBeInTheDocument();
    expect(within(cockpit).queryByLabelText("Saved layout")).not.toBeInTheDocument();
    expect(within(cockpit).queryByLabelText("Layout name")).not.toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: "Save layout" })).not.toBeInTheDocument();
    expect(within(cockpit).queryByLabelText("Delete layout")).not.toBeInTheDocument();
  });

  it("opens the cockpit with accessible tabs and opens a panel via the command palette", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });

    const cockpit = await screen.findByLabelText("Research cockpit");
    expect(cockpit).toBeInTheDocument();

    // dockview tabs are accessible <button>s here (the pilot's div tabs were the
    // a11y blocker). The default linked layout opens several panels.
    const tabs = await within(cockpit).findAllByRole("button");
    expect(tabs.length).toBeGreaterThan(0);

    // A company-scoped panel type binds to the view company (card 106f8a7), so
    // choose one before opening a Quality panel via the command palette (⌘K).
    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_cdr");
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Quality");
    await user.keyboard("{Enter}");

    const qualityTabs = await within(cockpit).findAllByRole("button", { name: /Quality/ });
    expect(qualityTabs.length).toBeGreaterThan(0);
  });

  it("opens a company directly into its curated dashboard as FOLLOW panels with unprefixed titles (U-Ra)", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR dashboard" }));

    const cockpit = await screen.findByLabelText("Research cockpit");
    // The dashboard renders company-scoped FOLLOW panels: their tab titles are
    // kind-only (the header selector carries the company context), NOT prefixed
    // with the ticker (D4), and the linked Inspector stays closed.
    expect(await within(cockpit).findByRole("button", { name: "Fundamentals" })).toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: "Notebook" })).toBeInTheDocument();
    expect(within(cockpit).queryByRole("button", { name: /GPW:CDR · Fundamentals/ })).toBeNull();
    // The view-company selector reflects the opened company (D2).
    expect((within(cockpit).getByLabelText("View company") as HTMLSelectElement).value).toBe(
      "company_gpw_cdr",
    );
    expect(within(cockpit).queryByRole("button", { name: "Inspector" })).not.toBeInTheDocument();
  });

  it("closes the active panel from the keyboard (Alt+W)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // Activate the Inspector panel via its accessible tab, then close it with
    // the keyboard model (Alt+W closes the active panel — DockLayout).
    const inspectorTab = await within(cockpit).findByRole("button", { name: "Inspector" });
    await user.click(inspectorTab);
    await user.keyboard("{Alt>}w{/Alt}");

    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "Inspector" })).toBeNull();
    });
  });

  // Own timeout budget: two full dockview teardown/rebuild cycles routinely
  // exceed the 5s default under loaded test workers (React 19 migration).
  it("rebuilds a working layout after closing a panel then resetting (no blank app)", { timeout: 20000 }, async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // Close the Inspector panel (the × on its accessible tab), then Reset layout.
    // While dockview settles its initial layout it re-creates tab DOM, so a
    // single click can land on a node detached between query and dispatch and
    // silently vanish (React 19 timing under loaded workers). Model a user
    // retrying: click the currently-attached close control until the tab is
    // gone.
    await within(cockpit).findByRole("button", { name: "Close Inspector" });
    await waitFor(
      () => {
        const close = within(cockpit).queryByRole("button", { name: "Close Inspector" });
        if (close) {
          fireEvent.click(close);
        }
        expect(within(cockpit).queryByRole("button", { name: "Inspector" })).toBeNull();
      },
      { timeout: 10000 },
    );
    await user.click(within(cockpit).getByRole("button", { name: "Reset layout" }));

    // The default layout must come back — not a blank cockpit. The dockview
    // rebuild is slow under loaded test workers — give it a real timeout.
    expect(
      await within(cockpit).findByRole("button", { name: "Inspector" }, { timeout: 5000 }),
    ).toBeInTheDocument();
    expect(
      await within(cockpit).findByRole("button", { name: "Feed" }, { timeout: 5000 }),
    ).toBeInTheDocument();
  });

  it("applies a preset composing FOLLOW panels for the view company (Dashboard redesign)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // Presets follow the view company (epic c793ca1): pick a view company first,
    // then the Sezon raportów preset opens the Report Season global plus
    // company-scoped Report comparison / Fundamentals / Claims — as FOLLOW panels.
    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_cdr");
    await user.selectOptions(within(cockpit).getByLabelText("Preset"), "earnings-season");

    const tabs = await within(cockpit).findAllByRole("button", { name: /Report Season/ });
    expect(tabs.length).toBeGreaterThan(0);
    expect(
      (await within(cockpit).findAllByRole("button", { name: /Fundamentals/ })).length,
    ).toBeGreaterThan(0);
    // Follow panels are kind-only (no ticker prefix) — they track the view company,
    // they do not freeze it. A "GPW:CDR · …" pinned tab must NOT appear.
    expect(within(cockpit).queryByRole("button", { name: /GPW:CDR ·/ })).not.toBeInTheDocument();
    // The Preset selector reflects the active preset (visual-first UX): the user
    // sees which arrangement is showing, not a reset-to-placeholder select.
    expect(within(cockpit).getByLabelText("Preset")).toHaveValue("earnings-season");
  });

  it("Dowody / Research preset composes the research evidence panel", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // The retired standalone Research screen becomes a Dashboard preset (epic
    // c793ca1). Selecting the "evidence" preset composes the research evidence
    // panel as the sole global — proven here by its tab title. The panel's
    // real evidence render (following the view company) is proven in the browser
    // spec cockpit-view-company.spec.ts (jsdom dockview does not mount panel
    // bodies after a layout rebuild).
    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_cdr");
    await user.selectOptions(within(cockpit).getByLabelText("Preset"), "evidence");

    const tabs = await within(cockpit).findAllByRole("button", { name: /Research/ });
    expect(tabs.length).toBeGreaterThan(0);
  });

  it("opens a global app screen as a cockpit panel (phase 4c)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // Global panels render the real, context-driven screens (Watchlists here).
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Watchlists");
    await user.keyboard("{Enter}");

    expect(await within(cockpit).findByLabelText("Watchlist name")).toBeInTheDocument();
  });

  it("loads a section-gated screen's data when hosted as a cockpit panel (phase 4c/6)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // Research evidence loads only while its section is active; as a cockpit
    // panel (activeSection === "Cockpit") that gate must also fire, or the panel
    // renders empty. Assert the timeline actually shows data, not just a frame.
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Research");
    await user.keyboard("{Enter}");

    // "CDR follow-up note" is research-evidence-only (not shown by the default
    // Feed/Inspector/Claims panels), so finding it proves the Research panel's
    // section-gated data loaded while hosted in the cockpit.
    expect(await within(cockpit).findByText("CDR follow-up note")).toBeInTheDocument();
  });

  it("opens the full editable Fundamentals panel for a company (phase 4b)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // A company-scoped panel type binds to the view company (card 106f8a7): choose
    // one, then launch a Fundamentals panel via the command palette; it must render
    // the real editable panel (period/fact forms), not a read-only matrix.
    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_cdr");
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Fundamentals");
    await user.keyboard("{Enter}");

    // The editable panel's period form proves we render the real FundamentalsPanel
    // (the old cockpit version was a read-only matrix with no forms).
    expect(
      await within(cockpit).findByRole("heading", { name: "New reporting period" }),
    ).toBeInTheDocument();
    expect(
      await within(cockpit).findByRole("heading", { name: "Reporting periods" }),
    ).toBeInTheDocument();
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

  it("offers a saved layout as a command-palette launcher entry", async () => {
    const user = userEvent.setup();
    // Plain renderApp(): the cockpit must not be mounted yet when "Deep dive" is
    // created, or its own savedLayouts snapshot (fetched once on mount) would miss
    // the newly created layout — it only refetches on its own mount, not when a
    // sibling (the sidebar) saves a new one.
    renderApp();

    // Named layouts are created via the sidebar "+ New view" creator (ADR 0057
    // decision 5) — the legacy toolbar's free-text save/load/delete controls were
    // removed as a duplicate of this flow (issue 432de51).
    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "New view" }));
    await user.type(await screen.findByLabelText("View name"), "Deep dive");
    await user.click(screen.getByRole("button", { name: /Create view/i }));

    const cockpit = await screen.findByLabelText("Research cockpit");

    // The saved layout is now launchable from the command palette.
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Load layout");
    expect(
      await screen.findByRole("button", { name: "Load layout: Deep dive" }),
    ).toBeInTheDocument();
  });
});

// U-Ra (ADR 0076): a cockpit view carries one "view company"; company-scoped
// panels follow it by default and retarget in place on switch; a single panel may
// pin a different company; saved views persist the context.
describe("Research cockpit — view company context (U-Ra)", () => {
  async function openCdrDashboard(user: ReturnType<typeof userEvent.setup>) {
    renderApp();
    await user.click(await screen.findByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR dashboard" }));
    return screen.findByLabelText("Research cockpit");
  }

  it("retargets a FOLLOW panel's content in place when the view company changes", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrDashboard(user);

    // The follow Feed panel shows CD PROJEKT's feed item at first.
    expect(
      await within(cockpit).findByRole("button", {
        name: "Open company feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
    // Tab stays kind-only (no ticker prefix).
    expect(within(cockpit).getByRole("button", { name: "Feed" })).toBeInTheDocument();

    // Switch the view company via the header selector.
    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_kgh");

    // Content retargets in place to KGHM; the tab title is still kind-only "Feed".
    expect(
      await within(cockpit).findByRole("button", {
        name: "Open company feed item: Transcript-derived note candidate waits for future provider work",
      }),
    ).toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: "Feed" })).toBeInTheDocument();
    // Same reason as the unpin swap below: the outgoing company's item can survive
    // one render past the incoming one, so wait for it to go rather than sample.
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
    const cockpit = await openCdrDashboard(user);

    // Freeze the follow Feed panel onto CD PROJEKT via its tab pin toggle. The
    // add-panel surface no longer offers per-company entries (card 106f8a7), so
    // pinning is reached through the panel's own "Pin company" affordance.
    const feedTab = (await within(cockpit).findByRole("button", { name: "Feed" })).closest(
      ".cockpit-tab",
    ) as HTMLElement;
    await user.click(within(feedTab).getByRole("button", { name: "Pin company" }));
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    // Set the view company to KGH — the pinned panel must not move.
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
    const cockpit = await openCdrDashboard(user);

    // The follow Feed panel offers a pin affordance on its own tab (each follow
    // tab has a "Pin company" button, so scope to the Feed tab specifically).
    const feedTab = (await within(cockpit).findByRole("button", { name: "Feed" })).closest(
      ".cockpit-tab",
    ) as HTMLElement;
    await user.click(within(feedTab).getByRole("button", { name: "Pin company" }));

    // follow → pinned freezes CD PROJEKT: the tab gains the ticker prefix.
    expect(await within(cockpit).findByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();

    // Switching the view company leaves the now-pinned panel on CD PROJEKT.
    await user.selectOptions(within(cockpit).getByLabelText("View company"), "company_gpw_kgh");
    await waitFor(() => {
      expect(within(cockpit).getByRole("button", { name: "GPW:CDR · Feed" })).toBeInTheDocument();
    });

    // pinned → follow rejoins the view company (now KGH): kind-only title returns.
    await user.click(within(cockpit).getByRole("button", { name: "Follow view company" }));
    expect(await within(cockpit).findByRole("button", { name: "Feed" })).toBeInTheDocument();
    // The unpin swaps the tab node, so the prefixed title can outlive the arrival
    // of the kind-only one by a render: await its *removal* instead of sampling
    // the single tick after the positive assertion resolves.
    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "GPW:CDR · Feed" })).toBeNull();
    });
    expect(
      await within(cockpit).findByRole("button", {
        name: "Open company feed item: Transcript-derived note candidate waits for future provider work",
      }),
    ).toBeInTheDocument();
  });

  it("offers no per-company palette entries — the header selector is the one way to retarget", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    // Owner dogfooding round 2: enumerating every tracked company as a
    // "Switch view company: <ticker>" command duplicated the header selector
    // and bloated the palette — the selector is the single retarget surface.
    await user.type(await screen.findByLabelText("Search commands"), "Switch view company");
    expect(screen.queryAllByRole("button", { name: /Switch view company/ })).toHaveLength(0);
    expect(within(cockpit).getByLabelText("View company")).toBeInTheDocument();
  });

  it("styles the palette search through the design system (search-box), not a bare input", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // Both cockpit search surfaces wear the shared search-box dressing — a bare
    // native input (owner dogfooding round 2) reads as unfinished.
    expect(
      (await within(cockpit).findByLabelText("Filter feed items")).closest(".search-box"),
    ).not.toBeNull();
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    const input = await screen.findByLabelText("Search commands");
    expect(input.closest(".search-box")).not.toBeNull();
  });
});

// Card 106f8a7 (owner dogfooding): the add-panel surface must list GENERIC
// company-scoped panel TYPES (bound to the view company) plus global panels —
// never a per-company entry for every tracked company (the old flow bloated the
// palette to hundreds of "Open panel: <TICKER> · <Kind>" rows).
describe("Research cockpit — add-panel surface (card 106f8a7)", () => {
  async function openCdrDashboard(user: ReturnType<typeof userEvent.setup>) {
    renderApp();
    await user.click(await screen.findByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR dashboard" }));
    return screen.findByLabelText("Research cockpit");
  }

  it("lists each generic company-scoped panel type once, with no per-company duplication", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrDashboard(user);

    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Open panel");

    // Each generic company-scoped type appears exactly once (the follow entry) —
    // no ticker-prefixed duplicate.
    expect(screen.getAllByRole("button", { name: "Open panel: Fundamentals" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Open panel: Coverage" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Open panel: Decision journal" })).toHaveLength(1);

    // No per-company entries: nothing prefixes a company ticker onto a panel kind.
    expect(screen.queryAllByRole("button", { name: /Open panel: GPW:/ })).toHaveLength(0);
    expect(screen.queryAllByRole("button", { name: /Open panel: NC:/ })).toHaveLength(0);
    expect(screen.queryAllByRole("button", { name: /Open panel: .+ · / })).toHaveLength(0);
  });

  it("keeps the global panels group present alongside the company-scoped types", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrDashboard(user);

    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Open panel");

    // Global singletons stay reachable from the same surface (own group).
    expect(screen.getByRole("button", { name: "Open panel: Report Season" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open panel: Research" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Open panel: Journal (all companies)" }),
    ).toBeInTheDocument();
  });

  it("adds a company-scoped panel bound to the view company (kind-only tab, no ticker prefix)", async () => {
    const user = userEvent.setup();
    const cockpit = await openCdrDashboard(user);

    // Report comparison is not a default dashboard panel — add it from the palette.
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(
      await screen.findByLabelText("Search commands"),
      "Open panel: Report comparison",
    );
    await user.click(
      await screen.findByRole("button", { name: "Open panel: Report comparison" }),
    );

    // It opens as a FOLLOW panel bound to the view company: kind-only tab title,
    // no ticker prefix — so switching the view company will retarget it in place.
    expect(
      await within(cockpit).findByRole("button", { name: "Report comparison" }),
    ).toBeInTheDocument();
    expect(
      within(cockpit).queryByRole("button", { name: /GPW:CDR · Report comparison/ }),
    ).toBeNull();
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
