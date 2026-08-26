import { describe, expect, it, vi } from "vitest";
import { handleCloseRequested } from "./handleCloseRequested";

describe("handleCloseRequested", () => {
  it("close request with a dirty tool prevents default and asks", () => {
    const preventDefault = vi.fn();
    const ask = vi.fn();
    handleCloseRequested({ preventDefault }, { isDirty: () => true, ask });

    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  it("close request with a clean tool does nothing", () => {
    const preventDefault = vi.fn();
    const ask = vi.fn();
    handleCloseRequested({ preventDefault }, { isDirty: () => false, ask });

    expect(preventDefault).not.toHaveBeenCalled();
    expect(ask).not.toHaveBeenCalled();
  });
});
