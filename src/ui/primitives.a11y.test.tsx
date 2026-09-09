import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";

import { PrimitiveGallery } from "./PrimitiveGallery";

const AXE_OPTIONS = {
  rules: {
    region: { enabled: false },
    "color-contrast": { enabled: false },
    "heading-order": { enabled: false },
  },
};

// Axe smoke test over the full primitive gallery, so the shared library keeps a
// clean accessibility baseline (labelled controls, alert role on errors, valid
// roles/ARIA) and every primitive added to the gallery is covered automatically.
// Disabled rules: `region`/`color-contrast` (jsdom has no layout/contrast), and
// `heading-order` (the catalog deliberately renders SectionHeader at h2/h3/h4
// out of natural document order to show the variants).
describe("UI primitive gallery accessibility", () => {
  it("renders with no axe violations", async () => {
    const { container } = render(<PrimitiveGallery />);

    const results = await axe(container, AXE_OPTIONS);

    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  // Modal/FocusOverlay portal to document.body (outside the gallery's own
  // container), so their demos are scanned separately, over the whole body,
  // once each is actually open.
  it("Modal demo: opens on trigger, is axe-clean while open, and returns focus to the trigger on close", async () => {
    const user = userEvent.setup();
    render(<PrimitiveGallery />);

    const trigger = screen.getByRole("button", { name: "Open modal" });
    await user.click(trigger);

    expect(screen.getByRole("dialog", { name: "Modal demo" })).toBeInTheDocument();
    const results = await axe(document.body, AXE_OPTIONS);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Modal demo" })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("FocusOverlay demo: opens on trigger, is axe-clean while open, and returns focus to the trigger on close", async () => {
    const user = userEvent.setup();
    render(<PrimitiveGallery />);

    const trigger = screen.getByRole("button", { name: "Open focus overlay" });
    await user.click(trigger);

    expect(screen.getByRole("dialog", { name: "Focus overlay demo" })).toBeInTheDocument();
    const results = await axe(document.body, AXE_OPTIONS);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);

    await user.click(screen.getByRole("button", { name: "Exit focus mode" }));
    expect(screen.queryByRole("dialog", { name: "Focus overlay demo" })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
});
