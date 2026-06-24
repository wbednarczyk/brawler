import { describe, it } from "vitest";
import { expect, renderApp, screen, waitFor } from "./test/appWorkflowHarness";

const forbiddenNormalUserTerms = [
  "SQLite",
  "Tauri",
  "adapter",
  "schema",
  "database",
  "module",
  "collector",
  "local",
] as const;

function visibleAppText() {
  return document.body.textContent ?? "";
}

function expectNoForbiddenNormalUserTerms(section: string) {
  const renderedText = visibleAppText();
  const forbiddenTerm = forbiddenNormalUserTerms.find((term) =>
    new RegExp(`\\b${term}\\b`, "i").test(renderedText),
  );

  expect(
    forbiddenTerm,
    `${section} rendered normal-user implementation wording: ${forbiddenTerm ?? ""}`,
  ).toBeUndefined();
}

describe("normal user UI guardrails", () => {
  it("does not expose implementation wording in normal app sections", async () => {
    // Render each section in isolation via its initial section. The slimmed
    // top-nav (ADR 0053 phase 6) no longer has buttons for Notebooks/Events
    // (they are Cockpit panels), but the wording guard must still cover them.
    const sectionHeadings = [
      "Inbox",
      "Companies",
      "Watchlists",
      "Notebooks",
      "Events",
      "Transcripts",
      "Sources",
      "Settings",
    ] as const;

    for (const heading of sectionHeadings) {
      const { unmount } = renderApp({ section: heading });
      await waitFor(() => {
        expect(screen.getByRole("heading", { name: heading })).toBeInTheDocument();
      });
      expectNoForbiddenNormalUserTerms(heading);
      unmount();
    }

    // Diagnostics stays hidden from the slimmed nav unless developer mode is on.
    const { unmount } = renderApp({ section: "Cockpit" });
    expect(screen.queryByRole("button", { name: "Diagnostics" })).not.toBeInTheDocument();
    unmount();
  });
});
