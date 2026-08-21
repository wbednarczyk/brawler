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

// Anti-filename hard gate (docs/testing.md "UI dogfooding finding ⇒ overlay"),
// carried forward to Dziś v2 (F2 S4): a row STATEMENT is prose; a filename is
// metadata (`documentTitle.splitDocumentTitle` moves it to the row's quiet
// `.dayq-row-meta` line — `FilingRow`/`AutopilotRunRow`/`AttentionRow`, F2
// S4). This gate renders the Dziś day queue across the scenarios that carry
// document-title evidence — including the dogfooding-state overlays — and
// asserts NO rendered `.dayq-row-title` ever carries a document extension
// (the single-source `FILENAME_EXTENSION` pattern), and none carries a glued
// extension. Regressing the split — or assuming a title is always clean
// prose — reddens here in CI forever.

/** The scenarios whose day queue carries report-document titles worth policing. */
const SCENARIOS: { label: string; spec: ScenarioSpec }[] = [
  { label: "rich base", spec: { base: "rich" } },
  { label: "morning-review", spec: { base: "rich", overlays: ["morning-review"] } },
  { label: "today-dense", spec: { base: "rich", overlays: ["today-dense"] } },
  { label: "orphaned-evidence", spec: { base: "rich", overlays: ["orphaned-evidence"] } },
  { label: "pruned-feed", spec: { base: "rich", overlays: ["pruned-feed"] } },
];

/** Every rendered row statement. */
function statements(): string[] {
  return [...document.querySelectorAll<HTMLElement>(".dayq-row-title")].map(
    (node) => node.textContent ?? "",
  );
}

describe("Today day-queue copy — no filename ever renders as a row statement (owner dogfooding 2026-07-23)", () => {
  for (const { label, spec } of SCENARIOS) {
    it(`${label}: no row statement carries a document extension`, async () => {
      seedScenario(spec);
      renderApp({ section: "Today" });

      await screen.findByRole("heading", { name: "Today" });
      // `get_today_view` (F2 S1) is the screen's one async read — wait for its
      // rows to land before scanning statements.
      await waitFor(() =>
        expect(document.querySelectorAll(".dayq-row").length).toBeGreaterThan(0),
      );

      const rendered = statements();
      expect(rendered.length).toBeGreaterThan(0);
      for (const statement of rendered) {
        // (a) No document extension anywhere in a statement (it belongs on the meta line).
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

  it("pruned-feed glued snapshot: the human statement leads, the filename drops to the meta line", async () => {
    seedScenario({ base: "rich", overlays: ["pruned-feed"] });
    renderApp({ section: "Today" });

    // The split moved the human part into the statement…
    const statement = await screen.findByText(PRUNED_GLUED_HUMAN);
    expect(statement).toBeInTheDocument();
    // …and the filename (with its extension) survives on the row's quiet meta
    // line, NOT lost and NOT in the statement.
    const meta = statement.closest(".dayq-row-body")?.querySelector(".dayq-row-meta");
    expect(meta?.textContent).toBe(PRUNED_GLUED_FILENAME);
    expect(FILENAME_EXTENSION.test(meta?.textContent ?? "")).toBe(true);
  });
});
