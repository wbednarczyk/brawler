import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "./ErrorBoundary";

// Guardrail for the white-screen class of bug (a cockpit panel threw and blanked
// the whole app, nav included). A boundary must contain the failure so siblings
// survive and the user gets a recovery action.
function Bomb({ explode }: { explode: boolean }) {
  if (explode) throw new Error("boom");
  return <div>panel ok</div>;
}

describe("ErrorBoundary", () => {
  it("contains a child render error and keeps siblings alive", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <div>
        <span>nav stays</span>
        <ErrorBoundary fallback={(error, reset) => (
          <button type="button" onClick={reset}>{`recover: ${error.message}`}</button>
        )}>
          <Bomb explode />
        </ErrorBoundary>
      </div>,
    );

    // The sibling (the nav, in the real app) is untouched, and the fallback shows.
    expect(screen.getByText("nav stays")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "recover: boom" })).toBeInTheDocument();
    consoleError.mockRestore();
  });
});
