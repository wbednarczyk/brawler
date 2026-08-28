import type { Page } from "@playwright/test";
import type { ScenarioOverlayName } from "../../../src/test/scenarios/overlays";

// Typed Playwright bridge onto `window.__brawlerMock` (ADR 0081 Q2, Radicle
// a9992e2 / src/test/browserSmokeRuntime.ts). Every call crosses via
// `page.evaluate`, so arguments/results are JSON-serialized — no class
// instances (e.g. `Error`), only plain data.

export type ScenarioName = "empty" | "minimal" | "rich";

// Re-exported from the runtime's own definition, never hand-copied: a local
// copy silently drifted behind six overlays before epic #40 S1 caught it.
export type { ScenarioOverlayName };

export type ScenarioSpec = {
  base: ScenarioName;
  overlays?: readonly ScenarioOverlayName[];
};

export type InvocationPhase = "before-handler" | "after-handler";

export type InvocationMatch = {
  command: string;
  args?: Record<string, unknown>;
  phase?: InvocationPhase;
};

export type PendingInvocation = {
  id: string;
  command: string;
  args: Record<string, unknown> | undefined;
  phase: InvocationPhase;
};

export type MockCommandError = { code: string; message: string };

/** A persistent failure rule: `command` rejects with `error` on EVERY call. */
export type ChaosRule = { command: string; error: MockCommandError };

interface BrawlerMockBridge {
  reset(spec?: ScenarioName | ScenarioSpec): void;
  hold(match: InvocationMatch): string;
  pending(): readonly PendingInvocation[];
  release(id: string): void;
  reject(id: string, error: MockCommandError): void;
  releaseAll(): void;
  failNext(command: string, error: MockCommandError): void;
  chaos(command: string, error: MockCommandError): void;
  clearChaos(): void;
}

declare global {
  interface Window {
    __brawlerMock?: BrawlerMockBridge;
  }
}

type PrimingAction =
  | { kind: "scenario"; spec: ScenarioName | ScenarioSpec }
  | { kind: "chaos"; rules: readonly ChaosRule[] };

/**
 * Applies a priming action to the bridge BEFORE the app boots. Bootstrap reads
 * (companies/feed/attention/autopilot) happen once at mount, so a post-`openApp`
 * `page.evaluate` is too late for them and races under worker load.
 * `page.addInitScript` runs synchronously as the FIRST script on the page: it
 * traps the `window.__brawlerMock` assignment `installBrowserSmokeRuntime`
 * performs and applies every queued action in that same synchronous tick,
 * strictly before React mounts.
 */
async function primeBeforeBoot(page: Page, action: PrimingAction): Promise<void> {
  await page.addInitScript((next) => {
    type Action = typeof next;
    const carrier = window as unknown as { __brawlerPriming?: Action[] };
    const queue: Action[] = (carrier.__brawlerPriming ??= []);
    queue.push(next);
    // The first call owns the trap; later calls just extend the queue.
    if (queue.length > 1) return;
    let installed: unknown;
    Object.defineProperty(window, "__brawlerMock", {
      configurable: true,
      get() {
        return installed;
      },
      set(bridge) {
        const mock = bridge as {
          reset: (spec: unknown) => void;
          chaos: (command: string, error: unknown) => void;
        };
        // Scenario resets run BEFORE chaos rules regardless of call order:
        // `reset()` replaces the whole store, so the reverse order would drop
        // them.
        for (const item of queue) {
          if (item.kind === "scenario") mock.reset(item.spec);
        }
        for (const item of queue) {
          if (item.kind !== "chaos") continue;
          for (const rule of item.rules) mock.chaos(rule.command, rule.error);
        }
        installed = bridge;
      },
    });
  }, action);
}

/** Seed a scenario (optionally with overlays) BEFORE the app boots. Use over
 * `openApp` + `resetMockScenario` whenever the assertion depends on what the
 * FIRST mount read. */
export async function primeMockScenario(
  page: Page,
  spec: ScenarioName | ScenarioSpec,
): Promise<void> {
  await primeBeforeBoot(page, { kind: "scenario", spec });
}

/** Install persistent failure rules BEFORE the app boots (epic #40 S1): each
 * listed command rejects with its ADR 0070 envelope on EVERY call, including
 * the bootstrap read. The URL equivalent for a single command is
 * `?chaos=<command>[:<code>]`. */
export async function primeChaos(page: Page, rules: readonly ChaosRule[]): Promise<void> {
  await primeBeforeBoot(page, { kind: "chaos", rules });
}

/** Re-seed the mock store (optionally with Q2 overlays). Setup order inside
 * the browser runtime is always base -> projection -> overlays. */
export async function resetMockScenario(page: Page, spec?: ScenarioName | ScenarioSpec): Promise<void> {
  await page.evaluate((s) => window.__brawlerMock?.reset(s), spec);
}

/** Register interest in the NEXT matching command/job invocation; returns the
 * id it will be held under once a real call matches. */
export async function holdInvocation(page: Page, match: InvocationMatch): Promise<string> {
  const id = await page.evaluate((m) => window.__brawlerMock?.hold(m), match);
  if (!id) {
    throw new Error("window.__brawlerMock is not installed — is this a mock-runtime browser page?");
  }
  return id;
}

export async function pendingInvocations(page: Page): Promise<readonly PendingInvocation[]> {
  return (await page.evaluate(() => window.__brawlerMock?.pending())) ?? [];
}

/** Let a held invocation proceed/complete normally. */
export async function releaseInvocation(page: Page, id: string): Promise<void> {
  await page.evaluate((invocationId) => window.__brawlerMock?.release(invocationId), id);
}

/** Settle a held invocation with the shared ADR 0070 CommandError envelope
 * (delegates to the Deliverable A seam — see controlledAsync.ts). */
export async function rejectInvocation(page: Page, id: string, error: MockCommandError): Promise<void> {
  await page.evaluate(([invocationId, err]) => window.__brawlerMock?.reject(invocationId, err), [id, error] as const);
}

export async function releaseAllInvocations(page: Page): Promise<void> {
  await page.evaluate(() => window.__brawlerMock?.releaseAll());
}
