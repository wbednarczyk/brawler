// Pins the click-actionability wrapper in `setup.ts` (testing.md § Frontend
// test responsibilities rule 7, #493). Ordering is driven by the click
// sequence's own events or by microtask-only dispatch — never by wall-clock
// timing.
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { configure, getConfig, render, screen } from "@testing-library/react";
import userEvent, { userEvent as namedUserEvent } from "@testing-library/user-event";

function Gate({ disabled, onClick }: { disabled: boolean; onClick: () => void }) {
  return (
    <button disabled={disabled} onClick={onClick}>
      Gate <span>label</span>
    </button>
  );
}

function SelfRemoving({ onClick }: { onClick: () => void }) {
  const [gone, setGone] = useState(false);
  return gone ? null : (
    <button
      onClick={() => {
        onClick();
        setGone(true);
      }}
    >
      Remove me
    </button>
  );
}

// Reacts to `pointerdown` — the first event of user-event's click sequence —
// so the target changes deterministically BEFORE the click would dispatch.
function ChangesOnPress({ effect, onClick }: { effect: "remove" | "disable"; onClick: () => void }) {
  const [pressed, setPressed] = useState(false);
  if (pressed && effect === "remove") return null;
  return (
    <button disabled={pressed} onClick={onClick} onPointerDown={() => setPressed(true)}>
      Press me
    </button>
  );
}

async function withShortBudget(run: () => Promise<void>) {
  const previous = getConfig().asyncUtilTimeout;
  configure({ asyncUtilTimeout: 100 });
  try {
    await run();
  } finally {
    configure({ asyncUtilTimeout: previous });
  }
}

describe("user-event click actionability wrapper", () => {
  it("waits for a disabled button to become enabled, then clicks exactly once", async () => {
    const onClick = vi.fn();
    const { rerender } = render(<Gate disabled onClick={onClick} />);
    const button = screen.getByRole("button", { name: /Gate/ });
    // `delay: null` makes user-event's dispatch microtask-only, so after one
    // macrotask an unguarded click has already no-op'ed on the disabled button
    // — only then is it enabled.
    const user = userEvent.setup({ delay: null });

    const clicking = user.click(button);
    await new Promise((resolve) => setTimeout(resolve, 0));
    rerender(<Gate disabled={false} onClick={onClick} />);
    await clicking;

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("throws BY NAME when the target stays disabled past the timeout", async () => {
    await withShortBudget(async () => {
      const onClick = vi.fn();
      render(<Gate disabled onClick={onClick} />);
      const button = screen.getByRole("button", { name: /Gate/ });
      const user = userEvent.setup();

      await expect(user.click(button)).rejects.toThrow(/stayed disabled/);
      expect(onClick).not.toHaveBeenCalled();
    });
  });

  it("throws BY NAME, and fast, when the target is detached before the click", async () => {
    const onClick = vi.fn();
    const { unmount } = render(<Gate disabled={false} onClick={onClick} />);
    const button = screen.getByRole("button", { name: /Gate/ });
    unmount();
    const user = userEvent.setup();

    const start = Date.now();
    await expect(user.click(button)).rejects.toThrow(/detached/);
    expect(Date.now() - start).toBeLessThan(500);
  });

  it("throws BY NAME when the target is removed while the click is dispatching", async () => {
    const onClick = vi.fn();
    render(<ChangesOnPress effect="remove" onClick={onClick} />);
    const user = userEvent.setup();

    await expect(user.click(screen.getByRole("button", { name: "Press me" }))).rejects.toThrow(
      /detached/,
    );
    expect(onClick).not.toHaveBeenCalled();
  });

  it("throws BY NAME when the target becomes disabled while the click is dispatching", async () => {
    const onClick = vi.fn();
    render(<ChangesOnPress effect="disable" onClick={onClick} />);
    const user = userEvent.setup();

    await expect(user.click(screen.getByRole("button", { name: "Press me" }))).rejects.toThrow(
      /received no click/,
    );
    expect(onClick).not.toHaveBeenCalled();
  });

  it("fails fast when the target is detached while waiting for it to enable", async () => {
    const onClick = vi.fn();
    const { unmount } = render(<Gate disabled onClick={onClick} />);
    const button = screen.getByRole("button", { name: /Gate/ });
    const user = userEvent.setup();

    const start = Date.now();
    const clicking = user.click(button);
    unmount();

    await expect(clicking).rejects.toThrow(/detached/);
    expect(Date.now() - start).toBeLessThan(500);
  });

  it("accepts a click whose own handler removes the target", async () => {
    const onClick = vi.fn();
    render(<SelfRemoving onClick={onClick} />);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Remove me" }));

    expect(onClick).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Remove me" })).toBeNull();
  });

  it("guards the direct userEvent.click API (default import)", async () => {
    await withShortBudget(async () => {
      render(<Gate disabled onClick={vi.fn()} />);
      const button = screen.getByRole("button", { name: /Gate/ });

      await expect(userEvent.click(button)).rejects.toThrow(/stayed disabled/);
    });
  });

  it("guards the named `userEvent` export's setup().click too", async () => {
    await withShortBudget(async () => {
      render(<Gate disabled onClick={vi.fn()} />);
      const button = screen.getByRole("button", { name: /Gate/ });

      await expect(namedUserEvent.setup().click(button)).rejects.toThrow(/stayed disabled/);
    });
  });

  it("guards dblClick the same way", async () => {
    await withShortBudget(async () => {
      render(<Gate disabled onClick={vi.fn()} />);
      const button = screen.getByRole("button", { name: /Gate/ });

      await expect(userEvent.setup().dblClick(button)).rejects.toThrow(/stayed disabled/);
    });
  });

  it("clicks an enabled button, and a span inside an enabled button, normally", async () => {
    const onClick = vi.fn();
    render(<Gate disabled={false} onClick={onClick} />);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /Gate/ }));
    await user.click(screen.getByText("label"));

    expect(onClick).toHaveBeenCalledTimes(2);
  });
});
