import { describe, it } from "vitest";
import { expect, renderApp, screen, userEvent, waitFor } from "./test/appWorkflowHarness";

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
    const user = userEvent.setup();
    renderApp();

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
      if (heading !== "Inbox") {
        await user.click(screen.getByRole("button", { name: heading }));
      }

      await waitFor(() => {
        expect(screen.getByRole("heading", { name: heading })).toBeInTheDocument();
      });
      expectNoForbiddenNormalUserTerms(heading);
    }

    expect(screen.queryByRole("button", { name: "Diagnostics" })).not.toBeInTheDocument();
  });
});
