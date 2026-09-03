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
    .getAllByRole("option")
    .map((option) => option.textContent)
    .filter((label) => label && label !== "×");
}

function paletteInput() {
  return screen.getByRole("combobox", { name: "Search commands" });
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

    await user.click(screen.getByRole("option", { name: "Charlie" }));
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

// S2 (#197 F3c) — APG combobox + listbox semantics (plan § Design 6, contract
// § 7 palette recovery). The input is the combobox; the list is a listbox of
// options; aria-activedescendant tracks the active option so a screen reader
// announces it without moving DOM focus off the input.
describe("CommandPaletteProvider — combobox/listbox semantics", () => {
  const commands: PaletteCommand[] = [
    { id: "a", label: "Alpha", run: vi.fn(), actionKey: "test.a", verb: "open" },
    { id: "b", label: "Bravo", run: vi.fn(), actionKey: "test.b", verb: "open" },
    { id: "c", label: "Charlie", run: vi.fn(), actionKey: "test.c", verb: "open" },
  ];

  function renderPalette(cmds: PaletteCommand[] = commands) {
    render(
      <CommandPaletteProvider appCommands={cmds} text={identity}>
        <OpenButton />
      </CommandPaletteProvider>,
    );
  }

  it("input is a combobox wired to the listbox, each option carries a stable id", async () => {
    const user = userEvent.setup();
    renderPalette();
    await user.click(screen.getByRole("button", { name: "launch" }));

    const input = paletteInput();
    expect(input).toHaveAttribute("aria-expanded", "true");
    expect(input).toHaveAttribute("aria-autocomplete", "list");
    const listbox = screen.getByRole("listbox", { name: "Commands" });
    expect(input).toHaveAttribute("aria-controls", listbox.id);

    const options = within(listbox).getAllByRole("option");
    expect(options).toHaveLength(3);
    for (const option of options) {
      expect(option.id).toBeTruthy();
    }
    // The active option (first, at rest) is announced via aria-activedescendant
    // and always resolves to a rendered option.
    expect(input).toHaveAttribute("aria-activedescendant", options[0]!.id);
    expect(options[0]).toHaveAttribute("aria-selected", "true");
  });

  it("aria-activedescendant follows ArrowDown/ArrowUp/Home/End and always resolves", async () => {
    const user = userEvent.setup();
    renderPalette();
    await user.click(screen.getByRole("button", { name: "launch" }));

    const input = paletteInput();
    const optionIds = () => within(screen.getByRole("listbox")).getAllByRole("option").map((o) => o.id);

    await user.keyboard("{ArrowDown}");
    expect(input).toHaveAttribute("aria-activedescendant", optionIds()[1]);
    await user.keyboard("{ArrowDown}");
    expect(input).toHaveAttribute("aria-activedescendant", optionIds()[2]);
    // Clamped at the end.
    await user.keyboard("{ArrowDown}");
    expect(input).toHaveAttribute("aria-activedescendant", optionIds()[2]);

    await user.keyboard("{Home}");
    expect(input).toHaveAttribute("aria-activedescendant", optionIds()[0]);
    await user.keyboard("{End}");
    expect(input).toHaveAttribute("aria-activedescendant", optionIds()[2]);
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveAttribute("aria-activedescendant", optionIds()[1]);
  });

  it("Enter runs the active command and closes the palette", async () => {
    const user = userEvent.setup();
    const run = vi.fn();
    renderPalette([{ id: "a", label: "Alpha", run, actionKey: "test.a", verb: "open" }]);
    await user.click(screen.getByRole("button", { name: "launch" }));

    await user.keyboard("{Enter}");
    expect(run).toHaveBeenCalledTimes(1);
    expect(screen.getByText("state:closed")).toBeInTheDocument();
  });

  it("clicking an option runs it", async () => {
    const user = userEvent.setup();
    const run = vi.fn();
    renderPalette([{ id: "a", label: "Alpha", run, actionKey: "test.a", verb: "open" }]);
    await user.click(screen.getByRole("button", { name: "launch" }));

    await user.click(screen.getByRole("option", { name: "Alpha" }));
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("shows 'No matching commands.' when nothing matches", async () => {
    const user = userEvent.setup();
    renderPalette();
    await user.click(screen.getByRole("button", { name: "launch" }));

    await user.type(paletteInput(), "zzz-no-match");
    expect(screen.getByText("No matching commands.")).toBeInTheDocument();
    expect(within(screen.getByRole("listbox")).queryAllByRole("option")).toHaveLength(0);
  });

  it("Escape closes the palette and returns focus to the button that opened it", async () => {
    const user = userEvent.setup();
    renderPalette();

    const trigger = screen.getByRole("button", { name: "launch" });
    trigger.focus();
    await user.click(trigger);
    expect(paletteInput()).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.getByText("state:closed")).toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
});
