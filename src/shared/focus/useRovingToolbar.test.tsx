import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { useRovingToolbar } from "./useRovingToolbar";

// APG toolbar roving-tabindex (F3c S1, plan § Design 1): exactly one item
// carries `tabIndex=0` at a time; ArrowRight/ArrowLeft move it (wrapping),
// Home/End jump to the ends, and `focusItem` lets an owner (SpolkaScreen)
// move focus programmatically (the "entry"/"overview" focus intents).

function Toolbar({ count, initialIndex }: { count: number; initialIndex: number }) {
  const roving = useRovingToolbar({ count, initialIndex });
  return (
    <div role="toolbar" aria-label="Test toolbar">
      {Array.from({ length: count }, (_, i) => (
        <button key={i} type="button" {...roving.itemProps(i)}>
          Item {i}
        </button>
      ))}
    </div>
  );
}

describe("useRovingToolbar", () => {
  it("gives exactly one item tabIndex=0, at the initial index", () => {
    render(<Toolbar count={3} initialIndex={1} />);
    const items = screen.getAllByRole("button");
    expect(items.map((el) => el.getAttribute("tabindex"))).toEqual(["-1", "0", "-1"]);
  });

  it("ArrowRight moves the tab stop to the next item and focuses it", () => {
    render(<Toolbar count={3} initialIndex={0} />);
    const items = screen.getAllByRole("button");
    items[0].focus();
    fireEvent.keyDown(items[0], { key: "ArrowRight" });
    expect(items[1]).toHaveFocus();
    expect(items[1]).toHaveAttribute("tabindex", "0");
    expect(items[0]).toHaveAttribute("tabindex", "-1");
  });

  it("ArrowLeft wraps from the first item to the last", () => {
    render(<Toolbar count={3} initialIndex={0} />);
    const items = screen.getAllByRole("button");
    items[0].focus();
    fireEvent.keyDown(items[0], { key: "ArrowLeft" });
    expect(items[2]).toHaveFocus();
  });

  it("ArrowRight wraps from the last item to the first", () => {
    render(<Toolbar count={3} initialIndex={2} />);
    const items = screen.getAllByRole("button");
    items[2].focus();
    fireEvent.keyDown(items[2], { key: "ArrowRight" });
    expect(items[0]).toHaveFocus();
  });

  it("Home jumps to the first item, End to the last", () => {
    render(<Toolbar count={4} initialIndex={1} />);
    const items = screen.getAllByRole("button");
    items[1].focus();
    fireEvent.keyDown(items[1], { key: "End" });
    expect(items[3]).toHaveFocus();
    fireEvent.keyDown(items[3], { key: "Home" });
    expect(items[0]).toHaveFocus();
  });

  it("a plain Tab/click focus (onFocus) updates the tab stop without moving focus again", () => {
    render(<Toolbar count={3} initialIndex={0} />);
    const items = screen.getAllByRole("button");
    fireEvent.focus(items[2]);
    expect(items[2]).toHaveAttribute("tabindex", "0");
    expect(items[0]).toHaveAttribute("tabindex", "-1");
  });

  it("focusItem moves both the tab stop and actual DOM focus, and wraps", () => {
    function Harness() {
      const roving = useRovingToolbar({ count: 3, initialIndex: 0 });
      return (
        <div>
          <button type="button" onClick={() => roving.focusItem(-1)}>
            go
          </button>
          <div role="toolbar" aria-label="Test toolbar">
            {Array.from({ length: 3 }, (_, i) => (
              <button key={i} type="button" {...roving.itemProps(i)}>
                Item {i}
              </button>
            ))}
          </div>
        </div>
      );
    }
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "go" }));
    const items = screen.getAllByRole("button", { name: /^Item/ });
    expect(items[2]).toHaveFocus();
    expect(items[2]).toHaveAttribute("tabindex", "0");
  });
});
