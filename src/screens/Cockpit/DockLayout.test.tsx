import { describe, it, expect, vi } from "vitest";
import type { DockviewApi, DockviewGroupPanel } from "dockview";
import { popOutOrFloat } from "./DockLayout";

// Pop-out degradation (ADR 0053 decision 2A, Faza 5): the OS-window pop-out is a
// gated capability that must still be validated on native Windows, so the action
// must never lose a panel. Here we prove the fallback deterministically — jsdom
// and an un-bridged Tauri both make `addPopoutGroup` resolve false / throw, and
// the group must then float instead.

const group = {} as DockviewGroupPanel;

function makeApi(addPopoutGroup: ReturnType<typeof vi.fn>) {
  const addFloatingGroup = vi.fn();
  const api = { addPopoutGroup, addFloatingGroup } as unknown as Pick<
    DockviewApi,
    "addPopoutGroup" | "addFloatingGroup"
  >;
  return { api, addFloatingGroup };
}

describe("popOutOrFloat", () => {
  it("pops out to an OS window when one opens (no fallback)", async () => {
    const { api, addFloatingGroup } = makeApi(vi.fn().mockResolvedValue(true));
    expect(await popOutOrFloat(api, group)).toBe("popout");
    expect(addFloatingGroup).not.toHaveBeenCalled();
  });

  it("floats when the OS window is blocked (addPopoutGroup resolves false)", async () => {
    const { api, addFloatingGroup } = makeApi(vi.fn().mockResolvedValue(false));
    expect(await popOutOrFloat(api, group)).toBe("float");
    expect(addFloatingGroup).toHaveBeenCalledWith(group);
  });

  it("floats when addPopoutGroup throws (un-bridged Tauri)", async () => {
    const { api, addFloatingGroup } = makeApi(vi.fn().mockRejectedValue(new Error("blocked")));
    expect(await popOutOrFloat(api, group)).toBe("float");
    expect(addFloatingGroup).toHaveBeenCalledWith(group);
  });
});
