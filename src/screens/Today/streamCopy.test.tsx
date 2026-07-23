import { describe, it } from "vitest";
import {
  expect,
  renderApp,
  screen,
  seedScenario,
  waitFor,
} from "../../test/appWorkflowHarness";
import type { ScenarioSpec } from "../../test/scenarios/scenarios";
import {
  PRUNED_GLUED_FILENAME,
  PRUNED_GLUED_HUMAN,
} from "../../test/scenarios/overlays";
import { FILENAME_EXTENSION, FILENAME_EXTENSION_GLUE } from "./documentTitle";

// Anti-filename hard gate (owner dogfooding 2026-07-23; docs/testing.md "UI
// dogfooding finding ⇒ overlay"). A row STATEMENT is prose; a filename is
// metadata (documentTitle.splitDocumentTitle drops it to a quiet link line). This
// gate renders the Today stream across the scenarios that carry document-title
// attention evidence — including the dogfooding-state overlays — and asserts NO
// rendered `.today-row-title` ever carries a document extension (the single-source
// FILENAME_EXTENSION pattern), and none carries a glued extension (the exact class
// that leaked the owner's filename into a statement). Regressing the split — or
// assuming an evidenceTitle is always clean prose — reddens here in CI forever.

/** The scenarios whose stream carries report-document titles worth policing. */
const SCENARIOS: { label: string; spec: ScenarioSpec }[] = [
  { label: "rich base", spec: { base: "rich" } },
  { label: "morning-review", spec: { base: "rich", overlays: ["morning-review"] } },
  { label: "today-dense", spec: { base: "rich", overlays: ["today-dense"] } },
  { label: "orphaned-evidence", spec: { base: "rich", overlays: ["orphaned-evidence"] } },
  { label: "pruned-feed", spec: { base: "rich", overlays: ["pruned-feed"] } },
];

/** Reveal every collapsed group/aggregate so member statements enter the DOM too. */
function expandAllDisclosures() {
  // Two passes: an aggregate's expanded autopilot members carry their own disclosures.
  for (let pass = 0; pass < 2; pass += 1) {
    const buttons = [...document.querySelectorAll<HTMLElement>(".today-row-disclosure")];
    for (const button of buttons) {
      if (button.getAttribute("aria-expanded") === "false") button.click();
    }
  }
}

/** Every rendered row statement (top-level rows AND expanded members). */
function statements(): string[] {
  return [...document.querySelectorAll<HTMLElement>(".today-row-title")].map(
    (node) => node.textContent ?? "",
  );
}

describe("Today stream copy — no filename ever renders as a row statement (owner dogfooding 2026-07-23)", () => {
  for (const { label, spec } of SCENARIOS) {
    it(`${label}: no row statement carries a document extension`, async () => {
      seedScenario(spec);
      renderApp({ section: "Today" });

      await screen.findByRole("heading", { name: "Today" });
      // The attention category loads on its own async effect; wait for its rows.
      await waitFor(() =>
        expect(
          document.querySelectorAll('li[data-category="attention"]').length,
        ).toBeGreaterThan(0),
      );
      expandAllDisclosures();

      const rendered = statements();
      expect(rendered.length).toBeGreaterThan(0);
      for (const statement of rendered) {
        // (a) No document extension anywhere in a statement (it belongs on the link line).
        expect(
          FILENAME_EXTENSION.test(statement),
          `row statement leaked a filename extension: ${JSON.stringify(statement)}`,
        ).toBe(false);
        // (b) The glue class specifically (…".xhtmlHuman…") — reddens on its own.
        expect(
          FILENAME_EXTENSION_GLUE.test(statement),
          `row statement glued a filename to prose: ${JSON.stringify(statement)}`,
        ).toBe(false);
      }
    });
  }

  it("pruned-feed glued snapshot: the human statement leads, the filename drops to a link line", async () => {
    seedScenario({ base: "rich", overlays: ["pruned-feed"] });
    renderApp({ section: "Today" });

    // The split moved the human part into the statement…
    const statement = await screen.findByText(PRUNED_GLUED_HUMAN);
    expect(statement).toBeInTheDocument();
    // …and the filename (with its extension) survives on the quiet document link,
    // NOT lost and NOT in the statement.
    const docLink = document.querySelector(".today-row-doc-link");
    expect(docLink?.textContent).toBe(PRUNED_GLUED_FILENAME);
    expect(FILENAME_EXTENSION.test(docLink?.textContent ?? "")).toBe(true);
  });
});
