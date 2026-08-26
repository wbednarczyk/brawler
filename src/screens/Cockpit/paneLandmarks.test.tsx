import { describe, it } from "vitest";
import { expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import { saveCockpitLayout } from "../../api/cockpit";

// Duplicate-kind panel landmarks (issue #142). Hosting two panels of the same
// kind — two companies' report diffs side by side, or a pinned panel next to the
// follow panel of the same kind — must never render two
// `<section aria-label="Report comparison">` landmarks carrying the SAME
// accessible name: that would be an axe `landmark-unique` violation, and real
// navigation noise for a screen-reader user who cannot tell the two apart.
//
// The strategy (docs/ui-authoring.md § Landmarks): a landmark belongs to the
// SCREEN, never to a panel hosted in a dock pane. Inside a pane a titled block is
// a `role="group"` carrying its name — announced, but not a landmark — so no
// number of same-kind panes can collide. The dock tab names the pane, and tab
// titles are unique by construction (kind-keyed ids, ticker-prefixed titles for
// pinned panels).
//
// This guard walks every company-scoped panel kind and asserts its pane
// contributes no landmark. It reddens the moment a panel body reintroduces a
// named <section>/<aside>/role="region" — the class, not the one panel that had it.
const PANEL_KINDS = [
  "Basic info",
  "Fundamentals",
  "Coverage",
  "Report comparison",
  "Claims",
  "Quality",
  "Report documents",
  "Feed",
  "Notebook",
  "Decision journal",
  "Short selling (KNF)",
  "Warning signals",
  "Analyst recommendations",
] as const;

// PinnedKind ids (CockpitScreen.tsx), same order as PANEL_KINDS.
const PANEL_PINNED_KINDS = [
  "basicInfo",
  "fundamentals",
  "coverage",
  "reportDiff",
  "claims",
  "quality",
  "documents",
  "companyFeed",
  "companyNotebook",
  "decisionJournal",
  "shortPositions",
  "redFlags",
  "analystRecommendations",
] as const;

const LANDMARK_SELECTOR = [
  "section[aria-label]",
  "section[aria-labelledby]",
  "[role='region']",
  "main",
  "nav",
  "aside",
].join(", ");

describe("cockpit pane landmarks", () => {
  // Opening and activating all 13 panel kinds runs ~3s alone, well past the 5s
  // default once the suite runs at full parallelism.
  it("no company-scoped panel contributes a landmark of its own", { timeout: 30_000 }, async () => {
    const user = userEvent.setup();
    // "Open panel: …" no longer exists (F3a S3 freeze, ADR 0107 decision 5) —
    // seed a named view carrying all 13 kinds as FOLLOW panels through the API
    // the removed add-panel flow used to call, then open it read-only.
    await saveCockpitLayout({
      name: "Landmarks check",
      panelsJson: JSON.stringify({
        pinned: PANEL_PINNED_KINDS.map((kind) => ({ id: `follow:${kind}`, kind, mode: "follow" })),
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
    const { container } = renderApp({ section: "Cockpit" });

    const cockpit = await screen.findByLabelText("Research cockpit");
    await user.click(within(cockpit).getByRole("button", { name: /Commands/ }));
    await user.type(await screen.findByLabelText("Search commands"), "Landmarks check");
    await user.click(await screen.findByRole("button", { name: "Open view: Landmarks check" }));

    const offenders: Record<string, string[]> = {};

    for (const kind of PANEL_KINDS) {
      // Every kind is already open (declaratively, via the seeded view);
      // activate its tab so this kind is the body rendered in its group.
      const tab = (await within(cockpit).findAllByRole("button", { name: kind }))[0];
      await user.click(tab);

      // sol R1 finding 10: Quality's evaluation history (`<aside>`, now a
      // `role="group"` div) only renders once `history.length > 0` — never
      // exercised by this guard before, so the regression it introduced
      // passed vacuously. Produce one real evaluation through the actual
      // "Evaluate" action before checking this kind's pane for landmarks.
      if (kind === "Quality") {
        const evaluateButton = within(cockpit).queryByRole("button", { name: "Evaluate" });
        if (evaluateButton) {
          await user.click(evaluateButton);
          await within(cockpit).findByText("Evaluation history");
        }
      }

      for (const pane of Array.from(container.querySelectorAll(".cockpit-pane"))) {
        const landmarks = Array.from(pane.querySelectorAll(LANDMARK_SELECTOR)).map(
          (el) =>
            el.getAttribute("aria-label") ??
            el.getAttribute("aria-labelledby") ??
            el.tagName.toLowerCase(),
        );
        if (landmarks.length > 0) {
          offenders[kind] = [...new Set([...(offenders[kind] ?? []), ...landmarks])];
        }
      }
    }

    expect(
      offenders,
      'A cockpit panel must not add a landmark: two panes of the same kind collide on the same accessible name. Use role="group" + aria-label instead.',
    ).toEqual({});
  });
});
