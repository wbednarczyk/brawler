// Controlled-async invocation control (ADR 0081 Q2, Radicle a9992e2).
//
// Wraps ONE `invoke` function with hold/pending/release/reject semantics so a
// test can force two independent commands/jobs to settle in a chosen order —
// without a second router. Wired ONCE around `MockRuntime.invoke` in
// `runtime.ts`'s `createMockRuntime`; never inside individual handlers.
//
// `before-handler` holds an invocation BEFORE its handler runs (simulates
// delayed execution) — legal for reads AND mutations, since nothing has
// happened yet. `after-handler` computes the handler's real response first,
// then holds delivery of that already-computed value (simulates a stale
// network/IPC completion) — legal ONLY for read handlers, because a mutating
// handler has already changed the store by that point and a caller trying to
// hold it there would let mutation happen before release.

import type { CommandError } from "../../api/generated/CommandError";

export type InvocationPhase = "before-handler" | "after-handler";

export interface InvocationMatch {
  command: string;
  args?: Record<string, unknown>;
  phase?: InvocationPhase;
}

export interface PendingInvocation {
  readonly id: string;
  readonly command: string;
  readonly args: Record<string, unknown> | undefined;
  readonly phase: InvocationPhase;
}

export interface MockRuntimeControls {
  /** Register interest in the NEXT matching invocation; returns the id that
   * invocation will be held under once it arrives (see `pending()`). */
  hold(match: InvocationMatch): string;
  /** Invocations currently held, awaiting `release`/`reject`. */
  pending(): readonly PendingInvocation[];
  /** Let a held invocation proceed/complete normally. */
  release(id: string): void;
  /** Settle a held invocation with a rejection. A `CommandError` (ADR 0070
   * envelope) delegates through the Deliverable A seam (`failNext`,
   * `runtime.ts`) for `before-handler` holds so the SAME rejection path a real
   * typed backend failure uses is exercised, not reproduced here. A bare
   * `Error`, or any `after-handler` hold, rejects directly — nothing to
   * delegate to since the handler already ran (or the caller opted out of the
   * typed envelope). */
  reject(id: string, error: CommandError | Error): void;
  /** Release every currently held invocation (test cleanup / reset). */
  releaseAll(): void;
}

type Args = Record<string, unknown> | undefined;
type BaseInvoke = (command: string, args?: Args) => Promise<unknown>;
/** Deliverable A seam (Radicle 5be14c9): queue a one-shot CommandError
 * rejection for the next matching invocation, rather than reproducing the
 * envelope-shaping here. */
type FailNext = (command: string, error: CommandError) => void;

interface Registration {
  id: string;
  match: InvocationMatch;
}

interface HeldInvocation extends PendingInvocation {
  settle(): void;
  fail(error: CommandError | Error): void;
}

/**
 * Mirrors `runtime.ts`'s own `unwrap()` — `{ input: X }` → X, else the args
 * object itself — so `InvocationMatch.args` matches the EFFECTIVE business
 * args (e.g. `companyId`) a test author reasons about, not the `{ input }`
 * envelope most commands use. Duplicated here (not imported) to keep this
 * module decoupled from `runtime.ts` per the module doc comment above.
 */
function unwrapArgs(args: Args): Record<string, unknown> {
  if (args && typeof args === "object" && "input" in args && args.input && typeof args.input === "object") {
    return args.input as Record<string, unknown>;
  }
  return (args ?? {}) as Record<string, unknown>;
}

function argsMatch(expected: Record<string, unknown> | undefined, actual: Args): boolean {
  if (!expected) return true;
  const provided = unwrapArgs(actual);
  return Object.entries(expected).every(([key, value]) => provided[key] === value);
}

function isCommandErrorShape(error: CommandError | Error): error is CommandError {
  return !(error instanceof Error);
}

export interface ControlledAsync {
  /** The wrapped `invoke` — install this as `MockRuntime.invoke`. */
  invoke: BaseInvoke;
  controls: MockRuntimeControls;
  /** Clears registrations and rejects every currently held invocation so no
   * promise leaks across a runtime `reset()`. */
  reset(): void;
}

/**
 * Build controlled-async wrapping around `baseInvoke` (the runtime's own
 * command router) and `failNext` (Deliverable A's seam). Kept decoupled from
 * `MockRuntime`'s full shape so the hold/match logic is unit-testable with a
 * bare stub invoke.
 */
export function createControlledAsync(baseInvoke: BaseInvoke, failNext: FailNext): ControlledAsync {
  let counter = 0;
  const registrations: Registration[] = [];
  const held = new Map<string, HeldInvocation>();

  function takeRegistration(command: string, args: Args, phase: InvocationPhase): Registration | null {
    const index = registrations.findIndex(
      (registration) =>
        registration.match.command === command &&
        (registration.match.phase ?? "before-handler") === phase &&
        argsMatch(registration.match.args, args),
    );
    if (index === -1) return null;
    return registrations.splice(index, 1)[0];
  }

  function holdBeforeHandler(id: string, command: string, args: Args): Promise<unknown> {
    return new Promise((resolve, reject) => {
      held.set(id, {
        id,
        command,
        args,
        phase: "before-handler",
        settle: () => {
          baseInvoke(command, args).then(resolve, reject);
        },
        fail: (error) => {
          if (!isCommandErrorShape(error)) {
            reject(error);
            return;
          }
          failNext(command, error);
          baseInvoke(command, args).then(resolve, reject);
        },
      });
    });
  }

  function holdAfterHandler(id: string, command: string, args: Args, value: unknown): Promise<unknown> {
    return new Promise((resolve, reject) => {
      held.set(id, {
        id,
        command,
        args,
        phase: "after-handler",
        // The handler already ran (read-only) — settling releases the
        // ALREADY-computed response, simulating a stale-but-arriving reply.
        settle: () => resolve(value),
        fail: (error) => reject(error),
      });
    });
  }

  function invoke(command: string, args?: Args): Promise<unknown> {
    const before = takeRegistration(command, args, "before-handler");
    if (before) {
      return holdBeforeHandler(before.id, command, args);
    }
    const after = takeRegistration(command, args, "after-handler");
    if (after) {
      return baseInvoke(command, args).then((value) => holdAfterHandler(after.id, command, args, value));
    }
    return baseInvoke(command, args);
  }

  function takeHeld(id: string): HeldInvocation {
    const invocation = held.get(id);
    if (!invocation) {
      throw new Error(`No pending invocation held with id "${id}"`);
    }
    held.delete(id);
    return invocation;
  }

  const controls: MockRuntimeControls = {
    hold(match) {
      counter += 1;
      const id = `held_${counter}`;
      registrations.push({ id, match: { ...match } });
      return id;
    },
    pending() {
      return Array.from(held.values()).map(({ id, command, args, phase }) => ({ id, command, args, phase }));
    },
    release(id) {
      takeHeld(id).settle();
    },
    reject(id, error) {
      takeHeld(id).fail(error);
    },
    releaseAll() {
      for (const invocation of Array.from(held.values())) {
        held.delete(invocation.id);
        invocation.settle();
      }
    },
  };

  return {
    invoke,
    controls,
    reset() {
      registrations.length = 0;
      for (const invocation of Array.from(held.values())) {
        held.delete(invocation.id);
        invocation.fail(new Error("Mock runtime reset while an invocation was held"));
      }
    },
  };
}
