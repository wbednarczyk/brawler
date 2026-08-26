import { useState } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import {
  CommandPaletteProvider,
  useCommandPalette,
  useCommandPaletteCommands,
  type PaletteCommand,
} from "./commandPalette";

const identity = (s: string) => s;

function OpenButton() {
  const { open, close, isOpen } = useCommandPalette();
  return (
    <>
      <button type="button" onClick={open}>
        launch
      </button>
      <button type="button" onClick={close}>
        dismiss
      </button>
      <span>{isOpen ? "state:open" : "state:closed"}</span>
    </>
  );
}

function Contributor({ commands }: { commands: PaletteCommand[] }) {
  useCommandPaletteCommands("contrib", commands);
  return null;
}

function paletteLabels() {
  const dialog = screen.getByRole("dialog", { name: "Command palette" });
  return within(dialog)
    .getAllByRole("button")
    .map((button) => button.textContent)
    .filter((label) => label && label !== "×");
}

describe("CommandPaletteProvider", () => {
  it("merges app + contextual commands, app first, deduped by id with contextual winning", async () => {
    const user = userEvent.setup();
    const appCommands: PaletteCommand[] = [
      { id: "a", label: "Alpha", run: vi.fn(), actionKey: "test.a", verb: "open" },
      { id: "b", label: "Bravo", run: vi.fn(), actionKey: "test.b", verb: "open" },
    ];
    const charlie = vi.fn();
    const contextual: PaletteCommand[] = [
      { id: "b", label: "Bravo CTX", run: vi.fn(), actionKey: "test.b", verb: "open" },
      { id: "c", label: "Charlie", run: charlie, actionKey: "test.c", verb: "open" },
    ];

    render(
      <CommandPaletteProvider appCommands={appCommands} text={identity}>
        <OpenButton />
        <Contributor commands={contextual} />
      </CommandPaletteProvider>,
    );

    await user.click(screen.getByRole("button", { name: "launch" }));

    // App order preserved; contextual "Bravo CTX" wins on id collision at Bravo's
    // position; remaining contextual "Charlie" appended.
    expect(paletteLabels()).toEqual(["Alpha", "Bravo CTX", "Charlie"]);

    await user.click(screen.getByRole("button", { name: "Charlie" }));
    expect(charlie).toHaveBeenCalledTimes(1);
    // Running a command closes the palette.
    expect(screen.getByText("state:closed")).toBeInTheDocument();
  });

  it("unregisters contextual commands on unmount", async () => {
    const user = userEvent.setup();
    const appCommands: PaletteCommand[] = [{ id: "a", label: "Alpha", run: vi.fn(), actionKey: "test.a", verb: "open" }];

    function Harness() {
      const [mounted, setMounted] = useState(true);
      return (
        <CommandPaletteProvider appCommands={appCommands} text={identity}>
          <OpenButton />
          {mounted ? (
            <Contributor commands={[{ id: "c", label: "Charlie", run: vi.fn(), actionKey: "test.c", verb: "open" }]} />
          ) : null}
          <button type="button" onClick={() => setMounted(false)}>
            drop
          </button>
        </CommandPaletteProvider>
      );
    }

    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "launch" }));
    expect(paletteLabels()).toEqual(["Alpha", "Charlie"]);
    await user.click(screen.getByRole("button", { name: "dismiss" }));

    await user.click(screen.getByRole("button", { name: "drop" }));
    await user.click(screen.getByRole("button", { name: "launch" }));
    expect(paletteLabels()).toEqual(["Alpha"]);
  });

  it("opens and closes via the context handles", async () => {
    const user = userEvent.setup();
    render(
      <CommandPaletteProvider appCommands={[{ id: "a", label: "Alpha", run: vi.fn(), actionKey: "test.a", verb: "open" }]} text={identity}>
        <OpenButton />
      </CommandPaletteProvider>,
    );

    expect(screen.getByText("state:closed")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "launch" }));
    expect(screen.getByText("state:open")).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Command palette" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "dismiss" }));
    expect(screen.getByText("state:closed")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
  });
});
