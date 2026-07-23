import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SeverityLegend } from "./SeverityLegend";

describe("SeverityLegend", () => {
  it("is collapsed by default and opens on click", async () => {
    const user = userEvent.setup();
    render(<SeverityLegend />);

    const toggle = screen.getByRole("button", { name: /severity levels mean/i });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("dialog")).toBeNull();

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("lists the three severity levels with translated labels and the 3-day aging rule", async () => {
    const user = userEvent.setup();
    render(<SeverityLegend />);
    await user.click(screen.getByRole("button", { name: /severity levels mean/i }));

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Urgent")).toBeInTheDocument();
    expect(within(dialog).getByText("Notable")).toBeInTheDocument();
    expect(within(dialog).getByText("Routine")).toBeInTheDocument();
    // The aging rule is discoverable in the legend.
    expect(within(dialog).getByText(/3 days/i)).toBeInTheDocument();
  });

  it("closes on Escape (keyboard reachable)", async () => {
    const user = userEvent.setup();
    render(<SeverityLegend />);
    const toggle = screen.getByRole("button", { name: /severity levels mean/i });
    await user.click(toggle);
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(toggle).toHaveAttribute("aria-expanded", "false");
  });
});
