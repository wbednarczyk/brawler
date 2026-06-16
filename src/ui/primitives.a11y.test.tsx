import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { axe } from "jest-axe";

import { PrimitiveGallery } from "./PrimitiveGallery";

// Axe smoke test over the full primitive gallery, so the shared library keeps a
// clean accessibility baseline (labelled controls, alert role on errors, valid
// roles/ARIA) and every primitive added to the gallery is covered automatically.
// Disabled rules: `region`/`color-contrast` (jsdom has no layout/contrast), and
// `heading-order` (the catalog deliberately renders SectionHeader at h2/h3/h4
// out of natural document order to show the variants).
describe("UI primitive gallery accessibility", () => {
  it("renders with no axe violations", async () => {
    const { container } = render(<PrimitiveGallery />);

    const results = await axe(container, {
      rules: {
        region: { enabled: false },
        "color-contrast": { enabled: false },
        "heading-order": { enabled: false },
      },
    });

    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
