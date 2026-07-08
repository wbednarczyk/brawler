import { describe, it, expect } from "vitest";
import { useState } from "react";
import { render, screen, fireEvent, act } from "@testing-library/react";

import { focusRowAfterRemove, useFocusAfterRemove } from "./focusAfterRemove";

describe("focusRowAfterRemove", () => {
  function mount(count: number) {
    const container = document.createElement("div");
    for (let index = 0; index < count; index += 1) {
      const row = document.createElement("button");
      row.className = "row";
      row.textContent = `row-${index}`;
      container.append(row);
    }
    document.body.append(container);
    return container;
  }

  it("focuses the row now occupying the removed index", () => {
    // Started with 4 rows, removed index 1 → 3 remain; focus lands on the row
    // that slid into slot 1 (was row-2).
    const container = mount(3);
    focusRowAfterRemove(container, 1, { rowSelector: ".row" });
    expect(document.activeElement).toBe(container.querySelectorAll(".row")[1]);
    container.remove();
  });

  it("clamps to the last row when the tail row was removed", () => {
    const container = mount(2);
    focusRowAfterRemove(container, 5, { rowSelector: ".row" });
    expect(document.activeElement).toBe(container.querySelectorAll(".row")[1]);
    container.remove();
  });

  it("focuses a per-row target when a focusSelector is given", () => {
    const container = document.createElement("div");
    const row = document.createElement("div");
    row.className = "row";
    const inner = document.createElement("button");
    inner.className = "primary";
    row.append(inner);
    container.append(row);
    document.body.append(container);

    focusRowAfterRemove(container, 0, { rowSelector: ".row", focusSelector: ".primary" });
    expect(document.activeElement).toBe(inner);
    container.remove();
  });

  it("falls back to the list container when no rows remain", () => {
    const container = mount(0);
    focusRowAfterRemove(container, 0, { rowSelector: ".row" });
    expect(document.activeElement).toBe(container);
    expect(container).toHaveAttribute("tabindex", "-1");
    container.remove();
  });
});

describe("useFocusAfterRemove", () => {
  function Harness({ onExternalDelete }: { onExternalDelete?: (fn: (key: string) => void) => void }) {
    const [items, setItems] = useState(["a", "b", "c"]);
    const { listRef } = useFocusAfterRemove<HTMLUListElement>(items, {
      rowSelector: ".t-row",
    });
    // Expose an out-of-list delete to model a detail-pane delete button.
    if (onExternalDelete) {
      onExternalDelete((key) => setItems((current) => current.filter((value) => value !== key)));
    }
    return (
      <div>
        <ul ref={listRef}>
          {items.map((item) => (
            <li key={item}>
              <button
                className="t-row"
                onClick={() => setItems((current) => current.filter((value) => value !== item))}
              >
                {item}
              </button>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  it("moves focus to the row that slides into the removed slot", () => {
    render(<Harness />);
    const rowB = screen.getByText("b");
    rowB.focus();
    fireEvent.click(rowB);
    // b removed → the row now in slot 1 is "c"; focus follows it.
    expect(document.activeElement).toBe(screen.getByText("c"));
  });

  it("clamps to the new last row when the tail row is removed", () => {
    render(<Harness />);
    const rowC = screen.getByText("c");
    rowC.focus();
    fireEvent.click(rowC);
    expect(document.activeElement).toBe(screen.getByText("b"));
  });

  it("moves focus into the list even when the delete control was outside it", () => {
    let deleteExternally: (key: string) => void = () => {};
    render(<Harness onExternalDelete={(fn) => (deleteExternally = fn)} />);
    // Focus starts on <body> (nothing focused), as it would after an external
    // delete button unmounts — the hook still lands focus on the next row.
    act(() => {
      deleteExternally("a");
    });
    expect(document.activeElement).toBe(screen.getByText("b"));
  });

  it("does not steal focus when the user is working elsewhere", () => {
    let deleteExternally: (key: string) => void = () => {};
    render(
      <>
        <input aria-label="elsewhere" />
        <Harness onExternalDelete={(fn) => (deleteExternally = fn)} />
      </>,
    );
    const elsewhere = screen.getByLabelText("elsewhere");
    elsewhere.focus();
    act(() => {
      deleteExternally("a");
    });
    // Focus stays in the unrelated input — background removals never yank focus.
    expect(document.activeElement).toBe(elsewhere);
  });
});
