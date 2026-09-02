import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ConfigBanner, type ConfigCondition } from "./ConfigBanner";

/**
 * The config-state banner (ADR 0087 dec. 5): a property of the app stated ONCE
 * above the stream, dismissible, never repeated per row. Rendered directly with
 * explicit conditions (TodayScreen wires none until D3a).
 */
describe("ConfigBanner (ADR 0087 dec. 5)", () => {
  it("renders nothing when there are no conditions", () => {
    const { container } = render(<ConfigBanner conditions={[]} />);
    expect(container.querySelector(".today-config-banner")).toBeNull();
  });

  it("renders a condition once, with its detail, and dismisses it in place", async () => {
    const user = userEvent.setup();
    const conditions: ConfigCondition[] = [
      { id: "src_down", message: "Source unreachable for 2 days", detail: "media may be delayed." },
    ];
    render(<ConfigBanner conditions={conditions} />);

    expect(screen.getByText("Source unreachable for 2 days")).toBeInTheDocument();
    expect(screen.getByText("media may be delayed.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("Source unreachable for 2 days")).not.toBeInTheDocument();
  });

  it("fires a condition's action", async () => {
    const user = userEvent.setup();
    let clicked = false;
    render(
      <ConfigBanner
        conditions={[
          {
            id: "x",
            message: "Provider misconfigured",
            action: {
              label: "Sources",
              onClick: () => {
                clicked = true;
              },
            },
          },
        ]}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Sources" }));
    expect(clicked).toBe(true);
  });
});
