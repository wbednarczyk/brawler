import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CreateViewModal } from "./CreateViewModal";

describe("CreateViewModal (ADR 0057 visual-first grid picker)", () => {
  it("uses presets, a two-way slider/input, and returns the spec", async () => {
    const user = userEvent.setup();
    const onCreate = vi.fn();
    render(<CreateViewModal open onClose={() => {}} onCreate={onCreate} />);

    // Create is disabled until the view is named.
    const create = screen.getByRole("button", { name: /Create view/i });
    expect(create).toBeDisabled();

    // A preset sets cols/rows; the exact number inputs reflect it (visual-first).
    await user.click(screen.getByRole("button", { name: /2.3/ }));
    const spinbuttons = screen.getAllByRole("spinbutton");
    const sliders = screen.getAllByRole("slider");
    expect((spinbuttons[0] as HTMLInputElement).value).toBe("2");
    expect((spinbuttons[1] as HTMLInputElement).value).toBe("3");

    // Typing the exact value updates the slider too (two-way binding).
    fireEvent.change(spinbuttons[0], { target: { value: "4" } });
    expect((sliders[0] as HTMLInputElement).value).toBe("4");

    // Dragging the slider updates the number input (the other direction).
    fireEvent.change(sliders[1], { target: { value: "5" } });
    expect((spinbuttons[1] as HTMLInputElement).value).toBe("5");

    // Naming enables Create; it returns the chosen name + grid.
    await user.type(screen.getByLabelText("View name"), "Earnings");
    expect(create).toBeEnabled();
    await user.click(create);
    expect(onCreate).toHaveBeenCalledWith({ name: "Earnings", cols: 4, rows: 5 });
  });
});
