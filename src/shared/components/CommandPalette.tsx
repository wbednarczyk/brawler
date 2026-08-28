import { useEffect, useState } from "react";
import { Modal, SearchField } from "../../ui";
import type { Verb } from "../verbs";

// Shared command palette — the keyboard-first launcher (⌘K). A self-contained,
// controlled presentation unit: it renders a filtered, arrow/enter-navigable
// list of {id,label,run} commands inside a Modal. The command set is supplied
// by the caller — the global palette (src/app/commandPalette.tsx, mounted in
// AppShell) feeds it the merged app + contextual list. Filter + keyboard nav
// live here.

// `actionKey`/`verb` (ADR 0104 dec. 3, F3a S3): stable, label-independent
// identity for a command plus its dictionary verb — the copy gate
// (src/app/paletteCopy.test.ts) checks every producer's labels against
// `verb` and that no two verbs share one `actionKey`.
export type PaletteCommand = { id: string; label: string; run: () => void; actionKey: string; verb: Verb };

export function CommandPalette({
  open,
  commands,
  onClose,
  text,
}: {
  open: boolean;
  commands: PaletteCommand[];
  onClose: () => void;
  text: (s: string) => string;
}) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
    }
  }, [open]);

  const filtered = query.trim()
    ? commands.filter((command) => command.label.toLowerCase().includes(query.trim().toLowerCase()))
    : commands;
  const clampedActive = Math.min(active, Math.max(0, filtered.length - 1));

  function run(index: number) {
    const command = filtered[index];
    if (command) {
      command.run();
      onClose();
    }
  }

  return (
    <Modal open={open} onClose={onClose} title={text("Command palette")} ariaLabel={text("Command palette")}>
      <div className="command-palette">
        <SearchField
          ariaLabel={text("Search commands")}
          className="search-box"
          placeholder={text("Type to filter commands…")}
          value={query}
          onChange={(value) => {
            setQuery(value);
            setActive(0);
          }}
          inputProps={{
            autoFocus: true,
            onKeyDown: (event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setActive((index) => Math.min(index + 1, filtered.length - 1));
              } else if (event.key === "ArrowUp") {
                event.preventDefault();
                setActive((index) => Math.max(index - 1, 0));
              } else if (event.key === "Enter") {
                event.preventDefault();
                run(clampedActive);
              }
            },
          }}
        />
        <ul className="command-palette-list">
          {filtered.length === 0 ? (
            <li className="command-palette-empty">{text("No matching commands.")}</li>
          ) : null}
          {filtered.map((command, index) => (
            <li key={command.id}>
              <button
                type="button"
                className={["command-palette-item", index === clampedActive ? "is-active" : ""]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => run(index)}
                onMouseEnter={() => setActive(index)}
              >
                {command.label}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Modal>
  );
}
