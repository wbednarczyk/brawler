import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

import { ToastProvider, useUndoableDelete, type UndoableDeleteConfig } from "./index";

function Harness({ config }: { config: Omit<UndoableDeleteConfig, "message" | "undoLabel"> }) {
  const run = useUndoableDelete();
  return (
    <button
      type="button"
      onClick={() => run({ message: "Note deleted", undoLabel: "Undo", ...config })}
    >
      delete
    </button>
  );
}

function renderHarness(config: Omit<UndoableDeleteConfig, "message" | "undoLabel">) {
  return render(
    <ToastProvider>
      <Harness config={config} />
    </ToastProvider>,
  );
}

describe("useUndoableDelete", () => {
  it("performs the delete immediately, then shows an undo toast", async () => {
    const perform = vi.fn().mockResolvedValue(undefined);
    const onPerformed = vi.fn();
    renderHarness({ perform, restore: vi.fn().mockResolvedValue(undefined), onPerformed });

    fireEvent.click(screen.getByRole("button", { name: "delete" }));

    await waitFor(() => expect(perform).toHaveBeenCalledTimes(1));
    expect(onPerformed).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("Note deleted")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
  });

  it("calls restore and onRestored when Undo is clicked", async () => {
    const restore = vi.fn().mockResolvedValue(undefined);
    const onRestored = vi.fn();
    renderHarness({ perform: vi.fn().mockResolvedValue(undefined), restore, onRestored });

    fireEvent.click(screen.getByRole("button", { name: "delete" }));
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(restore).toHaveBeenCalledTimes(1));
    expect(onRestored).toHaveBeenCalledTimes(1);
  });

  it("routes a delete failure to onError and shows no toast", async () => {
    const onError = vi.fn();
    const boom = new Error("nope");
    renderHarness({ perform: vi.fn().mockRejectedValue(boom), restore: vi.fn(), onError });

    fireEvent.click(screen.getByRole("button", { name: "delete" }));

    await waitFor(() => expect(onError).toHaveBeenCalledWith(boom));
    expect(screen.queryByText("Note deleted")).toBeNull();
  });
});
