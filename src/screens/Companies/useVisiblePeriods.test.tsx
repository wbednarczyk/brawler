import { useRef } from "react";
import { describe, expect, it, beforeEach, afterEach } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  useVisiblePeriods,
  periodExpanderAccessibleName,
  periodExpanderVisibleLabel,
} from "./useVisiblePeriods";

// A capturing fake ResizeObserver — the real one is stubbed as a no-op in
// src/test/setup.ts (jsdom has none), so a resize-driven recompute must be
// simulated by invoking the constructor callback the hook installed.
class FakeResizeObserver {
  static instances: FakeResizeObserver[] = [];
  callback: ResizeObserverCallback;
  disconnected = false;
  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    FakeResizeObserver.instances.push(this);
  }
  observe() {}
  unobserve() {}
  disconnect() {
    this.disconnected = true;
  }
  fire() {
    this.callback([], this as unknown as ResizeObserver);
  }
}

const identity = (value: string) => value;

function Harness(props: {
  total: number;
  clientWidth: number;
  periodWidth: number;
  stickyWidth: number;
  expanderWidth?: number;
  measureKey?: string;
  variant?: string;
  onResult: (result: ReturnType<typeof useVisiblePeriods>) => void;
}) {
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const result = useVisiblePeriods({
    scrollerRef,
    total: props.total,
    measureKey: props.measureKey ?? "k",
    measureExpanderWidth: () => props.expanderWidth ?? 0,
    measureVariant: () => props.variant ?? "",
    measurePeriodWidth: () => props.periodWidth,
    measureFixedWidth: () => props.stickyWidth,
  });
  props.onResult(result);
  return (
    <div
      ref={(node) => {
        scrollerRef.current = node;
        if (node) Object.defineProperty(node, "clientWidth", { value: props.clientWidth, configurable: true });
      }}
    >
      <span>visibleCount:{result.visibleCount}</span>
      <span>hiddenCount:{result.hiddenCount}</span>
      <span>expanded:{String(result.expanded)}</span>
      <button type="button" onClick={result.toggle}>
        toggle
      </button>
    </div>
  );
}

describe("useVisiblePeriods", () => {
  let originalRO: typeof ResizeObserver;

  beforeEach(() => {
    originalRO = globalThis.ResizeObserver;
    FakeResizeObserver.instances = [];
    globalThis.ResizeObserver = FakeResizeObserver as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    globalThis.ResizeObserver = originalRO;
  });

  it("derives capacity from the mocked scroller/period widths with no upper cap", () => {
    // available = 1000 - 100(sticky) = 900; periodWidth 100 -> floor(9).
    render(<Harness total={20} clientWidth={1000} periodWidth={100} stickyWidth={100} onResult={() => {}} />);
    expect(screen.getByText("visibleCount:9")).toBeInTheDocument();
    expect(screen.getByText("hiddenCount:11")).toBeInTheDocument();
  });

  it("reserves the expander column only when some periods must hide", () => {
    // available = 1000 - 100 = 900; 9 periods × 100 fit exactly → all shown,
    // no expander; a tenth period forces the reservation (900 - 44 → 8).
    const { rerender } = render(
      <Harness total={9} clientWidth={1000} periodWidth={100} stickyWidth={100} expanderWidth={44} onResult={() => {}} />,
    );
    expect(screen.getByText("visibleCount:9")).toBeInTheDocument();
    expect(screen.getByText("hiddenCount:0")).toBeInTheDocument();
    rerender(
      <Harness total={10} clientWidth={1000} periodWidth={100} stickyWidth={100} expanderWidth={44} onResult={() => {}} />,
    );
    expect(screen.getByText("visibleCount:8")).toBeInTheDocument();
    expect(screen.getByText("hiddenCount:2")).toBeInTheDocument();
  });

  it("re-measures when the presentation key or the displayed column set changes", () => {
    const { rerender } = render(
      <Harness total={20} clientWidth={1000} periodWidth={100} stickyWidth={100} measureKey="a" variant="value" onResult={() => {}} />,
    );
    expect(screen.getByText("visibleCount:9")).toBeInTheDocument();
    // Wider cells under a new key (a value edit, a late provenance chip).
    rerender(
      <Harness total={20} clientWidth={1000} periodWidth={150} stickyWidth={100} measureKey="b" variant="value" onResult={() => {}} />,
    );
    expect(screen.getByText("visibleCount:6")).toBeInTheDocument();
    // A tier flip that unfolds columns (wider group) under the SAME key: the
    // resize recompute notices the variant and measures again.
    rerender(
      <Harness total={20} clientWidth={1000} periodWidth={300} stickyWidth={100} measureKey="b" variant="value,qoq,yoy" onResult={() => {}} />,
    );
    act(() => FakeResizeObserver.instances[FakeResizeObserver.instances.length - 1]?.fire());
    expect(screen.getByText("visibleCount:3")).toBeInTheDocument();
  });

  it("clamps a tiny available width up to the minimum capacity of 1", () => {
    // available = 50 - 100 = negative -> floor negative -> clamp to 1.
    render(<Harness total={5} clientWidth={50} periodWidth={100} stickyWidth={100} onResult={() => {}} />);
    expect(screen.getByText("visibleCount:1")).toBeInTheDocument();
    expect(screen.getByText("hiddenCount:4")).toBeInTheDocument();
  });

  it("falls back to capacity 1 when the period width is not yet measurable (zero)", () => {
    render(<Harness total={5} clientWidth={1000} periodWidth={0} stickyWidth={0} onResult={() => {}} />);
    expect(screen.getByText("visibleCount:1")).toBeInTheDocument();
  });

  it("shows no hidden periods and full capacity when total fits", () => {
    render(<Harness total={3} clientWidth={1000} periodWidth={100} stickyWidth={100} onResult={() => {}} />);
    expect(screen.getByText("visibleCount:3")).toBeInTheDocument();
    expect(screen.getByText("hiddenCount:0")).toBeInTheDocument();
  });

  it("expanded state ignores capacity changes from a later resize", async () => {
    const user = userEvent.setup();
    render(<Harness total={20} clientWidth={1000} periodWidth={100} stickyWidth={100} onResult={() => {}} />);
    expect(screen.getByText("visibleCount:9")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "toggle" }));
    expect(screen.getByText("expanded:true")).toBeInTheDocument();
    expect(screen.getByText("visibleCount:20")).toBeInTheDocument();

    // Simulate a resize to a much narrower width — capacity would shrink to
    // 1, but the expanded user must not be yanked back.
    expect(FakeResizeObserver.instances).toHaveLength(1);
    FakeResizeObserver.instances[0].fire();
    expect(screen.getByText("visibleCount:20")).toBeInTheDocument();
    expect(screen.getByText("hiddenCount:0")).toBeInTheDocument();
  });

  it("disconnects the ResizeObserver on unmount", () => {
    const { unmount } = render(
      <Harness total={5} clientWidth={1000} periodWidth={100} stickyWidth={0} onResult={() => {}} />,
    );
    expect(FakeResizeObserver.instances).toHaveLength(1);
    expect(FakeResizeObserver.instances[0].disconnected).toBe(false);
    unmount();
    expect(FakeResizeObserver.instances[0].disconnected).toBe(true);
  });
});

describe("periodExpanderVisibleLabel", () => {
  it("shows Expand earlier when collapsed, Collapse earlier when expanded", () => {
    expect(periodExpanderVisibleLabel(false, identity)).toBe("Expand earlier");
    expect(periodExpanderVisibleLabel(true, identity)).toBe("Collapse earlier");
  });
});

describe("periodExpanderAccessibleName", () => {
  it("names the hidden count in English, singular vs plural", () => {
    expect(periodExpanderAccessibleName(false, 1, "en", identity)).toBe("Expand 1 earlier period");
    expect(periodExpanderAccessibleName(false, 9, "en", identity)).toBe("Expand 9 earlier periods");
  });

  it("declines the Polish adjective + noun together (three plural categories)", () => {
    expect(periodExpanderAccessibleName(false, 1, "pl", identity)).toBe("Rozwiń 1 starszy okres");
    expect(periodExpanderAccessibleName(false, 3, "pl", identity)).toBe("Rozwiń 3 starsze okresy");
    expect(periodExpanderAccessibleName(false, 12, "pl", identity)).toBe("Rozwiń 12 starszych okresów");
  });

  it("drops the count and uses the Collapse label when expanded", () => {
    expect(periodExpanderAccessibleName(true, 9, "en", identity)).toBe("Collapse earlier");
    expect(periodExpanderAccessibleName(true, 9, "pl", identity)).toBe("Collapse earlier");
  });
});
