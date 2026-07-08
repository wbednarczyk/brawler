import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { Skeleton } from "./index";

describe("Skeleton", () => {
  it("renders a single bar block variant as a polite loading status", () => {
    render(<Skeleton />);
    const node = screen.getByRole("status");
    expect(node).toHaveClass("ui-skeleton", "ui-skeleton-block");
    expect(node).toHaveAttribute("aria-busy", "true");
    expect(node.querySelectorAll(".ui-skeleton-bar")).toHaveLength(1);
  });

  it("renders count rows for the list-row variant", () => {
    render(<Skeleton variant="list-row" count={4} label="Loading notes…" />);
    const node = screen.getByRole("status", { name: "Loading notes…" });
    expect(node).toHaveClass("ui-skeleton-list-row");
    expect(node.querySelectorAll(".ui-skeleton-bar")).toHaveLength(4);
  });

  it("keeps a site className", () => {
    const { container } = render(<Skeleton className="x-panel-loading" />);
    expect(container.querySelector(".ui-skeleton.x-panel-loading")).not.toBeNull();
  });
});
