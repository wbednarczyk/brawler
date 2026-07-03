import { describe, it } from "vitest";
import {
  expect,
  renderApp,
  renderAppDefaultShell,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

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

    // Open a Quality panel via the command palette (⌘K launcher).
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Quality");
    await user.keyboard("{Enter}");

    const qualityTabs = await within(cockpit).findAllByRole("button", { name: /Quality/ });
    expect(qualityTabs.length).toBeGreaterThan(0);
  });

  it("opens a company directly into its curated dashboard panels (ADR 0057)", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR dashboard" }));

    const cockpit = await screen.findByLabelText("Research cockpit");
    // The dashboard renders the company-scoped panels (not the default linked
    // triad) — so its tabs are present and the linked Inspector is not.
    expect((await within(cockpit).findAllByRole("button", { name: /· Fundamentals/ })).length).toBeGreaterThan(0);
    expect(within(cockpit).getAllByRole("button", { name: /· Notebook/ }).length).toBeGreaterThan(0);
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

  it("rebuilds a working layout after closing a panel then resetting (no blank app)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // Close the Inspector panel (the × on its accessible tab), then Reset layout.
    await user.click(await within(cockpit).findByRole("button", { name: "Close Inspector" }));
    await waitFor(() => {
      expect(within(cockpit).queryByRole("button", { name: "Inspector" })).toBeNull();
    });
    await user.click(within(cockpit).getByRole("button", { name: "Reset layout" }));

    // The default layout must come back — not a blank cockpit.
    expect(await within(cockpit).findByRole("button", { name: "Inspector" })).toBeInTheDocument();
    expect(within(cockpit).getByRole("button", { name: "Feed" })).toBeInTheDocument();
  });

  it("applies a built-in preset that composes the right panels (phase 4d)", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Cockpit" });
    const cockpit = await screen.findByLabelText("Research cockpit");

    // The Earnings season preset opens the Report Season global panel plus
    // company-scoped Report comparison / Fundamentals / Claims.
    await user.selectOptions(within(cockpit).getByLabelText("Preset"), "earnings-season");

    await waitFor(async () => {
      const tabs = await within(cockpit).findAllByRole("button", { name: /Report Season/ });
      expect(tabs.length).toBeGreaterThan(0);
    });
    expect(
      (await within(cockpit).findAllByRole("button", { name: /Fundamentals/ })).length,
    ).toBeGreaterThan(0);
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

    // Launch a Fundamentals panel via the command palette; it must render the
    // real editable panel (period/fact forms), not a read-only matrix.
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
      within(cockpit).getByRole("button", {
        name: "Inspect feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();

    await user.type(within(cockpit).getByLabelText("Filter feed items"), "KGH");

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
