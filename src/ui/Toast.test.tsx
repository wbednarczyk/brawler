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
      <button
        type="button"
        onClick={() => show({ message: "Attention item", persistent: true })}
      >
        persistent
      </button>
      <button
        type="button"
        onClick={() =>
          show({
            message: "Signal fired",
            persistent: true,
            actionLabel: "View evidence",
            onAction: () => onActionSpy(),
          })
        }
      >
        persistent-with-action
      </button>
      <button
        type="button"
        onClick={() =>
          show({
            message: "Attention item with sync",
            persistent: true,
            onDismiss: () => onDismissSpy(),
          })
        }
      >
        persistent-with-dismiss-sync
      </button>
    </div>
  );
}

const onActionSpy = vi.fn();
const onDismissSpy = vi.fn();

function renderHarness() {
  return render(
    <ToastProvider>
      <ToastHarness />
    </ToastProvider>,
  );
}

// Harness for the persistent-overflow cap (bug: unbounded attention toasts
// covered the sidebar nav): six buttons, each firing one distinct persistent
// toast, so a test can push the queue past PERSISTENT_VISIBLE_CAP (3).
function OverflowHarness() {
  const { show } = useToast();
  return (
    <div>
      {Array.from({ length: 6 }, (_, index) => (
        <button
          key={index}
          type="button"
          onClick={() => show({ message: `alert-${index + 1}`, persistent: true, tone: "caution" })}
        >
          fire-{index + 1}
        </button>
      ))}
    </div>
  );
}

const overflowClickSpy = vi.fn();

function renderOverflowHarness() {
  return render(
    <ToastProvider
      onPersistentOverflowClick={overflowClickSpy}
      persistentOverflowLabel={(hiddenCount) => `+${hiddenCount} more`}
    >
      <OverflowHarness />
    </ToastProvider>,
  );
}

function fireOverflowToasts(count: number) {
  for (let i = 1; i <= count; i += 1) {
    fireEvent.click(screen.getByRole("button", { name: `fire-${i}` }));
  }
}

beforeEach(() => {
  vi.useFakeTimers();
  onActionSpy.mockReset();
  onDismissSpy.mockReset();
  overflowClickSpy.mockReset();
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

  it("persistent toast does not auto-dismiss after the timer", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "persistent" }));
    expect(screen.getByText("Attention item")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(60_000);
    });
    expect(screen.getByText("Attention item")).toBeInTheDocument();
  });

  it("renders a persistent toast as an assertive role=alert region", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "persistent" }));
    const toast = screen.getByRole("alert");
    expect(toast).toHaveTextContent("Attention item");
  });

  it("dismiss button removes a persistent toast", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "persistent" }));
    expect(screen.getByText("Attention item")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("Attention item")).toBeNull();
  });

  it("invokes the click-through action on a persistent toast without discarding it silently", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "persistent-with-action" }));
    fireEvent.click(screen.getByRole("button", { name: "View evidence" }));
    expect(onActionSpy).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Signal fired")).toBeNull();
  });

  it("calls onDismiss (domain sync) when the explicit dismiss control is clicked (ADR 0068 T4)", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "persistent-with-dismiss-sync" }));
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismissSpy).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Attention item with sync")).toBeNull();
  });

  it("does not call onDismiss when the toast is removed via its click-through action", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "persistent-with-action" }));
    fireEvent.click(screen.getByRole("button", { name: "View evidence" }));
    expect(onDismissSpy).not.toHaveBeenCalled();
  });

  it("spares persistent toasts from MAX_STACK eviction when transient toasts overflow", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "persistent" }));
    fireEvent.click(screen.getByRole("button", { name: "one" }));
    fireEvent.click(screen.getByRole("button", { name: "two" }));
    fireEvent.click(screen.getByRole("button", { name: "three" }));
    fireEvent.click(screen.getByRole("button", { name: "four" }));
    // Persistent toast survives even though 4 transient toasts were queued.
    expect(screen.getByText("Attention item")).toBeInTheDocument();
    // Transient queue is still capped at MAX_STACK (3): oldest transient dropped.
    expect(screen.queryByText("toast-one")).toBeNull();
    expect(screen.getByText("toast-two")).toBeInTheDocument();
    expect(screen.getByText("toast-three")).toBeInTheDocument();
    expect(screen.getByText("toast-four")).toBeInTheDocument();
  });
});

describe("Toast — persistent-overflow cap (bug fix: unbounded attention toasts covered the sidebar nav)", () => {
  it("caps persistent toasts at 3 and collapses the rest into a '+N more' summary row", () => {
    renderOverflowHarness();
    fireOverflowToasts(6);

    // Only the 3 most recent persistent toasts render as full alert toasts.
    expect(screen.getAllByRole("alert")).toHaveLength(3);
    expect(screen.queryByText("alert-1")).toBeNull();
    expect(screen.queryByText("alert-2")).toBeNull();
    expect(screen.queryByText("alert-3")).toBeNull();
    expect(screen.getByText("alert-4")).toBeInTheDocument();
    expect(screen.getByText("alert-5")).toBeInTheDocument();
    expect(screen.getByText("alert-6")).toBeInTheDocument();

    // The summary row covers the 3 hidden (oldest) toasts and renders as the
    // last item, using the caller-supplied translated label.
    const summary = screen.getByRole("button", { name: "+3 more" });
    expect(summary).toBeInTheDocument();
  });

  it("invokes the caller's overflow callback when the summary row is clicked", () => {
    renderOverflowHarness();
    fireOverflowToasts(6);
    fireEvent.click(screen.getByRole("button", { name: "+3 more" }));
    expect(overflowClickSpy).toHaveBeenCalledTimes(1);
  });

  it("renders no summary row while the persistent count is at or under the cap", () => {
    renderOverflowHarness();
    fireOverflowToasts(3);
    expect(screen.getAllByRole("alert")).toHaveLength(3);
    expect(screen.queryByRole("button", { name: /more$/ })).toBeNull();
  });

  it("still caps at 3 (silently, no summary row) when no overflow callback is supplied — back-compat for existing ToastProvider call sites", () => {
    render(
      <ToastProvider>
        <OverflowHarness />
      </ToastProvider>,
    );
    fireOverflowToasts(6);
    expect(screen.getAllByRole("alert")).toHaveLength(3);
    expect(screen.queryByRole("button", { name: /more$/ })).toBeNull();
  });

  it("transient MAX_STACK cap is unaffected by the persistent-overflow cap", () => {
    renderHarness();
    fireEvent.click(screen.getByRole("button", { name: "one" }));
    fireEvent.click(screen.getByRole("button", { name: "two" }));
    fireEvent.click(screen.getByRole("button", { name: "three" }));
    fireEvent.click(screen.getByRole("button", { name: "four" }));
    expect(screen.getAllByRole("status")).toHaveLength(3);
    expect(screen.queryByText("toast-one")).toBeNull();
  });
});
