import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Modal } from "./Modal";

describe("Modal", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <Modal open={false} onClose={() => {}} title="Title">
        body
      </Modal>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders a labelled dialog with title, body and footer when open", () => {
    render(
      <Modal open onClose={() => {}} title="Review" ariaLabel="Review dialog" footer={<button>OK</button>}>
        <p>body content</p>
      </Modal>,
    );
    const dialog = screen.getByRole("dialog", { name: "Review dialog" });
    expect(dialog).toBeInTheDocument();
    expect(screen.getByText("body content")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "OK" })).toBeInTheDocument();
  });

  it("closes on Escape and on the close button", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose} title="Review">
        body
      </Modal>,
    );

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(1);

    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
