import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FocusOverlay } from "./FocusOverlay";

describe("FocusOverlay", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <FocusOverlay open={false} onClose={() => {}} title="Title">
        body
      </FocusOverlay>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders a labelled full-screen surface with eyebrow, title, actions and body", () => {
    render(
      <FocusOverlay
        open
        onClose={() => {}}
        title="Reading"
        ariaLabel="Report comparison"
        eyebrow="GPW:CDR"
        actions={<button>Save</button>}
      >
        <p>diff content</p>
      </FocusOverlay>,
    );
    const surface = screen.getByRole("dialog", { name: "Report comparison" });
    expect(surface).toBeInTheDocument();
    expect(screen.getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getByText("diff content")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeInTheDocument();
  });

  it("exits on Escape and on the close button", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <FocusOverlay open onClose={onClose} title="Reading">
        body
      </FocusOverlay>,
    );

    await user.click(screen.getByRole("button", { name: "Exit focus mode" }));
    expect(onClose).toHaveBeenCalledTimes(1);

    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
