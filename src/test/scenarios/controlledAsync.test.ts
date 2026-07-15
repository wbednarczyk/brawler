import { describe, expect, it } from "vitest";

import { createControlledAsync } from "./controlledAsync";
import type { CommandError } from "../../api/generated/CommandError";

describe("controlled-async — hold/pending/release/reject (ADR 0081 Q2, Radicle a9992e2)", () => {
  it("an invocation with no matching hold passes straight through", async () => {
    const ca = createControlledAsync(() => Promise.resolve("value"), () => {});
    await expect(ca.invoke("list_x", {})).resolves.toBe("value");
    expect(ca.controls.pending()).toEqual([]);
  });

  it("before-handler hold defers the handler until release, then runs it", async () => {
    const invokeCalls: string[] = [];
    const stubInvoke = (command: string) => {
      invokeCalls.push(command);
      return Promise.resolve("resolved-value");
    };
    const ca = createControlledAsync(stubInvoke, () => {});
    const id = ca.controls.hold({ command: "list_x" });
    const promise = ca.invoke("list_x", {});
    expect(invokeCalls).toHaveLength(0); // handler hasn't run — held before-handler
    expect(ca.controls.pending()).toEqual([{ id, command: "list_x", args: {}, phase: "before-handler" }]);
    ca.controls.release(id);
    await expect(promise).resolves.toBe("resolved-value");
    expect(invokeCalls).toEqual(["list_x"]);
  });

  it("two held invocations complete newest-before-oldest in a chosen order", async () => {
    const stubInvoke = (command: string, args?: Record<string, unknown>) => Promise.resolve({ command, args });
    const ca = createControlledAsync(stubInvoke, () => {});
    const olderId = ca.controls.hold({ command: "list_x" });
    const newerId = ca.controls.hold({ command: "list_x" });
    const olderPromise = ca.invoke("list_x", { intent: "older" });
    const newerPromise = ca.invoke("list_x", { intent: "newer" });
    expect(ca.controls.pending().map((p) => p.id).sort()).toEqual([newerId, olderId].sort());

    const completionOrder: string[] = [];
    void olderPromise.then(() => completionOrder.push("older"));
    void newerPromise.then(() => completionOrder.push("newer"));

    ca.controls.release(newerId);
    await newerPromise;
    ca.controls.release(olderId);
    await olderPromise;

    expect(completionOrder).toEqual(["newer", "older"]);
  });

  it("argument matching holds only the intended invocation", async () => {
    const stubInvoke = (command: string, args?: Record<string, unknown>) => Promise.resolve({ command, args });
    const ca = createControlledAsync(stubInvoke, () => {});
    const id = ca.controls.hold({ command: "list_x", args: { companyId: "c1" } });

    // A different company's invocation does NOT match — passes straight through.
    await expect(ca.invoke("list_x", { companyId: "c2" })).resolves.toEqual({
      command: "list_x",
      args: { companyId: "c2" },
    });
    expect(ca.controls.pending()).toEqual([]);

    // The matching invocation IS held.
    const promise = ca.invoke("list_x", { companyId: "c1" });
    expect(ca.controls.pending()).toHaveLength(1);
    ca.controls.release(id);
    await expect(promise).resolves.toEqual({ command: "list_x", args: { companyId: "c1" } });
  });

  it("argument matching unwraps the { input } envelope (mirrors runtime.ts's unwrap()), so InvocationMatch.args names the effective business args", async () => {
    const stubInvoke = (command: string, args?: Record<string, unknown>) => Promise.resolve({ command, args });
    const ca = createControlledAsync(stubInvoke, () => {});
    const id = ca.controls.hold({ command: "list_research_evidence", args: { companyId: "company_gpw_cdr" } });
    // The real api layer wraps command args as `{ input: {...} }`.
    const promise = ca.invoke("list_research_evidence", { input: { companyId: "company_gpw_cdr", limit: 100 } });
    expect(ca.controls.pending()).toHaveLength(1);
    ca.controls.release(id);
    await expect(promise).resolves.toEqual({
      command: "list_research_evidence",
      args: { input: { companyId: "company_gpw_cdr", limit: 100 } },
    });
  });

  it("reset cleans pending work — no promise leaks across tests", async () => {
    const stubInvoke = () => Promise.resolve("unused");
    const ca = createControlledAsync(stubInvoke, () => {});
    ca.controls.hold({ command: "list_x" });
    const promise = ca.invoke("list_x", {});
    ca.reset();
    await expect(promise).rejects.toThrow(/reset/i);
    expect(ca.controls.pending()).toEqual([]);
  });

  it("releaseAll releases every currently held invocation", async () => {
    const stubInvoke = () => Promise.resolve("v");
    const ca = createControlledAsync(stubInvoke, () => {});
    ca.controls.hold({ command: "a" });
    ca.controls.hold({ command: "b" });
    const pa = ca.invoke("a", {});
    const pb = ca.invoke("b", {});
    ca.controls.releaseAll();
    await expect(pa).resolves.toBe("v");
    await expect(pb).resolves.toBe("v");
    expect(ca.controls.pending()).toEqual([]);
  });

  it("reject(before-handler, CommandError) delegates to the failNext seam (Deliverable A, 5be14c9) instead of shaping the rejection itself", async () => {
    const failNextCalls: Array<{ command: string; error: CommandError }> = [];
    const invokeCalls: string[] = [];
    const stubInvoke = (command: string) => {
      invokeCalls.push(command);
      return Promise.resolve("handler-ran");
    };
    const ca = createControlledAsync(stubInvoke, (command, error) => failNextCalls.push({ command, error }));
    const id = ca.controls.hold({ command: "create_x" });
    const promise = ca.invoke("create_x", {});
    expect(invokeCalls).toHaveLength(0);

    const error: CommandError = { code: "internal", message: "boom" };
    ca.controls.reject(id, error);

    // Delegation, not reproduction: the seam saw the SAME error, and the
    // handler ran exactly once (through the seam's queued rejection).
    expect(failNextCalls).toEqual([{ command: "create_x", error }]);
    expect(invokeCalls).toEqual(["create_x"]);
    await expect(promise).resolves.toBe("handler-ran");
  });

  it("reject with a bare Error bypasses the seam and rejects directly", async () => {
    const failNextCalls: unknown[] = [];
    const ca = createControlledAsync(() => Promise.resolve("unused"), (...args) => failNextCalls.push(args));
    const id = ca.controls.hold({ command: "create_x" });
    const promise = ca.invoke("create_x", {});
    ca.controls.reject(id, new Error("raw failure"));
    await expect(promise).rejects.toThrow("raw failure");
    expect(failNextCalls).toHaveLength(0);
  });

  it("after-handler hold captures the already-computed READ response, then holds delivery", async () => {
    const invokeCalls: string[] = [];
    const stubInvoke = (command: string) => {
      invokeCalls.push(command);
      return Promise.resolve({ value: 42 });
    };
    const ca = createControlledAsync(stubInvoke, () => {});
    const id = ca.controls.hold({ command: "list_x", phase: "after-handler" });
    const promise = ca.invoke("list_x", {});
    await Promise.resolve(); // flush the microtask that runs the (already-settled) read handler
    expect(invokeCalls).toEqual(["list_x"]); // the read handler already ran
    expect(ca.controls.pending()).toEqual([{ id, command: "list_x", args: {}, phase: "after-handler" }]);
    ca.controls.release(id);
    await expect(promise).resolves.toEqual({ value: 42 });
  });

  it("after-handler reject discards the computed value and rejects directly — never re-runs the handler, even for a CommandError shape", async () => {
    const failNextCalls: unknown[] = [];
    const invokeCalls: string[] = [];
    const stubInvoke = (command: string) => {
      invokeCalls.push(command);
      return Promise.resolve({ value: 1 });
    };
    const ca = createControlledAsync(stubInvoke, (...args) => failNextCalls.push(args));
    const id = ca.controls.hold({ command: "list_x", phase: "after-handler" });
    const promise = ca.invoke("list_x", {});
    await Promise.resolve();
    ca.controls.reject(id, { code: "internal", message: "stale drop" });
    await expect(promise).rejects.toMatchObject({ code: "internal", message: "stale drop" });
    expect(failNextCalls).toHaveLength(0);
    expect(invokeCalls).toEqual(["list_x"]);
  });
});
