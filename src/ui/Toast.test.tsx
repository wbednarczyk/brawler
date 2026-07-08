import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { act, render, screen, fireEvent } from "@testing-library/react";

import { ToastProvider, useToast } from "./index";

// A tiny harness that exposes the toast API through buttons so tests can drive
// it the way a real screen would (via the useToast hook), never by reaching
// into internals.
function ToastHarness() {
  const { show } = useToast();
  return (
    <div>
      <button
        type="button"
        onClick={() =>
          show({ message: "Deleted note", actionLabel: "Undo", onAction: () => onActionSpy() })
        }
      >
        delete
      </button>
      <button type="button" onClick={() => show({ message: "toast-one" })}>
        one
      </button>
      <button type="button" onClick={() => show({ message: "toast-two" })}>
        two
      </button>
      <button type="button" onClick={() => show({ message: "toast-three" })}>
        three
      </button>
      <button type="button" onClick={() => show({ message: "toast-four" })}>
        four
      </button>
    </div>
  );
}

const onActionSpy = vi.fn();

function renderHarness() {
  return render(
    <ToastProvider>
      <ToastHarness />
    </ToastProvider>,
  );
}

beforeEach(() => {
  vi.useFakeTimers();
  onActionSpy.mockReset();
});

afterEach(() => {
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
});

describe("Toast", () => {
  it("shows a toast with role=status carrying the message", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "delete" }));
    const toast = screen.getByRole("status");
    expect(toast).toHaveTextContent("Deleted note");
  });

  it("renders the action button and invokes onAction then dismisses on click", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "delete" }));
    const undo = screen.getByRole("button", { name: "Undo" });
    fireEvent.click(undo);
    expect(onActionSpy).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Deleted note")).toBeNull();
  });

  it("auto-dismisses after 6s", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "delete" }));
    expect(screen.getByText("Deleted note")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(6000);
    });
    expect(screen.queryByText("Deleted note")).toBeNull();
  });

  it("persists on hover and resumes the timer on leave", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "delete" }));
    const toast = screen.getByRole("status");
    fireEvent.mouseEnter(toast);
    act(() => {
      vi.advanceTimersByTime(9000);
    });
    expect(screen.getByText("Deleted note")).toBeInTheDocument();
    fireEvent.mouseLeave(toast);
    act(() => {
      vi.advanceTimersByTime(6000);
    });
    expect(screen.queryByText("Deleted note")).toBeNull();
  });

  it("caps the stack at 3, dropping the oldest", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "one" }));
    fireEvent.click(screen.getByRole("button", { name: "two" }));
    fireEvent.click(screen.getByRole("button", { name: "three" }));
    fireEvent.click(screen.getByRole("button", { name: "four" }));
    expect(screen.queryByText("toast-one")).toBeNull();
    expect(screen.getByText("toast-two")).toBeInTheDocument();
    expect(screen.getByText("toast-three")).toBeInTheDocument();
    expect(screen.getByText("toast-four")).toBeInTheDocument();
    expect(screen.getAllByRole("status")).toHaveLength(3);
  });
});
